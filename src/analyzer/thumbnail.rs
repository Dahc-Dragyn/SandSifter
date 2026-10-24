use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::time::Duration;
use tracing::{debug, warn};

/// The 7 distinct professional visual clickbait archetypes + CleanAuthentic baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VisualClickbaitArchetype {
    /// Packaging is genuine, unexaggerated, and mechanically aligned with video kinematics.
    CleanAuthentic,
    /// High skin exposure, provocative/erotic framing, swimwear/bikini focus (borderline racy).
    SexuallySuggestiveMetadata,
    /// Exaggerated wide-eyed / gaping-mouth reactions (e.g. MrBeast face distortion).
    GazeReactionDistortion,
    /// Visual redirection markers such as high-contrast red arrows, circles, and pointer glyphs.
    DirectingDevices,
    /// Artificially synthesized disaster, composite fire, smoke, sinking, or life-or-death peril.
    ManufacturedPeril,
    /// High-value cars, stacks of cash, or luxury/dramatic assets absent from the actual footage.
    PhantomAsset,
    /// High-contrast all-caps tabloid text banners, screaming warning labels, and stickers.
    TabloidStickers,
    /// Unnatural neon color boosting, extreme chromatic vibrancy, and glowing subject halos.
    SensoryHyperSaturation,
}

impl VisualClickbaitArchetype {
    /// Human-facing terminal badge.
    pub fn badge(&self) -> &'static str {
        match self {
            Self::CleanAuthentic => "[AUTHENTIC]",
            Self::SexuallySuggestiveMetadata => "[SUGGESTIVE METADATA]",
            Self::GazeReactionDistortion => "[GAZE DISTORTION]",
            Self::DirectingDevices => "[DIRECTING ARROWS]",
            Self::ManufacturedPeril => "[MANUFACTURED PERIL]",
            Self::PhantomAsset => "[PHANTOM ASSET]",
            Self::TabloidStickers => "[TABLOID STICKERS]",
            Self::SensoryHyperSaturation => "[HYPER SATURATION]",
        }
    }

    /// Categorizes whether the archetype is an overt bait-and-switch deception.
    pub fn is_deceptive(&self) -> bool {
        matches!(
            self,
            Self::PhantomAsset | Self::ManufacturedPeril | Self::DirectingDevices
        )
    }

    /// Checks if this archetype represents sexually suggestive metadata.
    pub fn is_suggestive(&self) -> bool {
        matches!(self, Self::SexuallySuggestiveMetadata)
    }
}

/// Comprehensive forensic analysis of thumbnail visual packaging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThumbnailForensicReport {
    /// Suggestive metadata score (0.0 to 1.0, where >= 0.70 is heavily suggestive/racy)
    pub suggestive_metadata_score: f64,
    /// Primary dominant packaging archetype detected
    pub primary_archetype: VisualClickbaitArchetype,
    /// Any secondary or supplementary visual clickbait archetypes observed
    pub secondary_archetypes: Vec<VisualClickbaitArchetype>,
    /// Visual focal elements identified by vision model (e.g. "swimwear", "red arrow")
    pub detected_focal_elements: Vec<String>,
    /// Packaging integrity index (0.0 to 1.0, where 1.0 = aligned, <0.4 = bait-and-switch)
    pub packaging_integrity_index: f64,
    /// Estimated CTR multiplier driven solely by packaging optics (1.0 = baseline)
    pub exogenous_ctr_modifier: f64,
    /// Professional diagnostic summary of thumbnail packaging strategy
    pub diagnostic_note: String,
    /// Whether burned-in text, presenter facial features, or hero elements collide with the bottom-right timestamp overlay (x >= 0.75, y >= 0.80)
    #[serde(default)]
    pub timestamp_collision_detected: bool,
    /// Specific detail or bounding zone of timestamp collision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision_detail: Option<String>,
    /// Percentage of burned-in thumbnail text that duplicates the video title (0.0 to 100.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_text_redundancy_pct: Option<f64>,
    /// Raw text detected inside the thumbnail (OCR)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_ocr_text: Option<String>,
}

