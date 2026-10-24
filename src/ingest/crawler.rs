use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::info;

use crate::analyzer::{AudioTranscriber, HookAligner, NarrativeClassifier, VadClassifier};
use crate::ingest::importer::md5_hash;
use crate::ingest::streamer::StreamIngestor;
use crate::math::baseline::{BaselineDiagnostics, BaselineEngine};
use crate::models::channel::WatchlistChannel;
use crate::models::video::{VideoChannelDetails, VideoDetail, VideoSnapshot};
use crate::storage::SandsifterDb;

/// Evaluated single video item resulting from a channel or playlist crawl.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrawledVideoItem {
    pub title: String,
    pub video_url: String,
    pub views: u64,
    pub previous_views: Option<u64>,
    pub view_growth: u64,
    #[serde(default)]
    pub channel_multiplier: f64,
    #[serde(default)]
    pub series_multiplier: f64,
    pub outlier_multiplier: f64,
    pub is_viral_outlier: bool,
    pub is_accelerating: bool,
    pub published_at: Option<String>,
    #[serde(default)]
    pub spoken_hook: Option<String>,
    #[serde(default)]
    pub hook_madlib: Option<String>,
    #[serde(default)]
    pub speech_onset_ms: Option<u32>,
    #[serde(default)]
    pub wpm_0_to_35: Option<f64>,
}

/// Comprehensive crawl summary tailored for creator intelligence and velocity tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrawlSummary {
    pub channel_handle: String,
    pub channel_name: String,
    pub platform: String,
    pub total_shorts_scanned: usize,
    pub author_baseline_views: u64,
    pub viral_outliers: Vec<CrawledVideoItem>,
    pub accelerating_videos: Vec<CrawledVideoItem>,
    pub all_videos: Vec<CrawledVideoItem>,
    pub takeaway_for_creator: String,
    #[serde(default)]
    pub diagnostics: Option<BaselineDiagnostics>,
}

impl CrawlSummary {
    /// Formats the crawl insights into an intuitive, creator-focused report.
    pub fn to_creator_report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# 📚 Creator Shorts Intelligence: {}\n\n", self.channel_name));
        out.push_str(&format!("**Tracked Channel**: `{}` | **Platform**: {}\n\n", self.channel_handle, self.platform));

        out.push_str("## 🎯 Author Baseline Breakdown\n");
        out.push_str(&format!("- **Typical Performance**: **{} views** per video\n", self.author_baseline_views));
        out.push_str("  *(This is the author's true baseline — calculated by filtering out viral luck and hyper-spikes to show what their loyal audience consistently delivers.)*\n");
        out.push_str(&format!("- **Total Videos Screened**: {} videos\n", self.total_shorts_scanned));
        out.push_str(&format!("- **Breakout Viral Hits Discovered**: {} videos\n\n", self.viral_outliers.len()));

        if let Some(diag) = &self.diagnostics {
            out.push_str("### 📊 Intermediate Empirical Baseline Diagnostics (Mathematical Transparency)\n");
            out.push_str(&format!("- **Sample Size**: {} historical videos\n", diag.sample_size));
            out.push_str(&format!("- **Raw Median**: {} views\n", diag.raw_median));
            out.push_str(&format!("- **Interquartile Range (IQR)**: Q1 = {:.0} views | Q3 = {:.0} views (IQR = {:.0})\n", diag.q1, diag.q3, diag.iqr));
            out.push_str(&format!("- **Outlier IQR Clamp Ceiling**: {:.0} views (1.5x IQR above Q3)\n", diag.upper_clamp_bound));
            out.push_str(&format!("- **Clamped Hyper-Spikes**: {} viral videos clamped to prevent skew\n", diag.clamped_spikes_count));
            out.push_str(&format!("- **Noise-Filtered Baseline**: **{} views**\n\n", diag.winsorized_baseline));
        }

