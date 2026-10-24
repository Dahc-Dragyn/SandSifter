use serde::{Deserialize, Serialize};

/// Visual pacing classification based on Average Shot Length (ASL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutVelocityClassification {
    /// ASL <= 2.5s (e.g. MrBeast, rapid TikTok/Shorts pacing, jump-cut heavy)
    RapidFastPaced,
    /// 2.5s < ASL <= 5.0s (standard YouTube narrative storytelling)
    StandardNarrative,
    /// ASL > 5.0s (contemplative, cinematic documentary, long unedited takes)
    SlowContemplative,
}

impl CutVelocityClassification {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RapidFastPaced => "⚡ Rapid / High Velocity (Jump-Cut Heavy)",
            Self::StandardNarrative => "🎬 Standard Narrative (Balanced Cuts)",
            Self::SlowContemplative => "🌿 Slow / Contemplative (Long Cinematic Takes)",
        }
    }
}

/// Maximum believable single-camera hold duration on edited YouTube content before
/// classifying the interval as an FFmpeg streaming cut-detection dropout.
pub const STREAMING_DROPOUT_THRESHOLD_SEC: f64 = 120.0;

/// Visual kinetics metrics for an analyzed video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualPacingMetrics {
    /// Analyzed duration in seconds
    pub duration_seconds: f64,
    /// Total detected scene transitions (cuts)
    pub total_scene_cuts: usize,
    /// Average Shot Length (ASL = duration / cuts)
    pub average_shot_length_sec: f64,
    /// Pacing classification
    pub cut_classification: CutVelocityClassification,
    /// Timestamps of detected scene cuts (in seconds)
    pub cut_timestamps: Vec<f64>,
    /// Actionable tactical rule for the creator
    pub tactical_rule: String,
    /// Count of detected streaming cut-detection dropouts (>120s unbroken intervals)
    #[serde(default)]
    pub streaming_dropouts_count: usize,
}

pub struct VisualPacingEngine;

impl VisualPacingEngine {
    /// Calculates Average Shot Length (ASL).
    /// Enforces max(1) on cuts to prevent division-by-zero on static single-shot videos.
    pub fn calculate_asl(duration_seconds: f64, scene_cuts: usize) -> f64 {
        let cuts = scene_cuts.max(1) as f64;
        let asl = duration_seconds.max(1.0) / cuts;
        (asl * 10.0).round() / 10.0
    }

    /// Inspects detected shot intervals. If an unbroken shot interval exceeds 120.0s (2 minutes)
    /// on an edited video, flags it as a Streaming Cut-Detection Dropout rather than a true single-take camera hold.
    pub fn detect_streaming_dropouts(duration_seconds: f64, cut_timestamps: &[f64]) -> Vec<(f64, f64)> {
        let mut dropouts = Vec::new();
        let mut sorted: Vec<f64> = cut_timestamps.iter().copied().filter(|&c| c > 0.05 && c < duration_seconds).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted.dedup();

        let mut prev = 0.0;
        for &c in &sorted {
            if c - prev > STREAMING_DROPOUT_THRESHOLD_SEC {
                dropouts.push((prev, c));
            }
            prev = c;
        }
        if duration_seconds - prev > STREAMING_DROPOUT_THRESHOLD_SEC {
            dropouts.push((prev, duration_seconds));
        }
        dropouts
    }

    /// Classifies visual pacing based on Average Shot Length.
    pub fn classify_asl(asl: f64) -> CutVelocityClassification {
        if asl <= 2.5 {
            CutVelocityClassification::RapidFastPaced
        } else if asl <= 5.0 {
            CutVelocityClassification::StandardNarrative
        } else {
            CutVelocityClassification::SlowContemplative
        }
    }

