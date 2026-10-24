use sandsifter::models::{VideoChannelDetails, VideoDetail, WatchlistChannel};
use sandsifter::storage::*;

#[test]
fn test_database_channel_and_video_persistence() {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");

    // 1. Insert creator channel
    let channel = WatchlistChannel {
        uuid: "chan-001".to_string(),
        platform: "tiktok".to_string(),
        handle: "@datasciencedaily".to_string(),
        display_name: Some("Data Science Daily".to_string()),
        avatar_url: None,
        follower_count: Some(85_000),
        baseline_median_views: 12_000,
        video_count: Some(42),
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };

    db.upsert_channel(&channel).expect("Channel insert failed");

    let fetched = db
        .get_channel_by_handle("@datasciencedaily")
        .expect("Fetch failed")
        .expect("Channel missing");

    assert_eq!(fetched.uuid, "chan-001");
    assert_eq!(fetched.baseline_median_views, 12_000);

    // 2. Insert video
    let video = VideoDetail {
        uuid: "vid-101".to_string(),
        caption: Some("Why you should stop using Pandas".to_string()),
        url: "https://tiktok.com/@datasciencedaily/video/101".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "chan-001".to_string(),
            platform: "tiktok".to_string(),
            handle: "@datasciencedaily".to_string(),
            display_name: Some("Data Science Daily".to_string()),
            avatar_url: None,
            follower_count: Some(85_000),
            baseline_median_views: Some(12_000),
        },
        view_count: 450_000,
        like_count: Some(32_000),
        comment_count: Some(1_400),
        share_count: Some(5_200),
        outlier_score: 37.5,
        engagement_rate: 8.57,
        published_at: "2026-10-01T14:30:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };

    db.upsert_video(&video).expect("Video insert failed");

    // 3. Query top outliers
    let outliers = db.list_top_outliers(2.0, 10).expect("Outliers query failed");
    assert_eq!(outliers.len(), 1);
    assert_eq!(outliers[0].uuid, "vid-101");
    assert_eq!(outliers[0].outlier_score, 37.5);
}

#[test]
fn test_cosine_similarity_math() {
    // Identical unit vectors -> 1.0
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);

    // Orthogonal vectors -> 0.0
    let c = vec![0.0, 1.0, 0.0];
    assert!(cosine_similarity(&a, &c).abs() < 1e-5);

    // Opposite vectors -> -1.0
    let d = vec![-1.0, 0.0, 0.0];
    assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-5);

    // Zero vector -> 0.0 (safe fallback)
    let zero = vec![0.0, 0.0, 0.0];
    assert_eq!(cosine_similarity(&a, &zero), 0.0);
}

#[test]
fn test_reciprocal_rank_fusion_scoring() {
    let vector_candidates = vec!["doc-A".to_string(), "doc-B".to_string(), "doc-C".to_string()];
    let lexical_candidates = vec!["doc-B".to_string(), "doc-A".to_string(), "doc-D".to_string()];

    let fused = reciprocal_rank_fusion(&vector_candidates, &lexical_candidates, 60.0);

    // doc-A is vector rank 1 and lexical rank 2: 1/(60+1) + 1/(60+2) = 1/61 + 1/62 = 0.01639 + 0.01613 = 0.03252
    // doc-B is vector rank 2 and lexical rank 1: 1/(60+2) + 1/(60+1) = 0.03252
    // Both doc-A and doc-B should be at top with highest fused score
    assert_eq!(fused.len(), 4);
    assert!(fused[0].id == "doc-A" || fused[0].id == "doc-B");
    assert!(fused[1].id == "doc-A" || fused[1].id == "doc-B");
    assert!(fused[0].rrf_score > fused[2].rrf_score);
    assert_eq!(fused[3].id, "doc-D");
}