        if !self.viral_outliers.is_empty() {
            out.push_str("## 🔥 Top Viral Outliers (What Captivated Viewers)\n");
            out.push_str("These videos broke past the creator's standard audience and reached the broader algorithm:\n\n");
            for (idx, vid) in self.viral_outliers.iter().take(10).enumerate() {
                out.push_str(&format!(
                    "{}. **\"{}\"**\n   - 🚀 **{:.1}x Virality Multiplier** ({} total views vs. {} normal baseline)\n",
                    idx + 1,
                    vid.title,
                    vid.outlier_multiplier,
                    vid.views,
                    self.author_baseline_views,
                ));
                if let Some(hook) = &vid.spoken_hook {
                    out.push_str(&format!("   - 🎣 **Opening Spoken Hook (0-3.5s)**: \"{}\"\n", hook));
                }
                if let Some(madlib) = &vid.hook_madlib {
                    out.push_str(&format!("   - 🧩 **Madlib Formula**: \"{}\"\n", madlib));
                }
                if let Some(onset) = vid.speech_onset_ms {
                    let sec_str = if onset == 0 { "0.0s (Instant)".to_string() } else { format!("{:.1}s", onset as f64 / 1000.0) };
                    out.push_str(&format!("   - ⚡ **Dead Air Lag**: {}\n", sec_str));
                }
                if let Some(wpm) = vid.wpm_0_to_35 {
                    out.push_str(&format!("   - ⚡ **Speaking Pace**: {:.0} WPM\n", wpm));
                }
                out.push_str(&format!("   - 🔗 **Watch Link**: {}\n\n", vid.video_url));
            }
        }

        // Complete Deep Audio Forensic Hook Matrix
        let deep_extracted_count = self.all_videos.iter().filter(|v| v.spoken_hook.is_some()).count();
        if deep_extracted_count > 0 {
            out.push_str("## 🎙️ Complete Forensic Hook Matrix (Deep Audio Extraction)\n\n");
            out.push_str("| # | Video Title | Channel Mult | Series Mult | Views | Dead Air Lag | Speaking Pace | Opening Spoken Hook |\n");
            out.push_str("|---|-------------|:------------:|:-----------:|:-----:|:------------:|:-------------:|----------------------|\n");
            for (idx, vid) in self.all_videos.iter().enumerate() {
                let onset_str = match vid.speech_onset_ms {
                    Some(0) => "0.0s (Instant)".to_string(),
                    Some(ms) if ms < 3500 => format!("{:.1}s", ms as f64 / 1000.0),
                    Some(_) => "3.5s+ (Slow)".to_string(),
                    None => "No Speech".to_string(),
                };
                let wpm_str = vid.wpm_0_to_35.map(|w| format!("{:.0} WPM", w)).unwrap_or_else(|| "0 WPM".to_string());
                let hook_str = vid.spoken_hook.as_deref().unwrap_or("-");
                // Sanitize pipe characters for markdown table
                let clean_title = vid.title.replace('|', "-");
                let clean_hook = hook_str.replace('|', "-");
                let chan_m = if vid.channel_multiplier > 0.0 { vid.channel_multiplier } else { vid.outlier_multiplier };
                let ser_m = if vid.series_multiplier > 0.0 { vid.series_multiplier } else { vid.outlier_multiplier };
                out.push_str(&format!(
                    "| {} | {} | {:.2}x | {:.2}x | {} | {} | {} | \"{}\" |\n",
                    idx + 1,
                    clean_title,
                    chan_m,
                    ser_m,
                    vid.views,
                    onset_str,
                    wpm_str,
                    clean_hook
                ));
            }
            out.push('\n');
        }

        if !self.accelerating_videos.is_empty() {
            out.push_str("## ⚡ Algorithm Momentum Alert (Gaining Views Right Now!)\n");
            out.push_str("These videos showed positive view acceleration between snapshots:\n\n");
            for vid in &self.accelerating_videos {
                out.push_str(&format!(
                    "- **\"{}\"**: Gained **+{} new views** since the previous snapshot!\n",
                    vid.title, vid.view_growth
                ));
            }
            out.push('\n');
        }

        out.push_str("## 💡 Creator Action Plan for Next Video\n");
        out.push_str(&format!("{}\n", self.takeaway_for_creator));

        out
    }
}

pub struct ChannelCrawler;