impl Default for ThumbnailForensicReport {
    fn default() -> Self {
        Self::clean_authentic()
    }
}

impl ThumbnailForensicReport {
    /// Creates a deterministic clean authentic report (zero penalties, 1.0 integrity).
    pub fn clean_authentic() -> Self {
        Self {
            suggestive_metadata_score: 0.0,
            primary_archetype: VisualClickbaitArchetype::CleanAuthentic,
            secondary_archetypes: Vec::new(),
            detected_focal_elements: Vec::new(),
            packaging_integrity_index: 1.0,
            exogenous_ctr_modifier: 1.0,
            diagnostic_note: "Clean authentic packaging aligned with content kinematics.".to_string(),
            timestamp_collision_detected: false,
            collision_detail: None,
            title_text_redundancy_pct: None,
            thumbnail_ocr_text: None,
        }
    }

    /// Attaches timestamp collision details.
    pub fn with_timestamp_collision(mut self, detected: bool, detail: Option<String>) -> Self {
        self.timestamp_collision_detected = detected;
        self.collision_detail = detail;
        self
    }

    /// Evaluates element coordinates and attaches timestamp collision if in the bottom-right quadrant (x >= 0.75, y >= 0.80).
    pub fn with_element_coordinates(self, x: f64, y: f64, label: &str) -> Self {
        let (detected, detail) = ThumbnailVisionEngine::evaluate_timestamp_dead_zone(x, y, label);
        self.with_timestamp_collision(detected, detail)
    }

    /// Attaches verbal redundancy audit between thumbnail text and video title.
    pub fn with_title_verbal_audit(mut self, title: &str, ocr_text: &str) -> Self {
        let redundancy = ThumbnailVisionEngine::calculate_title_text_redundancy(ocr_text, title);
        self.thumbnail_ocr_text = Some(ocr_text.to_string());
        if redundancy >= 50.0 {
            self.title_text_redundancy_pct = Some(redundancy);
        }
        self
    }

    /// Emits the recommendation to shift to Complementarity if verbal redundancy >= 50%.
    pub fn verbal_redundancy_recommendation(&self) -> Option<String> {
        if let Some(redundancy) = self.title_text_redundancy_pct {
            if redundancy >= 50.0 {
                return Some(format!(
                    "⚠️ Verbal Redundancy: Thumbnail text duplicates title by {:.0}%; wastes persuasive canvas space. Shift from Reinforcement to Complementarity.",
                    redundancy
                ));
            }
        }
        None
    }

    /// Checks if packaging creates an exogenous CTR pull beyond intrinsic video content.
    pub fn is_exogenous_ctr_driver(&self) -> bool {
        self.suggestive_metadata_score >= 0.70
            || self.primary_archetype.is_deceptive()
            || self.primary_archetype.is_suggestive()
            || self.secondary_archetypes.iter().any(|a| a.is_deceptive() || a.is_suggestive())
    }

    /// Determines whether packaging exhibits high expectation mismatch (bait-and-switch).
    pub fn has_expectation_mismatch(&self, early_drop_rate: Option<f64>) -> bool {
        let severe_early_drop = early_drop_rate.map(|r| r.abs() >= 3.0).unwrap_or(false);
        (self.suggestive_metadata_score >= 0.70
            || self.primary_archetype.is_deceptive()
            || self.packaging_integrity_index < 0.50)
            && severe_early_drop
    }
}

/// Vision engine for thumbnail forensics using Gemini 2.5 Flash-Lite with offline fallback.
#[derive(Clone)]
pub struct ThumbnailVisionEngine {
    http: Client,
    api_key: Option<String>,
    model: String,
}

impl Default for ThumbnailVisionEngine {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ThumbnailVisionEngine {
    /// Creates a new ThumbnailVisionEngine instance. Reads `GEMINI_API_KEY` from env if not provided.
    pub fn new(api_key: Option<String>) -> Self {
        let key = api_key.or_else(|| env::var("GEMINI_API_KEY").ok());
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            api_key: key,
            // User mandate: gemini-2.5-flash-lite (most cost-effective multimodal vision tier)
            model: "gemini-2.5-flash-lite".to_string(),
        }
    }

