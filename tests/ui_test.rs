use comfy_table::Color;
use sandsifter::math::outlier::OutlierTier;
use sandsifter::models::{VideoChannelDetails, VideoDetail};
use sandsifter::ui::TerminalTableRenderer;

#[test]
fn test_number_formatting_with_commas() {
    assert_eq!(TerminalTableRenderer::format_number(0), "0");
    assert_eq!(TerminalTableRenderer::format_number(42), "42");
    assert_eq!(TerminalTableRenderer::format_number(999), "999");
    assert_eq!(TerminalTableRenderer::format_number(1_000), "1,000");
    assert_eq!(TerminalTableRenderer::format_number(12_345), "12,345");
    assert_eq!(TerminalTableRenderer::format_number(485_000), "485,000");
    assert_eq!(TerminalTableRenderer::format_number(1_000_000), "1,000,000");
}

#[test]
fn test_classify_video_tiers_and_colors() {
    // 1. Breakout Viral (>5.0x) -> Green
    let v_viral = VideoDetail {
        uuid: "v1".to_string(),
        caption: Some("Viral Hit".to_string()),
        url: "https://tiktok.com/@creator/1".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "c1".to_string(),
            platform: "tiktok".to_string(),
            handle: "@creator".to_string(),
            display_name: None,
            avatar_url: None,
            follower_count: None,
            baseline_median_views: Some(10_000),
        },
        view_count: 85_000,
        like_count: Some(8_000),
        comment_count: Some(500),
        share_count: Some(1_000),
        outlier_score: 8.5,
        engagement_rate: 11.17,
        published_at: "2026-10-01T00:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };
    let (tier, is_paid, color, badge) = TerminalTableRenderer::classify_video_tier(&v_viral);
    assert_eq!(tier, OutlierTier::BreakoutViral);
    assert!(!is_paid);
    assert_eq!(color, Color::Green);
    assert!(badge.contains("🟩"));
    assert!(badge.contains(">5x"));

    // 2. Winner Outlier (2.0x-5.0x) -> Cyan / Blue
    let v_winner = VideoDetail {
        outlier_score: 3.2,
        ..v_viral.clone()
    };
    let (tier, is_paid, color, badge) = TerminalTableRenderer::classify_video_tier(&v_winner);
    assert_eq!(tier, OutlierTier::WinnerOutlier);
    assert!(!is_paid);
    assert_eq!(color, Color::Cyan);
    assert!(badge.contains("🟦"));
    assert!(badge.contains("2x-5x"));

    // 3. Baseline Reach (<2.0x) -> Yellow
    let v_baseline = VideoDetail {
        outlier_score: 1.2,
        ..v_viral.clone()
    };
    let (tier, is_paid, color, badge) = TerminalTableRenderer::classify_video_tier(&v_baseline);
    assert_eq!(tier, OutlierTier::Baseline);
    assert!(!is_paid);
    assert_eq!(color, Color::Yellow);
    assert!(badge.contains("🟨"));

    // 4. Paid Boost / Inorganic Ad (< 2.0% engagement with high views) -> Red
    let v_paid = VideoDetail {
        view_count: 500_000,
        like_count: Some(1_200), // 1200 / 500000 = 0.24% (< 2.0% gate)
        comment_count: Some(50),
        share_count: Some(20),
        engagement_rate: 0.25,
        outlier_score: 15.0,
        ..v_viral.clone()
    };
    let (_tier, is_paid, color, badge) = TerminalTableRenderer::classify_video_tier(&v_paid);
    assert!(is_paid);
    assert_eq!(color, Color::Red);
    assert!(badge.contains("🟥"));
    assert!(badge.contains("Paid Boost"));
}