    /// Debounces rapid successive cut timestamps to eliminate false positives caused by
    /// camera shake, whip-pans, motion blur, and exposure hunting.
    ///
    /// Algorithm:
    /// 1. Enforces a Minimum Shot Duration Guard (`min_shot_duration: f64 = 0.5s`):
    ///    Any cut occurring within `< min_shot_duration` of the previously accepted cut is discarded.
    /// 2. Motion vs. Hard Cut Suppression:
    ///    When multiple frames trigger in rapid succession (`raw_delta < 0.25s`), they represent
    ///    a continuous multi-frame blur / camera motion flurry rather than an isolated transition.
    ///    The debouncer suppresses flurry re-triggers until the camera settles for at least `min_shot_duration`.
    pub fn debounce_cuts(cut_timestamps: &[f64], min_shot_duration: Option<f64>) -> Vec<f64> {
        let min_dur = min_shot_duration.unwrap_or(0.5);
        let mut debounced = Vec::new();
        let mut last_raw_ts: Option<f64> = None;
        let mut in_motion_flurry = false;

        for &ts in cut_timestamps {
            if let Some(prev_raw) = last_raw_ts {
                let raw_delta = ts - prev_raw;
                if raw_delta < 0.25 {
                    // Continuous multi-frame blur / shake cluster detected
                    in_motion_flurry = true;
                } else if raw_delta >= min_dur {
                    // Camera held steady, motion has settled
                    in_motion_flurry = false;
                }
            }
            last_raw_ts = Some(ts);

            let can_accept = match debounced.last() {
                None => true,
                Some(&last_accepted) => {
                    let delta_accepted = ts - last_accepted;
                    delta_accepted >= min_dur && !in_motion_flurry
                }
            };

            if can_accept {
                debounced.push(ts);
            }
        }

        debounced
    }

    /// Evaluates visual kinetics and shot pacing from detected cut timestamps.
    /// Automatically applies temporal debouncing with a minimum shot duration guard (0.5s).
    pub fn evaluate_visual_pacing(
        duration_seconds: f64,
        cut_timestamps: &[f64],
    ) -> VisualPacingMetrics {
        Self::evaluate_visual_pacing_with_debounce(duration_seconds, cut_timestamps, Some(0.5))
    }

    /// Evaluates visual kinetics with explicit minimum shot duration debouncing.
    pub fn evaluate_visual_pacing_with_debounce(
        duration_seconds: f64,
        cut_timestamps: &[f64],
        min_shot_duration: Option<f64>,
    ) -> VisualPacingMetrics {
        let debounced_cuts = Self::debounce_cuts(cut_timestamps, min_shot_duration);
        let cuts = debounced_cuts.len();
        let asl = Self::calculate_asl(duration_seconds, cuts);
        let classification = Self::classify_asl(asl);
        let rule = Self::synthesize_pacing_rule(asl, classification);
        let dropouts = Self::detect_streaming_dropouts(duration_seconds, &debounced_cuts);
        let streaming_dropouts_count = dropouts.len();

        VisualPacingMetrics {
            duration_seconds,
            total_scene_cuts: cuts,
            average_shot_length_sec: asl,
            cut_classification: classification,
            cut_timestamps: debounced_cuts,
            tactical_rule: rule,
            streaming_dropouts_count,
        }
    }

    /// Synthesizes a practical, actionable rule based on observed shot length.
    pub fn synthesize_pacing_rule(asl: f64, classification: CutVelocityClassification) -> String {
        match classification {
            CutVelocityClassification::RapidFastPaced => {
                format!(
                    "Hyper-Active Visual Velocity (ASL: {:.1}s): Scene changes every {:.1} seconds keep dopamine high. Ensure on-screen text and dialogue have enough visual anchoring so viewers don't feel overwhelmed.",
                    asl, asl
                )
            }
            CutVelocityClassification::StandardNarrative => {
                format!(
                    "Optimal Narrative Pacing (ASL: {:.1}s): Balanced cutting rhythm every {:.1} seconds matches the YouTube baseline for clear storytelling without stagnation.",
                    asl, asl
                )
            }
            CutVelocityClassification::SlowContemplative => {
                format!(
                    "Static / Extended Takes (ASL: {:.1}s): Holding shots for {:.1} seconds risks visual fatigue unless tension or high-stakes B-roll carries the frame. Introduce B-roll cutaways or zoom punches every 3-4 seconds.",
                    asl, asl
                )
            }
        }
    }

    /// Synthesizes comparative cohort guidance comparing High Performers vs Low Performers.
    pub fn synthesize_comparative_rule(asl_high: f64, asl_low: f64) -> String {
        let diff = asl_high - asl_low;
        if diff <= -1.2 {
            format!(
                "Tighten your visual cuts: Winning videos change camera angles every {:.1}s compared to {:.1}s on underperforming uploads. Cut away before shots linger.",
                asl_high, asl_low
            )
        } else if diff >= 1.2 {
            format!(
                "Let your shots breathe: Winning videos hold scenes for {:.1}s (vs frantic {:.1}s cuts). In this niche, cinematic composure outperforms hyperactive jump cuts.",
                asl_high, asl_low
            )
        } else {
            format!(
                "Maintain visual cadence: Both cohorts average ~{:.1}s per shot. Focus on hook stakes rather than changing cutting tempo.",
                asl_high
            )
        }
    }
}
