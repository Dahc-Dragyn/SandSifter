use sandsifter::mcp::server::McpServer;
use sandsifter::math::retention::{HeatmapPoint, RetentionEngine, VttCue};
use sandsifter::math::topics::{TopicMomentumEngine, TopicVideoInput, TrendClassification};
use sandsifter::math::visual::{CutVelocityClassification, VisualPacingEngine};
use serde_json::json;

#[test]
fn test_mcp_formats_watchlist_tool() {
    let mut server = McpServer::new();
    let res = server.execute_tool("/formats-watchlist", &json!({ "channel": "@Vanwives" }));

    assert!(!res.is_error);
    let content = &res.content[0].text;

    // Checks Average Shot Length benchmarks
    assert!(content.contains("Average Shot Length (ASL)"));
    assert!(content.contains("Rapid / High Velocity"));
    assert!(content.contains("Standard Narrative"));
    assert!(content.contains("Slow / Contemplative"));

    // Checks narrative format archetype lifts
    assert!(content.contains("Peril / Jeopardy Cold Open"));
    assert!(content.contains("+45% to +60%"));
    assert!(content.contains("Asset / Hazard Focus"));

    // Checks golden editing rules
    assert!(content.contains("The 1.3s Cinematic Pause"));
    assert!(content.contains("Measured Delivery (130-150 WPM)"));
}

#[test]
fn test_mcp_topic_tool_execution() {
    let mut server = McpServer::new();
    let res = server.execute_tool("/topic", &json!({ "topic": "Dark Romance Books", "days": 7, "limit": 10 }));

    assert!(!res.is_error);
    let content = &res.content[0].text;
    assert!(content.contains("Semantic Topic Momentum") || content.contains("Topic Momentum Analysis"));
    assert!(content.contains("Dark Romance Books"));
}

#[test]
fn test_mcp_analyze_tool_structure() {
    let mut server = McpServer::new();
    let res = server.execute_tool("/analyze", &json!({
        "video_url": "https://www.youtube.com/watch?v=e7DU59tVRjA",
        "include_retention": true,
        "include_pacing": false
    }));

    assert!(!res.is_error);
    let content = &res.content[0].text;

    assert!(content.contains("Full-Funnel Deconstructed Video Intelligence"));
    assert!(content.contains("Sub-3.5s Acoustic & Timing Alignment"));
    assert!(content.contains("Forensic Narrative Breakdown"));
    assert!(content.contains("Empirical Engagement Floor Check"));
}

#[test]
fn test_full_funnel_cross_engine_composition() {
    // 1. Retention Engine Analysis
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 10.0, value: 0.95 },
        HeatmapPoint { start_time: 10.0, end_time: 20.0, value: 0.60 }, // -35% dip
        HeatmapPoint { start_time: 20.0, end_time: 30.0, value: 0.85 }, // +25% spike
    ];
    let cues = vec![
        VttCue { start_sec: 9.0, end_sec: 14.0, text: "Let me check the battery terminals here.".to_string() },
        VttCue { start_sec: 19.0, end_sec: 25.0, text: "Look at the sparks flying everywhere!".to_string() },
    ];
    let retention_report = RetentionEngine::analyze_curve("vid_1", "Battery Fire", 30.0, &heatmap, &cues);
    assert_eq!(retention_report.major_dips.len(), 1);
    assert_eq!(retention_report.major_spikes.len(), 1);

    // 2. Visual Kinetics Engine Analysis
    let scene_cuts = vec![3.2, 7.8, 11.0, 16.5, 22.0];
    let pacing_metrics = VisualPacingEngine::evaluate_visual_pacing(30.0, &scene_cuts);
    assert_eq!(pacing_metrics.cut_classification, CutVelocityClassification::SlowContemplative);
    assert_eq!(pacing_metrics.total_scene_cuts, 5);

    // 3. Topic Momentum Engine Analysis
    let cohort = vec![
        TopicVideoInput::new("DIY Lithium Battery Upgrade", "Boat power build", "SailingLife", "https://yt.com/1", 50_000, 3.0),
        TopicVideoInput::new("Lithium Battery Fire Risk", "Emergency electrical failure", "SailingLife", "https://yt.com/2", 120_000, 3.0),
    ];
    let topic_report = TopicMomentumEngine::analyze_cohort_trends("Lithium Battery", 7, &cohort);
    assert_eq!(topic_report.total_videos_analyzed, 2);
    let battery_trend = topic_report.all_entities.iter().find(|t| t.entity == "battery");
    assert!(battery_trend.is_some());
    assert_eq!(battery_trend.unwrap().classification, TrendClassification::BaselineVolume);
}
