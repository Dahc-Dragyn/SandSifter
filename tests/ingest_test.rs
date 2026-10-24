use sandsifter::ingest::*;

#[test]
fn test_url_platform_detection() {
    // 1. TikTok
    let (p1, id1) = MediaResolver::parse_video_url("https://www.tiktok.com/@creator/video/7123456789012345678?is_from_webapp=1");
    assert_eq!(p1, MediaPlatform::TikTok);
    assert_eq!(id1.as_deref(), Some("7123456789012345678"));

    // 2. Instagram Reel
    let (p2, id2) = MediaResolver::parse_video_url("https://www.instagram.com/reel/C8xyz12345/?utm_source=ig_web_copy_link");
    assert_eq!(p2, MediaPlatform::Instagram);
    assert_eq!(id2.as_deref(), Some("C8xyz12345"));

    // 3. YouTube Shorts
    let (p3, id3) = MediaResolver::parse_video_url("https://www.youtube.com/shorts/dQw4w9WgXcQ");
    assert_eq!(p3, MediaPlatform::YouTubeShorts);
    assert_eq!(id3.as_deref(), Some("dQw4w9WgXcQ"));

    // 4. YouTu.be shortlink
    let (p4, id4) = MediaResolver::parse_video_url("https://youtu.be/dQw4w9WgXcQ?t=5");
    assert_eq!(p4, MediaPlatform::YouTubeShorts);
    assert_eq!(id4.as_deref(), Some("dQw4w9WgXcQ"));

    // 5. Unknown URL
    let (p5, id5) = MediaResolver::parse_video_url("https://example.com/not-a-video");
    assert_eq!(p5, MediaPlatform::Unknown);
    assert_eq!(id5, None);
}

#[test]
fn test_stream_manifest_resolution() {
    let resolver = MediaResolver;

    let tiktok_url = "https://www.tiktok.com/@datasciencedaily/video/987654321";
    let resolved = resolver.resolve_stream(tiktok_url).expect("Resolution failed");

    assert_eq!(resolved.platform, MediaPlatform::TikTok);
    assert_eq!(resolved.video_id, "987654321");
    assert!(resolved.is_hls_stream);
    assert!(resolved.direct_manifest_url.unwrap().contains(".m3u8"));
    assert!(resolved.audio_stream_url.unwrap().contains(".m4a"));
}

#[test]
fn test_proxy_pool_rotation_and_quarantine() {
    let proxies = vec![
        "http://proxy1.internal:8080".to_string(),
        "http://proxy2.internal:8080".to_string(),
    ];
    let pool = ProxyPool::new(proxies);

    assert_eq!(pool.total_proxies(), 2);

    // Round-robin cycling
    let p1 = pool.next_proxy().unwrap();
    let p2 = pool.next_proxy().unwrap();
    let p3 = pool.next_proxy().unwrap();

    assert_eq!(p1, "http://proxy1.internal:8080");
    assert_eq!(p2, "http://proxy2.internal:8080");
    assert_eq!(p3, "http://proxy1.internal:8080");

    // Quarantine proxy 1 after 3 failures
    pool.report_failure("http://proxy1.internal:8080");
    pool.report_failure("http://proxy1.internal:8080");
    pool.report_failure("http://proxy1.internal:8080");

    // Now only proxy 2 should be returned
    assert_eq!(pool.next_proxy().unwrap(), "http://proxy2.internal:8080");
    assert_eq!(pool.next_proxy().unwrap(), "http://proxy2.internal:8080");

    // Recover proxy 1 on success
    pool.report_success("http://proxy1.internal:8080");
    let recovered = pool.next_proxy().unwrap();
    assert!(recovered == "http://proxy1.internal:8080" || recovered == "http://proxy2.internal:8080");
}