#[test]
fn test_hybrid_search_retrieval() {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");

    // Insert parent channel and videos first to satisfy foreign key constraints
    let channel = WatchlistChannel {
        uuid: "chan-001".to_string(),
        platform: "tiktok".to_string(),
        handle: "@datasciencedaily".to_string(),
        display_name: None,
        avatar_url: None,
        follower_count: None,
        baseline_median_views: 10_000,
        video_count: None,
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };
    db.upsert_channel(&channel).unwrap();

    for vid_id in &["vid-101", "vid-102", "vid-103"] {
        let video = VideoDetail {
            uuid: vid_id.to_string(),
            caption: None,
            url: format!("https://tiktok.com/@datasciencedaily/video/{}", vid_id),
            thumbnail: None,
            channel: VideoChannelDetails {
                uuid: "chan-001".to_string(),
                platform: "tiktok".to_string(),
                handle: "@datasciencedaily".to_string(),
                display_name: None,
                avatar_url: None,
                follower_count: None,
                baseline_median_views: Some(10_000),
            },
            view_count: 50_000,
            like_count: None,
            comment_count: None,
            share_count: None,
            outlier_score: 5.0,
            engagement_rate: 6.0,
            published_at: "2026-10-01T00:00:00Z".to_string(),
            velocity_views_per_day: None,
            analysis: None,
            speech_onset_ms: None,
            wpm_0_to_35: None,
            spoken_hook: None,
        };
        db.upsert_video(&video).unwrap();
    }

    // Index 3 hooks with embeddings
    let embed_pandas = generate_mock_embedding("Stop using pandas for big datasets", 64);
    let embed_rust = generate_mock_embedding("Why I rewrite all microservices in Rust", 64);
    let embed_general = generate_mock_embedding("Three tips to wake up earlier", 64);

    db.insert_hook(
        "hook-1",
        "vid-101",
        "Stop using pandas for big datasets. You are destroying your RAM.",
        Some("Stop using [X] for [Y]. You are destroying your [Z]."),
        3.2,
        &embed_pandas,
    )
    .unwrap();

    db.insert_hook(
        "hook-2",
        "vid-102",
        "Why I rewrite all microservices in Rust for zero latency.",
        Some("Why I rewrite [X] in [Y] for [Z]."),
        3.5,
        &embed_rust,
    )
    .unwrap();

    db.insert_hook(
        "hook-3",
        "vid-103",
        "Three tips to wake up earlier every single day.",
        None,
        2.8,
        &embed_general,
    )
    .unwrap();

    // Query for pandas with both text and query embedding
    let query_text = "pandas RAM";
    let query_vec = generate_mock_embedding("pandas big datasets memory", 64);

    let start = std::time::Instant::now();
    let results = db
        .search_hooks_hybrid(query_text, Some(&query_vec), 10)
        .expect("Hybrid search failed");
    let elapsed = start.elapsed();

    // Verification gate: sub-millisecond retrieval
    assert!(
        elapsed.as_millis() < 50,
        "Retrieval took too long: {:?}",
        elapsed
    );

    // Pandas hook must rank #1
    assert!(!results.is_empty());
    assert_eq!(results[0].hook_uuid, "hook-1");
    assert!(results[0].lexical_match);
    assert!(results[0].vector_similarity.is_some());
    assert!(results[0].rrf_score > 0.0);
}

#[test]
fn test_snapshot_persistence_and_channel_lookup() {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");

    let channel = WatchlistChannel {
        uuid: "chan-002".to_string(),
        platform: "youtube".to_string(),
        handle: "@aiexpert".to_string(),
        display_name: Some("AI Expert".to_string()),
        avatar_url: None,
        follower_count: Some(50_000),
        baseline_median_views: 20_000,
        video_count: Some(10),
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };
    db.upsert_channel(&channel).expect("Channel insert failed");

    let video = VideoDetail {
        uuid: "vid-202".to_string(),
        caption: Some("Deep Learning from scratch".to_string()),
        url: "https://youtube.com/shorts/202".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "chan-002".to_string(),
            platform: "youtube".to_string(),
            handle: "@aiexpert".to_string(),
            display_name: Some("AI Expert".to_string()),
            avatar_url: None,
            follower_count: Some(50_000),
            baseline_median_views: Some(20_000),
        },
        view_count: 85_000,
        like_count: Some(4_200),
        comment_count: Some(310),
        share_count: Some(600),
        outlier_score: 4.25,
        engagement_rate: 6.01,
        published_at: "2026-10-02T12:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };
    db.upsert_video(&video).expect("Video insert failed");

    // Insert snapshot
    let snapshot = sandsifter::models::VideoSnapshot {
        uuid: "snap-001".to_string(),
        video_uuid: "vid-202".to_string(),
        view_count: 85_000,
        like_count: Some(4_200),
        comment_count: Some(310),
        share_count: Some(600),
        recorded_at: "2026-10-02T12:00:00Z".to_string(),
    };
    db.insert_snapshot(&snapshot).expect("Snapshot insert failed");

    assert_eq!(db.count_snapshots().expect("Count failed"), 1);

    // Channel video lookup tests
    let by_handle = db.get_videos_by_channel("@aiexpert").expect("Query failed");
    assert_eq!(by_handle.len(), 1);
    assert_eq!(by_handle[0].uuid, "vid-202");

    let by_raw_handle = db.get_videos_by_channel("aiexpert").expect("Query failed");
    assert_eq!(by_raw_handle.len(), 1);
    assert_eq!(by_raw_handle[0].uuid, "vid-202");
}

