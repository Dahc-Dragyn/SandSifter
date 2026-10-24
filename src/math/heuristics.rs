use serde::{Deserialize, Serialize};

/// Audit verdict assessing whether social video metrics reflect organic virality or paid promotion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementAudit {
    pub engagement_rate_percent: f64,
    pub is_organic: bool,
    pub is_likely_paid_or_boosted: bool,
    pub screening_reason: Option<String>,
}

/// Heuristics engine for identifying artificial distribution, bot pods, and paid boosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeuristicsEngine;

impl HeuristicsEngine {
    /// Standard organic engagement cutoff percentage (2.0%).
    /// Videos below this floor on high view counts represent paid spend (TikTok Spark Ads / Meta Boosts).
    pub const PAID_SCREENING_THRESHOLD_PERCENT: f64 = 2.0;

    /// Computes the engagement rate as a percentage:
    /// `Engagement Rate = (Likes + Comments + Shares) / Views * 100.0`
    pub fn calculate_engagement_rate(views: u64, likes: u64, comments: u64, shares: u64) -> f64 {
        if views == 0 {
            return 0.0;
        }
        let total_interactions = likes.saturating_add(comments).saturating_add(shares);
        ((total_interactions as f64) / (views as f64)) * 100.0
    }

    /// Performs an engagement audit against the 2.0% screening threshold.
    pub fn audit_engagement(
        views: u64,
        likes: u64,
        comments: u64,
        shares: u64,
        custom_threshold: Option<f64>,
    ) -> EngagementAudit {
        let rate = Self::calculate_engagement_rate(views, likes, comments, shares);
        let threshold = custom_threshold.unwrap_or(Self::PAID_SCREENING_THRESHOLD_PERCENT);

        // Videos with very low views (< 500) aren't statistically flagged as paid ads
        let is_statistically_significant = views >= 1000;

        if is_statistically_significant && rate < threshold {
            EngagementAudit {
                engagement_rate_percent: rate,
                is_organic: false,
                is_likely_paid_or_boosted: true,
                screening_reason: Some(format!(
                    "Engagement rate ({:.2}%) fell below organic threshold ({:.2}%). Likely boosted via paid spend.",
                    rate, threshold
                )),
            }
        } else {
            EngagementAudit {
                engagement_rate_percent: rate,
                is_organic: true,
                is_likely_paid_or_boosted: false,
                screening_reason: None,
            }
        }
    }

    /// Identifies the natural delineation point between winners and losers in a sorted cohort of view counts.
    /// Looks for the largest relative drop-off (gap) between adjacent videos.
    /// If no significant gap exists, falls back to the median view count.
    pub fn find_winner_loser_delineation(views_descending: &[u64]) -> Option<u64> {
        if views_descending.is_empty() {
            return None;
        }
        if views_descending.len() < 3 {
            return views_descending.first().copied();
        }

        let mut max_ratio_drop = 1.0;
        let mut cutoff_view = views_descending[views_descending.len() / 2];

        // Search top 50% for largest relative cliff
        let half = views_descending.len() / 2;
        for i in 0..half {
            let current = views_descending[i] as f64;
            let next = views_descending[i + 1] as f64;

            if next > 0.0 {
                let drop_ratio = current / next;
                if drop_ratio > max_ratio_drop && drop_ratio >= 1.5 {
                    max_ratio_drop = drop_ratio;
                    cutoff_view = views_descending[i];
                }
            }
        }

        Some(cutoff_view)
    }
}
