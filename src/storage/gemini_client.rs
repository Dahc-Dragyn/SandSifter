use reqwest::Client;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::storage::embeddings::{
    generate_mock_embedding, GeminiContent, GeminiEmbeddingRequest, GeminiEmbeddingResponse,
    GeminiPart,
};

/// Client for generating semantic embeddings via Google Gemini text-embedding models.
/// Includes character tracking and cost estimation for economical batch processing.
#[derive(Clone)]
pub struct GeminiEmbeddingClient {
    http: Client,
    api_key: Option<String>,
    model: String,
    characters_processed: Arc<AtomicUsize>,
    api_calls: Arc<AtomicUsize>,
}

impl Default for GeminiEmbeddingClient {
    fn default() -> Self {
        Self::new(None)
    }
}

impl GeminiEmbeddingClient {
    /// Creates a new Gemini embedding client. If `api_key` is None, loads `GEMINI_API_KEY` from env.
    pub fn new(api_key: Option<String>) -> Self {
        let key = api_key.or_else(|| env::var("GEMINI_API_KEY").ok());
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            api_key: key,
            model: "text-embedding-004".to_string(),
            characters_processed: Arc::new(AtomicUsize::new(0)),
            api_calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Number of cumulative characters embedded by this client instance.
    pub fn character_count(&self) -> usize {
        self.characters_processed.load(Ordering::Relaxed)
    }

    /// Number of cumulative embedding calls made by this client instance.
    pub fn call_count(&self) -> usize {
        self.api_calls.load(Ordering::Relaxed)
    }

    /// Estimated cost in USD based on Gemini's $0.00002 per 1,000 characters pricing tier.
    /// Note: 1,500 requests per day are completely free under Google AI Studio rate limits.
    pub fn estimated_cost_usd(&self) -> f64 {
        (self.character_count() as f64 / 1_000.0) * 0.00002
    }

    /// Human-readable cost summary for creators and system reports.
    pub fn cost_summary(&self) -> String {
        format!(
            "Gemini text-embedding-004: {} call(s), {} characters (~${:.6} USD | 1,500 daily requests are 100% free)",
            self.call_count(),
            self.character_count(),
            self.estimated_cost_usd()
        )
    }

    /// Generates a 768-dimensional embedding vector for the provided text.
    /// Falls back gracefully to deterministic offline embeddings if API key is unset or offline.
    pub async fn embed_text(&self, text: &str) -> Vec<f32> {
        let char_count = text.chars().count();
        self.characters_processed.fetch_add(char_count, Ordering::Relaxed);
        self.api_calls.fetch_add(1, Ordering::Relaxed);

        let key = match &self.api_key {
            Some(k) if !k.trim().is_empty() => k,
            _ => {
                debug!("GEMINI_API_KEY not set; using deterministic offline mock embedding.");
                return generate_mock_embedding(text, 768);
            }
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            self.model, key
        );

        let body = GeminiEmbeddingRequest {
            model: format!("models/{}", self.model),
            content: GeminiContent {
                parts: vec![GeminiPart {
                    text: text.to_string(),
                }],
            },
        };

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<GeminiEmbeddingResponse>().await {
                        Ok(data) => return data.embedding.values,
                        Err(e) => {
                            warn!("Failed to deserialize Gemini embedding response: {}", e);
                        }
                    }
                } else {
                    warn!("Gemini embedding API returned status: {}", resp.status());
                }
            }
            Err(e) => {
                warn!("HTTP error connecting to Gemini embedding API: {}", e);
            }
        }

        // Offline fallback guarantee
        generate_mock_embedding(text, 768)
    }

    /// Synchronous / blocking wrapper for embedding text safely across any thread or runtime context.
    pub fn embed_text_blocking(&self, text: &str) -> Vec<f32> {
        let client = self.clone();
        let text_owned = text.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| tokio::runtime::Runtime::new().unwrap());
            rt.block_on(client.embed_text(&text_owned))
        })
        .join()
        .unwrap_or_else(|_| {
            self.characters_processed.fetch_add(text.chars().count(), Ordering::Relaxed);
            self.api_calls.fetch_add(1, Ordering::Relaxed);
            generate_mock_embedding(text, 768)
        })
    }
}