#[tokio::test]
async fn test_gemini_client_cost_tracking_and_blocking_embed() {
    let client = GeminiEmbeddingClient::new(None); // Offline deterministic mode
    assert_eq!(client.character_count(), 0);
    assert_eq!(client.call_count(), 0);
    assert_eq!(client.estimated_cost_usd(), 0.0);

    let text1 = "Stop reading boring romance books";
    let vec1 = client.embed_text(text1).await;
    assert_eq!(vec1.len(), 768);
    assert_eq!(client.call_count(), 1);
    assert_eq!(client.character_count(), text1.chars().count());

    let text2 = "If you love enemies to lovers with dark secrets";
    let vec2 = client.embed_text_blocking(text2);
    assert_eq!(vec2.len(), 768);
    assert_eq!(client.call_count(), 2);
    assert_eq!(client.character_count(), text1.chars().count() + text2.chars().count());

    // Check pricing: (chars / 1000) * 0.00002
    let expected_cost = (client.character_count() as f64 / 1_000.0) * 0.00002;
    assert!((client.estimated_cost_usd() - expected_cost).abs() < 1e-9);

    let summary = client.cost_summary();
    assert!(summary.contains("Gemini text-embedding-004: 2 call(s)"));
    assert!(summary.contains("1,500 daily requests are 100% free"));
}

#[tokio::test]
async fn test_hook_indexing_and_creator_formatting() {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");

    // Parent channel & video
    let channel = WatchlistChannel {
        uuid: "chan-book".to_string(),
        platform: "tiktok".to_string(),
        handle: "@creator_studio".to_string(),
        display_name: Some("Creator Studio".to_string()),
        avatar_url: None,
        follower_count: Some(25_000),
        baseline_median_views: 8_000,
        video_count: Some(15),
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };
    db.upsert_channel(&channel).unwrap();

    let video = VideoDetail {
        uuid: "vid-book-1".to_string(),
        caption: Some("Why villain romance tropes always hit different".to_string()),
        url: "https://tiktok.com/@creator_studio/video/1".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "chan-book".to_string(),
            platform: "tiktok".to_string(),
            handle: "@creator_studio".to_string(),
            display_name: Some("Creator Studio".to_string()),
            avatar_url: None,
            follower_count: Some(25_000),
            baseline_median_views: Some(8_000),
        },
        view_count: 96_000,
        like_count: Some(8_500),
        comment_count: Some(720),
        share_count: Some(1_100),
        outlier_score: 12.0,
        engagement_rate: 10.75,
        published_at: "2026-10-01T00:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };
    db.upsert_video(&video).unwrap();

    let client = GeminiEmbeddingClient::new(None);
    let hook_text = "Stop reading fantasy books that treat the villain like a misunderstood bad boy.";
    let madlib = "Stop reading [Genre] that treats [Archetype] like [Cliché].";

    let hook_id = db
        .index_hook_with_embedding("vid-book-1", hook_text, Some(madlib), 3.2, &client)
        .await
        .expect("Hook indexing failed");

    assert!(hook_id.starts_with("hook_"));
    assert_eq!(db.count_hooks().unwrap(), 1);

    // Hybrid query for "villain fantasy"
    let query_vec = client.embed_text("villain fantasy romance").await;
    let results = db
        .search_hooks_hybrid("fantasy villain", Some(&query_vec), 5)
        .expect("Hybrid search failed");

    assert_eq!(results.len(), 1);
    let r = &results[0];
    assert_eq!(r.hook_uuid, hook_id);
    assert_eq!(r.video_uuid, "vid-book-1");
    assert_eq!(r.channel_handle.as_deref(), Some("@creator_studio"));
    assert_eq!(r.outlier_score, Some(12.0));

    // Test creator-friendly formatted report
    let formatted = r.format_for_creator(1);
    assert!(formatted.contains("Hook #1"));
    assert!(formatted.contains("Stop reading fantasy books"));
    assert!(formatted.contains("Stop reading [Genre] that treats [Archetype] like [Cliché]."));
    assert!(formatted.contains("12.0x Reader Resonance"));
    assert!(formatted.contains("@creator_studio"));
}

