use serde::{Deserialize, Serialize};

/// Algorithmic classification tier for short-form video performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlierTier {
    /// Multiplier < 1.0x (Underperformed channel standard)
    Underperformer,
    /// 1.0x <= Multiplier < 2.0x (Standard audience reach)
    Baseline,
    /// 2.0x <= Multiplier < 5.0x (Statistical Outlier / Winner)
    WinnerOutlier,
    /// Multiplier >= 5.0x (Breakout Viral Phenomenon)
    BreakoutViral,
}

impl std::fmt::Display for OutlierTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutlierTier::Underperformer => write!(f, "Underperformer (<1.0x)"),
            OutlierTier::Baseline => write!(f, "Baseline (1.0x-2.0x)"),
            OutlierTier::WinnerOutlier => write!(f, "Winner Outlier (2.0x-5.0x)"),
            OutlierTier::BreakoutViral => write!(f, "Breakout Viral (>=5.0x)"),
        }
    }
}

/// Outlier Scoring and Virality Decay Engine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct OutlierEngine;

impl OutlierEngine {
    /// Calculates the Outlier Multiplier `M = views / baseline`.
    /// Returns 0.0 if baseline is 0.
    pub fn calculate_multiplier(views: u64, baseline: u64) -> f64 {
        if baseline == 0 {
            return 0.0;
        }
        (views as f64) / (baseline as f64)
    }

    /// Maps a raw outlier multiplier into an operational tier.
    pub fn classify_tier(multiplier: f64) -> OutlierTier {
        if multiplier < 1.0 {
            OutlierTier::Underperformer
        } else if multiplier < 2.0 {
            OutlierTier::Baseline
        } else if multiplier < 5.0 {
            OutlierTier::WinnerOutlier
        } else {
            OutlierTier::BreakoutViral
        }
    }

    /// Evaluates if an outlier multiplier warrants automated deep multimodal analysis.
    /// Default threshold is 2.5x over baseline.
    pub fn warrants_deep_analysis(multiplier: f64, threshold: f64) -> bool {
        multiplier >= threshold
    }

    /// Normalizes raw views against algorithmic time-decay half-life:
    /// `V_decayed = V_raw * exp(-lambda * days_elapsed)`
    /// 
    /// Standard lambda for short-form video (TikTok/Reels) is ~0.0866 (half-life of ~8 days).
    pub fn time_decayed_views(raw_views: u64, days_elapsed: f64, lambda: f64) -> f64 {
        if days_elapsed <= 0.0 {
            return raw_views as f64;
        }
        (raw_views as f64) * (-lambda * days_elapsed).exp()
    }

    /// Calculates normalized view velocity (views per hour elapsed).
    pub fn view_velocity_per_hour(views: u64, hours_elapsed: f64) -> f64 {
        if hours_elapsed <= 0.0 {
            return views as f64;
        }
        (views as f64) / hours_elapsed
    }

    /// Default momentum sensitivity dampener (lambda).
    pub const DEFAULT_MOMENTUM_LAMBDA: f64 = 0.25;

    /// Computes the Channel-Velocity Normalized Outlier Multiplier (M_norm / CVNOS):
    /// M_norm = V_video / (B_c * (1 + lambda * ln(V_7 / V_30)))
    ///
    /// - `v_video`: Video lifetime view count.
    /// - `baseline_bc`: Winsorized rolling median channel baseline (B_c).
    /// - `v_7`: Total channel views over the preceding 7 days.
    /// - `v_30`: Total channel views over the preceding 30 days.
    /// - `lambda`: Momentum dampening coefficient (defaults to 0.25 if <= 0.0).
    pub fn calculate_m_norm(
        v_video: u64,
        baseline_bc: u64,
        v_7: u64,
        v_30: u64,
        lambda: f64,
    ) -> f64 {
        if baseline_bc == 0 {
            return 0.0;
        }

        let l = if lambda <= 0.0 { Self::DEFAULT_MOMENTUM_LAMBDA } else { lambda };

        // Fallback to standard multiplier if channel velocity data is zero or missing
        if v_30 == 0 || v_7 == 0 {
            return ((v_video as f64) / (baseline_bc as f64) * 100.0).round() / 100.0;
        }

        let ratio = (v_7 as f64) / (v_30 as f64);
        let safe_ratio = ratio.clamp(0.001, 10.0);
        let momentum_factor = (1.0 + l * safe_ratio.ln()).max(0.1);
        let denominator = (baseline_bc as f64) * momentum_factor;

        let m_norm = (v_video as f64) / denominator;
        (m_norm * 100.0).round() / 100.0
    }

    /// Evaluates full CVNOS metrics including momentum dampener and breakout classification.
    pub fn evaluate_cvnos(
        v_video: u64,
        baseline_bc: u64,
        v_7: u64,
        v_30: u64,
        lambda: f64,
        breakout_threshold: f64,
    ) -> CvnosMetrics {
        let raw_mult = Self::calculate_multiplier(v_video, baseline_bc);
        let m_norm = Self::calculate_m_norm(v_video, baseline_bc, v_7, v_30, lambda);

        let l = if lambda <= 0.0 { Self::DEFAULT_MOMENTUM_LAMBDA } else { lambda };
        let momentum_factor = if v_30 > 0 && v_7 > 0 {
            let ratio = ((v_7 as f64) / (v_30 as f64)).clamp(0.001, 10.0);
            ((1.0 + l * ratio.ln()).max(0.1) * 100.0).round() / 100.0
        } else {
            1.0
        };

        CvnosMetrics {
            v_video,
            baseline_bc,
            raw_multiplier: (raw_mult * 100.0).round() / 100.0,
            v_7,
            v_30,
            momentum_factor,
            m_norm,
            is_content_driven_breakout: m_norm >= breakout_threshold,
        }
    }
}

/// Detailed metrics decomposition for Channel-Velocity Normalized Outlier Score (CVNOS).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CvnosMetrics {
    pub v_video: u64,
    pub baseline_bc: u64,
    pub raw_multiplier: f64,
    pub v_7: u64,
    pub v_30: u64,
    pub momentum_factor: f64,
    pub m_norm: f64,
    pub is_content_driven_breakout: bool,
}