#[test]
fn test_render_watchlist_utf8_table() {
    let videos = vec![
        VideoDetail {
            uuid: "v1".to_string(),
            caption: Some("Why you should stop using Pandas".to_string()),
            url: "https://tiktok.com/@datasciencedaily/1".to_string(),
            thumbnail: None,
            channel: VideoChannelDetails {
                uuid: "c1".to_string(),
                platform: "tiktok".to_string(),
                handle: "@datasciencedaily".to_string(),
                display_name: None,
                avatar_url: None,
                follower_count: None,
                baseline_median_views: Some(11_200),
            },
            view_count: 485_000,
            like_count: Some(45_000),
            comment_count: Some(3_200),
            share_count: Some(5_800),
            outlier_score: 43.3,
            engagement_rate: 11.13,
            published_at: "2026-10-01T00:00:00Z".to_string(),
        velocity_views_per_day: None,
            analysis: None,
            speech_onset_ms: None,
            wpm_0_to_35: None,
            spoken_hook: None,
        },
        VideoDetail {
            uuid: "v2".to_string(),
            caption: Some("DuckDB benchmark on 50GB".to_string()),
            url: "https://tiktok.com/@datasciencedaily/2".to_string(),
            thumbnail: None,
            channel: VideoChannelDetails {
                uuid: "c1".to_string(),
                platform: "tiktok".to_string(),
                handle: "@datasciencedaily".to_string(),
                display_name: None,
                avatar_url: None,
                follower_count: None,
                baseline_median_views: Some(11_200),
            },
            view_count: 118_000,
            like_count: Some(10_000),
            comment_count: Some(800),
            share_count: Some(1_200),
            outlier_score: 10.5,
            engagement_rate: 10.19,
            published_at: "2026-10-02T00:00:00Z".to_string(),
            velocity_views_per_day: None,
            analysis: None,
            speech_onset_ms: None,
            wpm_0_to_35: None,
            spoken_hook: None,
        },
    ];

    let table_str = TerminalTableRenderer::render_watchlist_table(&videos);

    // Verify UTF-8 borders and table headers
    assert!(table_str.contains("Status / Tier"));
    assert!(table_str.contains("Multiplier"));
    assert!(table_str.contains("Creator"));
    assert!(table_str.contains("Views"));
    assert!(table_str.contains("Baseline"));
    assert!(table_str.contains("Eng. Rate"));
    assert!(table_str.contains("Title / Caption"));

    // Verify row contents
    assert!(table_str.contains("@datasciencedaily"));
    assert!(table_str.contains("485,000"));
    assert!(table_str.contains("11,200"));
    assert!(table_str.contains("43.3x"));
    assert!(table_str.contains("10.5x"));
    assert!(table_str.contains("🟩 Breakout (>5x)"));
}

#[test]
fn test_render_import_tables() {
    let baselines = vec![("@datasciencedaily".to_string(), 11_200)];
    let outliers = vec![VideoDetail {
        uuid: "v1".to_string(),
        caption: Some("Breakout Python tip".to_string()),
        url: "https://tiktok.com/@datasciencedaily/1".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "c1".to_string(),
            platform: "tiktok".to_string(),
            handle: "@datasciencedaily".to_string(),
            display_name: None,
            avatar_url: None,
            follower_count: None,
            baseline_median_views: Some(11_200),
        },
        view_count: 350_000,
        like_count: Some(30_000),
        comment_count: Some(2_000),
        share_count: Some(4_000),
        outlier_score: 31.25,
        engagement_rate: 10.28,
        published_at: "2026-10-01T00:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    }];

    let output = TerminalTableRenderer::render_import_tables(1, 10, 10, &baselines, &outliers);

    assert!(output.contains("Batch Ingestion Overview"));
    assert!(output.contains("Channels Ingested / Updated"));
    assert!(output.contains("Historical Videos Ingested"));
    assert!(output.contains("Performance Snapshots Recorded"));
    assert!(output.contains("Creator Algorithmic Baselines"));
    assert!(output.contains("@datasciencedaily"));
    assert!(output.contains("11,200 views"));
    assert!(output.contains("Detected Top Outlier Videos"));
    assert!(output.contains("31.2x"));
}