    /// Evaluates whether normalized bounding coordinates (x, y) land within the bottom-right timestamp dead zone (x >= 0.75, y >= 0.80).
    pub fn evaluate_timestamp_dead_zone(x: f64, y: f64, label: &str) -> (bool, Option<String>) {
        if x >= 0.75 && y >= 0.80 {
            (
                true,
                Some(format!(
                    "Text/Focal element '{}' detected at (x={:.2}, y={:.2}) in bottom-right dead zone ($x \\ge 0.75, y \\ge 0.80$); physically occluded by YouTube timestamp overlay.",
                    label, x, y
                )),
            )
        } else {
            (false, None)
        }
    }

    /// Calculates verbal redundancy percentage between thumbnail OCR text and video title.
    /// Flags if thumbnail text duplicates keywords already present in the first 50 characters of the title.
    pub fn calculate_title_text_redundancy(thumbnail_text: &str, title: &str) -> f64 {
        let stopwords = [
            "the", "a", "an", "and", "or", "to", "of", "in", "for", "with", "on", "at", 
            "from", "by", "is", "it", "my", "our", "we", "i", "you", "this", "that", "be", "as"
        ];
        let normalize = |s: &str| -> Vec<String> {
            s.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty() && w.len() > 1 && !stopwords.contains(w))
                .map(|w| w.to_string())
                .collect()
        };

        let thumb_words = normalize(thumbnail_text);
        if thumb_words.is_empty() {
            return 0.0;
        }

        // Isolate words within the first 50 characters of the title
        let first_50_chars: String = title.chars().take(50).collect();
        let title_words = normalize(&first_50_chars);

        let mut matches = 0;
        for tw in &thumb_words {
            if title_words.contains(tw)
                || (tw == "ev" && (title_words.contains(&"electric".to_string()) || title_words.contains(&"vehicle".to_string())))
                || (tw == "ai" && (title_words.contains(&"artificial".to_string()) || title_words.contains(&"intelligence".to_string())))
            {
                matches += 1;
            }
        }

