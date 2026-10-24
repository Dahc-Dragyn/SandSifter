use crate::math::{LoudnessMetrics, VisualPacingEngine, VisualPacingMetrics};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tracing::{info, warn};

/// Bounded stream chunk duration (5 minutes / 300 seconds) for network-resilient scene cut detection.
pub const STREAM_CHUNK_DURATION_SEC: f64 = 300.0;

/// Maximum simultaneous FFmpeg worker processes allowed during chunked stream decoding.
pub const MAX_CONCURRENT_STREAM_WORKERS: usize = 2;

/// Discrete 5-minute time slice of a remote video stream.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamChunk {
    pub index: usize,
    pub start_sec: f64,
    pub duration_sec: f64,
}

/// Alias for MediaSceneDetector conforming to system naming conventions.
pub type MediaEngine = MediaSceneDetector;

/// Checks if a source path is a remote HTTP/HTTPS network stream.
pub fn is_remote_url(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

pub struct MediaSceneDetector;

impl MediaSceneDetector {
    /// Splits `total_duration_sec` into contiguous slices: `[0.0, 300.0)`, `[300.0, 600.0)`, ..., `[N*300.0, total_duration_sec)`.
    /// Handles remainders cleanly without trailing sub-second micro-chunks (if remainder < 5.0s, append it to the final chunk).
    pub fn partition_stream_chunks(total_duration_sec: f64) -> Vec<StreamChunk> {
        if total_duration_sec <= 0.0 {
            return Vec::new();
        }
        if total_duration_sec <= STREAM_CHUNK_DURATION_SEC {
            return vec![StreamChunk {
                index: 0,
                start_sec: 0.0,
                duration_sec: (total_duration_sec * 100.0).round() / 100.0,
            }];
        }

        let mut chunks = Vec::new();
        let mut current_start = 0.0;
        let mut idx = 0;

        while current_start < total_duration_sec {
            let remaining = total_duration_sec - current_start;
            if remaining <= STREAM_CHUNK_DURATION_SEC {
                chunks.push(StreamChunk {
                    index: idx,
                    start_sec: (current_start * 100.0).round() / 100.0,
                    duration_sec: (remaining * 100.0).round() / 100.0,
                });
                break;
            }

            let after_this = remaining - STREAM_CHUNK_DURATION_SEC;
            if after_this < 5.0 {
                // If remainder < 5.0s, append it to the final chunk to avoid sub-second micro-chunks
                chunks.push(StreamChunk {
                    index: idx,
                    start_sec: (current_start * 100.0).round() / 100.0,
                    duration_sec: (remaining * 100.0).round() / 100.0,
                });
                break;
            } else {
                chunks.push(StreamChunk {
                    index: idx,
                    start_sec: (current_start * 100.0).round() / 100.0,
                    duration_sec: STREAM_CHUNK_DURATION_SEC,
                });
                current_start += STREAM_CHUNK_DURATION_SEC;
                idx += 1;
            }
        }

        chunks
    }

    /// Executes FFmpeg fast input seeking on a single discrete 5-minute chunk.
    /// Fast input seeking (`-ss` before `-i`) performs demuxer-level seek via HTTP Range requests.
    /// FFmpeg resets output stream PTS so the first decoded frame starts at `t = 0.0s`.
    /// Maps chunk-local timestamps `t_local` to global timeline: `T_global = chunk.start_sec + t_local`.
    pub fn detect_scene_cuts_single_chunk(
        url: &str,
        chunk: &StreamChunk,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        let thresh = scene_threshold.unwrap_or(0.35).clamp(0.01, 1.0);
        let args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-ss".to_string(),
            format!("{:.2}", chunk.start_sec),
            "-t".to_string(),
            format!("{:.2}", chunk.duration_sec),
            "-i".to_string(),
            url.to_string(),
            "-filter:v".to_string(),
            format!("select='gt(scene,{:.2})',showinfo", thresh),
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ];

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg scene detector for chunk {}: {}", chunk.index, e))?;

        let mut chunk_cuts = Vec::new();

        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(_) => break,
                };
                if let Some(pos) = line.find("pts_time:") {
                    let remainder = &line[pos + 9..];
                    if let Some(token) = remainder.split_whitespace().next() {
                        if let Ok(ts_local) = token.parse::<f64>() {
                            let t_global = chunk.start_sec + ts_local;
                            chunk_cuts.push((t_global * 100.0).round() / 100.0);
                        }
                    }
                }
            }
        }

        let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;
        if !status.success() && chunk_cuts.is_empty() {
            return Err(format!("ffmpeg chunk {} exited with error status", chunk.index));
        }

        Ok(chunk_cuts)
    }

    /// Executes single chunk cut detection with 1-retry fail-forward and 1.5s jitter on transient HTTP Range errors.
    fn detect_scene_cuts_chunk_with_retry(
        url: &str,
        chunk: &StreamChunk,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        match Self::detect_scene_cuts_single_chunk(url, chunk, scene_threshold) {
            Ok(cuts) => Ok(cuts),
            Err(err) => {
                warn!("⚠️ Chunk {} failed on first attempt ({}), retrying with 1.5s jitter...", chunk.index, err);
                std::thread::sleep(std::time::Duration::from_millis(1500));
                Self::detect_scene_cuts_single_chunk(url, chunk, scene_threshold)
            }
        }
    }

    /// Asynchronous bounded chunked stream seeking:
    /// Partitions remote video into discrete 5-minute chunks and decodes them concurrently
    /// with concurrency strictly capped at `MAX_CONCURRENT_STREAM_WORKERS` (2 permits).
    pub async fn detect_scene_cuts_chunked_async(
        url: &str,
        total_duration_sec: f64,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        let chunks = Self::partition_stream_chunks(total_duration_sec);
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        if chunks.len() == 1 {
            return Self::detect_scene_cuts_with_threshold(url, Some(chunks[0].duration_sec), scene_threshold);
        }

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_STREAM_WORKERS));
        let mut tasks = Vec::new();

        for chunk in chunks {
            let sem = semaphore.clone();
            let u = url.to_string();
            let thresh = scene_threshold;
            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(|e| e.to_string())?;
                tokio::task::spawn_blocking(move || {
                    Self::detect_scene_cuts_chunk_with_retry(&u, &chunk, thresh)
                })
                .await
                .map_err(|e| format!("Worker task join error: {}", e))?
            });
            tasks.push(task);
        }

        let mut all_cuts = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(cuts)) => all_cuts.extend(cuts),
                Ok(Err(e)) => warn!("⚠️ Chunk cut detection reported error: {}", e),
                Err(e) => warn!("⚠️ Task spawn error in chunk detection: {}", e),
            }
        }

        all_cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_cuts.dedup();

        let debounced = VisualPacingEngine::debounce_cuts(&all_cuts, Some(0.5));
        Ok(debounced)
    }

    /// Synchronous wrapper for chunked stream scene cut detection.
    /// Safely isolates the Tokio runtime within a worker thread to prevent nested block_on panics.
    pub fn detect_scene_cuts_chunked(
        url: &str,
        total_duration_sec: f64,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        let u = url.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            rt.block_on(Self::detect_scene_cuts_chunked_async(&u, total_duration_sec, scene_threshold))
        })
        .join()
        .map_err(|_| "Chunked scene detection thread panicked".to_string())?
    }

    /// Single-pass scene cut detection (alias for detect_scene_cuts_with_threshold).
    pub fn detect_scene_cuts_single_pass(
        stream_or_file: &str,
        sample_seconds: Option<f64>,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        Self::detect_scene_cuts_with_threshold(stream_or_file, sample_seconds, scene_threshold)
    }
    /// Extracts a low-resolution direct video stream URL (<360p) via yt-dlp to minimize network transfer.
    pub fn get_lowres_stream_url(video_url: &str) -> Result<String, String> {
        let output = Command::new("yt-dlp")
            .args([
                "-g",
                "-f", "worstvideo[height<=360]/worst",
                "--no-playlist",
                "--no-warnings",
                video_url,
            ])
            .output()
            .or_else(|_| {
                Command::new("uvx")
                    .args([
                        "yt-dlp",
                        "-g",
                        "-f", "worstvideo[height<=360]/worst",
                        "--no-playlist",
                        "--no-warnings",
                        video_url,
                    ])
                    .output()
            })
            .map_err(|e| format!("Failed to spawn yt-dlp stream resolver: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp stream extraction failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stream_url = stdout.lines().next().unwrap_or("").trim().to_string();
        if stream_url.is_empty() {
            return Err("yt-dlp returned empty stream URL".to_string());
        }

        Ok(stream_url)
    }

    /// Pipes a video stream directly into FFmpeg's scene change detector without writing frames to disk.
    /// Streams `pts_time` transitions line-by-line via BufReader for bounded memory execution.
    pub fn detect_scene_cuts(
        stream_or_file: &str,
        sample_seconds: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        Self::detect_scene_cuts_with_threshold(stream_or_file, sample_seconds, None)
    }

    /// Pipes a video stream directly into FFmpeg's scene change detector with a configurable threshold.
    /// Default threshold is 0.35 (35% pixel change). For dark-mode IDE screen shares or slide decks,
    /// 0.12 (12% pixel change) detects subtle coding tab switches and terminal updates.
    pub fn detect_scene_cuts_with_threshold(
        stream_or_file: &str,
        sample_seconds: Option<f64>,
        scene_threshold: Option<f64>,
    ) -> Result<Vec<f64>, String> {
        let thresh = scene_threshold.unwrap_or(0.35).clamp(0.01, 1.0);
        let mut args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
        ];

        // If sample duration is provided, limit stream ingestion before -i to save bandwidth
        let sample_sec_str;
        if let Some(sec) = sample_seconds {
            args.push("-t".to_string());
            sample_sec_str = format!("{:.1}", sec);
            args.push(sample_sec_str);
        }

        args.push("-i".to_string());
        args.push(stream_or_file.to_string());
        args.push("-filter:v".to_string());
        args.push(format!("select='gt(scene,{:.2})',showinfo", thresh));
        args.push("-f".to_string());
        args.push("null".to_string());
        args.push("-".to_string());

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg scene detector: {}", e))?;

        let mut raw_candidates = Vec::new();

        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line_res in reader.lines() {
                let line = match line_res {
                    Ok(l) => l,
                    Err(_) => break,
                };
                // Look for showinfo output: `pts_time:12.345`
                if let Some(pos) = line.find("pts_time:") {
                    let remainder = &line[pos + 9..];
                    if let Some(token) = remainder.split_whitespace().next() {
                        if let Ok(ts) = token.parse::<f64>() {
                            raw_candidates.push((ts * 100.0).round() / 100.0);
                        }
                    }
                }
            }
        }

        let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;
        if !status.success() && raw_candidates.is_empty() {
            return Err("ffmpeg scene detection exited with an error status".to_string());
        }

        // Apply temporal debouncing and motion flurry suppression (min_shot_duration: 0.5s)
        let debounced = VisualPacingEngine::debounce_cuts(&raw_candidates, Some(0.5));
        Ok(debounced)
    }

    /// End-to-end visual kinetics analysis: resolves stream, extracts scene cuts, and evaluates ASL metrics.
    pub fn analyze_video_visual_pacing(
        target_url_or_file: &str,
        sample_seconds: Option<f64>,
    ) -> Result<VisualPacingMetrics, String> {
        Self::analyze_video_visual_pacing_with_threshold(target_url_or_file, sample_seconds, None)
    }

    /// End-to-end visual kinetics analysis with configurable scene threshold.
    pub fn analyze_video_visual_pacing_with_threshold(
        target_url_or_file: &str,
        sample_seconds: Option<f64>,
        scene_threshold: Option<f64>,
    ) -> Result<VisualPacingMetrics, String> {
        let is_http = is_remote_url(target_url_or_file);

        let stream_url = if is_http {
            info!("📡 Resolving low-res video stream for visual scene analysis...");
            Self::get_lowres_stream_url(target_url_or_file)?
        } else {
            target_url_or_file.to_string()
        };

        let thresh_val = scene_threshold.unwrap_or(0.35);
        let sample_sec = sample_seconds.unwrap_or(180.0);

        // Local file immunity & short-circuit check:
        // Local files and short streams (<= 300s) execute direct single-pass with zero chunking overhead.
        // Long remote network streams (> 300s) route to bounded 5-minute chunked seeking.
        let cuts = if is_http && sample_sec > STREAM_CHUNK_DURATION_SEC {
            info!("🎬 Running chunked FFmpeg scene transition detector across {:.0}s sample (5-min chunks, concurrency: {})...", sample_sec, MAX_CONCURRENT_STREAM_WORKERS);
            Self::detect_scene_cuts_chunked(&stream_url, sample_sec, scene_threshold)?
        } else {
            info!("🎬 Running single-pass in-memory FFmpeg scene transition detector (threshold: {:.2})...", thresh_val);
            Self::detect_scene_cuts_with_threshold(&stream_url, sample_seconds, scene_threshold)?
        };

        let analyzed_duration = sample_seconds.unwrap_or_else(|| {
            cuts.last().copied().unwrap_or(60.0).max(10.0)
        });

        let metrics = VisualPacingEngine::evaluate_visual_pacing(analyzed_duration, &cuts);
        Ok(metrics)
    }

    /// Probes a local media file using ffprobe (or ffmpeg fallback) to determine exact duration in seconds.
    pub fn probe_local_duration_seconds(file_path: &str) -> Result<f64, String> {
        let output = Command::new("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                file_path,
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Ok(dur) = text.trim().parse::<f64>() {
                    if dur > 0.0 {
                        return Ok(dur);
                    }
                }
            }
        }

        // Fallback: ffmpeg -i <file> and parse Duration: HH:MM:SS.ss
        let out2 = Command::new("ffmpeg")
            .args(["-hide_banner", "-i", file_path])
            .output()
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let err_text = String::from_utf8_lossy(&out2.stderr);
        if let Some(pos) = err_text.find("Duration: ") {
            let slice = &err_text[pos + 10..];
            let dur_str = slice.split(',').next().unwrap_or("").trim();
            let parts: Vec<&str> = dur_str.split(':').collect();
            if parts.len() == 3 {
                let h: f64 = parts[0].parse().unwrap_or(0.0);
                let m: f64 = parts[1].parse().unwrap_or(0.0);
                let s: f64 = parts[2].parse().unwrap_or(0.0);
                let total = h * 3600.0 + m * 60.0 + s;
                if total > 0.0 {
                    return Ok(total);
                }
            }
        }

        Err(format!("Could not determine duration of local video file: {}", file_path))
    }

    /// Extracts PCM 16kHz mono audio into memory directly from a local video file via ffmpeg.
    pub fn extract_local_audio_pcm(file_path: &str, duration_limit_sec: Option<f64>) -> Result<Vec<u8>, String> {
        let mut args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            file_path.to_string(),
            "-vn".to_string(),
        ];
        if let Some(sec) = duration_limit_sec {
            args.push("-t".to_string());
            args.push(format!("{:.1}", sec));
        }
        args.push("-acodec".to_string());
        args.push("pcm_s16le".to_string());
        args.push("-ar".to_string());
        args.push("16000".to_string());
        args.push("-ac".to_string());
        args.push("1".to_string());
        args.push("-f".to_string());
        args.push("s16le".to_string());
        args.push("-".to_string());

        let output = Command::new("ffmpeg")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to spawn ffmpeg audio extractor: {}", e))?;

        if !output.status.success() && output.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffmpeg audio extraction failed: {}", stderr.trim()));
        }

        Ok(output.stdout)
    }

    /// Extracts PCM 16kHz mono audio into memory directly from a local video file or remote stream via ffmpeg.
    pub fn extract_audio_pcm(target_url_or_file: &str, duration_limit_sec: Option<f64>) -> Result<Vec<u8>, String> {
        let is_http = target_url_or_file.starts_with("http://") || target_url_or_file.starts_with("https://");
        let stream_url = if is_http {
            Self::get_audio_stream_url(target_url_or_file)
                .or_else(|_| Self::get_lowres_stream_url(target_url_or_file))?
        } else {
            target_url_or_file.to_string()
        };

        Self::extract_local_audio_pcm(&stream_url, duration_limit_sec)
    }

    /// Extracts an audio-only stream URL via yt-dlp to minimize network transfer during loudness analysis.
    pub fn get_audio_stream_url(video_url: &str) -> Result<String, String> {
        let output = Command::new("yt-dlp")
            .args([
                "-g",
                "-f", "bestaudio/worst",
                "--no-playlist",
                "--no-warnings",
                video_url,
            ])
            .output()
            .or_else(|_| {
                Command::new("uvx")
                    .args([
                        "yt-dlp",
                        "-g",
                        "-f", "bestaudio/worst",
                        "--no-playlist",
                        "--no-warnings",
                        video_url,
                    ])
                    .output()
            })
            .map_err(|e| format!("Failed to spawn yt-dlp audio stream resolver: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp audio stream extraction failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stream_url = stdout.lines().next().unwrap_or("").trim().to_string();
        if stream_url.is_empty() {
            return Err("yt-dlp returned empty audio stream URL".to_string());
        }

        Ok(stream_url)
    }

    /// Parses FFmpeg ebur128 output from a reader to extract (integrated_lufs, lra, true_peak).
    pub fn parse_ebur128_summary<R: BufRead>(reader: R) -> (Option<f64>, Option<f64>, Option<f64>) {
        let mut integrated_lufs = None;
        let mut loudness_range = None;
        let mut max_true_peak: Option<f64> = None;

        let mut in_summary = false;
        let mut in_integrated = false;
        let mut in_lra = false;
        let mut in_true_peak = false;

        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(_) => break,
            };

            let trimmed = line.trim();

            if trimmed.contains("Summary:") {
                in_summary = true;
                continue;
            }

            if in_summary {
                if trimmed.starts_with("Integrated loudness:") {
                    in_integrated = true;
                    in_lra = false;
                    in_true_peak = false;
                    continue;
                } else if trimmed.starts_with("Loudness range:") {
                    in_integrated = false;
                    in_lra = true;
                    in_true_peak = false;
                    continue;
                } else if trimmed.starts_with("True peak:") {
                    in_integrated = false;
                    in_lra = false;
                    in_true_peak = true;
                    continue;
                }

                if in_integrated && trimmed.starts_with("I:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            integrated_lufs = Some(val);
                        }
                    }
                } else if in_lra && trimmed.starts_with("LRA:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            loudness_range = Some(val);
                        }
                    }
                } else if in_true_peak && trimmed.starts_with("Peak:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            max_true_peak = Some(match max_true_peak {
                                Some(prev) => prev.max(val),
                                None => val,
                            });
                        }
                    }
                }
            } else {
                // Direct line fallback if Summary header was omitted
                if trimmed.starts_with("I:") && trimmed.contains("LUFS") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            integrated_lufs = Some(val);
                        }
                    }
                } else if trimmed.starts_with("LRA:") && trimmed.contains("LU") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            loudness_range = Some(val);
                        }
                    }
                } else if trimmed.starts_with("Peak:") && (trimmed.contains("dBFS") || trimmed.contains("dBTP")) {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(val) = parts[1].parse::<f64>() {
                            max_true_peak = Some(match max_true_peak {
                                Some(prev) => prev.max(val),
                                None => val,
                            });
                        }
                    }
                }
            }
        }

        (integrated_lufs, loudness_range, max_true_peak)
    }

    /// Runs an in-memory FFmpeg ebur128 analysis pass without writing disk artifacts.
    pub fn run_ebur128_pass(stream_or_file: &str, duration_seconds: Option<f64>) -> Result<(f64, f64, f64), String> {
        let mut args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
        ];

        let dur_str;
        if let Some(sec) = duration_seconds {
            args.push("-t".to_string());
            dur_str = format!("{:.1}", sec);
            args.push(dur_str);
        }

        args.push("-i".to_string());
        args.push(stream_or_file.to_string());
        args.push("-filter_complex".to_string());
        args.push("[0:a]ebur128=peak=true:dualmono=true[out]".to_string());
        args.push("-map".to_string());
        args.push("[out]".to_string());
        args.push("-f".to_string());
        args.push("null".to_string());
        args.push("-".to_string());

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg ebur128: {}", e))?;

        let (i_opt, lra_opt, tp_opt) = if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            Self::parse_ebur128_summary(reader)
        } else {
            (None, None, None)
        };

        let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;

        let i = i_opt.unwrap_or(-14.0);
        let lra = lra_opt.unwrap_or(6.0);
        let tp = tp_opt.unwrap_or(-1.0);

        if !status.success() && i_opt.is_none() {
            return Err("ffmpeg ebur128 analysis exited with error status".to_string());
        }

        Ok((i, lra, tp))
    }

    /// End-to-end extraction of EBU R128 loudness metrics across the full video/audio target and the opening 30s.
    pub fn extract_loudness_metrics(target: &str, is_local: bool) -> Result<LoudnessMetrics, String> {
        let stream_url = if is_local {
            target.to_string()
        } else if target.starts_with("http://") || target.starts_with("https://") {
            Self::get_audio_stream_url(target)
                .or_else(|_| Self::get_lowres_stream_url(target))?
        } else {
            target.to_string()
        };

        info!("🔊 Running in-memory FFmpeg EBU R128 audio loudness analysis...");

        // Pass 1: Audio loudness (cap remote HTTP streams to 300s sample to prevent hanging on 1hr+ videos)
        let sample_dur = if is_local { None } else { Some(300.0) };
        let (integrated, lra, true_peak) = Self::run_ebur128_pass(&stream_url, sample_dur)
            .unwrap_or((-14.0, 6.0, -1.0));

        // Pass 2: Cold open (opening 30s)
        let (cold_open, _, _) = Self::run_ebur128_pass(&stream_url, Some(30.0))
            .unwrap_or((integrated, 6.0, -1.0));

        Ok(LoudnessMetrics::evaluate(integrated, cold_open, lra, true_peak))
    }
}
