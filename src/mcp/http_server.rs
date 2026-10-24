use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use futures_util::stream;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::ingest::ChannelCrawler;
use crate::mcp::protocol::JsonRpcRequest;
use crate::mcp::server::McpServer;
use crate::pipeline::HookMachineEngine;
use crate::storage::SandsifterDb;

/// Shared application state across HTTP/SSE daemon connections.
#[derive(Clone)]
pub struct AppState {
    pub server: Arc<Mutex<McpServer>>,
    pub db: Option<SandsifterDb>,
}

/// Request payload for triggering channel crawls via HTTP API.
#[derive(Debug, Deserialize)]
pub struct CrawlRequest {
    pub channel: String,
    #[serde(default = "default_crawl_limit")]
    pub limit: usize,
}

fn default_crawl_limit() -> usize {
    30
}

/// Request payload for generating hook scripts via HTTP API.
#[derive(Debug, Deserialize)]
pub struct HookMachineRequest {
    pub channel: String,
    pub topic: String,
}

/// Query parameters for searching indexed hooks via HTTP API.
#[derive(Debug, Deserialize)]
pub struct HooksQuery {
    pub query: Option<String>,
    pub limit: Option<usize>,
}

/// Request payload for evaluating a video via Marginal Gains API.
#[derive(Debug, Deserialize)]
pub struct MarginalGainsRequest {
    pub video_url: String,
}

/// Embedded static assets from the production frontend build output (PushFrame Pattern).
#[derive(RustEmbed)]
#[folder = "web/dist/"]
pub struct EmbeddedFrontend;

/// Fallback handler serving embedded SPA assets and routing.
/// In local development, dynamically serves directly from `web/dist/` on disk so that
/// frontend changes take effect instantly on browser refresh without rebuilding or restarting the server.
pub async fn handle_embedded_asset(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() || path == "index.html" {
        path = "index.html".to_string();
    }

    // 1. Live disk reload: If web/dist/<path> exists on disk, serve directly from disk with no-cache
    let disk_path = std::path::Path::new("web/dist").join(&path);
    if disk_path.exists() && disk_path.is_file() {
        if let Ok(data) = std::fs::read(&disk_path) {
            let mime = mime_guess::from_path(&disk_path).first_or_octet_stream();
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                ],
                data,
            )
                .into_response();
        }
    }

    // 2. Embedded asset fallback (PushFrame pattern when binary is deployed standalone)
    match EmbeddedFrontend::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, "public, max-age=3600"),
                ],
                content.data,
            )
                .into_response()
        }
        None => {
            if path == "favicon.ico" {
                let disk_icon = std::path::Path::new("web/dist/icon.svg");
                if disk_icon.exists() {
                    if let Ok(data) = std::fs::read(disk_icon) {
                        return (
                            StatusCode::OK,
                            [(header::CONTENT_TYPE, "image/svg+xml")],
                            data,
                        )
                            .into_response();
                    }
                }
                if let Some(icon) = EmbeddedFrontend::get("icon.svg") {
                    return (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "image/svg+xml")],
                        icon.data,
                    )
                        .into_response();
                }
            }

            // SPA Fallback: for any unrecognized route, serve index.html (disk first, then embedded)
            let disk_index = std::path::Path::new("web/dist/index.html");
            if disk_index.exists() && disk_index.is_file() {
                if let Ok(data) = std::fs::read(disk_index) {
                    return (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
                        ],
                        data,
                    )
                        .into_response();
                }
            }

            if let Some(index) = EmbeddedFrontend::get("index.html") {
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    index.data,
                )
                    .into_response()
            } else {
                (StatusCode::NOT_FOUND, "404 Not Found").into_response()
            }
        }
    }
}

/// Creates the complete Axum router supporting MCP SSE, JSON-RPC, and REST API for web frontends.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health and MCP handshake routes
        .route("/", get(handle_root))
        .route("/health", get(handle_health))
        .route("/sse", get(handle_sse))
        .route("/message", post(handle_message))
        .route("/rpc", post(handle_message))
        // REST API routes for Creator Studio UI / Cloud Run daemon
        .route("/api/status", get(handle_api_status))
        .route("/api/watchlist", get(handle_api_watchlist))
        .route("/api/hooks", get(handle_api_hooks))
        .route("/api/crawl", post(handle_api_crawl))
        .route("/api/hook-machine", post(handle_api_hook_machine))
        .route("/api/script", post(handle_api_script))
        .route("/api/marginal-gains", post(handle_api_marginal_gains))
        .route(
            "/api/browse-local-video",
            get(handle_api_browse_local_video).post(handle_api_browse_local_video),
        )
        .route("/api/media-stream", get(handle_api_media_stream))
        .layer(CorsLayer::permissive())
        .fallback(handle_embedded_asset)
        .with_state(state)
}