#[tokio::test]
async fn test_in_memory_audio_streaming_zero_disk() {
    let pool = ProxyPool::default();
    let ingestor = StreamIngestor::new(pool, 1024 * 1024); // 1 MB limit

    // Ingest simulated stream URL
    let stream_url = "https://cdn.sandsifter.internal/audio/test_audio_stream.m4a";
    let audio_bytes = ingestor.ingest_audio_stream(stream_url).await.expect("Stream ingest failed");

    // Verify raw memory buffer was populated
    assert!(!audio_bytes.is_empty());
    // Verify WAV/audio header exists in memory
    assert_eq!(&audio_bytes[0..4], b"RIFF");
    assert_eq!(&audio_bytes[8..12], b"WAVE");

    // Verify zero temporary files created in workspace
    let temp_files_exist = std::fs::read_dir(".").unwrap().any(|entry| {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        name.ends_with(".tmp") || name.ends_with(".part")
    });
    assert!(!temp_files_exist, "Temporary files were left on disk!");
}

#[test]
fn test_batch_json_importer_and_empirical_seeding() {
    let db = sandsifter::storage::SandsifterDb::memory().expect("In-memory SQLite failed");

    let json_data = r#"[
        {
            "video_url": "https://www.youtube.com/shorts/vid1",
            "views": 10000,
            "likes": 500,
            "comments": 50,
            "shares": 25,
            "channel": "@techforensics",
            "published_at": "2026-03-01T10:00:00Z"
        },
        {
            "video_url": "https://www.youtube.com/shorts/vid2",
            "views": 12000,
            "likes": 600,
            "comments": 60,
            "shares": 30,
            "channel": "@techforensics",
            "published_at": "2026-03-02T10:00:00Z"
        },
        {
            "video_url": "https://www.youtube.com/shorts/vid3",
            "views": 11000,
            "likes": 550,
            "comments": 55,
            "shares": 28,
            "channel": "@techforensics",
            "published_at": "2026-03-03T10:00:00Z"
        },
        {
            "video_url": "https://www.youtube.com/shorts/vid4",
            "views": 9500,
            "likes": 480,
            "comments": 45,
            "shares": 20,
            "channel": "@techforensics",
            "published_at": "2026-03-04T10:00:00Z"
        },
        {
            "video_url": "https://www.youtube.com/shorts/vid_viral",
            "views": 250000,
            "likes": 25000,
            "comments": 1200,
            "shares": 3400,
            "channel": "@techforensics",
            "published_at": "2026-03-05T10:00:00Z"
        }
    ]"#;

    let res = BatchImporter::import_from_json_str(&db, json_data, None)
        .expect("Import failed");

    assert_eq!(res.channels_updated, 1);
    assert_eq!(res.videos_imported, 5);
    assert_eq!(res.snapshots_recorded, 5);

    // Baseline should reflect rolling median of the cohort (~11,000)
    let (_, baseline) = res.channel_baselines[0];
    assert!((9500..=12500).contains(&baseline), "Baseline was {}", baseline);

    // Database verification: videos persisted
    let videos = db.get_videos_by_channel("@techforensics").expect("Fetch failed");
    assert_eq!(videos.len(), 5);

    // Database verification: snapshots persisted
    let snap_count = db.count_snapshots().expect("Count failed");
    assert_eq!(snap_count, 5);

    // Outlier multiplier verification on viral video: 250k / 11k ~ 22.7x
    let viral = videos.iter().find(|v| v.view_count == 250000).expect("Viral video missing");
    assert!(viral.outlier_score > 20.0, "Outlier score was {}", viral.outlier_score);

    // Run The Hook Machine with this empirical dataset
    let report = sandsifter::pipeline::HookMachineEngine::execute("@techforensics", "Reverse Engineering", &videos);
    assert_eq!(report.total_videos_analyzed, 5);
    assert_eq!(report.creator_baseline_views, baseline);
    assert_eq!(report.outlier_videos_found, 1);
}

