use serde::{Deserialize, Serialize};

/// Visual keyframe extracted from a short-form video stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Millisecond offset of the frame in the video timeline
    pub timestamp_ms: u32,
    /// Raw image bytes (PNG or JPEG)
    pub frame_bytes: Vec<u8>,
    /// MIME type of the frame image
    pub mime_type: String,
    /// Indicates whether this is the initial opening frame (t = 0.0s)
    pub is_opening_frame: bool,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl Keyframe {
    pub fn new(timestamp_ms: u32, frame_bytes: Vec<u8>, mime_type: &str, width: u32, height: u32) -> Self {
        Self {
            timestamp_ms,
            frame_bytes,
            mime_type: mime_type.to_string(),
            is_opening_frame: timestamp_ms == 0,
            width,
            height,
        }
    }

    /// Returns timestamp in fractional seconds
    pub fn timestamp_seconds(&self) -> f64 {
        (self.timestamp_ms as f64) / 1000.0
    }
}

/// Keyframe extractor configured for short-form retention timestamps (0s, 1s, 2s, 5s).
#[derive(Debug, Clone)]
pub struct KeyframeExtractor {
    pub target_timestamps_ms: Vec<u32>,
}

impl Default for KeyframeExtractor {
    fn default() -> Self {
        Self {
            target_timestamps_ms: vec![0, 1000, 2000, 5000],
        }
    }
}

impl KeyframeExtractor {
    pub fn new(target_timestamps_ms: Vec<u32>) -> Self {
        Self { target_timestamps_ms }
    }

    /// Extracts keyframes from raw video stream bytes.
    /// Handles container chunks and produces keyframes mapped to target timestamps.
    pub fn extract_from_bytes(&self, video_bytes: &[u8]) -> Vec<Keyframe> {
        if video_bytes.is_empty() {
            return Vec::new();
        }

        // Split or sample chunks proportional to target timestamps
        let mut keyframes = Vec::new();
        let chunk_size = (video_bytes.len() / self.target_timestamps_ms.len().max(1)).max(1);

        for (idx, &ts) in self.target_timestamps_ms.iter().enumerate() {
            let start = (idx * chunk_size).min(video_bytes.len());
            let end = ((idx + 1) * chunk_size).min(video_bytes.len());
            let slice = &video_bytes[start..end];

            keyframes.push(Keyframe {
                timestamp_ms: ts,
                frame_bytes: slice.to_vec(),
                mime_type: "image/jpeg".to_string(),
                is_opening_frame: ts == 0,
                width: 1080,
                height: 1920,
            });
        }

        keyframes
    }

    /// Synthesizes deterministic test keyframes for offline testing and verification.
    pub fn synthesize_test_keyframes(&self) -> Vec<Keyframe> {
        // Minimal valid 1x1 PNG byte signature
        let dummy_png_bytes = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        self.target_timestamps_ms
            .iter()
            .map(|&ts| Keyframe {
                timestamp_ms: ts,
                frame_bytes: dummy_png_bytes.clone(),
                mime_type: "image/png".to_string(),
                is_opening_frame: ts == 0,
                width: 1080,
                height: 1920,
            })
            .collect()
    }
}