// ─────────────────────────────────────────────────────────────────────────────
// MCP SSE & JSON-RPC Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_root(headers: HeaderMap) -> impl IntoResponse {
    let accepts_html = headers
        .get(header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.contains("text/html") && !s.starts_with("*/*"))
        .unwrap_or(false);

    if accepts_html {
        if let Some(index) = EmbeddedFrontend::get("index.html") {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                index.data,
            )
                .into_response();
        }
    }

    Json(json!({
        "service": "sandsifter",
        "version": env!("CARGO_PKG_VERSION"),
        "mcp_version": "2024-11-05",
        "status": "ready",
        "description": "High-Performance Outlier Intelligence & MCP Engine for Short-Form Video"
    }))
    .into_response()
}

async fn handle_health() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "timestamp": "2026-10-09T12:00:00Z"
        })),
    )
}

/// Establishes the MCP 2024-11-05 Server-Sent Events stream.
async fn handle_sse(
    Query(params): Query<HashMap<String, String>>,
) -> Sse<impl futures_util::stream::Stream<Item = Result<Event, Infallible>>> {
    let session_id = params
        .get("sessionId")
        .cloned()
        .unwrap_or_else(|| format!("sess_{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));

    let endpoint_url = format!("/message?sessionId={}", session_id);
    let initial_event = Event::default().event("endpoint").data(endpoint_url);

    let stream = stream::iter(vec![Ok(initial_event)]);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// Processes incoming MCP JSON-RPC requests over HTTP POST.
async fn handle_message(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let mut server = state.server.lock().await;
    let maybe_response = server.handle_request(request);

    match maybe_response {
        Some(response) => (StatusCode::OK, Json(serde_json::to_value(response).unwrap_or(Value::Null))),
        None => (
            StatusCode::ACCEPTED,
            Json(json!({ "status": "acknowledged" })),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// REST API Handlers for Web Frontends / Cloud Run Daemons
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_api_status(State(state): State<AppState>) -> Json<Value> {
    let (channels, videos, snapshots, hooks) = match &state.db {
        Some(db) => (
            db.list_channels().map(|c| c.len()).unwrap_or(0),
            db.count_videos().unwrap_or(0),
            db.count_snapshots().unwrap_or(0),
            db.count_hooks().unwrap_or(0),
        ),
        None => (0, 0, 0, 0),
    };

    Json(json!({
        "status": "healthy",
        "channels_tracked": channels,
        "videos_tracked": videos,
        "snapshots_tracked": snapshots,
        "hooks_indexed": hooks,
        "embedding_model": "text-embedding-004",
        "llm_tier": "gemini-2.5-flash-lite"
    }))
}

async fn handle_api_watchlist(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(20);
    let min_multiplier = params
        .get("min_multiplier")
        .and_then(|m| m.parse::<f64>().ok())
        .unwrap_or(1.5);

    let outliers = match &state.db {
        Some(db) => db.list_top_outliers(min_multiplier, limit).unwrap_or_default(),
        None => Vec::new(),
    };

    Json(json!({
        "count": outliers.len(),
        "outliers": outliers
    }))
}

async fn handle_api_hooks(
    State(state): State<AppState>,
    Query(q): Query<HooksQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(10);
    let query_str = q.query.unwrap_or_default();

    let results = match &state.db {
        Some(db) => {
            let client = crate::storage::gemini_client::GeminiEmbeddingClient::default();
            let query_vec = if !query_str.trim().is_empty() {
                Some(client.embed_text_blocking(&query_str))
            } else {
                None
            };
            db.search_hooks_hybrid(&query_str, query_vec.as_deref(), limit).unwrap_or_default()
        }
        None => Vec::new(),
    };

    Json(json!({
        "query": query_str,
        "count": results.len(),
        "results": results
    }))
}

async fn handle_api_crawl(
    State(state): State<AppState>,
    Json(payload): Json<CrawlRequest>,
) -> (StatusCode, Json<Value>) {
    let db = match &state.db {
        Some(d) => d,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database not configured on server" })),
            );
        }
    };

    match ChannelCrawler::crawl_channel(db, &payload.channel, payload.limit) {
        Ok(summary) => (
            StatusCode::OK,
            Json(json!({
                "status": "success",
                "channel": summary.channel_handle,
                "videos_crawled": summary.total_shorts_scanned,
                "baseline_median_views": summary.author_baseline_views,
                "viral_outliers_count": summary.viral_outliers.len(),
                "accelerating_videos_count": summary.accelerating_videos.len(),
                "summary": summary
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err })),
        ),
    }
}

async fn handle_api_hook_machine(
    State(state): State<AppState>,
    Json(payload): Json<HookMachineRequest>,
) -> Json<Value> {
    let videos = match &state.db {
        Some(db) => db.get_videos_by_channel(&payload.channel).unwrap_or_default(),
        None => Vec::new(),
    };

    let report = HookMachineEngine::execute(&payload.channel, &payload.topic, &videos);

    Json(json!({
        "channel": payload.channel,
        "topic": payload.topic,
        "report": report,
        "markdown": report.to_markdown()
    }))
}

async fn handle_api_script(
    State(state): State<AppState>,
    Json(payload): Json<crate::pipeline::ScriptRequest>,
) -> (StatusCode, Json<Value>) {
    let videos = match &state.db {
        Some(db) => db.get_videos_by_channel(&payload.channel_handle).unwrap_or_default(),
        None => Vec::new(),
    };

    let script = crate::pipeline::ScriptGenerator::generate(&payload, &videos);

    (
        StatusCode::OK,
        Json(json!({
            "status": "success",
            "script": script,
            "markdown": script.markdown_teleprompter
        })),
    )
}

async fn handle_api_marginal_gains(
    Json(payload): Json<MarginalGainsRequest>,
) -> (StatusCode, Json<Value>) {
    match crate::math::marginal_gains::MarginalGainsEngine::evaluate_target(&payload.video_url) {
        Ok(scorecard) => {
            let markdown = crate::math::marginal_gains::MarginalGainsEngine::render_markdown_scorecard(&scorecard);
            let flight_sheet = crate::export::FlightSheet::from_scorecard(&scorecard);
            (
                StatusCode::OK,
                Json(json!({
                    "status": "success",
                    "scorecard": scorecard,
                    "flight_sheet": flight_sheet,
                    "markdown": markdown
                })),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "message": format!("Failed to evaluate marginal gains scorecard: {}", e)
            })),
        ),
    }
}

/// HTTP 206 Partial Content Media Streamer for local video drafts.
/// Allows HTML5 `<video>` elements to scrub and seek large video files with bounded RAM (<25 MB ceiling).
async fn handle_api_media_stream(
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let raw_path = match params.get("path") {
        Some(p) if !p.trim().is_empty() => p.trim(),
        _ => return (StatusCode::BAD_REQUEST, "Missing 'path' query parameter").into_response(),
    };

    let path = std::path::Path::new(raw_path);
    if !path.exists() || !path.is_file() {
        return (StatusCode::NOT_FOUND, "File not found or is not a regular file").into_response();
    }

    // Security: Validate video extension
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let is_video = matches!(ext.as_str(), "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v");
    if !is_video {
        return (
            StatusCode::BAD_REQUEST,
            "Unsupported media type: file must be a video (.mp4, .mov, .mkv, .webm, .avi)",
        )
            .into_response();
    }

    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to open file: {}", e),
            )
                .into_response()
        }
    };

    let metadata = match file.metadata().await {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read file metadata: {}", e),
            )
                .into_response()
        }
    };

    let total_len = metadata.len();
    if total_len == 0 {
        return (
            StatusCode::OK,
            [(header::CONTENT_LENGTH, "0")],
            axum::body::Body::empty(),
        )
            .into_response();
    }

    let content_type = match ext.as_str() {
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        _ => "video/mp4",
    };

    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    // Parse HTTP Range header (e.g., "bytes=0-1048575" or "bytes=100-")
    let range_header = headers.get(header::RANGE).and_then(|h| h.to_str().ok());

    let (start, end) = match range_header {
        Some(range_str) if range_str.starts_with("bytes=") => {
            let spec = &range_str["bytes=".len()..];
            let mut parts = spec.split('-');
            let start_opt = parts.next().and_then(|s| s.parse::<u64>().ok());
            let end_opt = parts.next().and_then(|s| s.parse::<u64>().ok());

            match (start_opt, end_opt) {
                (Some(s), Some(e)) => (s, e.min(total_len - 1)),
                (Some(s), None) => (s, total_len - 1),
                (None, Some(suffix_len)) => {
                    let s = total_len.saturating_sub(suffix_len);
                    (s, total_len - 1)
                }
                _ => (0, total_len - 1),
            }
        }
        _ => (0, total_len - 1),
    };

    if start > end || start >= total_len {
        return (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [
                (header::CONTENT_RANGE, format!("bytes */{}", total_len)),
                (header::ACCEPT_RANGES, "bytes".to_string()),
            ],
            axum::body::Body::empty(),
        )
            .into_response();
    }

    if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to seek file: {}", e),
        )
            .into_response();
    }

    let chunk_len = end - start + 1;

    // Stream requested byte range in bounded 64 KB chunks (<25 MB RAM ceiling)
    let stream = futures_util::stream::unfold(
        (file, chunk_len),
        |(mut file, remaining)| async move {
            if remaining == 0 {
                return None;
            }
            let to_read = std::cmp::min(remaining, 64 * 1024) as usize;
            let mut buf = vec![0u8; to_read];
            match file.read(&mut buf[..to_read]).await {
                Ok(n) if n > 0 => {
                    buf.truncate(n);
                    let new_remaining = remaining.saturating_sub(n as u64);
                    Some((Ok::<_, std::io::Error>(bytes::Bytes::from(buf)), (file, new_remaining)))
                }
                _ => None,
            }
        },
    );

    let body = axum::body::Body::from_stream(stream);

    (
        StatusCode::PARTIAL_CONTENT,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
            (header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, total_len)),
            (header::CONTENT_LENGTH, chunk_len.to_string()),
        ],
        body,
    )
        .into_response()
}

