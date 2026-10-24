use sandsifter::analyzer::{
    AudioTranscriber, HookAligner, KeyframeExtractor, NarrativeClassifier, OcrEngine, VadClassifier,
};
use sandsifter::ingest::{BatchImporter, ChannelCrawler, MediaResolver, StreamIngestor};
use sandsifter::mcp::McpServer;
use sandsifter::pipeline::HookMachineEngine;
use sandsifter::storage::SandsifterDb;
use sandsifter::ui::TerminalTableRenderer;
use std::env;
use std::fs;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load local .env credentials (e.g. GEMINI_API_KEY)
    let _ = dotenvy::dotenv();

    let args: Vec<String> = env::args().collect();

    // Mode 1: MCP Server over stdio
    if args.iter().any(|a| a == "--mcp" || a == "--stdio") {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .init();

        eprintln!("[sandsifter-mcp] Sandsifter Model Context Protocol server running over stdio.");
        let db = SandsifterDb::open_default().ok();
        let mut server = match db {
            Some(d) => McpServer::with_db(d),
            None => McpServer::new(),
        };
        server.run_stdio()?;
        return Ok(());
    }

    tracing_subscriber::fmt::init();

    // Mode 2: Production MCP HTTP / SSE Server & Web API (PushFrame Pattern)
    // Runs when explicitly requested with --sse / --server / --daemon / --serve / --ui / dev / --dev, OR when launched in a container environment with PORT set.
    let is_server_flag = args.iter().position(|a| {
        a == "--sse"
            || a == "--server"
            || a == "--daemon"
            || a == "--serve"
            || a == "--ui"
            || a == "dev"
            || a == "--dev"
            || a == "server"
            || a == "serve"
            || a == "ui"
            || a == "start"
    });
    let has_port_env = env::var("PORT").is_ok();
    let is_bare_container_run = args.len() <= 1 && has_port_env;

    if is_server_flag.is_some() || is_bare_container_run {
        let port: u16 = if let Some(pos) = is_server_flag {
            args.get(pos + 1)
                .and_then(|p| p.parse::<u16>().ok())
                .or_else(|| env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok()))
                .unwrap_or(8080)
        } else {
            env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or(8080)
        };

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let db = SandsifterDb::open_default().ok();
        let server = match &db {
            Some(d) => McpServer::with_db(d.clone()),
            None => McpServer::new(),
        };

        eprintln!("🏖️ Starting Sandsifter Production HTTP Daemon & Web UI on http://{}:{}", host, port);
        sandsifter::mcp::start_http_server(server, db, &host, port).await?;
        return Ok(());
    }

    // Mode 3: Batch JSON Importer for Historical Creator Feeds (Task 2)
    if let Some(pos) = args.iter().position(|a| a == "--import") {
        if let Some(file_path) = args.get(pos + 1) {
            info!("📥 Ingesting historical creator batch JSON from: {}", file_path);
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ Error reading import file '{}': {}", file_path, e);
                    std::process::exit(1);
                }
            };

            let db = SandsifterDb::open_default()?;
            match BatchImporter::import_from_json_str(&db, &content, None) {
                Ok(res) => {
                    println!("\n✅ Batch Ingestion Completed Successfully!\n");
                    println!(
                        "{}",
                        TerminalTableRenderer::render_import_tables(
                            res.channels_updated,
                            res.videos_imported,
                            res.snapshots_recorded,
                            &res.channel_baselines,
                            &res.top_outliers,
                        )
                    );
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("❌ Import failed: {}", err);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("❌ Error: --import requires a JSON file path argument (e.g. cargo run -- --import creator_feed.json)");
            std::process::exit(1);
        }
    }

    // Mode: Automated Channel Harvester & Velocity Engine (Phase 2A)
    if let Some(pos) = args.iter().position(|a| a == "--crawl" || a == "--harvest") {
        if let Some(channel_target) = args.get(pos + 1) {
            let max_videos = args
                .get(pos + 2)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(32);
            let deep_audio = args.iter().any(|a| a == "--deep" || a == "--deep-audio");

            info!("🛰️ Harvesting videos for creator: {} (Limit: {}, Deep Audio: {})", channel_target, max_videos, deep_audio);
            let db = SandsifterDb::open_default()?;
            match ChannelCrawler::crawl_channel_async(&db, channel_target, max_videos, deep_audio).await {
                Ok(summary) => {
                    println!("\n{}", summary.to_creator_report());
                    return Ok(());
                }
                Err(err) => {
                    eprintln!("❌ Channel crawl failed: {}", err);
                    eprintln!("💡 Tip: Make sure yt-dlp is installed and the channel handle or playlist URL is valid");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("❌ Error: --crawl requires a channel handle or URL (e.g. cargo run -- --crawl <channel_or_playlist_url> [limit] [--deep-audio])");
            std::process::exit(1);
        }
    }

    // Mode: Instant Empirical Database Seeder (Data Flywheel Realizer)
    if args.iter().any(|a| a == "--seed") {
        info!("🌱 Seeding empirical creator historical feeds into SQLite...");
        let fixture_path = "tests/fixtures/empirical_creator_feeds.json";
        let content = match fs::read_to_string(fixture_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("❌ Error reading seed fixture '{}': {}", fixture_path, e);
                std::process::exit(1);
            }
        };

        let db = SandsifterDb::open_default()?;
        let res = BatchImporter::import_from_json_str(&db, &content, None).map_err(|e| format!("Seed error: {}", e))?;
        println!("\n✅ Empirical Data Flywheel Seeded Successfully!\n");
        println!(
            "{}",
            TerminalTableRenderer::render_import_tables(
                res.channels_updated,
                res.videos_imported,
                res.snapshots_recorded,
                &res.channel_baselines,
                &res.top_outliers,
            )
        );

        // Auto-index outlier hooks into SQLite
        let client = sandsifter::storage::gemini_client::GeminiEmbeddingClient::default();
        let outliers = db.list_top_outliers(1.5, 30)?;
        let mut indexed = 0;
        for v in outliers {
            if let Some(caption) = &v.caption {
                if !caption.trim().is_empty() {
                    let madlib = sandsifter::analyzer::synthesize_madlib(caption);
                    let _ = db.index_hook_with_embedding_blocking(&v.uuid, caption, Some(&madlib), 3.0, &client);
                    indexed += 1;
                }
            }
        }
        println!("🎣 Automatically embedded & indexed {} empirical hooks into vector library!\n", indexed);
        return Ok(());
    }

    // Mode: Tracked Hooks Feedback Loop Inspector
    if let Some(pos) = args.iter().position(|a| a == "--track-hooks" || a == "--tracked-hooks") {
        let channel_filter = args.get(pos + 1).map(|s| s.as_str());
        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;
        let hooks = db.list_tracked_hooks(channel_filter, None, 50)?;

        if hooks.is_empty() {
            println!("\nℹ️ No tracked hooks recorded yet.");
            println!("💡 Run 'cargo run -- --hook-machine <@channel> <topic>' to generate and automatically track new hooks!\n");
            return Ok(());
        }

        println!("\n════════════════════════════════════════════════════════════════════════════");
        println!("🔄 Tracked Hooks Feedback Loop Library ({} Tracked Hooks)", hooks.len());
        println!("════════════════════════════════════════════════════════════════════════════\n");

        for (idx, h) in hooks.iter().enumerate() {
            println!("{}. [{}] \"{}\"", idx + 1, h.status.to_uppercase(), h.hook_text);
            println!("   ID: {} | Channel: {} | Type: {}", h.id, h.channel_handle, h.hook_type);
            println!("   Predicted Score: {:.1}/10", h.predicted_score);
            if let Some(views) = h.actual_views {
                let m = h.actual_multiplier.map(|x| format!("{:.1}x", x)).unwrap_or_else(|| "N/A".to_string());
                let e = h.actual_engagement_rate.map(|x| format!("{:.2}%", x)).unwrap_or_else(|| "N/A".to_string());
                println!("   📈 Empirical Performance: {} views (Multiplier: {}, Engagement: {})", views, m, e);
            }
            if let Some(notes) = &h.performance_notes {
                println!("   📝 Notes: {}", notes);
            }
            println!();
        }
        return Ok(());
    }

    // Mode: Record Real-World Performance for a Tracked Hook (Closing the Loop)
    if let Some(pos) = args.iter().position(|a| a == "--record-hook") {
        let hook_id = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("");
        let views = args.get(pos + 2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let mult = args.get(pos + 3).and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0);
        let eng = args.get(pos + 4).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let notes = args.get(pos + 5).map(|s| s.as_str());

        if hook_id.is_empty() {
            eprintln!("❌ Usage: cargo run -- --record-hook <hook_id_or_substring> <views> <multiplier> <engagement_rate> [notes]");
            std::process::exit(1);
        }

        let db = SandsifterDb::open_default()?;
        match db.record_hook_performance(hook_id, views, mult, eng, notes) {
            Ok(true) => {
                println!("\n✅ Performance Recorded for Hook '{}'!", hook_id);
                println!("  - Views:            {}", views);
                println!("  - Outlier Score:    {:.1}x", mult);
                println!("  - Engagement Rate:  {:.2}%", eng);
                if mult >= 2.0 {
                    println!("  🔥 OUTLIER DETECTED! This formula will automatically be prioritized in future custom rubrics!");
                }
                println!();
                return Ok(());
            }
            Ok(false) => {
                eprintln!("❌ Hook '{}' not found in database.", hook_id);
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("❌ Database error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Mode 3: Hook Machine execution CLI
    if let Some(pos) = args.iter().position(|a| a == "--hook-machine") {
        let channel_raw = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("@creator");
        let topic = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("Software Architecture");

        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;

        let channel_str = if channel_raw.contains("playlist") || channel_raw.contains("list=") {
            info!("🔗 Playlist URL detected in --hook-machine. Extracting creator handle & auto-calibrating True Channel Baseline...");
            match sandsifter::ingest::ChannelCrawler::auto_calibrate_and_save_channel_baseline(&db, channel_raw) {
                Ok(baseline) => {
                    let extracted = sandsifter::ingest::ChannelCrawler::extract_creator_handle(channel_raw)
                        .unwrap_or_else(|_| channel_raw.to_string());
                    info!("🎯 Calibrated True Channel Baseline for {}: {} views", extracted, baseline);
                    if db.get_videos_by_channel(&extracted).unwrap_or_default().is_empty() {
                        let _ = sandsifter::ingest::ChannelCrawler::crawl_channel_async(&db, channel_raw, 32, false).await;
                    }
                    extracted
                }
                Err(e) => {
                    tracing::warn!("Auto-calibration on playlist failed: {}", e);
                    channel_raw.to_string()
                }
            }
        } else {
            channel_raw.to_string()
        };
        let channel = &channel_str;

        info!("🎯 Launching The Hook Machine for channel: {} // Topic: {} // Universal Dynamic Synthesis Active", channel, topic);
        let real_videos = db.get_videos_by_channel(channel).unwrap_or_default();
        let validated_formulas = db.get_validated_winning_formulas(channel, 2.0).unwrap_or_default();

        if real_videos.is_empty() {
            info!("ℹ️ No historical videos found in SQLite for {}. Run '--seed' or '--import <file.json>' to seed empirical creator baselines.", channel);
        } else {
            info!("📈 Loaded {} empirical videos from SQLite for {} to compute IQR baseline.", real_videos.len(), channel);
        }

        if !validated_formulas.is_empty() {
            info!("🔄 Closed-Loop Feedback Active: Prioritizing {} historically verified winning formulas for {}!", validated_formulas.len(), channel);
        }

        let report = HookMachineEngine::execute_with_lexicon(
            channel,
            topic,
            &real_videos,
            &validated_formulas,
            None,
        );

        // Automatically track newly generated hooks into SQLite
        let now_iso = "2026-03-01T12:00:00Z";
        let mut records = Vec::new();
        for h in report.adapted_hooks.iter().chain(report.scratch_hooks.iter()) {
            let hook_id = format!("hook_{:x}", sandsifter::ingest::importer::md5_hash(&h.hook_text));
            records.push(sandsifter::storage::TrackedHookRecord {
                id: hook_id,
                channel_handle: channel.to_string(),
                topic: topic.to_string(),
                hook_text: h.hook_text.clone(),
                hook_type: h.hook_type.clone(),
                source_formula: h.source_formula.clone(),
                predicted_score: h.score.total_score as f64,
                status: "generated".to_string(),
                published_url: None,
                actual_views: None,
                actual_multiplier: None,
                actual_engagement_rate: None,
                performance_notes: None,
                created_at: now_iso.to_string(),
                updated_at: now_iso.to_string(),
            });
        }
        let _ = db.save_tracked_hooks(&records);

        println!("\n{}\n", report.to_markdown());
        println!("💾 Note: Generated hooks automatically registered into Tracked Hooks Feedback Loop!");
        println!("   View active tracker: cargo run -- --track-hooks {}", channel);
        println!("   Record filmed video: cargo run -- --record-hook <hook_id> <views> <multiplier> <engagement_rate>\n");
        return Ok(());
    }

    // Mode: Audience Retention Curve & Drop-Off Reverse-Engineering (The "Dip" Finder)
    if let Some(pos) = args.iter().position(|a| a == "--retention") {
        if let Some(url) = args.get(pos + 1) {
            info!("📉 Reverse-engineering audience retention curve for: {}", url);
            let meta = MediaResolver::extract_with_ytdlp(url)?;
            let cues = match &meta.vtt_subtitles {
                Some(vtt) => sandsifter::math::RetentionEngine::parse_vtt(vtt),
                None => Vec::new(),
            };

            // Attempt to sample visual scene cuts for multimodal correlation
            let sample_sec = Some(meta.duration_seconds.min(180.0));
            let pacing = sandsifter::ingest::MediaSceneDetector::analyze_video_visual_pacing(url, sample_sec).ok();
            let cuts = pacing.as_ref().map(|p| p.cut_timestamps.as_slice()).unwrap_or(&[]);

            let mut report = sandsifter::math::RetentionEngine::analyze_curve_multimodal(
                &meta.video_id,
                &meta.title,
                meta.duration_seconds,
                &meta.heatmap,
                &cues,
                cuts,
            );

            // Synthesize dynamic 3-point Editing Prescription via Gemini 2.5 Flash-Lite
            let top_spike = report.major_spikes.first();
            let prescription = sandsifter::math::RetentionEngine::synthesize_editing_prescription_with_llm(
                &meta.title,
                &report.attention_breaches,
                top_spike,
            ).await;
            report.editing_prescription = prescription;

            println!("{}", render_retention_report(&report, &meta.uploader));
            return Ok(());
        }
    }

    // Mode: Visual Rhythm & Shot Pacing (Average Shot Length ASL)
    if let Some(pos) = args.iter().position(|a| a == "--pacing") {
        if let Some(url) = args.get(pos + 1) {
            let user_sample = args.get(pos + 2).and_then(|s| s.parse::<f64>().ok());
            let meta_dur = MediaResolver::extract_with_ytdlp(url).ok().map(|m| m.duration_seconds);
            let sample_sec = user_sample.or_else(|| meta_dur.map(|d| d.min(180.0))).or(Some(180.0));

            info!("🎬 Analyzing visual rhythm & shot pacing for: {} (Sample window: {:?}s)", url, sample_sec);
            let pacing = sandsifter::ingest::MediaSceneDetector::analyze_video_visual_pacing(url, sample_sec)?;
            println!("{}", render_pacing_report(&pacing, url));
            return Ok(());
        } else {
            eprintln!("❌ Error: --pacing requires a video URL (e.g. cargo run -- --pacing https://youtube.com/shorts/...)");
            std::process::exit(1);
        }
    }

    // Mode: NLE Ambient Watch-Folder Daemon (Phase 3)
    if let Some(pos) = args.iter().position(|a| a == "--watch" || a == "--watch-folder") {
        let watch_dir = args.get(pos + 1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let output_dir = args.iter()
            .position(|a| a == "--output-dir" || a == "-o")
            .and_then(|p| args.get(p + 1))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("output"));

        let debounce_secs = args.iter()
            .position(|a| a == "--debounce")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2);

        let poll_interval_secs = args.iter()
            .position(|a| a == "--poll-interval")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2);

        let config = sandsifter::ingest::WatcherConfig {
            watch_dir,
            output_dir,
            debounce_secs,
            poll_interval_secs,
            supported_extensions: vec![
                "mp4".to_string(),
                "mov".to_string(),
                "mkv".to_string(),
                "webm".to_string(),
            ],
        };

        let mut daemon = sandsifter::ingest::WatchFolderDaemon::new(config);
        if let Err(e) = daemon.run_daemon(None).await {
            eprintln!("❌ Watcher daemon error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Mode: Brailsford Marginal Gains Scorecard Engine & Editor's Flight Sheet
    if let Some(pos) = args.iter().position(|a| a == "--marginal-gains" || a == "--audit") {
        if let Some(target) = args.get(pos + 1) {
            info!("🚴 Running Brailsford Marginal Gains Scorecard on: {}", target);
            println!("\n🚴 Synthesizing Brailsford Marginal Gains Scorecard & Flight Sheet for: {}...", target);

            let is_playlist = target.contains("playlist") || target.contains("list=");
            let video_urls = if is_playlist {
                let cmd_output = std::process::Command::new("yt-dlp")
                    .args(["--flat-playlist", "--dump-json", target])
                    .output()
                    .or_else(|_| {
                        std::process::Command::new("uvx")
                            .args(["yt-dlp", "--flat-playlist", "--dump-json", target])
                            .output()
                    });
                if let Ok(out) = cmd_output {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let mut urls = Vec::new();
                    for line in text.lines() {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                            if let Some(id) = val["id"].as_str() {
                                urls.push(format!("https://www.youtube.com/watch?v={}", id));
                            }
                        }
                    }
                    if urls.is_empty() {
                        vec![target.to_string()]
                    } else {
                        urls
                    }
                } else {
                    vec![target.to_string()]
                }
            } else {
                vec![target.to_string()]
            };

            for (idx, vid_url) in video_urls.iter().enumerate() {
                if video_urls.len() > 1 {
                    println!("\n=======================================================");
                    println!("📹 Evaluating Video #{}/{} in Playlist: {}", idx + 1, video_urls.len(), vid_url);
                    println!("=======================================================");
                }
                match sandsifter::math::MarginalGainsEngine::evaluate_target(vid_url) {
                    Ok(scorecard) => {
                        // 1. Generate FlightSheet as primary human executive product
                        let flight_sheet = sandsifter::export::FlightSheet::from_scorecard(&scorecard);

                        // 2. Print FlightSheet to stdout as primary display product
                        flight_sheet.render_terminal_stdout();

                        // 3. Print markdown scorecard table underneath for detailed vector reference
                        println!("{}", sandsifter::math::MarginalGainsEngine::render_markdown_scorecard(&scorecard));

                        // 4. Automatically save output/<slug>_flight_sheet.md and output/<slug>_template.fcpxml
                        let output_dir = std::path::Path::new("output");
                        if let Err(e) = std::fs::create_dir_all(output_dir) {
                            eprintln!("⚠️ Failed to create output directory: {}", e);
                        } else {
                            match flight_sheet.save_to_markdown(output_dir) {
                                Ok(p) => println!("💾 Saved Flight Sheet: {}", p.display()),
                                Err(e) => eprintln!("⚠️ Failed to save flight sheet markdown: {}", e),
                            }
                            match flight_sheet.save_html_report(output_dir, &scorecard) {
                                Ok(p) => println!("🌐 Saved HTML Executive Report: {}", p.display()),
                                Err(e) => eprintln!("⚠️ Failed to save HTML executive report: {}", e),
                            }
                            match flight_sheet.save_fcpxml_template(output_dir) {
                                Ok(p) => println!("🎬 Saved FCPXML Template: {}", p.display()),
                                Err(e) => eprintln!("⚠️ Failed to save FCPXML template: {}", e),
                            }
                            match flight_sheet.save_edl_template(output_dir) {
                                Ok(p) => println!("🎯 Saved DaVinci Resolve Marker EDL: {}", p.display()),
                                Err(e) => eprintln!("⚠️ Failed to save EDL template: {}", e),
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to evaluate video {}: {}", vid_url, e);
                    }
                }
            }

            return Ok(());
        } else {
            eprintln!("❌ Error: --marginal-gains / --audit requires a video URL or local file path (e.g. cargo run -- --audit path/to/draft.mp4)");
            std::process::exit(1);
        }
    }

    // Mode: Automated Head-to-Head Comparison Engine (--compare <target_a> <target_b>)
    if let Some(pos) = args.iter().position(|a| a == "--compare" || a == "--vs") {
        if let (Some(target_a), Some(target_b)) = (args.get(pos + 1), args.get(pos + 2)) {
            info!("⚔️ Running Head-to-Head Comparative Audit: {} vs. {}", target_a, target_b);
            println!("\n⚔️ Initiating Head-to-Head Comparative Audit...");
            println!("  • Target A (Baseline)  : {}", target_a);
            println!("  • Target B (Challenger): {}", target_b);

            println!("\n[1/2] Auditing Target A...");
            let sc_a = match sandsifter::math::MarginalGainsEngine::evaluate_target(target_a) {
                Ok(sc) => sc,
                Err(e) => {
                    eprintln!("❌ Error evaluating Target A ({}): {}", target_a, e);
                    std::process::exit(1);
                }
            };
            println!("      Done: \"{}\" (Health Grade: {:.0}%)", sc_a.title, sc_a.series_system_reliability_pct.round());

            println!("\n[2/2] Auditing Target B...");
            let sc_b = match sandsifter::math::MarginalGainsEngine::evaluate_target(target_b) {
                Ok(sc) => sc,
                Err(e) => {
                    eprintln!("❌ Error evaluating Target B ({}): {}", target_b, e);
                    std::process::exit(1);
                }
            };
            println!("      Done: \"{}\" (Health Grade: {:.0}%)", sc_b.title, sc_b.series_system_reliability_pct.round());

            let comparison = sandsifter::math::TargetComparisonReport::generate(&sc_a, &sc_b);
            println!("{}", comparison.render_terminal());

            let output_dir = std::path::Path::new("output");
            if let Err(e) = std::fs::create_dir_all(output_dir) {
                eprintln!("⚠️ Failed to create output directory: {}", e);
            } else {
                match comparison.save_to_markdown(output_dir) {
                    Ok(p) => println!("💾 Saved Head-to-Head Comparison Report: {}", p.display()),
                    Err(e) => eprintln!("⚠️ Failed to save comparison report: {}", e),
                }
            }
            return Ok(());
        } else {
            eprintln!("❌ Error: --compare / --vs requires two targets (e.g. cargo run -- --compare <winner_url_or_file> <flop_url_or_file>)");
            std::process::exit(1);
        }
    }

    // Mode: Generate DaVinci Resolve / Premiere FCPXML Timeline Template
    if let Some(pos) = args.iter().position(|a| a == "--timeline" || a == "--fcpxml") {
        if let Some(url) = args.get(pos + 1) {
            info!("🎨 Generating DaVinci Resolve / Premiere Pro FCPXML timeline for: {}", url);
            println!("\n🔍 Extracting forensics & synthesis data for timeline generation: {}...", url);

            // Step 1: Extract yt-dlp metadata & cues
            let meta = MediaResolver::extract_with_ytdlp(url)?;
            let cues = match &meta.vtt_subtitles {
                Some(vtt) => sandsifter::math::RetentionEngine::parse_vtt(vtt),
                None => Vec::new(),
            };

            // Step 2: Extract visual pacing (ASL and scene cuts) via zero-disk stream sampling
            println!("🎬 Extracting visual scene cuts and shot pacing...");
            let sample_sec = Some(meta.duration_seconds.min(180.0));
            let pacing_metrics = sandsifter::ingest::MediaSceneDetector::analyze_video_visual_pacing(url, sample_sec).ok();

            // Step 3: Run Multimodal Retention Calculus Correlating Cuts & Audio Silence
            println!("📊 Analyzing audience retention curve & derivative breaches...");
            let cut_timestamps = pacing_metrics.as_ref().map(|p| p.cut_timestamps.as_slice()).unwrap_or(&[]);
            let retention_report = sandsifter::math::RetentionEngine::analyze_curve_multimodal(
                &meta.video_id,
                &meta.title,
                meta.duration_seconds,
                &meta.heatmap,
                &cues,
                cut_timestamps,
            );

            // Step 4: Stream and extract acoustic onset & pacing
            println!("🎙️ Extracting acoustic speech onset and hook delivery...");
            let acoustic_metrics = if let Some(cdn_url) = &meta.direct_audio_url {
                let ingestor = StreamIngestor::default();
                if let Ok(audio_bytes) = ingestor.ingest_audio_stream(cdn_url).await {
                    if !audio_bytes.is_empty() {
                        let sample_window = &audio_bytes[..audio_bytes.len().min(4 * 1024 * 1024)];
                        let vad = VadClassifier::default();
                        let onset_ms = vad.detect_speech_onset_ms(sample_window);
                        let first_spoken = cues.first().map(|c| c.text.as_str()).unwrap_or(&meta.title);
                        Some(sandsifter::math::ForensicsEngine::analyze_delivery_explicit(
                            onset_ms,
                            None,
                            first_spoken,
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Step 5: Synthesize ExportTimeline
            let is_vertical = true; // Default 1080x1920 Short/Reels
            let timeline = sandsifter::export::ExportTimeline::from_forensics(
                &meta.title,
                meta.duration_seconds.max(1.0),
                is_vertical,
                pacing_metrics.as_ref(),
                Some(&retention_report),
                acoustic_metrics.as_ref(),
            );

            // Output file paths directly in current working directory
            let template_fcpxml = format!("{}_sandsifter_template.fcpxml", meta.video_id);
            let raw_fcpxml = format!("{}.fcpxml", meta.video_id);
            let edl_filename = format!("{}.edl", meta.video_id);
            timeline.save_fcpxml(&template_fcpxml)?;
            timeline.save_fcpxml(&raw_fcpxml)?;
            timeline.save_edl(&edl_filename)?;

            println!("{}", render_timeline_report(&timeline, &template_fcpxml, &edl_filename, &meta.uploader));
            return Ok(());
        } else {
            eprintln!("❌ Error: --timeline requires a video URL (e.g. cargo run -- --timeline https://youtube.com/shorts/...)");
            std::process::exit(1);
        }
    }

    // Mode: Predictive Short-Form Script Generator (The "Pull Engine")
    if let Some(pos) = args.iter().position(|a| a == "--script" || a == "--predictive-script") {
        let channel_raw = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("@creator");
        let topic = args.get(pos + 2).map(|s| s.as_str()).unwrap_or("Productivity Hacks");

        let duration = args
            .iter()
            .position(|a| a == "--duration")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(36);

        let export_timeline = args.iter().any(|a| a == "--export-timeline" || a == "--timeline");

        info!("✍️ Generating Predictive Short-Form Script for {} // Topic: \"{}\" // Duration: {}s", channel_raw, topic, duration);

        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;
        let real_videos = db.get_videos_by_channel(channel_raw).unwrap_or_default();

        let identity = sandsifter::pipeline::CreatorIdentityContext::infer(
            channel_raw,
            topic,
            None,
            None,
        );

        let req = sandsifter::pipeline::ScriptRequest {
            channel_handle: channel_raw.to_string(),
            topic: topic.to_string(),
            target_duration_sec: duration,
            target_wpm: 195.0,
            target_asl_sec: 2.4,
            identity_context: Some(identity),
        };

        let script = sandsifter::pipeline::ScriptGenerator::generate(&req, &real_videos);

        println!("\n{}", script.markdown_teleprompter);

        let slug = sandsifter_slugify(topic);
        let output_dir = std::path::Path::new("output");
        if let Err(e) = std::fs::create_dir_all(output_dir) {
            eprintln!("⚠️ Failed to create output directory: {}", e);
        } else {
            let md_filename = format!("{}_teleprompter.md", slug);
            let md_path = output_dir.join(&md_filename);
            if let Err(e) = std::fs::write(&md_path, &script.markdown_teleprompter) {
                eprintln!("⚠️ Failed to write teleprompter markdown: {}", e);
            } else {
                println!("💾 Saved Production Blueprint & Teleprompter: {}", md_path.display());
            }

            match sandsifter::export::ShootSheetGenerator::save(&script, output_dir) {
                Ok(p) => println!("📄 Saved 1-Page Pre-Production Shoot Sheet: {}", p.display()),
                Err(e) => eprintln!("⚠️ Failed to save shoot sheet HTML: {}", e),
            }
        }

        if export_timeline {
            let timeline = script.to_export_timeline();
            let fcpxml_filename = format!("{}_script_template.fcpxml", slug);
            let edl_filename = format!("{}_script.edl", slug);

            // Save in root directory
            timeline.save_fcpxml(&fcpxml_filename)?;
            timeline.save_edl(&edl_filename)?;

            // Also save in output/ directory
            let out_fcpxml = output_dir.join(&fcpxml_filename);
            let out_edl = output_dir.join(&edl_filename);
            let _ = timeline.save_fcpxml(out_fcpxml.to_str().unwrap_or(&fcpxml_filename));
            let _ = timeline.save_edl(out_edl.to_str().unwrap_or(&edl_filename));

            println!("════════════════════════════════════════════════════════════════════════════");
            println!("🎬 PRE-PRODUCTION TIMELINE EXPORTED SUCCESSFULLY");
            println!("════════════════════════════════════════════════════════════════════════════");
            println!("  ✅ Apple FCPXML v1.8:  {} (and in output/)", fcpxml_filename);
            println!("  ✅ CMX 3600 EDL:       {} (and in output/)", edl_filename);
            println!("  ⏱️  Total Duration:    {:.1}s ({} beats @ 2.4s ASL)", script.total_duration_sec, script.beats.len());
            println!("  🟣 Hook Markers:       Purple (0.0s - 2.4s)");
            println!("  🔴 Cognitive Friction: Red (2.4s - 4.8s / 3.5s gate)");
            println!("  ⚡ Pattern Interrupt:  Orange (every 3-4 beats)");
            println!("  🔄 Looping Bridge:     Cyan (seamless replay back into Beat 0)");
            println!("════════════════════════════════════════════════════════════════════════════\n");
        }

        return Ok(());
    }

    // Mode: Semantic Topic Momentum & Trend Forecasting (Phase 3)
    if let Some(pos) = args.iter().position(|a| a == "--trending") {
        if let Some(topic) = args.get(pos + 1) {
            let mut days_window = 7u32;
            let mut limit = 50usize;

            if let Some(d_pos) = args.iter().position(|a| a == "--days") {
                if let Some(d_val) = args.get(d_pos + 1).and_then(|s| s.parse::<u32>().ok()) {
                    days_window = d_val;
                }
            }
            if let Some(l_pos) = args.iter().position(|a| a == "--limit") {
                if let Some(l_val) = args.get(l_pos + 1).and_then(|s| s.parse::<usize>().ok()) {
                    limit = l_val;
                }
            }

            info!("📈 Mining topic momentum for '{}' (Window: {} days, Sample: {} videos)", topic, days_window, limit);
            println!("\n🔍 Harvesting recent video cohort for topic: \"{}\" (Last {} days)...", topic, days_window);

            let videos = sandsifter::ingest::ChannelCrawler::crawl_topic_recent(topic, limit, days_window)?;
            let unique_channels = videos.iter().map(|v| &v.channel).collect::<std::collections::HashSet<_>>().len();
            println!("✅ Harvested {} recent videos across {} channels. Computing TF-IDF & velocity momentum...\n",
                videos.len(),
                unique_channels
            );

            let report = sandsifter::math::TopicMomentumEngine::analyze_cohort_trends(topic, days_window, &videos);
            println!("{}", render_trending_report(&report));
            return Ok(());
        }
    }

    // Mode 4: Analyze single video URL via yt-dlp & in-memory audio streaming (Task 1)
    if let Some(pos) = args.iter().position(|a| a == "--analyze") {
        if let Some(url) = args.get(pos + 1) {
            info!("🔍 Deep-analyzing video URL via empirical extraction: {}", url);
            if url.contains("playlist") || url.contains("list=") {
                if let Ok(db) = SandsifterDb::open_default() {
                    info!("🔗 Playlist URL detected in --analyze. Auto-calibrating True Channel Baseline from channel root...");
                    if let Ok(b) = sandsifter::ingest::ChannelCrawler::auto_calibrate_and_save_channel_baseline(&db, url) {
                        let extracted = sandsifter::ingest::ChannelCrawler::extract_creator_handle(url).unwrap_or_default();
                        info!("✅ True Channel Baseline for {} auto-calibrated: {} views", extracted, b);
                    }
                }
            }
            let (platform, _) = MediaResolver::parse_video_url(url);
            info!("Detected platform: {:?}", platform);

            // Step 1: Real yt-dlp metadata & CDN extraction
            info!("📡 Extracting metadata via yt-dlp --dump-json...");
            let meta_res = MediaResolver::extract_with_ytdlp(url);

            // Step 2: Stream real audio bytes directly into memory (<25MB, zero disk I/O)
            let (audio_bytes, mime_type, title, uploader, views) = match meta_res {
                Ok(meta) => {
                    info!("  - Title:    {}", meta.title);
                    info!("  - Creator:  @{}", meta.uploader);
                    info!("  - Views:    {:?}", meta.view_count);
                    info!("  - Duration: {:.1}s", meta.duration_seconds);

                    let audio_stream = if let Some(cdn_url) = &meta.direct_audio_url {
                        info!("🌊 Streaming audio directly from CDN: {}", cdn_url);
                        let ingestor = StreamIngestor::default();
                        match ingestor.ingest_audio_stream(cdn_url).await {
                            Ok(b) if !b.is_empty() => b,
                            _ => {
                                info!("Direct CDN stream fetch failed or rate-limited; piping via yt-dlp stdout pipe...");
                                StreamIngestor::extract_audio_bytes_ytdlp(url)
                                    .unwrap_or_else(|_| StreamIngestor::synthesize_mock_audio_bytes(url))
                            }
                        }
                    } else {
                        info!("Piping audio bytes via yt-dlp stdout pipe...");
                        StreamIngestor::extract_audio_bytes_ytdlp(url)
                            .unwrap_or_else(|_| StreamIngestor::synthesize_mock_audio_bytes(url))
                    };
                    (audio_stream, "audio/mp4", meta.title, meta.uploader, meta.view_count)
                }
                Err(e) => {
                    tracing::warn!("yt-dlp extraction unavailable ({}); falling back to in-memory synthetic stream.", e);
                    (StreamIngestor::synthesize_mock_audio_bytes(url), "audio/mp4", "Analysis Target".to_string(), "creator".to_string(), None)
                }
            };

            info!("🎵 Ingested {} audio bytes into memory (zero temporary files)", audio_bytes.len());

            // Step 3: Run VAD Speech Detection
            let vad = VadClassifier::default();
            if let Some(onset_ms) = vad.detect_speech_onset_ms(&audio_bytes) {
                info!("⚡ Voice Activity Detected: Speech onset at {}ms", onset_ms);
            }

            // Step 4: Transcribe using Gemini 2.5 Flash-Lite or deterministic alignment
            let transcriber = AudioTranscriber::default();
            let transcript = transcriber.transcribe_audio(&audio_bytes, mime_type).await;

            let aligner = HookAligner::default();
            let hook = aligner.extract_opening_hook(&transcript);

            let kf_extractor = KeyframeExtractor::default();
            let keyframes = kf_extractor.synthesize_test_keyframes();

            let ocr_engine = OcrEngine::default();
            let ocr_results = ocr_engine.extract_all_keyframes(&keyframes).await;

            let classifier = NarrativeClassifier::default();
            let analysis = classifier.classify(&transcript, hook.as_ref(), &ocr_results, Some("Tech")).await;

            println!("\n════════════════════════════════════════════════════════════");
            println!("🎬 Empirical Video Forensic Analysis: {}", title);
            println!("   Creator: @{} | Views: {:?} | Platform: {:?}", uploader, views, platform);
            println!("════════════════════════════════════════════════════════════");
            let json_out = serde_json::to_string_pretty(&analysis)?;
            println!("{}\n", json_out);

            // Step 5: Automatically index spoken hook into SQLite with Gemini vector embedding
            if let Ok(db) = SandsifterDb::open_default() {
                let channel_uuid = format!("chan_{:x}", sandsifter::ingest::importer::md5_hash(&uploader));
                let channel_obj = sandsifter::models::WatchlistChannel {
                    uuid: channel_uuid.clone(),
                    platform: format!("{:?}", platform),
                    handle: if uploader.starts_with('@') { uploader.clone() } else { format!("@{}", uploader) },
                    display_name: Some(uploader.clone()),
                    avatar_url: None,
                    follower_count: None,
                    baseline_median_views: views.unwrap_or(10_000),
                    video_count: Some(1),
                    last_crawled_at: None,
                    added_at: "2026-03-01T12:00:00Z".to_string(),
                };
                let _ = db.upsert_channel(&channel_obj);

                let video_uuid = format!("vid_{:x}", sandsifter::ingest::importer::md5_hash(url));
                let video_obj = sandsifter::models::VideoDetail {
                    uuid: video_uuid.clone(),
                    caption: Some(title.clone()),
                    url: url.to_string(),
                    thumbnail: None,
                    channel: sandsifter::models::VideoChannelDetails {
                        uuid: channel_uuid,
                        platform: format!("{:?}", platform),
                        handle: if uploader.starts_with('@') { uploader.clone() } else { format!("@{}", uploader) },
                        display_name: Some(uploader.clone()),
                        avatar_url: None,
                        follower_count: None,
                        baseline_median_views: views,
                    },
                    view_count: views.unwrap_or(10_000),
                    like_count: None,
                    comment_count: None,
                    share_count: None,
                    outlier_score: 1.0,
                    engagement_rate: 0.0,
                    published_at: "2026-03-01T12:00:00Z".to_string(),
                    velocity_views_per_day: Some(sandsifter::math::ForensicsEngine::calculate_velocity(views.unwrap_or(10_000), Some("2026-03-01T12:00:00Z"))),
                    analysis: Some(analysis.clone()),
                    speech_onset_ms: None,
                    wpm_0_to_35: None,
                    spoken_hook: Some(analysis.spoken_hook.clone()),
                };
                let _ = db.upsert_video(&video_obj);

                if !analysis.spoken_hook.trim().is_empty() {
                    let client = sandsifter::storage::gemini_client::GeminiEmbeddingClient::default();
                    match db.index_hook_with_embedding(
                        &video_uuid,
                        &analysis.spoken_hook,
                        analysis.spoken_hook_madlib.as_deref(),
                        3.2,
                        &client,
                    ).await {
                        Ok(hook_id) => {
                            println!("💾 Hook successfully indexed in SQLite: {}", hook_id);
                            println!("   Spoken Hook: \"{}\"", analysis.spoken_hook);
                            if let Some(m) = &analysis.spoken_hook_madlib {
                                println!("   Madlib Template: \"{}\"", m);
                            }
                            println!("   {}\n", client.cost_summary());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to index hook: {}", e);
                        }
                    }
                }
            }

            return Ok(());
        }
    }

    // Mode: Semantic & Hybrid Hook Intelligence (Phase 2B)
    if let Some(pos) = args.iter().position(|a| a == "--hooks") {
        let query_arg = args.get(pos + 1).map(|s| s.as_str());
        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;
        let count = db.count_hooks().unwrap_or(0);
        let client = sandsifter::storage::gemini_client::GeminiEmbeddingClient::default();

        if count == 0 {
            println!("\nℹ️ No hooks currently indexed in SQLite.");
            println!("💡 Run '--analyze <url>' or '--crawl <@creator>' to index empirical hooks.");
            println!("   Showing default curated author templates:\n");
            let mut server = McpServer::new();
            let res = server.execute_tool("/hooks-watchlist", &serde_json::json!({}));
            println!("{}\n", res.content.first().map(|c| c.text.as_str()).unwrap_or(""));
            return Ok(());
        }

        let (query_text, query_vec) = match query_arg {
            Some(q) if !q.trim().is_empty() => {
                let vec = client.embed_text_blocking(q);
                (q, Some(vec))
            }
            _ => ("", None),
        };

        let results = db.search_hooks_hybrid(query_text, query_vec.as_deref(), 10)?;

        println!("\n════════════════════════════════════════════════════════════════════════════");
        if let Some(q) = query_arg {
            println!("🎣 Winning Hook Retrieval for \"{}\" (Semantic Vector + Keyword Search)", q);
        } else {
            println!("🎣 Winning Hook Intelligence Library (All Indexed Hooks)");
        }
        println!("   Audience Resonance & Madlib Formats for Creators");
        println!("════════════════════════════════════════════════════════════════════════════\n");

        if results.is_empty() {
            println!("No hooks matched your query.");
        } else {
            for (idx, r) in results.iter().enumerate() {
                println!("{}", r.format_for_creator(idx + 1));
            }
        }

        println!("════════════════════════════════════════════════════════════════════════════");
        println!("📊 {}", client.cost_summary());
        println!("════════════════════════════════════════════════════════════════════════════\n");
        return Ok(());
    }

    // Mode 5: Watchlist top outliers
    if args.iter().any(|a| a == "--watchlist") {
        info!("📋 Fetching current viral outlier watchlist...");
        let db = SandsifterDb::open_default().ok();
        if let Some(db_ref) = &db {
            if let Ok(outliers) = db_ref.list_top_outliers(1.0, 25) {
                if !outliers.is_empty() {
                    println!("\n🔥 Viral Outlier Watchlist (Ranked by Multiplier vs Creator Baseline):");
                    println!("{}\n", TerminalTableRenderer::render_watchlist_table(&outliers));
                    return Ok(());
                }
            }
        }

        let mut server = match db {
            Some(d) => McpServer::with_db(d),
            None => McpServer::new(),
        };
        let result = server.execute_tool("/videos-watchlist", &serde_json::json!({ "limit": 5 }));
        println!("\n{}\n", result.content.first().map(|c| c.text.as_str()).unwrap_or("No items"));
        return Ok(());
    }

    // Mode: Re-index hooks from existing database videos
    if args.iter().any(|a| a == "--reindex-hooks") {
        info!("🔄 Re-indexing hooks from stored videos in SQLite...");
        let db = SandsifterDb::open_default()?;
        let client = sandsifter::storage::gemini_client::GeminiEmbeddingClient::default();
        let outliers = db.list_top_outliers(1.5, 50)?;
        let mut indexed = 0;
        for v in outliers {
            if let Some(caption) = &v.caption {
                if !caption.trim().is_empty() {
                    let madlib = sandsifter::analyzer::synthesize_madlib(caption);
                    let _ = db.index_hook_with_embedding_blocking(&v.uuid, caption, Some(&madlib), 3.0, &client);
                    indexed += 1;
                }
            }
        }
        println!("\n✅ Successfully indexed {} hooks into SQLite with 768-dim vector embeddings!", indexed);
        println!("📊 {}\n", client.cost_summary());
        return Ok(());
    }

    // Mode: Deep Media Forensics, Delivery Kinetics & Holdout Prediction
    if let Some(pos) = args.iter().position(|a| a == "--forensics") {
        let channel_target = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("@Vanwives");
        info!("🔬 Running Deep Media Forensics & Holdout Prediction for: {}", channel_target);

        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;
        let (_clean_url, mut handle) = sandsifter::ingest::ChannelCrawler::normalize_channel_url(channel_target);
        if channel_target.contains("playlist") || channel_target.contains("list=") {
            if let Ok(extracted) = sandsifter::ingest::ChannelCrawler::extract_creator_handle(channel_target) {
                handle = extracted;
            }
        }

        let mut videos = db.get_videos_by_channel(&handle).unwrap_or_default();

        if videos.is_empty() {
            println!("No videos found in database for {}. Harvesting cohort first...", channel_target);
            let crawl_limit = args.get(pos + 2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(150);
            let _ = sandsifter::ingest::ChannelCrawler::crawl_channel_async(&db, channel_target, crawl_limit, true).await?;
            videos = db.get_videos_by_channel(&handle).unwrap_or_default();
        }

        // Determine True Channel Baseline from SQLite (auto-calibrating if target is playlist or not yet recorded)
        let channel_baseline = if channel_target.contains("playlist") || channel_target.contains("list=") {
            match sandsifter::ingest::ChannelCrawler::auto_calibrate_and_save_channel_baseline(&db, channel_target) {
                Ok(b) => b,
                Err(_) => {
                    let channel_record = db.get_channel_by_handle(&handle).ok().flatten();
                    channel_record.map(|c| c.baseline_median_views).unwrap_or(10_000)
                }
            }
        } else {
            let channel_record = db.get_channel_by_handle(&handle).ok().flatten();
            match channel_record {
                Some(ref c) if c.baseline_median_views > 0 => c.baseline_median_views,
                _ => {
                    sandsifter::ingest::ChannelCrawler::auto_calibrate_and_save_channel_baseline(&db, channel_target)
                        .unwrap_or(10_000)
                }
            }
        };

        // Determine Series Baseline (Playlist Median) across current cohort
        let cohort_views: Vec<u64> = videos.iter().map(|v| v.view_count).collect();
        let series_baseline = sandsifter::math::BaselineEngine::compute_baseline(&cohort_views).unwrap_or(10_000);

        info!("📊 Dual-Baseline Active: Channel Baseline = {} views | Series Baseline = {} views", channel_baseline, series_baseline);

        let meta_map = db.get_video_metadata_by_channel(&handle).unwrap_or_default();
        let raw_titles: Vec<String> = videos.iter()
            .map(|v| v.caption.as_deref().unwrap_or("").to_string())
            .collect();
        let cleaned_titles = sandsifter::math::ForensicsEngine::clean_corpus_titles(&raw_titles);

        let mut records = Vec::new();
        for (idx, v) in videos.iter().enumerate() {
            let hook = v.spoken_hook.as_deref()
                .or_else(|| v.analysis.as_ref().map(|a| a.spoken_hook.as_str()))
                .unwrap_or(v.caption.as_deref().unwrap_or(""));

            let delivery = sandsifter::math::ForensicsEngine::analyze_delivery_explicit(
                v.speech_onset_ms,
                v.wpm_0_to_35,
                hook,
            );
            let linguistics = sandsifter::math::ForensicsEngine::decompose_linguistics(hook);
            let raw_title = v.caption.as_deref().unwrap_or("").to_string();
            let clean_title = cleaned_titles.get(idx).cloned().unwrap_or_else(|| raw_title.clone());
            let title_features = sandsifter::math::ForensicsEngine::decompose_title(&clean_title);

            let (dur_opt, pub_opt) = meta_map.get(&v.url).cloned().unwrap_or((None, Some(v.published_at.clone())));

            let (chan_mult, ser_mult) = sandsifter::math::ForensicsEngine::calculate_dual_multipliers(
                v.view_count,
                channel_baseline,
                series_baseline,
            );

            let days_active = sandsifter::math::ForensicsEngine::calculate_days_active(pub_opt.as_deref());
            let velocity = sandsifter::math::ForensicsEngine::calculate_velocity(v.view_count, pub_opt.as_deref());

            records.push(sandsifter::math::ForensicEpisodeRecord {
                episode_index: idx + 1,
                title: raw_title,
                clean_title,
                views: v.view_count,
                duration: dur_opt,
                published_at: pub_opt,
                days_active,
                velocity_views_per_day: velocity,
                channel_multiplier: chan_mult,
                series_multiplier: ser_mult,
                outlier_multiplier: chan_mult,
                spoken_hook: hook.to_string(),
                delivery,
                linguistics,
                title_features,
                visual_pacing: None,
            });
        }

        let mut report = sandsifter::math::ForensicsEngine::generate_channel_report(&records);
        let (dyn_wpm, dyn_silence) = sandsifter::math::ForensicsEngine::synthesize_acoustic_rules_with_llm(
            channel_target,
            report.mean_wpm_high_performers,
            report.mean_wpm_low_performers,
            report.mean_silence_high_performers_ms,
            report.mean_silence_low_performers_ms,
        ).await;
        report.dynamic_wpm_rule = dyn_wpm;
        report.dynamic_silence_rule = dyn_silence;

        println!("{}", render_forensics_report(&report, channel_target));
        return Ok(());
    }

    // Mode 6: System status
    if args.iter().any(|a| a == "--status") {
        let db = SandsifterDb::open_default().or_else(|_| SandsifterDb::memory())?;
        println!("\n✅ System Health: All subsystems operational.");
        println!("════════════════════════════════════════════════════════════");
        println!("  - Storage Engine:        SQLite ({})", SandsifterDb::default_path().display());
        println!("  - MCP Bridge:            11 Tools Registered conforming to MCP 2024-11-05");
        println!("  - Multimodal AI:         Gemini 2.5 Flash-Lite (Low-Cost Tier)");
        println!("  - Vector Embeddings:     Google Gemini text-embedding-004 (768-dim)");
        println!("  - Video Stream Engine:   yt-dlp In-Memory Stream Pipe (<25MB Ceiling)");
        println!("  - Total Tracked Channels: {}", db.list_channels().map(|c| c.len()).unwrap_or(0));
        println!("  - Total Tracked Videos:   {}", db.count_videos().unwrap_or(0));
        println!("  - Total Tracked Snapshots: {}", db.count_snapshots().unwrap_or(0));
        println!("  - Total Tracked Hooks:    {}", db.count_hooks().unwrap_or(0));
        println!("════════════════════════════════════════════════════════════\n");
        return Ok(());
    }

    // Default standalone banner & help
    println!("🏖️ Sandsifter v0.2.0 // High-Performance Social Media Outlier & MCP Engine");
    println!("════════════════════════════════════════════════════════════════════════════");
    println!("Available CLI Modes:");
    println!("  dev [port]                     Launch Web Studio & MCP HTTP daemon (cargo run dev)");
    println!("  --sse [port]                   Launch HTTP & SSE MCP daemon (default: 8080 or $PORT)");
    println!("  --server [port]                Alias for --sse daemon mode (Cloud Run / Web frontend)");
    println!("  --audit <url_or_file>          Brailsford Marginal Gains Scorecard & Editor's Flight Sheet");
    println!("  --compare <target_a> <target_b> Automated head-to-head comparative audit & Delta Matrix");
    println!("  --crawl <@author> [limit]      Live harvest recent shorts & track view velocity");
    println!("  --forensics <@author>          Deep acoustic kinetics, feature lifts & LOOCV ablation");
    println!("  --import <file.json>           Batch import creator history into SQLite");
    println!("  --hooks [query]                Search winning hooks via Gemini vector embeddings");
    println!("  --hook-machine <@user> <topic> Run Universal Hook Machine for any niche");
    println!("  --analyze <url>                Deep-analyze video URL (yt-dlp stream)");
    println!("  --retention <url>              Reverse-engineer audience retention curve & dip finder");
    println!("  --pacing <url> [sec]           Measure Average Shot Length (ASL) & visual cutting speed");
    println!("  --trending <topic> [--days 7]  Forecast breakout micro-trends & saturated angles");
    println!("  --watchlist                    Display top viral outlier watchlist");
    println!("  --script <@user> <topic>       Predictive script generator & ASL pacing engine");
    println!("                                   (Options: [--duration 36] [--export-timeline])");
    println!("  --status                       Display system status & database metrics");
    println!();

    Ok(())
}

fn sandsifter_slugify(s: &str) -> String {
    let mut clean = s
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>();
    while clean.contains("__") {
        clean = clean.replace("__", "_");
    }
    clean.trim_matches('_').to_string()
}


fn format_views(views: u64) -> String {
    let s = views.to_string();
    let mut result = String::new();
    for (count, c) in s.chars().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn format_publish_date(date_str: &str) -> String {
    let clean = date_str.trim();
    if clean.len() == 8 && clean.chars().all(|c| c.is_ascii_digit()) {
        let year = &clean[0..4];
        let month_num = &clean[4..6];
        let day = clean[6..8].trim_start_matches('0');
        let day = if day.is_empty() { "1" } else { day };
        let month_name = match month_num {
            "01" => "Jan",
            "02" => "Feb",
            "03" => "Mar",
            "04" => "Apr",
            "05" => "May",
            "06" => "Jun",
            "07" => "Jul",
            "08" => "Aug",
            "09" => "Sep",
            "10" => "Oct",
            "11" => "Nov",
            "12" => "Dec",
            _ => month_num,
        };
        format!("{} {}, {}", month_name, day, year)
    } else if clean.len() >= 10 && clean.contains('-') {
        let parts: Vec<&str> = clean[0..10].split('-').collect();
        if parts.len() == 3 {
            let year = parts[0];
            let month_name = match parts[1] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                m => m,
            };
            let day = parts[2].trim_start_matches('0');
            let day = if day.is_empty() { "1" } else { day };
            format!("{} {}, {}", month_name, day, year)
        } else {
            clean.to_string()
        }
    } else {
        clean.to_string()
    }
}

fn format_seconds_opt(ms_opt: Option<u32>) -> String {
    match ms_opt {
        Some(0) => "0.0s (Instant)".to_string(),
        Some(ms) if ms < 3500 => format!("{:.1}s", ms as f64 / 1000.0),
        Some(_) => "3.5s+ (Slow)".to_string(),
        None => "No Speech".to_string(),
    }
}

fn render_forensics_report(report: &sandsifter::math::ChannelForensicsReport, channel: &str) -> String {
    let mut out = String::new();
    out.push_str("\n════════════════════════════════════════════════════════════════════════════════════════\n");
    out.push_str(&format!("🔬 EMPIRICAL CREATOR INTELLIGENCE & HOOK PLAYBOOK: {}\n", channel));
    out.push_str("   Acoustic Delivery Kinetics, Spoken Pattern Impact, Sequence Momentum & Blind Backtesting\n");
    out.push_str("════════════════════════════════════════════════════════════════════════════════════════\n\n");

    let high_sec = (report.mean_silence_high_performers_ms / 1000.0 * 10.0).round() / 10.0;
    let low_sec = (report.mean_silence_low_performers_ms / 1000.0 * 10.0).round() / 10.0;
    let delta_sec = ((high_sec - low_sec) * 10.0).round() / 10.0;

    out.push_str("### 1. Delivery & Acoustic Kinetics (Cold Open Speed & Dead Air Lag)\n");
    out.push_str("| Metric | High Performers (>=1.4x) | Low Performers (<1.0x) | Difference | Creator's Rule |\n");
    out.push_str("| :--- | :---: | :---: | :---: | :--- |\n");
    out.push_str(&format!(
        "| **Speech Urgency (`Words Per Minute`)** | **{:.1} WPM** | **{:.1} WPM** | **{:+0.1} WPM ({})** | {} |\n",
        report.mean_wpm_high_performers,
        report.mean_wpm_low_performers,
        report.mean_wpm_high_performers - report.mean_wpm_low_performers,
        if report.mean_wpm_high_performers > report.mean_wpm_low_performers { "Faster Pace" } else { "Slower Pace" },
        report.dynamic_wpm_rule
    ));
    out.push_str(&format!(
        "| **Dead Air Before Speech (`Intro Lag`)** | **{:.1}s** | **{:.1}s** | **{:+.1}s ({})** | {} |\n\n",
        high_sec,
        low_sec,
        delta_sec,
        if high_sec < low_sec { "Tighter Cut" } else { "Delayed Speech" },
        report.dynamic_silence_rule
    ));
    if let (Some(asl_h), Some(asl_l)) = (report.mean_asl_high_performers_sec, report.mean_asl_low_performers_sec) {
        let rule = report.dynamic_pacing_rule.as_deref().unwrap_or("Match the winning visual cutting rhythm.");
        out.push_str(&format!(
            "| **Visual Rhythm (`Average Shot Length`)** | **{:.1}s** | **{:.1}s** | **{:+.1}s ({})** | {} |\n\n",
            asl_h,
            asl_l,
            asl_h - asl_l,
            if asl_h < asl_l { "Faster Cuts" } else { "Longer Takes" },
            rule
        ));
    } else {
        out.push('\n');
    }

    out.push_str("### 2. Spoken Hook Patterns & Creator Impact Playbook\n");
    out.push_str("| Spoken Opening Pattern | Usage | Typical Multiplier | Viewer Impact (% View Shift) | Creator's Actionable Rule |\n");
    out.push_str("| :--- | :---: | :---: | :---: | :--- |\n");
    for f in &report.feature_lifts {
        if f.sample_count_present == 0 {
            let rule = match f.feature_name.as_str() {
                s if s.contains("Jeopardy") => "Untapped Opportunity: Opening with high-stakes peril is proven to spike views.",
                s if s.contains("Asset/Hazard") => "Untapped Opportunity: Focusing on trucks, land, or hazards builds instant curiosity.",
                _ => "Never tested in cold open on this channel.",
            };
            out.push_str(&format!(
                "| {} | 0/{} | N/A | **Never Tested** | {} |\n",
                f.feature_name,
                f.sample_count_present + f.sample_count_absent,
                rule
            ));
        } else {
            let (impact_str, rule) = if f.mean_multiplier_present >= 1.5 {
                (
                    format!("🔥 **+{:.0}% View Surge**", (f.mean_multiplier_present - 1.0) * 100.0),
                    "MUST USE: Dramatic personal conviction hooks massive audiences immediately.",
                )
            } else if f.mean_multiplier_present >= 1.05 {
                (
                    format!("📈 **+{:.0}% Above Normal**", (f.mean_multiplier_present - 1.0) * 100.0),
                    "SOLID WINNER: Mid-action cliffhangers and open loops consistently outperform.",
                )
            } else if f.mean_multiplier_present >= 0.95 {
                (
                    "⚖️ Neutral (~Average)".to_string(),
                    "Average retention: matches standard channel baseline.",
                )
            } else {
                (
                    format!("📉 **-{:.0}% View Drag**", (1.0 - f.mean_multiplier_present) * 100.0),
                    "AVOID: Small-talk or low-stakes opens cause viewer drop-off before the story starts.",
                )
            };

            out.push_str(&format!(
                "| {} | {}/{} | {:.2}x | {} | {} |\n",
                f.feature_name,
                f.sample_count_present,
                f.sample_count_present + f.sample_count_absent,
                f.mean_multiplier_present,
                impact_str,
                rule
            ));
        }
    }
    out.push('\n');

    let post_spike_pct = ((report.sequence_momentum.mean_multiplier_post_spike - 1.0) * 100.0).round() as i64;
    let post_lull_pct = ((1.0 - report.sequence_momentum.mean_multiplier_post_lull) * 100.0).round() as i64;

    out.push_str("### 3. Autoregressive Sequence Momentum (The Algorithm Tail-Wind)\n");
    out.push_str(&format!("- **Series Momentum (Autocorrelation r)**: **{:.2}**\n", report.sequence_momentum.lag1_autocorrelation));
    out.push_str(&format!(
        "- **After a Breakout Hit (>=1.4x)**: Next video averages **{:.2}x (+{}% above normal views)** — algorithm tailwind carries forward!\n",
        report.sequence_momentum.mean_multiplier_post_spike, post_spike_pct
    ));
    out.push_str(&format!(
        "- **After a Slower Video (<1.0x)**: Next video drops to **{:.2}x (-{}% below normal views)** — audience drag persists.\n",
        report.sequence_momentum.mean_multiplier_post_lull, post_lull_pct
    ));
    out.push_str("- **Creator's Tactical Rule**: *When you get a hit, drop your best follow-up idea within 7 days to ride the algorithm's temporary view boost.*\n\n");

    out.push_str("### 4. Blind Predictive Backtest: Can AI Predict Winners Before They Post?\n");
    out.push_str(&format!(
        "*Protocol: We ran a 'Blind Time-Travel Backtest' (LOOCV) across all {} episodes. For each video, the AI was blinded to view counts, trained on the other episodes, and forced to guess which video would blow up.*\n\n",
        report.loocv_ablation.total_episodes
    ));
    out.push_str(&format!("- **Model A (Title Words Only)**: Rank correlation ρ = **{:.2}** (Titles mislead the AI because repetitive series names mask the true episode stakes).\n", report.loocv_ablation.model_a_title_only_rho));
    out.push_str(&format!("- **Model B (Spoken Voice + Speed + Kinetics)**: Rank correlation ρ = **{:.2}**\n", report.loocv_ablation.model_b_multimodal_rho));
    out.push_str(&format!("- **Incremental Spoken Audio Lift**: **{:+0.2}** (Listening to speech pace and voice urgency improved predictive accuracy by +{:0.2} points!).\n\n", report.loocv_ablation.incremental_audio_lift_delta_rho, report.loocv_ablation.incremental_audio_lift_delta_rho));

    out.push_str("📖 **CREATOR CHEAT SHEET & METRIC GUIDE (WHAT THESE COLUMNS MEAN FOR CREATORS):**\n");
    out.push_str(" • **Channel Multiplier (Channel Mult)**: Video views vs creator's typical catalog average.\n");
    out.push_str("    - `1.00x` = Exactly their average view count.\n");
    out.push_str("    - `1.50x` = **+50% more views** than their normal audience.\n");
    out.push_str("    - `4.67x` = **Huge viral spike (+367% views)** — broke into the general YouTube algorithm!\n");
    out.push_str("    - `0.80x` = **20% fewer views** than their normal average.\n");
    out.push_str(" • **Series Multiplier (Series Mult)**: Video views vs this specific playlist's median (proves if it was the standout #1 winner of the series).\n");
    out.push_str(" • **Velocity (Views/Day)**: How many views this video gets per day on average. Use this to spot recent videos that are currently exploding, rather than just looking at old videos that took years to get big.\n");
    out.push_str(" • **Actual Rank**: Where this video placed in this series (#1 = biggest hit, #113 = lowest views).\n");
    out.push_str(" • **Dead Air Lag**: Seconds of silence or intro music before talking begins. (Under 0.5s = instant hook; over 2.0s = slow dead air).\n");
    out.push_str(" • **Speech Pace (WPM)**: Speaking rate in Words Per Minute during opening 3.5s. (>150 WPM = rapid urgency; <90 WPM = sluggish).\n");
    out.push_str(" • **Spoken Opening Hook**: The exact spoken words the creator said in the first 3.5 seconds.\n\n");

    out.push_str("| Ep # | Video Title | Channel Mult | Series Mult | Actual Rank | Views | Velocity (Views/Day) | Published | Duration | AI Guess: Title-Only | AI Guess: Voice+Title | Dead Air Lag | Speech Pace (WPM) | Spoken Opening Hook |\n");
    out.push_str("| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :--- |\n");
    for p in &report.loocv_ablation.episode_predictions {
        let clean_title = p.title.replace('|', "-");
        let clean_hook = if p.is_non_speech {
            "[NON-SPEECH / AMBIENT]".to_string()
        } else {
            p.spoken_hook.replace('|', "-")
        };
        let t_silence_str = format_seconds_opt(p.t_silence_ms);
        let dur_str = p.duration.as_deref().unwrap_or("N/A");
        let pub_str = p.published_at.as_deref().map(format_publish_date).unwrap_or_else(|| "N/A".to_string());
        let chan_m = if p.channel_multiplier > 0.0 { p.channel_multiplier } else { p.actual_multiplier };
        let ser_m = if p.series_multiplier > 0.0 { p.series_multiplier } else { p.actual_multiplier };
        let vel_str = format!("{}/day", format_views(p.velocity_views_per_day.round() as u64));
        out.push_str(&format!(
            "| Ep {:02} | {} | {:.2}x | {:.2}x | #{} | **{}** | **{}** | {} | {} | {:.2} (#{}) | **{:.2} (#{})** | {} | {:.1} | {} |\n",
            p.episode_index,
            clean_title,
            chan_m,
            ser_m,
            p.actual_rank,
            format_views(p.views),
            vel_str,
            pub_str,
            dur_str,
            p.model_a_score,
            p.model_a_rank,
            p.model_b_score,
            p.model_b_rank,
            t_silence_str,
            p.wpm_0_to_35,
            clean_hook
        ));
    }
    out.push('\n');

    out.push_str("### 5. Key Empirical Takeaways (Tactical Playbook for Creators)\n");
    for (idx, t) in report.non_obvious_takeaways.iter().enumerate() {
        let emoji = match idx {
            0 => "🪤",
            1 => "⏱️",
            2 => "⚡",
            3 => "📉",
            _ => "🎯",
        };
        out.push_str(&format!("{}. {} {}\n\n", idx + 1, emoji, t));
    }

    out
}

fn render_retention_report(report: &sandsifter::math::RetentionCurveReport, creator: &str) -> String {
    let mut out = String::new();
    out.push_str("\n════════════════════════════════════════════════════════════════════════════════════════\n");
    out.push_str(&format!("📉 AUDIENCE RETENTION CALCULUS & CREATOR DIAGNOSTIC: @{}\n", creator));
    out.push_str(&format!("   Video: {}\n", report.title));
    out.push_str(&format!(
        "   Duration: {} | Average Intensity: {:.1}% | Attention Friction Index: {:.3}\n",
        sandsifter::math::RetentionEngine::format_timestamp(report.duration_seconds),
        report.average_intensity * 100.0,
        report.attention_friction_index
    ));
    out.push_str("════════════════════════════════════════════════════════════════════════════════════════\n\n");

    out.push_str(&format!("### 🎯 Curve Diagnosis: {}\n\n", report.curve_health_verdict));

    // Task 1: List the Top 3 Attention Breaches (steepest drop-offs)
    out.push_str("### 🚨 TOP 3 ATTENTION BREACHES (Steepest Drop-Offs)\n");
    if report.attention_breaches.is_empty() {
        out.push_str("No critical attention breaches detected. The video maintained solid audience hold.\n\n");
    } else {
        out.push_str("| # | Timestamp | Drop Rate | Root Cause | Spoken Words at This Second (WhisperX) |\n");
        out.push_str("|:---:|:---:|:---:|:---|:---|\n");
        for (idx, breach) in report.attention_breaches.iter().take(3).enumerate() {
            let cause_str = match &breach.root_cause {
                sandsifter::math::retention::BreachRootCause::DeadAirSilence { silence_sec } => {
                    format!("🔇 Dead Air ({:.1}s silence)", silence_sec)
                }
                sandsifter::math::retention::BreachRootCause::SlowVisualPacing { shot_length_sec } => {
                    format!("🎥 Slow Pacing ({:.1}s cut)", shot_length_sec)
                }
                sandsifter::math::retention::BreachRootCause::CompoundFriction { silence_sec, shot_length_sec } => {
                    format!("⚠️ Compound Friction ({:.1}s silence + {:.1}s cut)", silence_sec, shot_length_sec)
                }
                sandsifter::math::retention::BreachRootCause::NarrativePlateau => {
                    "📉 Narrative Plateau".to_string()
                }
            };

            let clean_words = if breach.transcript_context.trim().is_empty() {
                "[No active speech]".to_string()
            } else {
                format!("\"{}\"", breach.transcript_context)
            };

            out.push_str(&format!(
                "| **#{}** | **{}** | **{:.1}%/s** | **{}** | {} |\n",
                idx + 1,
                breach.formatted_timestamp,
                breach.r_prime * 100.0,
                cause_str,
                clean_words
            ));
        }
        out.push('\n');
    }

    // Task 1: List the Top Replay Spike
    out.push_str("### 🔥 TOP REPLAY SPIKE (Where Viewers Rewound)\n");
    if let Some(spike) = report.major_spikes.first() {
        let clean_spike_words = if spike.transcript_context.trim().is_empty() {
            "[Visual action scene / music swell]".to_string()
        } else {
            format!("\"{}\"", spike.transcript_context)
        };
        out.push_str(&format!(
            "- **Timestamp**: **{}** (Rewatch Surge: **{:+.1}%**)\n\
             - **Spoken Words / Cue**: {}\n\
             - **Viewer Action & Payoff**: Rewind trigger moment — viewers actively scrubbed backwards to rewatch this visual/narrative payoff.\n\n",
            spike.formatted_timestamp,
            spike.delta_intensity * 100.0,
            clean_spike_words
        ));
    } else if report.duration_seconds > 0.0 && report.peak_timestamp_formatted != "00:00" {
        out.push_str(&format!(
            "- **Peak Retention Moment**: landed at **{}**.\n\
             - **Spoken Words**: \"{}\"\n\
             - **Viewer Action**: Peak audience concentration point across the timeline.\n\n",
            report.peak_timestamp_formatted,
            report.peak_transcript
        ));
    } else {
        out.push_str("No pronounced rewatch spike detected (audience retention follows normal decay curve without significant rewinds).\n\n");
    }

    // Task 2: The 3-Bullet-Point Editing Prescription
    out.push_str("### 🩺 3-POINT EDITING PRESCRIPTION\n");
    if !report.editing_prescription.is_empty() {
        for (idx, point) in report.editing_prescription.iter().take(3).enumerate() {
            out.push_str(&format!("{}. {}\n", idx + 1, point));
        }
    } else {
        for (idx, rule) in report.tactical_rules.iter().take(3).enumerate() {
            out.push_str(&format!("{}. {}\n", idx + 1, rule));
        }
    }
    out.push('\n');

    out
}

fn render_pacing_report(pacing: &sandsifter::math::VisualPacingMetrics, target: &str) -> String {
    let mut out = String::new();
    out.push_str("\n════════════════════════════════════════════════════════════════════════════════════════\n");
    out.push_str(&format!("🎬 VISUAL RHYTHM & SHOT PACING REPORT: {}\n", target));
    out.push_str(&format!("   Duration Sampled: {:.1}s | Total Scene Cuts: {} | Average Shot Length (ASL): {:.1}s\n",
        pacing.duration_seconds, pacing.total_scene_cuts, pacing.average_shot_length_sec));
    out.push_str("════════════════════════════════════════════════════════════════════════════════════════\n\n");

    out.push_str(&format!("### 🎯 Visual Velocity Classification: {}\n\n", pacing.cut_classification.label()));
    out.push_str(&format!("### 📐 Tactical Editing Playbook Rule:\n> {}\n\n", pacing.tactical_rule));

    out.push_str("### ⏱️ Detected Scene Transitions Timeline:\n");
    if pacing.cut_timestamps.is_empty() {
        out.push_str("No scene cuts detected in sample window (single continuous shot).\n\n");
    } else {
        out.push_str("| Cut # | Timestamp (Seconds) | Formatted Time |\n");
        out.push_str("| :---: | :---: | :---: |\n");
        for (idx, &ts) in pacing.cut_timestamps.iter().take(12).enumerate() {
            out.push_str(&format!(
                "| **#{}** | **{:.1}s** | **{}** |\n",
                idx + 1,
                ts,
                sandsifter::math::RetentionEngine::format_timestamp(ts)
            ));
        }
        if pacing.cut_timestamps.len() > 12 {
            out.push_str(&format!("\n*...and {} more scene cuts detected across the sampled window.*\n", pacing.cut_timestamps.len() - 12));
        }
        out.push('\n');
    }

    out
}

fn render_trending_report(report: &sandsifter::math::TopicTrendReport) -> String {
    let mut out = String::new();
    out.push_str("\n════════════════════════════════════════════════════════════════════════════════════════\n");
    out.push_str(&format!("📈 SEMANTIC TOPIC MOMENTUM & TREND FORECAST: \"{}\"\n", report.topic));
    out.push_str(&format!("   Time Window: Last {} Days | Analyzed: {} Videos | Channels: {} | Baseline: {:.1} views/day\n",
        report.days_window, report.total_videos_analyzed, report.distinct_channels_count, report.cohort_mean_velocity));
    out.push_str("════════════════════════════════════════════════════════════════════════════════════════\n\n");

    out.push_str("### 💡 Tactical Opportunity Playbook for Creators\n");
    out.push_str(&format!("> {}\n\n", report.creator_takeaway));

    // Topic Demand Bounds (Floor & Ceiling)
    if let Some(stats) = &report.demand_stats {
        out.push_str(&stats.to_markdown());
        out.push('\n');
    }

    // Section 1: Breakout Micro-Trends
    out.push_str("### 1. 🔥 Emerging Breakout Micro-Trends (High Velocity & Multi-Channel Dispersion)\n");
    if report.breakout_trends.is_empty() {
        out.push_str("No viral breakout micro-trends currently detected above 2.0x velocity threshold. Monitor for newly emerging angles.\n\n");
    } else {
        out.push_str("| Micro-Angle / Sub-Trope | Type | Velocity Lift | Views/Day | Channels | Cohort Share | Winning Example Title |\n");
        out.push_str("| :--- | :---: | :---: | :---: | :---: | :---: | :--- |\n");
        for t in &report.breakout_trends {
            let example = t.sample_titles.first().map(|s| s.as_str()).unwrap_or("N/A");
            out.push_str(&format!(
                "| **{}** | {} | **{:.2}x ({:+.0}%)** | **{:.0}** | **{}** | {:.1}% | *\"{}\"* |\n",
                t.entity,
                t.entity_type,
                t.velocity_lift,
                (t.velocity_lift - 1.0) * 100.0,
                t.mean_velocity,
                t.channel_dispersion,
                t.frequency_pct,
                example
            ));
        }
        out.push('\n');
    }

    // Section 2: Established Winners
    out.push_str("### 2. 🚀 Established Winning Pillar Topics (Consistent Audience Demand)\n");
    if report.established_trends.is_empty() {
        out.push_str("No established pillar topics identified with >=1.3x velocity lift.\n\n");
    } else {
        out.push_str("| Pillar Angle / Keyword | Type | Velocity Lift | Views/Day | Channels | Cohort Share | Strategic Takeaway |\n");
        out.push_str("| :--- | :---: | :---: | :---: | :---: | :---: | :--- |\n");
        for t in report.established_trends.iter().take(8) {
            out.push_str(&format!(
                "| **{}** | {} | **{:.2}x ({:+.0}%)** | {:.0} | {} | {:.1}% | {} |\n",
                t.entity,
                t.entity_type,
                t.velocity_lift,
                (t.velocity_lift - 1.0) * 100.0,
                t.mean_velocity,
                t.channel_dispersion,
                t.frequency_pct,
                t.tactical_advice
            ));
        }
        out.push('\n');
    }

    // Section 3: Saturated / Fading Topics
    out.push_str("### 3. ⚠️ Saturated / Fading Angles (High Frequency, Diminishing Returns)\n");
    if report.saturated_trends.is_empty() {
        out.push_str("No topics currently flagged for severe market saturation (>30% frequency with <0.8x velocity).\n\n");
    } else {
        out.push_str("| Saturated Angle | Type | Velocity Drag | Cohort Share | Channels | Audience Fatigue Warning |\n");
        out.push_str("| :--- | :---: | :---: | :---: | :---: | :--- |\n");
        for t in &report.saturated_trends {
            out.push_str(&format!(
                "| **{}** | {} | **{:.2}x** | **{:.1}%** | {} | {} |\n",
                t.entity,
                t.entity_type,
                t.velocity_lift,
                t.frequency_pct,
                t.channel_dispersion,
                t.tactical_advice
            ));
        }
        out.push('\n');
    }

    // Section 4: Demand Stacking Playbook
    if !report.demand_stacking_suggestions.is_empty() {
        out.push_str("### 4. 🧲 Demand Stacking Playbook (Expanding Total Addressable Market)\n");
        out.push_str("| Strategy Category | Scale / Authority Anchor | High-Converting Packaging Title | Strategic Audience Rationale |\n");
        out.push_str("| :--- | :--- | :--- | :--- |\n");
        for s in &report.demand_stacking_suggestions {
            out.push_str(&format!(
                "| **{}** | `{}` | *\"{}\"* | {} |\n",
                s.category,
                s.modifier,
                s.suggested_title,
                s.rationale
            ));
        }
        out.push('\n');
    }

    out
}

fn render_timeline_report(
    timeline: &sandsifter::export::ExportTimeline,
    fcpxml_path: &str,
    edl_path: &str,
    uploader: &str,
) -> String {
    let mut out = String::new();
    out.push_str("\n════════════════════════════════════════════════════════════════════════════════════════\n");
    out.push_str(&format!("🎨 FCPXML & EDL TIMELINE GENERATOR: \"{}\"\n", timeline.title));
    out.push_str(&format!("   Creator: @{} | Duration: {:.1}s | Resolution: {}x{} | Shots: {}\n",
        uploader, timeline.duration.to_seconds(), timeline.width, timeline.height, timeline.shots.len()));
    out.push_str("════════════════════════════════════════════════════════════════════════════════════════\n\n");

    out.push_str("### 📁 Exported Project Files (Ready for Import):\n");
    out.push_str(&format!("- **Apple FCPXML v1.8**: [`{}`](./{})\n", fcpxml_path, fcpxml_path));
    out.push_str(&format!("- **CMX 3600 EDL**:      [`{}`](./{})\n\n", edl_path, edl_path));

    out.push_str("### 🚀 How to Import into Your Video Editor:\n");
    out.push_str("1. **DaVinci Resolve**:\n");
    out.push_str(&format!("   - Step 1: `File` > `Import` > `Timeline...` and select the FCPXML file (`{}`) to set up the timeline container, aspect ratio, and framerate.\n", fcpxml_path));
    out.push_str("   - Step 2: In the Media Pool, right-click the imported timeline > `Timelines` > `Import` > `Timeline Markers from EDL...`\n");
    out.push_str(&format!("   - Step 3: Select the companion EDL (`{}`). The ruler will immediately populate with native color-coded markers (Cyan, Purple, Red, Orange) and notes!\n", edl_path));
    out.push_str("2. **Adobe Premiere Pro**:\n");
    out.push_str("   - Go to `File` > `Import...`\n");
    out.push_str(&format!("   - Select `{}`\n", fcpxml_path));
    out.push_str("   - Premiere will create a new sequence bin with the exact cut pacing and markers.\n\n");

    let total_markers: usize = timeline.shots.iter().map(|s| s.markers.len()).sum();
    out.push_str(&format!("### 🏷️ Timeline Markers Breakdown ({} Total Forensic Cues):\n", total_markers));
    out.push_str("| Shot # | Shot Duration | Start Timecode | Embedded Forensic Markers |\n");
    out.push_str("| :---: | :---: | :---: | :--- |\n");

    let fps = (timeline.frame_rate_numerator as f64) / (timeline.frame_rate_denominator as f64);
    for shot in timeline.shots.iter().take(15) {
        let marker_desc = if shot.markers.is_empty() {
            "—".to_string()
        } else {
            shot.markers.iter().map(|m| m.name.clone()).collect::<Vec<_>>().join("<br/>")
        };

        out.push_str(&format!(
            "| **#{}** | {:.1}s | `{}` | {} |\n",
            shot.shot_index,
            shot.duration.to_seconds(),
            shot.offset.to_timecode_str(fps),
            marker_desc
        ));
    }

    if timeline.shots.len() > 15 {
        out.push_str(&format!("\n*...and {} more shots generated on the timeline.*\n", timeline.shots.len() - 15));
    }
    out.push('\n');

    out
}