impl ChannelCrawler {
    /// Normalizes a user input string into a standard YouTube URL and handle.
    pub fn normalize_channel_url(input: &str) -> (String, String) {
        let trimmed = input.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            // Check if input is a playlist URL
            if trimmed.contains("playlist") || trimmed.contains("list=") {
                return (trimmed.to_string(), "@creator".to_string());
            }
            let clean = if trimmed.ends_with("/shorts") || trimmed.contains("/shorts") {
                trimmed.to_string()
            } else if trimmed.ends_with('/') {
                format!("{}shorts", trimmed)
            } else {
                format!("{}/shorts", trimmed)
            };
            let handle = if let Some(pos) = trimmed.find("/@") {
                let rem = &trimmed[pos + 1..];
                let h = rem.split('/').next().unwrap_or("creator");
                if h.starts_with('@') { h.to_string() } else { format!("@{}", h) }
            } else {
                "@creator".to_string()
            };
            (clean, handle)
        } else {
            let clean_handle = if trimmed.starts_with('@') {
                trimmed.to_string()
            } else {
                format!("@{}", trimmed)
            };
            let url = format!("https://www.youtube.com/{}/shorts", clean_handle);
            (url, clean_handle)
        }
    }

    /// Extracts the channel handle from a target, which can be an @handle, channel URL, playlist URL, or video URL.
    pub fn extract_creator_handle(target: &str) -> Result<String, String> {
        let trimmed = target.trim();
        if trimmed.starts_with('@') {
            return Ok(trimmed.to_string());
        }
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return Ok(format!("@{}", trimmed));
        }

        // If it's a channel URL with /@handle
        if let Some(pos) = trimmed.find("/@") {
            let rem = &trimmed[pos + 1..];
            let h = rem.split('/').next().unwrap_or("creator");
            let clean = if h.starts_with('@') { h.to_string() } else { format!("@{}", h) };
            return Ok(clean);
        }

        // Query yt-dlp to inspect the playlist or video metadata
        let output = Command::new("yt-dlp")
            .args([
                "--flat-playlist",
                "--dump-json",
                "--no-warnings",
                "--playlist-end",
                "1",
                trimmed,
            ])
            .output()
            .or_else(|_| {
                Command::new("uvx")
                    .args([
                        "yt-dlp",
                        "--flat-playlist",
                        "--dump-json",
                        "--no-warnings",
                        "--playlist-end",
                        "1",
                        trimmed,
                    ])
                    .output()
            })
            .map_err(|e| format!("Failed to run yt-dlp to extract creator handle: {}", e))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                    if let Some(uploader_id) = val["playlist_uploader_id"]
                        .as_str()
                        .or_else(|| val["uploader_id"].as_str())
                    {
                        let h = if uploader_id.starts_with('@') {
                            uploader_id.to_string()
                        } else {
                            format!("@{}", uploader_id)
                        };
                        return Ok(h);
                    }
                    if let Some(uploader_url) = val["uploader_url"].as_str() {
                        if let Some(pos) = uploader_url.find("/@") {
                            let rem = &uploader_url[pos + 1..];
                            let h = rem.split('/').next().unwrap_or("");
                            if !h.is_empty() {
                                let clean = if h.starts_with('@') { h.to_string() } else { format!("@{}", h) };
                                return Ok(clean);
                            }
                        }
                    }
                }
            }
        }

        Err(format!("Could not extract creator handle from target: {}", target))
    }

    /// Silently fetches the view counts of the last 30 videos from the creator's main channel root (e.g. /videos or /shorts)
    /// and calculates the True Channel Baseline using rolling median and IQR winsorization.
    pub fn sample_channel_root_baseline(channel_handle: &str) -> Result<(u64, BaselineDiagnostics), String> {
        let clean_handle = channel_handle.trim_start_matches('@');
        let videos_url = format!("https://www.youtube.com/@{}/videos", clean_handle);
        let shorts_url = format!("https://www.youtube.com/@{}/shorts", clean_handle);

        let fetch_views = |url: &str| -> Result<Vec<u64>, String> {
            let output = Command::new("yt-dlp")
                .args([
                    "--flat-playlist",
                    "--dump-json",
                    "--no-warnings",
                    "--playlist-end",
                    "30",
                    url,
                ])
                .output()
                .or_else(|_| {
                    Command::new("uvx")
                        .args([
                            "yt-dlp",
                            "--flat-playlist",
                            "--dump-json",
                            "--no-warnings",
                            "--playlist-end",
                            "30",
                            url,
                        ])
                        .output()
                })
                .map_err(|e| format!("Failed to execute yt-dlp on channel root: {}", e))?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut views = Vec::new();
            for line in stdout.lines() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                    if let Some(v) = val["view_count"].as_u64() {
                        views.push(v);
                    }
                }
            }
            Ok(views)
        };

        // Try /videos first; if empty, fallback to /shorts
        let mut views = fetch_views(&videos_url).unwrap_or_default();
        if views.is_empty() {
            views = fetch_views(&shorts_url).unwrap_or_default();
        }

        if views.is_empty() {
            return Err(format!("Could not fetch historical video views from channel root for {}", channel_handle));
        }

        let diag = BaselineEngine::compute_baseline_diagnostics(&views)
            .ok_or_else(|| "Failed to compute baseline diagnostics on channel root views".to_string())?;
        let baseline = diag.winsorized_baseline;
        Ok((baseline, diag))
    }

    /// Auto-calibrates the True Channel Baseline by sampling the channel root and saves it in SQLite `channels`.
    pub fn auto_calibrate_and_save_channel_baseline(
        db: &SandsifterDb,
        target_or_handle: &str,
    ) -> Result<u64, String> {
        let handle = Self::extract_creator_handle(target_or_handle)
            .unwrap_or_else(|_| target_or_handle.to_string());
        let norm_handle = if handle.starts_with('@') { handle } else { format!("@{}", handle) };

        info!("🎯 Auto-calibrating True Channel Baseline for {} from channel root...", norm_handle);
        let (baseline, _diag) = Self::sample_channel_root_baseline(&norm_handle)?;

        // Update or insert channel in SQLite
        let now_iso = "2026-03-01T12:00:00Z";
        let channel_uuid = format!("chan_{:x}", md5_hash(&norm_handle));
        let display_name = norm_handle.trim_start_matches('@').to_string();

        let updated = db.update_channel_baseline(&norm_handle, baseline).unwrap_or(false);
        if !updated {
            let channel_obj = WatchlistChannel {
                uuid: channel_uuid,
                platform: "YouTube".to_string(),
                handle: norm_handle.clone(),
                display_name: Some(display_name),
                avatar_url: None,
                follower_count: None,
                baseline_median_views: baseline,
                video_count: Some(30),
                last_crawled_at: Some(now_iso.to_string()),
                added_at: now_iso.to_string(),
            };
            let _ = db.upsert_channel(&channel_obj);
        }

        info!("✅ True Channel Baseline for {} calibrated & saved: {} views", norm_handle, baseline);
        Ok(baseline)
    }

    /// Synchronous wrapper for crawling a channel or playlist.
    pub fn crawl_channel(
        db: &SandsifterDb,
        target: &str,
        max_videos: usize,
    ) -> Result<CrawlSummary, String> {
        let db_clone = db.clone();
        let target_str = target.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            rt.block_on(Self::crawl_channel_async(&db_clone, &target_str, max_videos, false))
        })
        .join()
        .map_err(|_| "Crawl execution panicked".to_string())?
    }

    /// Asynchronous crawler supporting flat-playlist metadata extraction and deep audio extraction.
    pub async fn crawl_channel_async(
        db: &SandsifterDb,
        target: &str,
        max_videos: usize,
        deep_audio: bool,
    ) -> Result<CrawlSummary, String> {
        let (channel_url, mut normalized_handle) = Self::normalize_channel_url(target);
        let is_playlist = target.contains("playlist") || target.contains("list=");
        let platform = if is_playlist { "YouTube".to_string() } else { "YouTubeShorts".to_string() };

        info!("🛰️ Crawling {} via yt-dlp: {} (Max: {})", platform, channel_url, max_videos);

        let max_str = max_videos.to_string();
        let output = Command::new("yt-dlp")
            .args([
                "--flat-playlist",
                "--dump-json",
                "--no-warnings",
                "--playlist-end",
                &max_str,
                &channel_url,
            ])
            .output()
            .or_else(|_| {
                Command::new("uvx")
                    .args([
                        "yt-dlp",
                        "--flat-playlist",
                        "--dump-json",
                        "--no-warnings",
                        "--playlist-end",
                        &max_str,
                        &channel_url,
                    ])
                    .output()
            })
            .map_err(|e| format!("Failed to execute yt-dlp crawler: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp crawl failed: {}", err_msg.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut raw_items = Vec::new();
        let mut channel_name = normalized_handle.trim_start_matches('@').to_string();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(uploader) = val["playlist_uploader"]
                    .as_str()
                    .or_else(|| val["playlist_channel"].as_str())
                    .or_else(|| val["uploader"].as_str())
                    .or_else(|| val["channel"].as_str())
                {
                    channel_name = uploader.to_string();
                }

                if let Some(handle) = val["playlist_uploader_id"]
                    .as_str()
                    .or_else(|| val["uploader_id"].as_str())
                {
                    if normalized_handle == "@creator" {
                        normalized_handle = if handle.starts_with('@') {
                            handle.to_string()
                        } else {
                            format!("@{}", handle)
                        };
                    }
                }

                let title = val["title"].as_str().unwrap_or("Untitled").to_string();
                let views = val["view_count"].as_u64().unwrap_or(0);
                let url = val["url"]
                    .as_str()
                    .or_else(|| val["webpage_url"].as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        if let Some(id) = val["id"].as_str() {
                            format!("https://www.youtube.com/watch?v={}", id)
                        } else {
                            channel_url.clone()
                        }
                    });

                raw_items.push((title, url, views));
            }
        }

        if raw_items.is_empty() {
            return Err(format!("No entries found for target '{}'", target));
        }

        // 1. Calculate Baselines (Dual-Baseline Architecture)
        let view_counts: Vec<u64> = raw_items.iter().map(|(_, _, v)| *v).collect();
        let diagnostics = BaselineEngine::compute_baseline_diagnostics(&view_counts);
        let series_baseline = diagnostics
            .as_ref()
            .map(|d| d.winsorized_baseline)
            .unwrap_or(10_000);

        // Auto-calibrate True Channel Baseline when crawling a playlist
        let channel_baseline = if is_playlist {
            info!("🎯 Playlist detected! Auto-calibrating True Channel Baseline from channel root for {}...", normalized_handle);
            match Self::auto_calibrate_and_save_channel_baseline(db, &normalized_handle) {
                Ok(b) => {
                    info!("✅ True Channel Baseline for {} auto-calibrated: {} views (Series Baseline: {} views)", normalized_handle, b, series_baseline);
                    b
                }
                Err(e) => {
                    tracing::warn!("Failed to sample channel root ({}); falling back to series baseline.", e);
                    series_baseline
                }
            }
        } else {
            series_baseline
        };

        let now_iso = "2026-03-01T12:00:00Z";
        let channel_uuid = format!("chan_{:x}", md5_hash(&normalized_handle));

        // 2. Upsert Channel in SQLite
        let channel_obj = WatchlistChannel {
            uuid: channel_uuid.clone(),
            platform: platform.clone(),
            handle: normalized_handle.clone(),
            display_name: Some(channel_name.clone()),
            avatar_url: None,
            follower_count: None,
            baseline_median_views: channel_baseline,
            video_count: Some(raw_items.len() as u64),
            last_crawled_at: Some(now_iso.to_string()),
            added_at: now_iso.to_string(),
        };
        db.upsert_channel(&channel_obj)
            .map_err(|e| format!("Database error upserting channel: {}", e))?;

        // 3. Process each video, compare with previous snapshot to calculate velocity
        let mut crawled_videos = Vec::new();
        let mut viral_outliers = Vec::new();
        let mut accelerating_videos = Vec::new();
        let transcriber = AudioTranscriber::default();
        let classifier = NarrativeClassifier::default();
        let embedding_client = crate::storage::gemini_client::GeminiEmbeddingClient::default();

        for (idx, (title, url, views)) in raw_items.into_iter().enumerate() {
            let video_uuid = format!("vid_{:x}", md5_hash(&url));
            let (channel_multiplier, series_multiplier) = crate::math::ForensicsEngine::calculate_dual_multipliers(
                views,
                channel_baseline,
                series_baseline,
            );
            let multiplier = channel_multiplier;
            let is_viral = multiplier >= 2.0;

            // Check previous snapshot for velocity calculation
            let previous_snapshot = db.get_latest_snapshot(&video_uuid).ok().flatten();
            let previous_views = previous_snapshot.as_ref().map(|s| s.view_count);
            let view_growth = previous_views
                .map(|prev| views.saturating_sub(prev))
                .unwrap_or(0);
            let is_accelerating = view_growth > 0;

            // Deep Audio Extraction (Voice Activity Detection + Gemini STT)
            let mut spoken_hook_opt = None;
            let mut hook_madlib_opt = None;
            let mut speech_onset_opt = None;
            let mut wpm_opt = None;
            let mut analysis_opt = None;

            if deep_audio {
                info!("🎙️ [{}/{}] Deep audio extraction: \"{}\"", idx + 1, max_videos, title);
                match StreamIngestor::extract_audio_slice_and_pcm_ytdlp(&url, 0, 20) {
                    Ok((audio_bytes, mime, pcm_3_5s)) => {
                        let vad = VadClassifier::default();
                        let has_speech = vad.has_human_speech_in_0_to_35s(&pcm_3_5s);

                        if !has_speech {
                            info!("  🔇 No human speech detected in 0.0s-3.5s (Ambient/B-roll). Tagging as [NON-SPEECH / AMBIENT].");
                            spoken_hook_opt = Some("[NON-SPEECH / AMBIENT]".to_string());
                            hook_madlib_opt = None;
                            speech_onset_opt = None;
                            wpm_opt = Some(0.0);
                            analysis_opt = None; // Task 2: Do NOT let LLM grade narrative friction on non-speech!
                        } else {
                            let transcript = transcriber.transcribe_audio(&audio_bytes, &mime).await;

                            // Measure opening speaking pace over a full 5.0s post-onset speech window
                            let all_words: Vec<_> = transcript
                                .sentences
                                .iter()
                                .flat_map(|s| s.words.iter())
                                .collect();

                            let (t_silence, wpm_val) = if let Some(first_word) = all_words.first() {
                                let onset_ms = first_word.start_ms;
                                let window_end_ms = onset_ms + 5000;
                                let words_in_5s: Vec<_> = all_words
                                    .iter()
                                    .filter(|w| w.start_ms >= onset_ms && w.start_ms < window_end_ms)
                                    .collect();
                                let active_duration_sec = 5.0f64;
                                let wpm = crate::math::forensics::ForensicsEngine::calculate_stabilized_wpm(words_in_5s.len(), active_duration_sec);
                                (Some(onset_ms), wpm)
                            } else {
                                let onset = vad.detect_speech_onset_ms(&pcm_3_5s);
                                (onset, 0.0)
                            };

                            speech_onset_opt = t_silence;
                            wpm_opt = Some(wpm_val);

                            let hook_res = HookAligner::default().extract_opening_hook(&transcript);
                            let hook_text = hook_res.as_ref().map(|h| h.spoken_hook.clone()).unwrap_or_else(|| {
                                transcript.sentences.first().map(|s| s.text.clone()).unwrap_or_else(|| title.clone())
                            });

                            let madlib = crate::analyzer::synthesize_madlib(&hook_text);
                            let analysis = classifier.classify(&transcript, hook_res.as_ref(), &[], Some("Outdoor / Adventure")).await;

                            if !hook_text.trim().is_empty() {
                                let _ = db.index_hook_with_embedding(
                                    &video_uuid,
                                    &hook_text,
                                    Some(&madlib),
                                    multiplier.max(1.0),
                                    &embedding_client,
                                ).await;
                            }

                            spoken_hook_opt = Some(hook_text);
                            hook_madlib_opt = Some(madlib);
                            analysis_opt = Some(analysis);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Deep audio extraction skipped for {}: {}", url, e);
                    }
                }
            } else if is_viral && !title.trim().is_empty() {
                let madlib = crate::analyzer::synthesize_madlib(&title);
                let _ = db.index_hook_with_embedding_blocking(&video_uuid, &title, Some(&madlib), 3.0, &embedding_client);
            }

            let video_item = CrawledVideoItem {
                title: title.clone(),
                video_url: url.clone(),
                views,
                previous_views,
                view_growth,
                channel_multiplier,
                series_multiplier,
                outlier_multiplier: multiplier,
                is_viral_outlier: is_viral,
                is_accelerating,
                published_at: Some(now_iso.to_string()),
                spoken_hook: spoken_hook_opt.clone(),
                hook_madlib: hook_madlib_opt,
                speech_onset_ms: speech_onset_opt,
                wpm_0_to_35: wpm_opt,
            };

            if is_viral {
                viral_outliers.push(video_item.clone());
            }
            if is_accelerating {
                accelerating_videos.push(video_item.clone());
            }
            crawled_videos.push(video_item);

            // Upsert video record
            let video_obj = VideoDetail {
                uuid: video_uuid.clone(),
                caption: Some(title.clone()),
                url,
                thumbnail: None,
                channel: VideoChannelDetails {
                    uuid: channel_uuid.clone(),
                    platform: platform.clone(),
                    handle: normalized_handle.clone(),
                    display_name: Some(channel_name.clone()),
                    avatar_url: None,
                    follower_count: None,
                    baseline_median_views: Some(channel_baseline),
                },
                view_count: views,
                like_count: None,
                comment_count: None,
                share_count: None,
                outlier_score: multiplier,
                engagement_rate: 0.0,
                published_at: now_iso.to_string(),
                velocity_views_per_day: Some(crate::math::ForensicsEngine::calculate_velocity(views, Some(now_iso))),
                analysis: analysis_opt,
                speech_onset_ms: speech_onset_opt,
                wpm_0_to_35: wpm_opt,
                spoken_hook: spoken_hook_opt,
            };
            let _ = db.upsert_video(&video_obj);

            // Record new performance snapshot
            let snap_uuid = format!("snap_{:x}", md5_hash(&format!("{}_{}", video_uuid, views)));
            let snap = VideoSnapshot {
                uuid: snap_uuid,
                video_uuid,
                view_count: views,
                like_count: None,
                comment_count: None,
                share_count: None,
                recorded_at: now_iso.to_string(),
            };
            let _ = db.insert_snapshot(&snap);
        }

        // Sort viral outliers descending by multiplier
        viral_outliers.sort_by(|a, b| b.outlier_multiplier.partial_cmp(&a.outlier_multiplier).unwrap_or(std::cmp::Ordering::Equal));

        // Formulate personalized creator takeaway
        let takeaway = if let Some(top) = viral_outliers.first() {
            format!(
                "💡 **Actionable Hook Formula for Creators**:\n\
                The top breakout hit was *\"{}\"* ({:.1}x virality).\n\
                Apply this pattern to your projects: Introduce high-stakes curiosity or dramatic tension immediately at 0.0s, \
                keep the opening hook punchy, and deliver the resolution within the first chapter.",
                top.title, top.outlier_multiplier
            )
        } else {
            "💡 **Takeaway**: Consistent baseline observed. Test high-contrast curiosity or contrarian warning hooks to spike viewer velocity."
                .to_string()
        };

        Ok(CrawlSummary {
            channel_handle: normalized_handle,
            channel_name,
            platform,
            total_shorts_scanned: crawled_videos.len(),
            author_baseline_views: channel_baseline,
            viral_outliers,
            accelerating_videos,
            all_videos: crawled_videos,
            takeaway_for_creator: takeaway,
            diagnostics,
        })
    }

    /// Crawls recent videos for a given topic across YouTube to evaluate topic momentum.
    /// Runs yt-dlp to scrape recent videos matching topic within days_back window.
    pub fn crawl_topic_recent(
        topic: &str,
        limit: usize,
        days_back: u32,
    ) -> Result<Vec<crate::math::topics::TopicVideoInput>, String> {
        let max_items = limit.max(5);
        let date_filter = format!("today-{}days", days_back.max(1));
        let search_target = format!("ytsearch{}:{}", max_items, topic);

        info!("🌐 Scraping topic cohort for '{}' (Max: {}, Days: {})", topic, max_items, days_back);

        // Attempt 1: yt-dlp with dateafter filter
        let mut cmd = Command::new("yt-dlp");
        cmd.args([
            &search_target,
            "--dateafter",
            &date_filter,
            "--dump-json",
            "--no-warnings",
            "--no-playlist",
        ]);

        let output_res = cmd.output().or_else(|_| {
            let mut uv_cmd = Command::new("uvx");
            uv_cmd.args([
                "yt-dlp",
                &search_target,
                "--dateafter",
                &date_filter,
                "--dump-json",
                "--no-warnings",
                "--no-playlist",
            ]);
            uv_cmd.output()
        });

        let mut lines = Vec::new();
        if let Ok(output) = output_res {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for l in stdout.lines() {
                    let trimmed = l.trim();
                    if !trimmed.is_empty() {
                        lines.push(trimmed.to_string());
                    }
                }
            }
        }

        // Attempt 2: Fallback to YouTube upload-date sorted search URL if relevance search returned empty
        if lines.is_empty() {
            let encoded_topic = topic.replace(' ', "+");
            let search_url = format!("https://www.youtube.com/results?search_query={}&sp=CAI%253D", encoded_topic);
            let limit_str = max_items.to_string();

            let fallback_output = Command::new("yt-dlp")
                .args([
                    &search_url,
                    "--flat-playlist",
                    "--dump-json",
                    "--playlist-end",
                    &limit_str,
                    "--no-warnings",
                ])
                .output()
                .or_else(|_| {
                    Command::new("uvx")
                        .args([
                            "yt-dlp",
                            &search_url,
                            "--flat-playlist",
                            "--dump-json",
                            "--playlist-end",
                            &limit_str,
                            "--no-warnings",
                        ])
                        .output()
                });

            if let Ok(out) = fallback_output {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for l in stdout.lines() {
                        let trimmed = l.trim();
                        if !trimmed.is_empty() {
                            lines.push(trimmed.to_string());
                        }
                    }
                }
            }
        }

        if lines.is_empty() {
            return Err(format!("No video metadata returned by yt-dlp for topic '{}'", topic));
        }

        let mut results = Vec::new();
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for line in lines {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                let title = val["title"].as_str().unwrap_or("Untitled Video").to_string();
                let description = val["description"].as_str().unwrap_or("").to_string();
                let channel = val["uploader"]
                    .as_str()
                    .or_else(|| val["channel"].as_str())
                    .unwrap_or("Unknown Creator")
                    .to_string();
                let video_id = val["id"].as_str().unwrap_or("");
                let video_url = if let Some(u) = val["webpage_url"].as_str().or_else(|| val["url"].as_str()) {
                    u.to_string()
                } else if !video_id.is_empty() {
                    format!("https://www.youtube.com/watch?v={}", video_id)
                } else {
                    "".to_string()
                };

                let views = val["view_count"].as_u64().unwrap_or(0);

                // Determine days since upload
                let days_since_upload = if let Some(ts) = val["timestamp"].as_u64() {
                    if now_epoch >= ts {
                        ((now_epoch - ts) as f64 / 86400.0).max(0.1)
                    } else {
                        1.0
                    }
                } else if let Some(date_str) = val["upload_date"].as_str() {
                    if date_str.len() == 8 {
                        let y = date_str[0..4].parse::<i64>().unwrap_or(2026);
                        let m = date_str[4..6].parse::<i64>().unwrap_or(1);
                        let d = date_str[6..8].parse::<i64>().unwrap_or(1);
                        let diff_days = ((2026 - y) * 365 + (10 - m) * 30 + (10 - d)).max(1);
                        diff_days as f64
                    } else {
                        (days_back as f64 / 2.0).max(1.0)
                    }
                } else {
                    (days_back as f64 / 2.0).max(1.0)
                };

                results.push(crate::math::topics::TopicVideoInput::new(
                    title,
                    description,
                    channel,
                    video_url,
                    views,
                    days_since_upload,
                ));
            }
        }

        Ok(results)
    }
}

