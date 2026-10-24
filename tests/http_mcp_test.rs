use sandsifter::mcp::*;
use sandsifter::storage::SandsifterDb;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpListener;

async fn spawn_test_server() -> (String, SandsifterDb) {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");

    // Seed mock creator
    let channel = sandsifter::models::WatchlistChannel {
        uuid: "chan-test-1".to_string(),
        platform: "tiktok".to_string(),
        handle: "@creator_studio".to_string(),
        display_name: Some("Creator Studio".to_string()),
        avatar_url: None,
        follower_count: Some(25_000),
        baseline_median_views: 4_000,
        video_count: Some(12),
        last_crawled_at: None,
        added_at: "2026-10-01T00:00:00Z".to_string(),
    };
    db.upsert_channel(&channel).unwrap();

    let video = sandsifter::models::VideoDetail {
        uuid: "vid-test-1".to_string(),
        caption: Some("The dark fantasy trope you secretly love".to_string()),
        url: "https://tiktok.com/@creator_studio/video/999".to_string(),
        thumbnail: None,
        channel: sandsifter::models::VideoChannelDetails {
            uuid: "chan-test-1".to_string(),
            platform: "tiktok".to_string(),
            handle: "@creator_studio".to_string(),
            display_name: Some("Creator Studio".to_string()),
            avatar_url: None,
            follower_count: Some(25_000),
            baseline_median_views: Some(4_000),
        },
        view_count: 80_000,
        like_count: Some(8_000),
        comment_count: Some(500),
        share_count: Some(1_200),
        outlier_score: 20.0,
        engagement_rate: 12.1,
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
        "vid-test-1",
        "The dark fantasy trope you secretly love.",
        Some("The [Subgenre] trope you secretly love."),
        3.5,
        &client,
    )
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral test port");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let server = McpServer::with_db(db.clone());
    let db_clone = db.clone();

    tokio::spawn(async move {
        let _ = start_http_server_with_listener(server, Some(db_clone), listener).await;
    });

    // Short yield to let server start accepting
    tokio::time::sleep(Duration::from_millis(50)).await;

    (base_url, db)
}

#[tokio::test]
async fn test_http_root_metadata() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/", base_url))
        .send()
        .await
        .expect("Failed to GET /");

    assert_eq!(res.status(), 200);
    // Check CORS
    assert_eq!(
        res.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["service"], "sandsifter");
    assert_eq!(body["mcp_version"], "2024-11-05");
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn test_http_health_check() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Failed to GET /health");

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_http_sse_handshake() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/sse?sessionId=test-session-123", base_url))
        .send()
        .await
        .expect("Failed to GET /sse");

    assert_eq!(res.status(), 200);
    let content_type = res
        .headers()
        .get("content-type")
        .expect("Missing content-type")
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"));

    let body_chunk = res.text().await.unwrap();
    assert!(body_chunk.contains("event: endpoint"));
    assert!(body_chunk.contains("/message?sessionId=test-session-123"));
}

#[tokio::test]
async fn test_http_mcp_json_rpc_tools_list() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. Initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let res = client
        .post(format!("{}/message", base_url))
        .json(&init_req)
        .send()
        .await
        .expect("Failed to POST /message");

    assert_eq!(res.status(), 200);
    let init_resp: serde_json::Value = res.json().await.unwrap();
    assert_eq!(init_resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "sandsifter");

    // 2. Tools List
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let res = client
        .post(format!("{}/rpc", base_url))
        .json(&list_req)
        .send()
        .await
        .expect("Failed to POST /rpc");

    assert_eq!(res.status(), 200);
    let list_resp: serde_json::Value = res.json().await.unwrap();
    let tools = list_resp["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 10);
}

#[tokio::test]
async fn test_http_mcp_tool_call_watchlist() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "/videos-watchlist",
            "arguments": {
                "limit": 5
            }
        }
    });

    let res = client
        .post(format!("{}/message", base_url))
        .json(&call_req)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let resp: serde_json::Value = res.json().await.unwrap();
    assert_eq!(resp["result"]["isError"], false);
    let content = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(content.contains("The dark fantasy trope you secretly love"));
}

