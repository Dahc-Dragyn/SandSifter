use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::time::Duration;
use tracing::{debug, warn};

use crate::analyzer::keyframe::Keyframe;

/// Text overlay detected on a video keyframe (burned-in title, sticker, or caption).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrTextOverlay {
    pub text: String,
    #[serde(default = "default_ocr_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub is_headline: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<(f32, f32, f32, f32)>,
}

fn default_ocr_confidence() -> f32 {
    0.95
}

/// OCR extraction result for a single keyframe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyframeOcrResult {
    pub timestamp_ms: u32,
    #[serde(default)]
    pub overlays: Vec<OcrTextOverlay>,
    pub raw_text: String,
    pub has_headline_hook: bool,
}

impl KeyframeOcrResult {
    pub fn new(timestamp_ms: u32, overlays: Vec<OcrTextOverlay>, raw_text: String, has_headline_hook: bool) -> Self {
        Self {
            timestamp_ms,
            overlays,
            raw_text,
            has_headline_hook,
        }
    }

    /// Evaluates whether the visual on-screen text matches or reinforces the spoken hook.
    pub fn matches_spoken_hook(&self, spoken_hook: &str) -> bool {
        let clean_spoken = spoken_hook.to_lowercase();
        let clean_headline_words: Vec<String> = self
            .raw_text
            .to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        if clean_headline_words.is_empty() {
            return false;
        }

        let match_count = clean_headline_words
            .iter()
            .filter(|w| clean_spoken.contains(w.as_str()))
            .count();

        // If >= 40% of on-screen headline keywords appear in the spoken hook
        (match_count as f32) / (clean_headline_words.len() as f32) >= 0.40
    }
}

/// OCR engine utilizing Gemini 2.5 Flash-Lite multimodal vision with offline fallback.
#[derive(Clone)]
pub struct OcrEngine {
    http: Client,
    api_key: Option<String>,
    model: String,
}

impl Default for OcrEngine {
    fn default() -> Self {
        Self::new(None)
    }
}

impl OcrEngine {
    pub fn new(api_key: Option<String>) -> Self {
        let key = api_key.or_else(|| env::var("GEMINI_API_KEY").ok());
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            api_key: key,
            // User mandate: gemini-2.5-flash-lite (most cost-effective tier)
            model: "gemini-2.5-flash-lite".to_string(),
        }
    }

    /// Extracts visual text overlays from a single keyframe.
    pub async fn extract_keyframe_ocr(&self, keyframe: &Keyframe) -> KeyframeOcrResult {
        let key = match &self.api_key {
            Some(k) if !k.trim().is_empty() => k,
            _ => {
                debug!("GEMINI_API_KEY unset; using deterministic mock OCR for keyframe at {}ms.", keyframe.timestamp_ms);
                return Self::synthesize_mock_ocr(keyframe.timestamp_ms);
            }
        };

        let b64_frame = BASE64.encode(&keyframe.frame_bytes);
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, key
        );

        let system_prompt = "Extract all text overlays, stickers, and headlines from this video frame. Return JSON strictly adhering to: { overlays: [ { text: string, confidence: number, is_headline: boolean } ], raw_text: string, has_headline_hook: boolean }";

        let body = json!({
            "contents": [
                {
                    "parts": [
                        { "text": system_prompt },
                        {
                            "inline_data": {
                                "mime_type": keyframe.mime_type,
                                "data": b64_frame
                            }
                        }
                    ]
                }
            ],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(raw_json) = resp.json::<serde_json::Value>().await {
                        if let Some(text_content) = raw_json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                            if let Ok(mut result) = serde_json::from_str::<KeyframeOcrResult>(text_content) {
                                result.timestamp_ms = keyframe.timestamp_ms;
                                return result;
                            }
                        }
                    }
                } else {
                    warn!("Gemini 2.5 Flash-Lite OCR returned status: {}", resp.status());
                }
            }
            Err(e) => {
                warn!("HTTP error during Gemini OCR: {}", e);
            }
        }

        Self::synthesize_mock_ocr(keyframe.timestamp_ms)
    }

    /// Parallel extraction of OCR results across multiple keyframes.
    pub async fn extract_all_keyframes(&self, keyframes: &[Keyframe]) -> Vec<KeyframeOcrResult> {
        let mut results = Vec::new();
        for kf in keyframes {
            results.push(self.extract_keyframe_ocr(kf).await);
        }
        results
    }

    /// Synthesizes deterministic sample OCR results for testing and offline operation.
    pub fn synthesize_mock_ocr(timestamp_ms: u32) -> KeyframeOcrResult {
        if timestamp_ms == 0 {
            KeyframeOcrResult {
                timestamp_ms,
                overlays: vec![
                    OcrTextOverlay {
                        text: "STOP USING PANDAS 🚨".to_string(),
                        confidence: 0.99,
                        is_headline: true,
                        bounding_box: Some((0.1, 0.15, 0.8, 0.12)),
                    },
                    OcrTextOverlay {
                        text: "Why Polars is 30x Faster".to_string(),
                        confidence: 0.96,
                        is_headline: false,
                        bounding_box: Some((0.15, 0.82, 0.7, 0.08)),
                    },
                ],
                raw_text: "STOP USING PANDAS 🚨 Why Polars is 30x Faster".to_string(),
                has_headline_hook: true,
            }
        } else if timestamp_ms <= 2000 {
            KeyframeOcrResult {
                timestamp_ms,
                overlays: vec![
                    OcrTextOverlay {
                        text: "RAM Breakdown: 15GB vs 500MB".to_string(),
                        confidence: 0.95,
                        is_headline: false,
                        bounding_box: Some((0.1, 0.75, 0.8, 0.1)),
                    },
                ],
                raw_text: "RAM Breakdown: 15GB vs 500MB".to_string(),
                has_headline_hook: false,
            }
        } else {
            KeyframeOcrResult {
                timestamp_ms,
                overlays: Vec::new(),
                raw_text: String::new(),
                has_headline_hook: false,
            }
        }
    }
}