/// Evaluated video item resulting from rapid 50-batch niche scans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NicheScoutVideoItem {
    pub video_id: String,
    pub title: String,
    pub channel_handle: String,
    pub views: u64,
    pub published_at: Option<String>,
    pub hours_elapsed: f64,
    /// 48-hour velocity derivative: dV/dt (views per hour over the first 48 hours).
    pub velocity_derivative_48h: f64,
    pub is_48h_breakout: bool,
}

/// High-throughput, 50x quota-engineered batch crawler & niche velocity scanner.
/// Batches up to 50 video IDs per request to fully saturate API quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NicheOutlierScout;

impl NicheOutlierScout {
    /// Quota engineering batch ceiling (50 IDs per request for 1 quota unit).
    pub const BATCH_SIZE: usize = 50;
    /// Default breakout view velocity threshold (views/hour).
    pub const DEFAULT_48H_BREAKOUT_VELOCITY: f64 = 250.0;

    /// Chunks a collection of video IDs into batches of up to 50 IDs.
    pub fn chunk_video_ids(ids: &[String]) -> Vec<Vec<String>> {
        ids.chunks(Self::BATCH_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Calculates the 48-hour velocity derivative: dV/dt (views per hour).
    /// Clamps effective measurement window to 48.0 hours and lower-bounds to 0.1h to prevent division-by-zero.
    pub fn calculate_48h_velocity(views: u64, hours_elapsed: f64) -> f64 {
        let effective_hours = hours_elapsed.clamp(0.1, 48.0);
        let velocity = (views as f64) / effective_hours;
        (velocity * 10.0).round() / 10.0
    }

    /// Evaluates if a 48-hour velocity derivative meets the breakout candidate threshold.
    pub fn is_48h_breakout(velocity: f64, threshold: f64) -> bool {
        velocity >= threshold
    }

    /// Parses raw yt-dlp flat-playlist or batched API JSON lines into structured NicheScoutVideoItems.
    pub fn parse_batch_json(
        stdout: &str,
        channel_handle: &str,
        breakout_threshold_velocity: f64,
    ) -> Vec<NicheScoutVideoItem> {
        let mut items = Vec::new();
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(1760000000);

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let video_id = val["id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let title = val["title"].as_str().unwrap_or("Untitled").to_string();
                let views = val["view_count"].as_u64().unwrap_or(0);

                let (hours_elapsed, published_at) = if let Some(ts) = val["timestamp"].as_u64() {
                    let elapsed_sec = now_epoch.saturating_sub(ts);
                    let hours = (elapsed_sec as f64 / 3600.0).max(0.1);
                    (hours, Some(format!("{}", ts)))
                } else if let Some(upload_date) = val["upload_date"].as_str() {
                    (24.0, Some(upload_date.to_string()))
                } else {
                    (24.0, None)
                };

                let velocity = Self::calculate_48h_velocity(views, hours_elapsed);
                let is_breakout = Self::is_48h_breakout(velocity, breakout_threshold_velocity);

                items.push(NicheScoutVideoItem {
                    video_id,
                    title,
                    channel_handle: channel_handle.to_string(),
                    views,
                    published_at,
                    hours_elapsed: (hours_elapsed * 10.0).round() / 10.0,
                    velocity_derivative_48h: velocity,
                    is_48h_breakout: is_breakout,
                });
            }
        }
        items
    }
}