#[tokio::test]
async fn test_rest_api_status_and_watchlist() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. /api/status
    let status_res = client
        .get(format!("{}/api/status", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(status_res.status(), 200);
    let status_json: serde_json::Value = status_res.json().await.unwrap();
    assert_eq!(status_json["status"], "healthy");
    assert_eq!(status_json["channels_tracked"], 1);
    assert_eq!(status_json["videos_tracked"], 1);
    assert_eq!(status_json["hooks_indexed"], 1);

    // 2. /api/watchlist
    let wl_res = client
        .get(format!("{}/api/watchlist?limit=10&min_multiplier=1.5", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(wl_res.status(), 200);
    let wl_json: serde_json::Value = wl_res.json().await.unwrap();
    assert_eq!(wl_json["count"], 1);
    assert_eq!(
        wl_json["outliers"][0]["caption"],
        "The dark fantasy trope you secretly love"
    );

    // 3. /api/hooks search
    let hooks_res = client
        .get(format!("{}/api/hooks?query=trope", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(hooks_res.status(), 200);
    let hooks_json: serde_json::Value = hooks_res.json().await.unwrap();
    assert_eq!(hooks_json["count"], 1);
    assert!(hooks_json["results"][0]["hook_text"]
        .as_str()
        .unwrap()
        .contains("dark fantasy trope"));
}

#[tokio::test]
async fn test_rest_api_hook_machine() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = json!({
        "channel": "@creator_studio",
        "topic": "Enemies to Lovers"
    });

    let res = client
        .post(format!("{}/api/hook-machine", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["channel"], "@creator_studio");
    assert_eq!(body["topic"], "Enemies to Lovers");
    assert!(body["markdown"].as_str().unwrap().contains("The Hook Machine"));
}

#[tokio::test]
async fn test_rest_api_marginal_gains() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Testing API error response on invalid URL
    let payload = json!({
        "video_url": "https://invalid-test-domain-404.com/watch?v=123"
    });

    let res = client
        .post(format!("{}/api/marginal-gains", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // Invalid video URL should return 400 Bad Request
    assert_eq!(res.status(), 400);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "error");
}

#[tokio::test]
async fn test_http_embedded_theme_css() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/theme.css", base_url))
        .send()
        .await
        .expect("Failed to GET /theme.css");

    assert_eq!(res.status(), 200);
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/css"));

    let body = res.text().await.unwrap();
    assert!(body.contains("--yt-brand-red: #ff0000"));
    assert!(body.contains("--yt-spec-base-background"));
}

#[tokio::test]
async fn test_http_embedded_app_js() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/app.js", base_url))
        .send()
        .await
        .expect("Failed to GET /app.js");

    assert_eq!(res.status(), 200);
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("javascript"));

    let body = res.text().await.unwrap();
    assert!(body.contains("Sandsifter Studio"));
    assert!(body.contains("switchTab"));
}

#[tokio::test]
async fn test_http_embedded_app_css() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/app.css", base_url))
        .send()
        .await
        .expect("Failed to GET /app.css");

    assert_eq!(res.status(), 200);
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/css"));

    let body = res.text().await.unwrap();
    assert!(body.contains("app-header"));
    assert!(body.contains(".nav-chip"));
}

#[tokio::test]
async fn test_http_browser_html_root_negotiation() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{}/", base_url))
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9")
        .send()
        .await
        .expect("Failed to GET / with browser Accept header");

    assert_eq!(res.status(), 200);
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));

    let body = res.text().await.unwrap();
    assert!(body.contains("Sandsifter Studio"));
    assert!(body.contains("theme.css"));
    assert!(body.contains("app.js"));
}