/// Invokes the native OS file picker to let the user select a local video draft from the hard drive.
pub fn pick_local_video_file() -> Result<Option<String>, String> {
    if let Ok(mock) = std::env::var("SANDSIFTER_MOCK_PICKED_FILE") {
        if mock.trim().is_empty() {
            return Ok(None);
        } else {
            return Ok(Some(mock.trim().to_string()));
        }
    }

    #[cfg(target_os = "windows")]
    {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;

        let ps_script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Filter = "Video Files (*.mp4;*.mov;*.mkv;*.webm;*.avi)|*.mp4;*.mov;*.mkv;*.webm;*.avi|All Files (*.*)|*.*"
$dialog.Title = "Select Video for Sandsifter Telemetry Audit"
$dialog.Multiselect = $false
$dialog.RestoreDirectory = $true
$form = New-Object System.Windows.Forms.Form
$form.TopMost = $true
if ($dialog.ShowDialog($form) -eq [System.Windows.Forms.DialogResult]::OK) {
    [Console]::Out.Write($dialog.FileName)
}
$form.Dispose()
"#;

        let utf16: Vec<u8> = ps_script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let encoded = BASE64.encode(&utf16);

        let output = std::process::Command::new("powershell")
            .args(["-STA", "-NoProfile", "-EncodedCommand", &encoded])
            .output()
            .map_err(|e| format!("Failed to spawn native file picker dialog: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("File dialog error: {}", err));
        }

        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selected))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(None)
    }
}

