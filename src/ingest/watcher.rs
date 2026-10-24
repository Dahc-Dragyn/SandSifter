use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Configuration options for the NLE Ambient Watch-Folder Daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    /// Directory to monitor for newly exported video drafts.
    pub watch_dir: PathBuf,
    /// Directory where Flight Sheets, FCPXML templates, and EDLs are generated.
    pub output_dir: PathBuf,
    /// Debounce duration to verify the rendering engine has finished writing the file.
    pub debounce_secs: u64,
    /// Polling fallback interval to ensure no drops even on network drives.
    pub poll_interval_secs: u64,
    /// Supported video file extensions (e.g. mp4, mov, mkv, webm).
    pub supported_extensions: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            watch_dir: PathBuf::from("."),
            output_dir: PathBuf::from("output"),
            debounce_secs: 2,
            poll_interval_secs: 2,
            supported_extensions: vec![
                "mp4".to_string(),
                "mov".to_string(),
                "mkv".to_string(),
                "webm".to_string(),
            ],
        }
    }
}

/// Metadata snapshot tracking when a file was last processed to avoid redundant re-analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSignature {
    size_bytes: u64,
    modified_time: SystemTime,
}

/// Result summary emitted upon successfully evaluating a newly exported draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatcherProcessResult {
    pub file_path: PathBuf,
    pub video_title: String,
    pub duration_seconds: f64,
    pub cuttable_fat_seconds: f64,
    pub punch_in_cues_count: usize,
    pub flight_sheet_path: PathBuf,
    pub fcpxml_path: PathBuf,
    pub edl_path: PathBuf,
}

/// Helper to verify if a path represents an eligible, complete video draft candidate.
pub fn is_candidate_video(path: &Path, extensions: &[String]) -> bool {
    if !path.is_file() {
        return false;
    }
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // Skip hidden files, system files, and partial render locks
    if file_name.starts_with('.')
        || file_name.starts_with('~')
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".crdownload")
        || file_name.ends_with(".part")
    {
        return false;
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        extensions.iter().any(|e| e.to_lowercase() == ext_lower)
    } else {
        false
    }
}

/// Checks whether a file is stable on disk (size not changing) and unlocked by the rendering NLE.
pub fn is_file_stable_and_unlocked(path: &Path, debounce_duration: Duration) -> bool {
    let metadata_1 = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let size_1 = metadata_1.len();
    if size_1 == 0 {
        return false;
    }

    // Attempt to open read-only. On Windows, exclusive write locks by NLEs block this.
    if File::open(path).is_err() {
        return false;
    }

    if !debounce_duration.is_zero() {
        std::thread::sleep(debounce_duration);
        let metadata_2 = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if metadata_2.len() != size_1 {
            return false;
        }
    }

    true
}

/// Evaluates a local draft video and exports all production assets (.md, .fcpxml, .edl).
pub fn process_video_draft(
    file_path: &Path,
    output_dir: &Path,
) -> Result<WatcherProcessResult, Box<dyn std::error::Error + Send + Sync>> {
    let path_str = file_path.to_string_lossy();
    let scorecard = crate::math::MarginalGainsEngine::evaluate_local_draft(&path_str)?;
    let flight_sheet = crate::export::FlightSheet::from_scorecard(&scorecard);

    std::fs::create_dir_all(output_dir)?;
    let flight_sheet_path = flight_sheet.save_to_markdown(output_dir)?;
    let _ = flight_sheet.save_html_report(output_dir, &scorecard);
    let fcpxml_path = flight_sheet.save_fcpxml_template(output_dir)?;
    let edl_path = flight_sheet.save_edl_template(output_dir)?;

    let cuttable_fat_seconds = scorecard.cuttable_fat.as_ref().map(|f| f.total_cuttable_seconds).unwrap_or(0.0);
    let punch_in_cues_count = scorecard.cuttable_fat.as_ref().map(|f| f.punch_in_cues.len()).unwrap_or(0);
    let duration_seconds = scorecard.duration_seconds.unwrap_or(0.0);

    Ok(WatcherProcessResult {
        file_path: file_path.to_path_buf(),
        video_title: scorecard.title,
        duration_seconds,
        cuttable_fat_seconds,
        punch_in_cues_count,
        flight_sheet_path,
        fcpxml_path,
        edl_path,
    })
}

/// The NLE Ambient Watch-Folder Daemon monitoring local export directories.
pub struct WatchFolderDaemon {
    pub config: WatcherConfig,
    processed_files: HashMap<PathBuf, FileSignature>,
}

impl WatchFolderDaemon {
    /// Creates a new daemon instance with the specified configuration.
    pub fn new(config: WatcherConfig) -> Self {
        Self {
            config,
            processed_files: HashMap::new(),
        }
    }

