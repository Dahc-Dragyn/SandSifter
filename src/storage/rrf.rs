use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Standard Reciprocal Rank Fusion smoothing constant
pub const RRF_SMOOTHING_CONSTANT_K: f64 = 60.0;

/// Ranked candidate item produced by Reciprocal Rank Fusion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RrfRankedItem {
    pub id: String,
    pub rrf_score: f64,
    pub vector_rank: Option<usize>,
    pub lexical_rank: Option<usize>,
}

/// Fuses multiple ranked result lists using Reciprocal Rank Fusion (RRF):
/// `RRF_Score(item) = sum( 1.0 / (k + rank) )`
/// 
/// `k` is the smoothing constant (typically 60.0).
/// `rank` is 1-indexed.
pub fn reciprocal_rank_fusion(
    vector_results: &[String],
    lexical_results: &[String],
    k: f64,
) -> Vec<RrfRankedItem> {
    let mut scores: HashMap<String, (f64, Option<usize>, Option<usize>)> = HashMap::new();

    // Ingest vector search ranks
    for (idx, id) in vector_results.iter().enumerate() {
        let rank = idx + 1;
        let score_contrib = 1.0 / (k + rank as f64);
        let entry = scores.entry(id.clone()).or_insert((0.0, None, None));
        entry.0 += score_contrib;
        entry.1 = Some(rank);
    }

    // Ingest lexical search ranks
    for (idx, id) in lexical_results.iter().enumerate() {
        let rank = idx + 1;
        let score_contrib = 1.0 / (k + rank as f64);
        let entry = scores.entry(id.clone()).or_insert((0.0, None, None));
        entry.0 += score_contrib;
        entry.2 = Some(rank);
    }

    // Transform and sort descending by combined RRF score
    let mut items: Vec<RrfRankedItem> = scores
        .into_iter()
        .map(|(id, (rrf_score, vector_rank, lexical_rank))| RrfRankedItem {
            id,
            rrf_score,
            vector_rank,
            lexical_rank,
        })
        .collect();

    items.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    items
}
