use serde::{Deserialize, Serialize};

/// Comprehensive statistical diagnostics exposing intermediate baseline metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineDiagnostics {
    pub sample_size: usize,
    pub raw_median: u64,
    pub q1: f64,
    pub q3: f64,
    pub iqr: f64,
    pub upper_clamp_bound: f64,
    pub winsorized_baseline: u64,
    pub clamped_spikes_count: usize,
}

/// Robust Statistical Baseline Calculator for Social Media Creators.
/// Implements rolling median with Interquartile Range (IQR) winsorization
/// to eliminate skew from viral spikes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaselineEngine;

impl BaselineEngine {
    /// Computes full intermediate diagnostics including quartiles, IQR bounds, and clamp count.
    pub fn compute_baseline_diagnostics(views: &[u64]) -> Option<BaselineDiagnostics> {
        if views.is_empty() {
            return None;
        }

        let mut sorted = views.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();

        let mid = n / 2;
        let raw_median = if n.is_multiple_of(2) {
            ((sorted[mid - 1] as f64 + sorted[mid] as f64) / 2.0).round() as u64
        } else {
            sorted[mid]
        };

        if n < 4 {
            return Some(BaselineDiagnostics {
                sample_size: n,
                raw_median,
                q1: sorted[0] as f64,
                q3: sorted[n - 1] as f64,
                iqr: (sorted[n - 1] - sorted[0]) as f64,
                upper_clamp_bound: f64::INFINITY,
                winsorized_baseline: raw_median,
                clamped_spikes_count: 0,
            });
        }

        // Compute quartiles Q1 (25th percentile) and Q3 (75th percentile)
        let q1_idx = n / 4;
        let q3_idx = (3 * n) / 4;

        let q1 = sorted[q1_idx] as f64;
        let q3 = sorted[q3_idx] as f64;
        let iqr = q3 - q1;
        let upper_bound = q3 + 1.5 * iqr;

        // Winsorize: clamp values exceeding upper bound
        let mut clamped_count = 0usize;
        let winsorized: Vec<f64> = sorted
            .into_iter()
            .map(|v| {
                let vf = v as f64;
                if vf > upper_bound {
                    clamped_count += 1;
                    upper_bound
                } else {
                    vf
                }
            })
            .collect();

        // Calculate median of winsorized distribution
        let median = if n.is_multiple_of(2) {
            (winsorized[mid - 1] + winsorized[mid]) / 2.0
        } else {
            winsorized[mid]
        };

        Some(BaselineDiagnostics {
            sample_size: n,
            raw_median,
            q1,
            q3,
            iqr,
            upper_clamp_bound: upper_bound,
            winsorized_baseline: median.round() as u64,
            clamped_spikes_count: clamped_count,
        })
    }

    /// Computes the noise-filtered baseline median for a series of historical view counts.
    /// 
    /// - If fewer than 4 data points are provided, standard median is returned.
    /// - For 4 or more points, Interquartile Range (IQR) bounds are derived:
    ///   `Upper Bound = Q3 + 1.5 * IQR`
    ///   Values above the upper bound are clamped (winsorized) before computing the median.
    pub fn compute_baseline(views: &[u64]) -> Option<u64> {
        Self::compute_baseline_diagnostics(views).map(|d| d.winsorized_baseline)
    }

    /// Computes a rolling baseline over a sliding lookback window (e.g. latest 10 to 30 videos).
    pub fn compute_rolling_baseline(views: &[u64], window_size: usize) -> Option<u64> {
        if views.is_empty() {
            return None;
        }
        let start = views.len().saturating_sub(window_size);
        Self::compute_baseline(&views[start..])
    }
}