        let redundancy = (matches as f64 / thumb_words.len() as f64) * 100.0;
        redundancy.round()
    }

    /// Deterministic offline evaluation fallback for testing and offline execution.
    pub fn fallback_report(thumbnail_url: &str) -> ThumbnailForensicReport {
        let lower = thumbnail_url.to_lowercase();
        if lower.contains("suggestive")
            || lower.contains("bikini")
            || lower.contains("g-string")
            || lower.contains("swimwear")
            || lower.contains("racy")
        {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.85,
                primary_archetype: VisualClickbaitArchetype::SexuallySuggestiveMetadata,
                secondary_archetypes: vec![VisualClickbaitArchetype::SensoryHyperSaturation],
                detected_focal_elements: vec!["swimwear / beachwear".to_string(), "suggestive framing".to_string()],
                packaging_integrity_index: 0.42,
                exogenous_ctr_modifier: 1.45,
                diagnostic_note: "⚠️ Viewership heavily CTR-frontloaded via provocative packaging; content kinematics lag behind click promise.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("phantom") || lower.contains("fake_asset") || lower.contains("bait_switch") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.05,
                primary_archetype: VisualClickbaitArchetype::PhantomAsset,
                secondary_archetypes: vec![VisualClickbaitArchetype::TabloidStickers],
                detected_focal_elements: vec!["unshown hero asset".to_string(), "hyperbolic badge".to_string()],
                packaging_integrity_index: 0.30,
                exogenous_ctr_modifier: 1.35,
                diagnostic_note: "Deceptive packaging: phantom asset displayed in thumbnail not delivered in content.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("peril") || lower.contains("fire") || lower.contains("disaster") || lower.contains("sinking") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::ManufacturedPeril,
                secondary_archetypes: vec![VisualClickbaitArchetype::SensoryHyperSaturation],
                detected_focal_elements: vec!["composite smoke/fire".to_string(), "crisis overlay".to_string()],
                packaging_integrity_index: 0.38,
                exogenous_ctr_modifier: 1.25,
                diagnostic_note: "Manufactured peril: visual catastrophe composite exaggerates actual video risk.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("arrow") || lower.contains("circle") || lower.contains("directing") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::DirectingDevices,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["red arrow directing device".to_string()],
                packaging_integrity_index: 0.55,
                exogenous_ctr_modifier: 1.15,
                diagnostic_note: "Directing devices: artificial arrows focus attention on mundane object.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("gaze") || lower.contains("mouth") || lower.contains("reaction") || lower.contains("mrbeast") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::GazeReactionDistortion,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["exaggerated open mouth expression".to_string()],
                packaging_integrity_index: 0.65,
                exogenous_ctr_modifier: 1.20,
                diagnostic_note: "Gaze distortion: exaggerated reaction facial framing.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("tabloid") || lower.contains("sticker") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::TabloidStickers,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["screaming all-caps text banner".to_string()],
                packaging_integrity_index: 0.50,
                exogenous_ctr_modifier: 1.18,
                diagnostic_note: "Tabloid stickers: hyperbolic text labels overlay thumbnail.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("saturation") || lower.contains("neon") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::SensoryHyperSaturation,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["neon halo outline".to_string()],
                packaging_integrity_index: 0.70,
                exogenous_ctr_modifier: 1.12,
                diagnostic_note: "Sensory hyper-saturation: artificial chromatic vibrancy boost.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: None,
            }
        } else if lower.contains("timestamp_collision") || lower.contains("dead_zone") || lower.contains("bottom_right") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::DirectingDevices,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["lower-right text badge".to_string()],
                packaging_integrity_index: 0.60,
                exogenous_ctr_modifier: 1.0,
                diagnostic_note: "Timestamp collision: lower-right visual element occluded by duration badge.".to_string(),
                timestamp_collision_detected: true,
                collision_detail: Some("Text/Focal element detected in bottom-right dead zone ($x \\ge 0.75, y \\ge 0.80$); obstructed by YouTube timestamp overlay.".to_string()),
                title_text_redundancy_pct: None,
                thumbnail_ocr_text: Some("EPISODE 1".to_string()),
            }
        } else if lower.contains("redundant") || lower.contains("verbal_overlap") {
            ThumbnailForensicReport {
                suggestive_metadata_score: 0.0,
                primary_archetype: VisualClickbaitArchetype::TabloidStickers,
                secondary_archetypes: vec![],
                detected_focal_elements: vec!["redundant text banner".to_string()],
                packaging_integrity_index: 0.65,
                exogenous_ctr_modifier: 1.05,
                diagnostic_note: "Verbal redundancy: thumbnail text duplicates title headline.".to_string(),
                timestamp_collision_detected: false,
                collision_detail: None,
                title_text_redundancy_pct: Some(75.0),
                thumbnail_ocr_text: Some("CHEAPEST ELECTRIC CAR".to_string()),
            }
        } else {
            ThumbnailForensicReport::clean_authentic()
        }
    }

    /// Evaluates a thumbnail URL using Gemini 2.5 Flash-Lite multimodal vision,
    /// or returns deterministic offline fallback if API key is unset or network fails.
    pub async fn analyze_thumbnail_url(&self, thumbnail_url: &str) -> Result<ThumbnailForensicReport, Box<dyn std::error::Error + Send + Sync>> {
        let key = match &self.api_key {
            Some(k) if !k.trim().is_empty() => k,
            _ => {
                debug!("GEMINI_API_KEY unset; returning deterministic offline thumbnail forensic report.");
                return Ok(Self::fallback_report(thumbnail_url));
            }
        };

        if !thumbnail_url.starts_with("http://") && !thumbnail_url.starts_with("https://") {
            return Ok(Self::fallback_report(thumbnail_url));
        }

        // Fetch image bytes
        let img_bytes = match self.http.get(thumbnail_url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    warn!("Failed to read thumbnail bytes from {}: {}", thumbnail_url, e);
                    return Ok(Self::fallback_report(thumbnail_url));
                }
            },
            Ok(resp) => {
                warn!("Thumbnail fetch for {} failed with HTTP status: {}", thumbnail_url, resp.status());
                return Ok(Self::fallback_report(thumbnail_url));
            }
            Err(e) => {
                warn!("Network error fetching thumbnail {}: {}", thumbnail_url, e);
                return Ok(Self::fallback_report(thumbnail_url));
            }
        };

        let b64_img = BASE64.encode(&img_bytes);
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, key
        );

        let system_prompt = "You are a forensic computer vision engine auditing YouTube packaging integrity and visual clickbait taxonomy.\n\
        Analyze this video thumbnail and classify it according to the following strict JSON schema:\n\
        {\n\
          \"suggestive_metadata_score\": float (0.0 to 1.0, degree of sexually suggestive metadata, skin exposure, provocative posing, swimwear, cleavage, underwear, or clickbait eroticization),\n\
          \"primary_archetype\": one of [\"CleanAuthentic\", \"SexuallySuggestiveMetadata\", \"GazeReactionDistortion\", \"DirectingDevices\", \"ManufacturedPeril\", \"PhantomAsset\", \"TabloidStickers\", \"SensoryHyperSaturation\"],\n\
          \"secondary_archetypes\": array of string archetypes,\n\
          \"detected_focal_elements\": array of string descriptions (e.g. [\"swimwear\", \"red arrow\", \"exaggerated expression\"]),\n\
          \"packaging_integrity_index\": float (0.0 to 1.0, where 1.0 = authentic match to content, < 0.4 = bait-and-switch),\n\
          \"exogenous_ctr_modifier\": float (1.0 = baseline normal CTR, 1.4 = 40% CTR lift driven solely by packaging optics),\n\
          \"diagnostic_note\": string (1-2 sentences explaining visual packaging diagnosis),\n\
          \"timestamp_collision_detected\": boolean (true if any burned-in text, presenter facial features, or key hero objects are positioned in the bottom-right 15% quadrant of the frame: x >= 0.75, y >= 0.80, which is physically occluded by YouTube's timestamp badge),\n\
          \"collision_detail\": string or null (explanation of the collision if detected),\n\
          \"thumbnail_ocr_text\": string or null (exact burned-in headline or badge text extracted from the image),\n\
          \"title_text_redundancy_pct\": float or null (0.0 to 100.0, percentage of thumbnail words that duplicate common words in the title)\n\
        }\n\
        Return pure JSON matching this exact structure.";

        let body = json!({
            "contents": [
                {
                    "parts": [
                        { "text": system_prompt },
                        {
                            "inline_data": {
                                "mime_type": "image/jpeg",
                                "data": b64_img
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
            Ok(resp) if resp.status().is_success() => {
                if let Ok(raw_json) = resp.json::<serde_json::Value>().await {
                    if let Some(text_content) = raw_json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        if let Ok(report) = serde_json::from_str::<ThumbnailForensicReport>(text_content) {
                            return Ok(report);
                        }
                    }
                }
            }
            Ok(resp) => {
                warn!("Gemini 2.5 Flash-Lite thumbnail vision returned status: {}", resp.status());
            }
            Err(e) => {
                warn!("HTTP error during Gemini thumbnail vision: {}", e);
            }
        }

        Ok(Self::fallback_report(thumbnail_url))
    }

    /// Synchronous / blocking wrapper for evaluating thumbnail forensics safely across any thread context.
    pub fn analyze_thumbnail_url_blocking(&self, thumbnail_url: &str) -> ThumbnailForensicReport {
        let client = self.clone();
        let url_owned = thumbnail_url.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap_or_else(|_| tokio::runtime::Runtime::new().unwrap());
            rt.block_on(client.analyze_thumbnail_url(&url_owned))
                .unwrap_or_else(|_| Self::fallback_report(&url_owned))
        })
        .join()
        .unwrap_or_else(|_| Self::fallback_report(thumbnail_url))
    }

    /// Convenience blocking evaluator using default environment configuration.
    pub fn analyze_url_blocking(thumbnail_url: &str) -> ThumbnailForensicReport {
        let engine = Self::default();
        engine.analyze_thumbnail_url_blocking(thumbnail_url)
    }
}