    /// Single-pass scan over the watch directory. Identifies and processes newly finished exports.
    pub fn scan_once(&mut self) -> Vec<WatcherProcessResult> {
        let mut results = Vec::new();
        let entries = match std::fs::read_dir(&self.config.watch_dir) {
            Ok(e) => e,
            Err(err) => {
                warn!("⚠️ Unable to read watch directory {}: {}", self.config.watch_dir.display(), err);
                return results;
            }
        };

        let debounce = Duration::from_secs(self.config.debounce_secs);

        for entry in entries.flatten() {
            let path = entry.path();
            if !is_candidate_video(&path, &self.config.supported_extensions) {
                continue;
            }

            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size_bytes = meta.len();
            let modified_time = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let current_sig = FileSignature {
                size_bytes,
                modified_time,
            };

            // Check if this exact file signature was already processed
            if let Some(existing_sig) = self.processed_files.get(&path) {
                if *existing_sig == current_sig {
                    continue;
                }
            }

            // Verify the file has stabilized and is not locked by the rendering engine
            if !is_file_stable_and_unlocked(&path, debounce) {
                info!("⏳ Video file '{}' is currently rendering or locked; debouncing...", path.display());
                continue;
            }

            info!("⚡ New NLE export detected: {}", path.display());
            match process_video_draft(&path, &self.config.output_dir) {
                Ok(res) => {
                    println!("\n╔════════════════════════════════════════════════════════════════════════════════════════════════════╗");
                    println!("║                      🎬 NEW NLE VIDEO RENDER DETECTED & ANALYZED                                  ║");
                    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════════╝");
                    println!(" 📁 Render File   : {}", res.file_path.display());
                    println!(" 📹 Title         : {}", res.video_title);
                    println!(" ⏱️ Runtime       : {:.1}s", res.duration_seconds);
                    println!(" ✂️ Cuttable Fat  : {:.1}s across {} punch-in cue markers", res.cuttable_fat_seconds, res.punch_in_cues_count);
                    println!(" 📄 Flight Sheet  : {}", res.flight_sheet_path.display());
                    println!(" 🎞️ FCPXML Guide  : {}", res.fcpxml_path.display());
                    println!(" 🎯 Marker EDL    : {}", res.edl_path.display());
                    println!("══════════════════════════════════════════════════════════════════════════════════════════════════════\n");

                    self.processed_files.insert(path, current_sig);
                    results.push(res);
                }
                Err(err) => {
                    error!("❌ Failed to process video draft '{}': {}", path.display(), err);
                }
            }
        }

        results
    }

    /// Runs the continuous ambient watch-folder daemon until SIGINT (Ctrl+C) or an explicit shutdown signal.
    pub async fn run_daemon(
        &mut self,
        mut shutdown_rx: Option<tokio::sync::broadcast::Receiver<()>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        std::fs::create_dir_all(&self.config.watch_dir)?;
        std::fs::create_dir_all(&self.config.output_dir)?;

        let (notify_tx, notify_rx) = channel();
        let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
            Ok(w) => {
                let mut w = w;
                if let Err(e) = w.watch(&self.config.watch_dir, RecursiveMode::NonRecursive) {
                    warn!("⚠️ Failed to mount filesystem notify watcher on {}: {}. Falling back to polling.", self.config.watch_dir.display(), e);
                }
                Some(w)
            }
            Err(e) => {
                warn!("⚠️ Failed to initialize notify watcher: {}. Running pure polling loop.", e);
                None
            }
        };

        println!("\n╔════════════════════════════════════════════════════════════════════════════════════════════════════╗");
        println!("║                      🏖️ SANDSIFTER NLE AMBIENT WATCH-FOLDER DAEMON                                ║");
        println!("╚════════════════════════════════════════════════════════════════════════════════════════════════════╝");
        println!(" 👁️ Watching Directory : {}", self.config.watch_dir.display());
        println!(" 📁 Output Directory   : {}", self.config.output_dir.display());
        println!(" ⏳ Render Debounce    : {}s", self.config.debounce_secs);
        println!(" 🔄 Poll Interval      : {}s", self.config.poll_interval_secs);
        println!(" 🎬 Supported Formats  : {}", self.config.supported_extensions.join(", "));
        println!(" Press Ctrl+C at any time to stop.\n");

        // Initial scan of any files already present
        self.scan_once();

        let poll_interval = Duration::from_secs(self.config.poll_interval_secs.max(1));

        loop {
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {
                    // Drain any pending notify events
                    let mut triggered_by_event = false;
                    while let Ok(event_res) = notify_rx.try_recv() {
                        if let Ok(event) = event_res {
                            if event.paths.iter().any(|p| is_candidate_video(p, &self.config.supported_extensions)) {
                                triggered_by_event = true;
                            }
                        }
                    }

                    // Scan directory if event triggered or periodic sweep
                    if triggered_by_event || self.config.poll_interval_secs > 0 {
                        self.scan_once();
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\n🛑 Received interrupt signal (Ctrl+C). Shutting down Sandsifter Watch-Folder Daemon.");
                    break;
                }
                _ = async {
                    if let Some(ref mut rx) = shutdown_rx {
                        let _ = rx.recv().await;
                    } else {
                        futures_util::future::pending::<()>().await;
                    }
                } => {
                    info!("🛑 Shutdown signal received by watch-folder daemon.");
                    break;
                }
            }
        }

        // Clean drop of watcher
        drop(watcher.take());
        Ok(())
    }
}