#[test]
fn test_batch_importer_channel_feed_envelope() {
    let db = sandsifter::storage::SandsifterDb::memory().expect("In-memory SQLite failed");

    let json_data = r#"{
        "channel": "@rustguru",
        "platform": "YouTubeShorts",
        "videos": [
            {
                "url": "https://www.youtube.com/shorts/rust1",
                "view_count": 8000,
                "like_count": 400
            },
            {
                "url": "https://www.youtube.com/shorts/rust2",
                "view_count": 9200,
                "like_count": 480
            },
            {
                "url": "https://www.youtube.com/shorts/rust_mega",
                "view_count": 95000,
                "like_count": 7500
            }
        ]
    }"#;

    let res = BatchImporter::import_from_json_str(&db, json_data, None)
        .expect("Import failed");

    assert_eq!(res.channels_updated, 1);
    assert_eq!(res.videos_imported, 3);
    assert_eq!(res.snapshots_recorded, 3);

    let videos = db.get_videos_by_channel("@rustguru").expect("Fetch failed");
    assert_eq!(videos.len(), 3);
    let top = videos.iter().max_by_key(|v| v.view_count).unwrap();
    assert_eq!(top.view_count, 95000);
    assert!(top.outlier_score > 9.0);
}

#[test]
fn test_channel_crawler_url_normalization() {
    let (u1, h1) = ChannelCrawler::normalize_channel_url("@creator_studio");
    assert_eq!(u1, "https://www.youtube.com/@creator_studio/shorts");
    assert_eq!(h1, "@creator_studio");

    let (u2, h2) = ChannelCrawler::normalize_channel_url("author_jane");
    assert_eq!(u2, "https://www.youtube.com/@author_jane/shorts");
    assert_eq!(h2, "@author_jane");

    let (u3, h3) = ChannelCrawler::normalize_channel_url("https://www.youtube.com/@colleenhoover/shorts");
    assert_eq!(u3, "https://www.youtube.com/@colleenhoover/shorts");
    assert_eq!(h3, "@colleenhoover");

    let (u4, h4) = ChannelCrawler::normalize_channel_url("https://www.youtube.com/@colleenhoover");
    assert_eq!(u4, "https://www.youtube.com/@colleenhoover/shorts");
    assert_eq!(h4, "@colleenhoover");
}

#[test]
fn test_channel_crawler_summary_and_velocity_report() {
    let summary = CrawlSummary {
        channel_handle: "@creator_studio".to_string(),
        channel_name: "Creator Studio".to_string(),
        platform: "YouTubeShorts".to_string(),
        total_shorts_scanned: 15,
        author_baseline_views: 12000,
        viral_outliers: vec![
            CrawledVideoItem {
                title: "When the villain falls first (Trope breakdown)".to_string(),
                video_url: "https://www.youtube.com/shorts/outlier1".to_string(),
                views: 240000,
                previous_views: Some(190000),
                view_growth: 50000,
                channel_multiplier: 20.0,
                series_multiplier: 20.0,
                outlier_multiplier: 20.0,
                is_viral_outlier: true,
                is_accelerating: true,
                published_at: Some("2026-03-01T00:00:00Z".to_string()),
                spoken_hook: Some("When the villain falls first".to_string()),
                hook_madlib: Some("When the [NOUN] falls first".to_string()),
                speech_onset_ms: Some(150),
                wpm_0_to_35: Some(180.0),
            }
        ],
        accelerating_videos: vec![
            CrawledVideoItem {
                title: "When the villain falls first (Trope breakdown)".to_string(),
                video_url: "https://www.youtube.com/shorts/outlier1".to_string(),
                views: 240000,
                previous_views: Some(190000),
                view_growth: 50000,
                channel_multiplier: 20.0,
                series_multiplier: 20.0,
                outlier_multiplier: 20.0,
                is_viral_outlier: true,
                is_accelerating: true,
                published_at: Some("2026-03-01T00:00:00Z".to_string()),
                spoken_hook: Some("When the villain falls first".to_string()),
                hook_madlib: Some("When the [NOUN] falls first".to_string()),
                speech_onset_ms: Some(150),
                wpm_0_to_35: Some(180.0),
            }
        ],
        all_videos: Vec::new(),
        takeaway_for_creator: "Focus on character tension and reading tropes.".to_string(),
        diagnostics: None,
    };

    let report = summary.to_creator_report();
    assert!(report.contains("Creator Shorts Intelligence: Creator Studio"));
    assert!(report.contains("20.0x Virality Multiplier"));
    assert!(report.contains("Gained **+50000 new views**"));
    assert!(report.contains("Creator Action Plan"));
}


