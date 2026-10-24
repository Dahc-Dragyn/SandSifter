use sandsifter::mcp::*;
use serde_json::json;

#[test]
fn test_mcp_initialize_handshake() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: None,
    };

    let resp = server.handle_request(req).expect("Handshake response missing");
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, json!(1));

    let result = resp.result.expect("Result missing");
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "sandsifter");
}

#[test]
fn test_mcp_tools_list_catalog() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/list".to_string(),
        params: None,
    };

    let resp = server.handle_request(req).expect("Tools list response missing");
    let result = resp.result.expect("Result missing");
    let tools = result["tools"].as_array().expect("Tools array missing");

    // All 9 core tools + usage checker
    assert!(tools.len() >= 10);

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"/analyze"));
    assert!(tool_names.contains(&"/videos-watchlist"));
    assert!(tool_names.contains(&"/hooks-watchlist"));
    assert!(tool_names.contains(&"/formats-watchlist"));
    assert!(tool_names.contains(&"/topic"));
    assert!(tool_names.contains(&"/channels-add"));
    assert!(tool_names.contains(&"/channels-recap"));
    assert!(tool_names.contains(&"/channels-search"));
    assert!(tool_names.contains(&"/video-suggest"));
    assert!(tool_names.contains(&"current_org_usage"));
}

#[test]
fn test_mcp_call_analyze() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/analyze",
            "arguments": {
                "video_url": "https://tiktok.com/@datasciencedaily/video/12345678"
            }
        })),
    };

    let resp = server.handle_request(req).expect("Response missing");
    let result = resp.result.expect("Result missing");
    assert_eq!(result["isError"], false);

    let content = result["content"][0]["text"].as_str().expect("Content missing");
    assert!(content.contains("Full-Funnel Deconstructed Video Intelligence"));
    assert!(content.contains("Sub-3.5s Acoustic & Timing Alignment"));
    assert!(content.contains("Forensic Narrative Breakdown"));
}

#[test]
fn test_mcp_call_videos_watchlist() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/videos-watchlist",
            "arguments": {
                "limit": 5
            }
        })),
    };

    let resp = server.handle_request(req).expect("Response missing");
    let result = resp.result.expect("Result missing");
    let text = result["content"][0]["text"].as_str().expect("Text missing");

    assert!(text.contains("Top Outlier Videos from Watchlist:"));
    assert!(text.contains("@datasciencedaily"));
    assert!(text.contains("Multiplier:"));
}

#[test]
fn test_mcp_call_channels_add_and_recap() {
    let mut server = McpServer::new();

    // 1. Add channel
    let add_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(5)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/channels-add",
            "arguments": {
                "channel_urls": ["@techlead", "@tomasztunguz"]
            }
        })),
    };

    let add_resp = server.handle_request(add_req).expect("Add response missing");
    let add_res = add_resp.result.expect("Result missing");
    assert_eq!(add_res["isError"], false);

    // 2. Query recap
    let recap_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(6)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/channels-recap",
            "arguments": {
                "handle": "@datasciencedaily"
            }
        })),
    };

    let recap_resp = server.handle_request(recap_req).expect("Recap response missing");
    let recap_res = recap_resp.result.expect("Result missing");
    let recap_text = recap_res["content"][0]["text"].as_str().expect("Text missing");
    assert!(recap_text.contains("Channel Recap for @datasciencedaily"));
}

#[test]
fn test_mcp_unknown_method_error() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(99)),
        method: "invalid/nonexistent".to_string(),
        params: None,
    };

    let resp = server.handle_request(req).expect("Response missing");
    let err = resp.error.expect("Error missing");
    assert_eq!(err.code, JsonRpcError::METHOD_NOT_FOUND);
}

#[test]
fn test_mcp_call_hooks_watchlist_curated_fallback() {
    let mut server = McpServer::new();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(7)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/hooks-watchlist",
            "arguments": {}
        })),
    };

    let resp = server.handle_request(req).expect("Response missing");
    let result = resp.result.expect("Result missing");
    assert_eq!(result["isError"], false);

    let text = result["content"][0]["text"].as_str().expect("Text missing");
    assert!(text.contains("Winning Hooks & Madlibs Across Watchlist"));
    assert!(text.contains("Stop using [X] for [Y]"));
    assert!(text.contains("BookTok Subversion"));
}

#[test]
fn test_mcp_call_hooks_watchlist_with_mounted_database() {
    let db = sandsifter::storage::SandsifterDb::memory().unwrap();

    // Seed channel & video
    let channel = sandsifter::models::WatchlistChannel {
        uuid: "chan-author".to_string(),
        platform: "tiktok".to_string(),
        handle: "@creator_studio".to_string(),
        display_name: Some("Creator Studio".to_string()),
        avatar_url: None,
        follower_count: Some(20_000),
        baseline_median_views: 5_000,
        video_count: Some(10),
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };
    db.upsert_channel(&channel).unwrap();

    let video = sandsifter::models::VideoDetail {
        uuid: "vid-author-1".to_string(),
        caption: Some("The secret about writing morally gray characters".to_string()),
        url: "https://tiktok.com/@creator_studio/video/100".to_string(),
        thumbnail: None,
        channel: sandsifter::models::VideoChannelDetails {
            uuid: "chan-author".to_string(),
            platform: "tiktok".to_string(),
            handle: "@creator_studio".to_string(),
            display_name: Some("Creator Studio".to_string()),
            avatar_url: None,
            follower_count: Some(20_000),
            baseline_median_views: Some(5_000),
        },
        view_count: 75_000,
        like_count: Some(6_000),
        comment_count: Some(400),
        share_count: Some(800),
        outlier_score: 15.0,
        engagement_rate: 9.6,
        published_at: "2026-10-01T00:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };
    db.upsert_video(&video).unwrap();

    let client = sandsifter::storage::gemini_client::GeminiEmbeddingClient::new(None);
    db.index_hook_with_embedding_blocking(
        "vid-author-1",
        "The dark secret about morally gray characters that authors never tell you.",
        Some("The dark secret about [Character Archetype] that authors never tell you."),
        3.0,
        &client,
    )
    .unwrap();

    let mut server = McpServer::with_db(db);
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(8)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "/hooks-watchlist",
            "arguments": {
                "query": "morally gray author secret",
                "limit": 3
            }
        })),
    };

    let resp = server.handle_request(req).expect("Response missing");
    let result = resp.result.expect("Result missing");
    assert_eq!(result["isError"], false);

    let text = result["content"][0]["text"].as_str().expect("Text missing");
    assert!(text.contains("Winning Hooks for \"morally gray author secret\""));
    assert!(text.contains("The dark secret about morally gray characters"));
    assert!(text.contains("Fill-in-the-Blank Madlib:"));
    assert!(text.contains("15.0x Reader Resonance"));
    assert!(text.contains("@creator_studio"));
    assert!(text.contains("Gemini text-embedding-004"));
}
