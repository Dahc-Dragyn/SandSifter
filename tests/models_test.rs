use sandsifter::models::*;
use serde_json::json;

#[test]
fn test_video_detail_roundtrip_with_analysis() {
    let raw_json = json!({
        "uuid": "vid-12345678",
        "caption": "Why you should stop using Pandas for 10GB+ CSVs",
        "url": "https://tiktok.com/@datasciencedaily/video/12345678",
        "thumbnail": "https://cdn.sandcastles.ai/thumbs/12345678.webp",
        "channel": {
            "uuid": "chan-9999",
            "platform": "tiktok",
            "handle": "@datasciencedaily",
            "display_name": "Data Science Daily",
            "avatar_url": "https://cdn.sandcastles.ai/avatars/chan-9999.webp",
            "follower_count": 85000,
            "baseline_median_views": 12000
        },
        "view_count": 450000,
        "like_count": 32000,
        "comment_count": 1400,
        "share_count": 5200,
        "outlier_score": 37.5,
        "engagement_rate": 8.57,
        "published_at": "2026-10-01T14:30:00Z",
        "analysis": {
            "format_category": "Educational Breakdown",
            "format_type": "Contrarian Mythbuster",
            "format_flavor": "Fast-Paced Screen Split",
            "format_reasoning": "Subverts expectation within first 2 seconds using rapid visual benchmark.",
            "spoken_hook": "Stop using pandas for large datasets. You are destroying your RAM.",
            "spoken_hook_madlib": "Stop using [X] for [Y]. You are destroying your [Z].",
            "spoken_hook_category": "Direct Warning / Negative Framing",
            "hook_alignment": "Spoken hook matches headline on-screen text 100%.",
            "visual_layout_category": "Split Screen",
            "visual_layout_type": "Talking Head Top / Terminal Recording Bottom",
            "narrative_structure": {
                "structure_summary": "Problem statement -> agitate memory leak -> introduce Polars alternative.",
                "structure_sections": [
                    {
                        "section_name": "Opening Hook",
                        "section_description": "Initial status threat / warning to viewer.",
                        "transcript_sentences": [
                            "Stop using pandas for large datasets.",
                            "You are destroying your RAM."
                        ],
                        "start_second": 0.0,
                        "end_second": 3.2
                    },
                    {
                        "section_name": "Agitation & Proof",
                        "section_description": "Demonstrates RAM explosion on a 15GB CSV.",
                        "transcript_sentences": [
                            "Watch what happens when we read 15GB into memory.",
                            "Boom, OOM killed."
                        ],
                        "start_second": 3.2,
                        "end_second": 12.5
                    }
                ]
            },
            "seed": "Big Data Python optimizations",
            "topic": "Python Memory Performance",
            "angle": "Why Polars outperforms Pandas",
            "summary": "Demonstration showing why Polars is superior to Pandas on big data sets.",
            "supporting_evidence": "Terminal memory benchmark output shown at 0:08.",
            "common_belief": "Pandas is the default standard for all data tasks.",
            "contrarian_reality": "Pandas is inefficient and crashes on datasets larger than memory."
        }
    });

    let video: VideoDetail = serde_json::from_value(raw_json).expect("Deserialization failed");
    assert_eq!(video.uuid, "vid-12345678");
    assert_eq!(video.outlier_score, 37.5);
    assert_eq!(video.view_count, 450000);

    let analysis = video.analysis.as_ref().expect("Analysis missing");
    assert_eq!(
        analysis.spoken_hook_madlib.as_deref(),
        Some("Stop using [X] for [Y]. You are destroying your [Z].")
    );
    assert_eq!(analysis.narrative_structure.structure_sections.len(), 2);
    assert_eq!(
        analysis.common_belief.as_deref(),
        Some("Pandas is the default standard for all data tasks.")
    );

    // Verify round-trip re-serialization
    let serialized = serde_json::to_value(&video).expect("Serialization failed");
    let video_back: VideoDetail = serde_json::from_value(serialized).expect("Round-trip failed");
    assert_eq!(video, video_back);
}

#[test]
fn test_video_detail_partial_optional_fields() {
    let minimal_json = json!({
        "uuid": "vid-min-001",
        "url": "https://youtube.com/shorts/minimal",
        "channel": {
            "uuid": "chan-001",
            "platform": "youtube",
            "handle": "@coder"
        },
        "view_count": 1000,
        "published_at": "2026-10-07T00:00:00Z"
    });

    let video: VideoDetail = serde_json::from_value(minimal_json).expect("Minimal parse failed");
    assert_eq!(video.uuid, "vid-min-001");
    assert_eq!(video.caption, None);
    assert_eq!(video.like_count, None);
    assert_eq!(video.outlier_score, 0.0);
    assert_eq!(video.analysis, None);
}

#[test]
fn test_video_search_filters() {
    let filters = VideoSearchFilters {
        platform: Some("tiktok".to_string()),
        outlier_score_min: Some(3.0),
        views_min: Some(50000),
        engagement_rate_min: Some(2.0),
        lookback_interval: Some("30d".to_string()),
        include_analyzed: Some(true),
        ..Default::default()
    };

    let val = serde_json::to_value(&filters).unwrap();
    assert_eq!(val["platform"], "tiktok");
    assert_eq!(val["outlier_score_min"], 3.0);
    assert_eq!(val["views_min"], 50000);
}

#[test]
fn test_automation_rule_serde() {
    let rule_json = json!({
        "uuid": "rule-auto-1",
        "name": "Auto-Analyze 3x Outliers",
        "channel_uuid": "chan-9999",
        "outlier_score_min": 3.0,
        "views_min": 25000,
        "engagement_rate_min_percent": 2.0,
        "daily_limit": 5,
        "active": true,
        "managed": false,
        "created_at": "2026-10-01T00:00:00Z",
        "updated_at": "2026-10-01T00:00:00Z"
    });

    let rule: AutomationRule = serde_json::from_value(rule_json).expect("Rule parse failed");
    assert_eq!(rule.name, "Auto-Analyze 3x Outliers");
    assert_eq!(rule.outlier_score_min, 3.0);
    assert_eq!(rule.daily_limit, 5);
}

#[test]
fn test_mcp_tool_protocol_payloads() {
    let tool_def = McpToolDefinition {
        name: "/analyze".to_string(),
        description: "Deep-analyzes a video URL".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "video_url": { "type": "string" }
            },
            "required": ["video_url"]
        }),
    };

    let json_val = serde_json::to_value(&tool_def).unwrap();
    assert_eq!(json_val["name"], "/analyze");
    assert_eq!(json_val["inputSchema"]["type"], "object");

    let success_result = McpToolResult::success("Analysis complete: Outlier 5.2x");
    assert!(!success_result.is_error);
    assert_eq!(success_result.content[0].text, "Analysis complete: Outlier 5.2x");

    let error_result = McpToolResult::error("Video URL is private or restricted");
    assert!(error_result.is_error);
    assert_eq!(error_result.content[0].text, "Video URL is private or restricted");
}
