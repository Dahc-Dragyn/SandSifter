use serde::{Deserialize, Serialize};

/// Computes the cosine similarity between two floating-point embedding vectors:
/// `similarity = (A · B) / (||A|| * ||B||)`
/// 
/// Returns a value in [-1.0, 1.0], or 0.0 if either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Generates a deterministic mock embedding vector for offline unit tests and benchmarks.
pub fn generate_mock_embedding(text: &str, dimensions: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dimensions];
    let bytes = text.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        let idx = (i * 31 + (b as usize)) % dimensions;
        vec[idx] += (b as f32) / 255.0;
    }

    // Normalize vector to unit length
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut vec {
            *val /= norm;
        }
    }

    vec
}

/// Payload request for Google Gemini text-embedding models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingRequest {
    pub model: String,
    pub content: GeminiContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPart {
    pub text: String,
}

/// Response payload from Google Gemini embedding endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingResponse {
    pub embedding: GeminiEmbeddingValues,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingValues {
    pub values: Vec<f32>,
}