async fn handle_api_browse_local_video() -> (StatusCode, Json<Value>) {
    match tokio::task::spawn_blocking(pick_local_video_file).await {
        Ok(Ok(Some(path))) => (
            StatusCode::OK,
            Json(json!({
                "status": "success",
                "file_path": path
            })),
        ),
        Ok(Ok(None)) => (
            StatusCode::OK,
            Json(json!({
                "status": "cancelled",
                "message": "File selection was cancelled"
            })),
        ),
        Ok(Err(err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "error",
                "message": err
            })),
        ),
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "status": "error",
                "message": format!("Task join error: {}", join_err)
            })),
        ),
    }
}

/// Runs the HTTP daemon using an already-bound TcpListener.
pub async fn start_http_server_with_listener(
    server: McpServer,
    db: Option<SandsifterDb>,
    listener: tokio::net::TcpListener,
) -> Result<(), Box<dyn std::error::Error>> {
    let local_addr = listener.local_addr()?;
    let state = AppState {
        server: Arc::new(Mutex::new(server)),
        db,
    };

    let app = create_router(state);

    info!("🌐 Sandsifter HTTP & SSE MCP Daemon listening on http://{}", local_addr);
    eprintln!("[sandsifter-daemon] HTTP & SSE MCP Daemon listening on http://{}", local_addr);
    eprintln!("[sandsifter-daemon] Endpoints available:");
    eprintln!("  - Health check:     http://{}/health", local_addr);
    eprintln!("  - MCP SSE Stream:   http://{}/sse", local_addr);
    eprintln!("  - MCP JSON-RPC:     http://{}/message (POST) or /rpc", local_addr);
    eprintln!("  - REST Watchlist:   http://{}/api/watchlist", local_addr);
    eprintln!("  - REST Hooks:       http://{}/api/hooks?query=...", local_addr);
    eprintln!("  - REST Script:      http://{}/api/script (POST)", local_addr);


    axum::serve(listener, app).await?;
    Ok(())
}

/// Runs the HTTP daemon listening on host:port for remote agents, web UIs, and Cloud Run.
pub async fn start_http_server(
    server: McpServer,
    db: Option<SandsifterDb>,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    start_http_server_with_listener(server, db, listener).await
}