#[tokio::test]
async fn test_http_spa_fallback_routing() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Client-side routes like /audit or /watchlist should fall back to index.html
    for route in &["/audit", "/watchlist", "/hooks", "/unknown-route"] {
        let res = client
            .get(format!("{}{}", base_url, route))
            .send()
            .await
            .expect("Failed to test SPA fallback route");

        assert_eq!(res.status(), 200);
        let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("text/html"));

        let body = res.text().await.unwrap();
        assert!(body.contains("Sandsifter Studio"));
    }
}

#[tokio::test]
async fn test_http_browse_local_video_endpoint() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // Set mock picked file in environment
    std::env::set_var("SANDSIFTER_MOCK_PICKED_FILE", "C:\\Videos\\test_render.mp4");

    let res = client
        .post(format!("{}/api/browse-local-video", base_url))
        .send()
        .await
        .expect("Failed to call /api/browse-local-video");

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["file_path"], "C:\\Videos\\test_render.mp4");

    std::env::remove_var("SANDSIFTER_MOCK_PICKED_FILE");
}

#[tokio::test]
async fn test_http_media_stream_partial_content_range() {
    let (base_url, _) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. Create a dummy test video file with 4096 bytes
    let mut temp = tempfile::Builder::new()
        .suffix(".mp4")
        .tempfile()
        .expect("Failed to create temp video file");
    let test_bytes = vec![0xABu8; 4096];
    std::io::Write::write_all(&mut temp, &test_bytes).expect("Failed to write test bytes");
    let temp_path = temp.path().to_str().unwrap().to_string();

    // 2. Test Range request: bytes=0-1023 (first 1024 bytes)
    let res = client
        .get(format!("{}/api/media-stream", base_url))
        .query(&[("path", &temp_path)])
        .header("Range", "bytes=0-1023")
        .send()
        .await
        .expect("Failed to call /api/media-stream");

    assert_eq!(res.status(), 206);
    assert_eq!(res.headers().get("content-range").unwrap().to_str().unwrap(), "bytes 0-1023/4096");
    assert_eq!(res.headers().get("content-length").unwrap().to_str().unwrap(), "1024");
    assert_eq!(res.headers().get("accept-ranges").unwrap().to_str().unwrap(), "bytes");
    let content_type = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("video/mp4"));

    let body = res.bytes().await.unwrap();
    assert_eq!(body.len(), 1024);
    assert!(body.iter().all(|&b| b == 0xAB));

    // 3. Test Range request: bytes=1024-2047 (second 1024 bytes)
    let res2 = client
        .get(format!("{}/api/media-stream", base_url))
        .query(&[("path", &temp_path)])
        .header("Range", "bytes=1024-2047")
        .send()
        .await
        .expect("Failed to call /api/media-stream range 2");

    assert_eq!(res2.status(), 206);
    assert_eq!(res2.headers().get("content-range").unwrap().to_str().unwrap(), "bytes 1024-2047/4096");
    assert_eq!(res2.headers().get("content-length").unwrap().to_str().unwrap(), "1024");
    let body2 = res2.bytes().await.unwrap();
    assert_eq!(body2.len(), 1024);
    assert!(body2.iter().all(|&b| b == 0xAB));

    // 4. Test unsupported extension rejection (.txt)
    let mut bad_temp = tempfile::Builder::new()
        .suffix(".txt")
        .tempfile()
        .expect("Failed to create temp txt file");
    std::io::Write::write_all(&mut bad_temp, b"not a video").unwrap();
    let bad_path = bad_temp.path().to_str().unwrap().to_string();

    let res_bad = client
        .get(format!("{}/api/media-stream", base_url))
        .query(&[("path", &bad_path)])
        .send()
        .await
        .unwrap();
    assert_eq!(res_bad.status(), 400);

    // 5. Test out of range request (bytes=5000-6000)
    let res_oob = client
        .get(format!("{}/api/media-stream", base_url))
        .query(&[("path", &temp_path)])
        .header("Range", "bytes=5000-6000")
        .send()
        .await
        .unwrap();
    assert_eq!(res_oob.status(), 416);
}



