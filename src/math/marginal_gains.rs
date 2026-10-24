use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::analyzer::{
    suitability::VideoSuitabilityReport, thumbnail::VisualClickbaitArchetype,
    CommentTimestampCluster, PromiseGapEvaluator, PromiseGapReport, SuitabilityAuditor,
    ThumbnailForensicReport, ThumbnailVisionEngine,
};
use crate::math::shorts::{ShortsEngine, ShortsRetentionMetrics};
use crate::math::LoudnessMetrics;
use crate::math::retention::ReEngagementCheckpoint;
use crate::pipeline::NicheToneProfile;

/// Performance classification status for an individual telemetry vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorStatus {
    /// Performing at top-decile creator tier (Benchmark Leader).
    Elite,
    /// Operating comfortably within the high-retention window.
    Optimal,
    /// Minor friction or pacing inertia causing incremental viewer drop-off.
    MarginalDrag,
    /// Significant narrative, acoustic, or visual friction causing viewer bleed.
    CriticalFriction,
}

impl VectorStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Elite => "🟢 ELITE (Benchmark Leader)",
            Self::Optimal => "✅ OPTIMAL",
            Self::MarginalDrag => "⚠️ MARGINAL DRAG",
            Self::CriticalFriction => "🚨 CRITICAL FRICTION",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Elite => "ELITE",
            Self::Optimal => "OPTIMAL",
            Self::MarginalDrag => "DRAG",
            Self::CriticalFriction => "FRICTION",
        }
    }
}

/// Telemetry pipeline stage classification.
/// Decouples Stage 1: Impression Gate (Top-of-Funnel Potential) from
/// Stage 2: Immersion Mechanics (Retention Velocity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStage {
    /// Stage 1: Impression Gate (Top-of-Funnel Potential).
    /// Governed by Topic, Premise, Title Hook & Concrete Stakes (Vector 6).
    /// Determines whether viewers click at all.
    ImpressionGate,
    /// Stage 2: Immersion Mechanics (Retention Velocity).
    /// Governed by Acoustic Cold-Open (Vector 1), Visual Rhythm (Vector 2),
    /// Camera Hold (Vector 3), Retention Stability (Vector 4), and Voice Pacing (Vector 5).
    /// Determines whether viewers stay past minute 3.
    ImmersionMechanics,
}

impl PipelineStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ImpressionGate => "Impression Gate (Top-of-Funnel Potential)",
            Self::ImmersionMechanics => "Immersion Mechanics (Retention Velocity)",
        }
    }
}

/// An individual measured telemetry vector within the 6-point Brailsford Scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryVector {
    /// Display name of the telemetry vector (e.g. "Acoustic Cold-Open (t_silence)")
    pub name: String,
    /// Empirically measured value
    pub measured_value: f64,
    /// Target benchmark value for elite performance
    pub target_benchmark: f64,
    /// Physical measurement unit ("s", "WPM", "cuts", "s/cut", "score")
    pub unit: String,
    /// Normalized score between 0.0 and 100.0
    pub score_0_to_100: f64,
    /// Categorical health status
    pub status: VectorStatus,
    /// Concrete, actionable 1% editing adjustment with exact numbers
    pub one_percent_fix: String,
    /// Estimated audience retention / view multiplier lift percentage from applying the fix
    pub estimated_retention_lift_pct: f64,
}

impl TelemetryVector {
    /// Returns the creator-friendly human name for this telemetry vector.
    pub fn human_label(&self) -> &str {
        if self.name.contains("Cold-Open") || self.name.contains("t_silence") || self.name.contains("Intro Delay") {
            "Intro Delay (Time to First Spoken Word)"
        } else if self.name.contains("Visual Rhythm") || self.name.contains("ASL") || self.name.contains("Cutting Pace") {
            "First-Minute Cutting Pace (Shots & Cuts/Min)"
        } else if self.name.contains("Static Hold") || self.name.contains("Anti-Stagnation") || self.name.contains("Camera Hold") {
            "Longest Uncut Camera Hold (First Minute)"
        } else if self.name.contains("Retention Stability") || self.name.contains("R'(t)") || self.name.contains("Drop-Off") {
            "Mid-Video Viewer Drop-Off (Body Retention)"
        } else if self.name.contains("Audio Density") || self.name.contains("WPM") || self.name.contains("Voice Pacing") {
            "Voice Pacing & Breath Gaps"
        } else if self.name.contains("Stakes") || self.name.contains("Lexicon") || self.name.contains("Title Hook") || self.name.contains("Packaging") {
            "Title Hook & Concrete Stakes"
        } else {
            &self.name
        }
    }

    /// Categorizes vector into either Impression Gate (Top-of-Funnel Potential)
    /// or Immersion Mechanics (Retention Velocity).
    pub fn pipeline_stage(&self) -> PipelineStage {
        let label = self.human_label();
        let name = &self.name;
        if label.contains("Stakes") || name.contains("Stakes") || name.contains("Title Hook") || name.contains("Lexicon") || name.contains("Packaging") {
            PipelineStage::ImpressionGate
        } else {
            PipelineStage::ImmersionMechanics
        }
    }

    /// Returns human-readable pipeline stage label.
    pub fn stage_category(&self) -> &'static str {
        self.pipeline_stage().label()
    }

    /// Returns creator-friendly, human-understandable editor target guidelines without LaTeX or math symbols.
    pub fn editor_target(&self) -> String {
        let label = self.human_label();
        let name = &self.name;
        if label.contains("Intro Delay") || name.contains("Cold-Open") || name.contains("t_silence") {
            "Under 0.5s (Frames 1-12)".to_string()
        } else if label.contains("Cutting Pace") || name.contains("Visual Rhythm") || name.contains("ASL") {
            if self.target_benchmark <= 2.5 {
                "Cut every 2s (25+ cuts/min)".to_string()
            } else {
                "Cut every 3-5s (12-20 cuts/min)".to_string()
            }
        } else if label.contains("Camera Hold") || name.contains("Static Hold") {
            if self.target_benchmark <= 7.0 {
                "Max 7s per shot".to_string()
            } else if self.target_benchmark <= 12.0 {
                "Max 10s per shot".to_string()
            } else {
                "Max 10-14s per shot".to_string()
            }
        } else if label.contains("Drop-Off") || name.contains("Retention Stability") || name.contains("R'(t)") {
            "Steady decline (No steep cliffs)".to_string()
        } else if label.contains("Voice Pacing") || name.contains("Audio Density") || name.contains("WPM") {
            if self.target_benchmark <= 120.0 {
                "85-145 WPM (Ambient / ASMR)".to_string()
            } else {
                "150-200 WPM (Natural pace)".to_string()
            }
        } else if label.contains("Stakes") || name.contains("Packaging") || name.contains("Title Hook") || name.contains("Lexicon") {
            "High stakes / Clear scale".to_string()
        } else {
            format!("{:.1} {}", self.target_benchmark, self.unit)
        }
    }
}

/// Revealed viewer satisfaction telemetry (Comments & Likes per 1k views).
/// Encodes the axiom: Viewer Satisfaction > Raw Retention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ViewerSatisfactionMetrics {
    /// Percentage of viewers who liked the video (likes / views * 100.0)
    pub like_to_view_pct: f64,
    /// Percentage of viewers who commented (comments / views * 100.0)
    pub comment_to_view_pct: f64,
    /// Normalized comments per 1,000 views
    pub comments_per_1k_views: f64,
    /// Categorical satisfaction tier
    pub satisfaction_tier: String,
}

impl ViewerSatisfactionMetrics {
    pub fn new(views: u64, likes: u64, comments: u64) -> Self {
        if views == 0 {
            return Self {
                like_to_view_pct: 0.0,
                comment_to_view_pct: 0.0,
                comments_per_1k_views: 0.0,
                satisfaction_tier: "⚠️ Low Viewer Satisfaction / Passive Watch".to_string(),
            };
        }
        let v = views as f64;
        let like_to_view_pct = ((likes as f64 / v) * 100.0 * 100.0).round() / 100.0;
        let comment_to_view_pct = ((comments as f64 / v) * 100.0 * 1000.0).round() / 1000.0;
        let comments_per_1k_views = ((comments as f64 / v) * 1000.0 * 100.0).round() / 100.0;
        let engagement_pct = like_to_view_pct + (comments as f64 / v * 100.0);

        let satisfaction_tier = if comments_per_1k_views >= 2.0 || engagement_pct >= 5.0 {
            "💎 Diamond Satisfaction (High Comment Velocity)".to_string()
        } else if comments_per_1k_views >= 0.8 || engagement_pct >= 2.5 {
            "🟢 Healthy Satisfaction".to_string()
        } else if views >= 10_000_000 {
            // Mega-scale volume dampener: 10M+ views dilute per-view ratios
            if comments >= 50_000 || likes >= 2_000_000 {
                "💎 Diamond Satisfaction (High Comment Velocity)".to_string()
            } else if comments >= 10_000 || likes >= 500_000 {
                "🟢 Healthy Satisfaction".to_string()
            } else {
                "⚠️ Low Viewer Satisfaction / Passive Watch".to_string()
            }
        } else {
            "⚠️ Low Viewer Satisfaction / Passive Watch".to_string()
        };

        Self {
            like_to_view_pct,
            comment_to_view_pct,
            comments_per_1k_views,
            satisfaction_tier,
        }
    }

    /// Evaluates viewer satisfaction metrics with mega-scale volume awareness.
    pub fn evaluate(views: u64, likes: u64, comments: u64) -> Self {
        Self::new(views, likes, comments)
    }
}

/// Comprehensive 6-point telemetry scorecard inspired by Sir Dave Brailsford's
/// Aggregation of Marginal Gains philosophy and Reliability Engineering in Series Systems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarginalGainsScorecard {
    /// Video ID, URL, or Playlist ID
    pub target_id: String,
    /// Video title or playlist name
    pub title: String,
    /// Evaluated niche profile (e.g. "Outdoor / DIY Homestead")
    pub niche_profile: String,
    /// Detected or assigned NicheToneProfile (governs dynamic ASL and static hold expectations)
    #[serde(default)]
    pub tone_profile: Option<NicheToneProfile>,
    /// Composite weighted score across all 6 telemetry vectors (0.0 to 100.0)
    pub composite_brailsford_score: f64,
    /// Series system reliability percentage: Product(clamp(score_i / 100.0, 0.15, 1.0)) * 100.0
    pub series_system_reliability_pct: f64,
    /// Single Point of Failure (SPOF) if lowest vector score < 70.0
    pub single_point_of_failure: Option<String>,
    /// Viewer revealed satisfaction index comparing comments/likes per 1k views
    pub satisfaction_index: Option<ViewerSatisfactionMetrics>,
    /// Compounded multiplicative lift: Product(1 + lift_i)
    pub predicted_multiplier_lift: f64,
    /// All 6 evaluated telemetry vectors
    pub vectors: Vec<TelemetryVector>,
    /// Top 3 highest-ROI marginal gain tweaks (filtered for lift > 0.0%)
    pub top_3_marginal_gains: Vec<TelemetryVector>,
    /// Series funnel insight for sequel episodes (#2+) with high satisfaction despite normal funnel view drops
    #[serde(default)]
    pub series_funnel_insight: Option<String>,
    /// Optional informational note when an early outro drop is detected in the final 18% of the video
    #[serde(default)]
    pub outro_exit_note: Option<String>,
    /// Optional total video runtime in seconds from metadata
    #[serde(default)]
    pub duration_seconds: Option<f64>,
    /// Indicates whether this scorecard was synthesized from a local unreleased video render
    #[serde(default)]
    pub is_local_draft: bool,
    /// Optional thumbnail vision report capturing visual clickbait archetype and suggestive metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_report: Option<ThumbnailForensicReport>,
    /// Optional 0:00-0:05 Two-Part Handshake & Promise Gap report
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promise_gap_report: Option<PromiseGapReport>,
    /// Optional actual first spoken hook / transcript excerpt from the first 5 seconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_spoken_hook: Option<String>,
    /// Optional EBU R128 / ITU-R BS.1770 audio loudness metrics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness_metrics: Option<LoudnessMetrics>,
    /// Optional cumulative cuttable fat & punch-in editorial budget
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuttable_fat: Option<CuttableFatReport>,
    /// Optional crowd-sourced timestamp comment clusters (friction & praise)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comment_clusters: Vec<CommentTimestampCluster>,
    /// Optional Advertiser-Friendly Pre-Flight Gate report (v0.5.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suitability_report: Option<VideoSuitabilityReport>,
    /// Optional dedicated YouTube Shorts Retention Radar (v0.5.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorts_metrics: Option<ShortsRetentionMetrics>,
    /// Optional Re-Engagement Checkpoints (03:00 & 06:00 gates) for long-form content (v0.5.3)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub re_engagement_checkpoints: Vec<ReEngagementCheckpoint>,
    /// Optional Stair-Stepping Progression Format Evaluation (v0.5.3)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stair_stepping: Option<crate::pipeline::hook_machine::StairSteppingProgression>,
    /// Optional Strict Abrupt Ending & Outro Runway Report (v0.5.4)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outro_report: Option<crate::math::retention::OutroRunwayReport>,
    /// Optional audience retention time-series curve points (v0.5.4)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention_curve: Vec<crate::math::retention::HeatmapPoint>,
    /// Optional multimodal attention breaches & drop cliff events (v0.5.4)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attention_breaches: Vec<crate::math::retention::AttentionBreach>,
}

/// Midpoint digital punch-in or cutaway instruction to eliminate single-camera visual stagnation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PunchInCue {
    /// Formatted timecode (MM:SS) of the exact midpoint
    pub timecode: String,
    /// Midpoint timestamp in seconds where the 115% punch-in should be placed
    pub timecode_seconds: f64,
    /// Full duration of the continuous single-camera take
    pub hold_duration_seconds: f64,
    /// Digital zoom crop scale percentage (e.g. 115%)
    pub crop_scale_pct: u32,
    /// Plain-English editorial instruction for the timeline editor
    pub edit_instruction: String,
}

/// Cumulative editorial fat budget translating breath pauses, camera holds, and cold-open lag
/// into an exact runtime shaving meter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuttableFatReport {
    /// Cumulative dead air from inter-phrase breath pauses > 0.4s
    pub dead_air_breath_seconds: f64,
    /// Cumulative excess duration across camera holds exceeding niche benchmark
    pub camera_hold_fat_seconds: f64,
    /// Pre-speech cold-open lag exceeding 0.4s
    pub cold_open_lag_seconds: f64,
    /// Total reclaimable runtime in seconds without cutting narrative beats
    pub total_cuttable_seconds: f64,
    /// Number of micro-edits required to achieve the streamlined cut
    pub micro_edits_count: usize,
    /// Formatted original runtime (e.g. "14m 20s" or "14:20")
    pub original_runtime_formatted: String,
    /// Formatted streamlined runtime after micro-edits (e.g. "12m 06s" or "12:06")
    pub streamlined_runtime_formatted: String,
    /// Overall runtime reduction percentage (e.g. 15.6%)
    pub runtime_reduction_pct: f64,
    /// Dedicated 115% digital punch-in cues for extended camera takes
    pub punch_in_cues: Vec<PunchInCue>,
    /// Plain-English executive summary for the creator and editor
    pub executive_summary: String,
    /// Count of detected streaming cut-detection dropouts (>120s unbroken intervals)
    #[serde(default)]
    pub streaming_dropouts_count: usize,
}

impl MarginalGainsScorecard {
    /// Renders this scorecard to formatted markdown.
    pub fn to_markdown(&self) -> String {
        MarginalGainsEngine::render_markdown_scorecard(self)
    }
}

pub struct MarginalGainsEngine;

impl MarginalGainsEngine {
    /// Vector 1: Acoustic Cold-Open Discipline (t_silence) with OCR-Gated Text Hook awareness.
    /// Vector 1: Acoustic Cold-Open Discipline (t_silence) with OCR-Gated Text Hook and Sensory Opening awareness.
    /// Target Benchmark: <= 0.4s (Elite: <= 0.2s or frame-one On-Screen Text Hook)
    pub fn evaluate_cold_open_with_context(
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
        tone: Option<NicheToneProfile>,
        is_audio_first: bool,
    ) -> TelemetryVector {
        let raw_t = t_silence_sec.max(0.0);
        let t = if raw_t <= 0.25 { 0.0 } else { raw_t };
        let (score, status, fix, lift) = if let Some(snippet) = ocr_text_hook.filter(|s| !s.trim().is_empty()) {
            if t > 0.2 {
                (
                    95.0,
                    VectorStatus::Elite,
                    format!(
                        "Non-verbal text hook detected at frame one ({})—captures silent autoplay scrollers.",
                        snippet.trim()
                    ),
                    0.0,
                )
            } else {
                (
                    100.0,
                    VectorStatus::Elite,
                    "Maintain frame-one speech onset; zero bumper lag detected.".to_string(),
                    0.0,
                )
            }
        } else if t <= 0.2 {
            (
                100.0,
                VectorStatus::Elite,
                "Maintain frame-one speech onset; zero bumper lag detected.".to_string(),
                0.0,
            )
        } else if t <= 0.5 {
            (
                90.0,
                VectorStatus::Optimal,
                format!("Trim initial {:.2}s ambient lag by {:.2}s to land dialogue on frame 1.", t, (t - 0.2).max(0.0)),
                2.5,
            )
        } else if t <= 1.5 {
            (
                65.0,
                VectorStatus::MarginalDrag,
                format!("Eliminate introductory music swell: cut {:.1}s dead air so speech starts at 0.2s.", t),
                6.0,
            )
        } else if (is_audio_first
            || tone == Some(NicheToneProfile::AestheticLifestyleCozy)
            || tone == Some(NicheToneProfile::EducationalBreakdown)
            || tone == Some(NicheToneProfile::EntertainmentSpectacle)
            || tone == Some(NicheToneProfile::ConversationalPodcast))
            && t <= 45.0
        {
            (
                70.0,
                VectorStatus::Optimal,
                format!("Atmospheric sensory opening detected ({:.1}s vocal onset delay)—ambient soundscape & environmental audio establish tone; consider bringing first spoken line closer to <= 15s to maximize mobile autoplay capture.", t),
                4.0,
            )
        } else {
            (
                30.0,
                VectorStatus::CriticalFriction,
                if t >= 10.0 {
                    format!("Severe theme song bumper detected ({:.1}s vocal onset delay): trim intro music and land creator dialogue within 0.5s to halt autoplay bounce.", t)
                } else {
                    format!("Slash severe bumper lag: trim {:.1}s pre-speech gap down to under 0.5s (Frames 1-12) to halt autoplay drop-off.", t)
                },
                12.5,
            )
        };

        TelemetryVector {
            name: "Acoustic Cold-Open (t_silence)".to_string(),
            measured_value: (t * 10.0).round() / 10.0,
            target_benchmark: 0.4,
            unit: "s".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 1: Acoustic Cold-Open Discipline with Two-Part Handshake & Promise Gap awareness.
    /// If Promise Gap breach is detected (e.g. throat clearing, logo animation, or delay > 5.0s),
    /// applies a PromiseGapMismatch penalty (-15 points).
    pub fn evaluate_cold_open_with_promise_gap(
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
        tone: Option<NicheToneProfile>,
        is_audio_first: bool,
        promise_gap: Option<&PromiseGapReport>,
    ) -> TelemetryVector {
        let mut base_vec = Self::evaluate_cold_open_with_context(t_silence_sec, ocr_text_hook, tone, is_audio_first);

        if let Some(gap) = promise_gap {
            if gap.is_breach {
                // Suppress false PromiseGapMismatch penalties when dialogue begins immediately (t_silence <= 0.3s)
                // and verbal handshake passed (mentions title keywords in opening sentence), or when captions are pending.
                if (t_silence_sec <= 0.3 && gap.verbal_handshake_passed)
                    || gap.verbal_handshake_detail.contains("captions pending")
                    || gap.verbal_handshake_detail.contains("caption processing")
                {
                    return base_vec;
                }

                // Preserve physical acoustic timing: Vector 1 evaluates silence latency, not topical promise gap.
                // If dialogue begins immediately (t_silence <= 0.4s) and no throat-clearing occurred,
                // Vector 1 status must remain Elite/Optimal and must never be downgraded to MarginalDrag.
                // Topical mismatch penalties belong exclusively to Vector 6 (Title Hook & Stakes).
                if t_silence_sec <= 0.4 && !gap.throat_clearing_detected {
                    return base_vec;
                }

                let penalized_score = (base_vec.score_0_to_100 - 15.0).clamp(10.0, 100.0);
                base_vec.score_0_to_100 = penalized_score;
                base_vec.status = if penalized_score >= 80.0 {
                    VectorStatus::Elite
                } else if penalized_score >= 60.0 {
                    VectorStatus::Optimal
                } else if penalized_score >= 40.0 {
                    VectorStatus::MarginalDrag
                } else {
                    VectorStatus::CriticalFriction
                };
                if t_silence_sec > 0.4 && (base_vec.status == VectorStatus::Optimal || base_vec.status == VectorStatus::Elite) {
                    base_vec.status = VectorStatus::MarginalDrag;
                }
                base_vec.one_percent_fix = format!(
                    "PromiseGapMismatch (-15 pts): {} {}",
                    PromiseGapReport::BREACH_ALERT,
                    base_vec.one_percent_fix
                );
                base_vec.estimated_retention_lift_pct = (base_vec.estimated_retention_lift_pct + 5.0).max(8.0);
            }
        }

        base_vec
    }

    /// Vector 1: Acoustic Cold-Open Discipline (t_silence) with OCR-Gated Text Hook awareness.
    /// Target Benchmark: <= 0.4s (Elite: <= 0.2s or frame-one On-Screen Text Hook)
    pub fn evaluate_cold_open_with_ocr(
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
    ) -> TelemetryVector {
        Self::evaluate_cold_open_with_context(t_silence_sec, ocr_text_hook, None, false)
    }

    /// Vector 1: Acoustic Cold-Open Discipline (t_silence) - defaults to no OCR for backward compatibility.
    pub fn evaluate_cold_open(t_silence_sec: f64) -> TelemetryVector {
        Self::evaluate_cold_open_with_context(t_silence_sec, None, None, false)
    }

    /// Detects whether a video is an audio-first listening experience (ASMR, audiobook, ambient, soundscape).
    pub fn is_audio_first(title: &str, context: &str) -> bool {
        let text = format!("{} {}", title, context).to_lowercase();
        let tokens = [
            "asmr",
            "audiobook",
            "audio drama",
            "sleep story",
            "ambient",
            "soundscape",
            "guided meditation",
            "spoken word",
        ];
        tokens.iter().any(|&token| {
            if token == "asmr" {
                text.split_whitespace()
                    .any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == "asmr")
                    || text.contains("asmr")
            } else {
                text.contains(token)
            }
        })
    }

    /// Vector 2: Opening 60s Visual Rhythm (ASL & Cuts) conditioned on NicheToneProfile and Audio-First status.
    pub fn evaluate_visual_rhythm_with_context(
        asl_60s: f64,
        cuts_60s: usize,
        tone: NicheToneProfile,
        is_audio_first: bool,
    ) -> TelemetryVector {
        let asl = asl_60s.max(0.1);
        let cuts = cuts_60s;

        let (target_benchmark, score, status, fix, lift) = if is_audio_first && (asl > 3.0 || cuts <= 10) {
            let target = match tone {
                NicheToneProfile::HighStakesPanic => 2.2,
                NicheToneProfile::EntertainmentSpectacle => 4.0,
                NicheToneProfile::ConsumerReviewExplainer => 3.5,
                NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => 4.5,
                NicheToneProfile::EducationalBreakdown => 6.0,
                NicheToneProfile::NarrativeEmotionalPitch => 3.0,
                NicheToneProfile::ConversationalPodcast => 20.0,
            };
            (
                target,
                85.0,
                VectorStatus::Optimal,
                "Static audio-first canvas detected—visuals serve as ambient accompaniment; listener retention is driven by audio fidelity.".to_string(),
                2.0,
            )
        } else {
            match tone {
            NicheToneProfile::EntertainmentSpectacle => {
                let target = 4.0;
                if asl <= 3.5 && cuts >= 17 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite spectacle cutting rhythm ({:.1}s/cut, {} cuts/min); drives high viewer engagement.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 6.0 {
                    (
                        target,
                        85.0,
                        VectorStatus::Optimal,
                        format!("Measured spectacle pacing ({:.1}s/cut, {} cuts/min); balances visual scale with audience momentum.", asl, cuts),
                        3.0,
                    )
                } else if asl <= 9.0 {
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Pacing is dragging for spectacle format ({:.1}s/cut, {} cuts/min). Insert ~3 cutaways to reach <=4.0s/cut target.", asl, cuts),
                        8.0,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Severe spectacle drag ({:.1}s/cut, {} cuts/min). Cut every 3-5s to maintain energy.", asl, cuts),
                        14.0,
                    )
                }
            }
            NicheToneProfile::ConsumerReviewExplainer => {
                let target = 3.5;
                if asl <= 2.8 && cuts >= 21 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite consumer review cutting rhythm ({:.1}s/cut, {} cuts/min); excellent visual dynamism.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 3.5 && cuts >= 17 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Cutting rhythm is solid ({:.1}s/cut, {} cuts/min); maintains analytical engagement.", asl, cuts),
                        3.0,
                    )
                } else if asl <= 4.8 {
                    let needed_cuts = (17usize).saturating_sub(cuts).max(3);
                    (
                        target,
                        65.0,
                        VectorStatus::MarginalDrag,
                        format!("Currently {:.1}s/cut ({} cuts/min). Add ~{} cutaways or product close-ups in the first 60s to reach <=3.5s target pacing.", asl, cuts, needed_cuts),
                        7.0,
                    )
                } else {
                    let needed_cuts = (17usize).saturating_sub(cuts).max(6);
                    (
                        target,
                        35.0,
                        VectorStatus::CriticalFriction,
                        format!("Currently {:.1}s/cut ({} cuts/min). Review pacing is sluggish. Insert ~{} product B-roll or dynamic graphic cutaways to hit <=3.5s ASL.", asl, cuts, needed_cuts),
                        12.0,
                    )
                }
            }
            NicheToneProfile::HighStakesPanic => {
                let target = 2.2;
                if asl <= 1.8 && cuts >= 33 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("World-class visual cutting velocity ({:.1}s/cut, {} cuts/min); excellent high-stakes pacing.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 2.2 && cuts >= 27 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Pacing is healthy ({:.1}s/cut, {} cuts/min); add 2-3 extra micro-cutaways to hit elite <=1.8s ASL.", asl, cuts),
                        3.0,
                    )
                } else if asl <= 3.2 {
                    let needed_cuts = (27usize).saturating_sub(cuts).max(4);
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Currently {:.1}s/cut ({} cuts/min). Add ~{} B-roll cutaways in the first 60s to reach the <=2.2s/cut (27+ cuts/min) trailer rhythm.", asl, cuts, needed_cuts),
                        8.0,
                    )
                } else {
                    let needed_cuts = (27usize).saturating_sub(cuts).max(10);
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Currently {:.1}s/cut ({} cuts/min). Expectation Mismatch: High-stakes packaging expects rapid cuts, but edit is sluggish. Add ~{} B-roll cutaways in the first 60s to reach <=2.2s/cut (27+ cuts/min) trailer rhythm.", asl, cuts, needed_cuts),
                        15.0,
                    )
                }
            }
            NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => {
                let target = 5.0;
                if asl < 1.8 {
                    // Over-cutting / hyper-frantic penalty (Expectation Mismatch)
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Hyper-frantic cutting ({:.1}s/cut): pacing outruns documentary craft immersion; slow down cuts to 2.5s-4.0s to allow spatial scale to register.", asl),
                        7.0,
                    )
                } else if asl <= 3.5 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite documentary rhythm ({:.1}s/cut, {} cuts/min); natural immersive pacing balancing momentum and craft immersion.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 5.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Pacing is well-balanced ({:.1}s/cut, {} cuts/min); add 2-3 detail cutaways during longer explanation stretches to reach <=3.5s elite pacing.", asl, cuts),
                        3.0,
                    )
                } else if asl <= 7.0 {
                    let needed_cuts = (12usize).saturating_sub(cuts).max(3);
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Pacing is dragging ({:.1}s/cut, {} cuts/min). Insert ~{} cutaways or alternative camera angles to reach target <=5.0s/cut.", asl, cuts, needed_cuts),
                        8.0,
                    )
                } else {
                    let needed_cuts = (12usize).saturating_sub(cuts).max(5);
                    (
                        target,
                        30.0,
                        VectorStatus::CriticalFriction,
                        format!("Expectation Mismatch: Severe documentary drag ({:.1}s/cut, {} cuts/min). Static talking-head lock violates cinematic expectations: insert ~{} B-roll cuts to hit <=5.0s/cut target.", asl, cuts, needed_cuts),
                        14.0,
                    )
                }
            }
            NicheToneProfile::EducationalBreakdown => {
                let target = 6.0;
                if asl < 1.8 {
                    // Cognitive overload / frantic cuts penalty
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Expectation Mismatch: Frantic cuts ({:.1}s/cut) cause cognitive fatigue in educational content. Allow diagrams and screen shares to hold for 3.0s–5.0s.", asl),
                        6.0,
                    )
                } else if asl <= 4.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite educational pacing ({:.1}s/cut, {} cuts/min); clear visual transitions without diagram fatigue.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 6.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Good lecture/breakdown pacing ({:.1}s/cut, {} cuts/min); add visual callouts or code zoom-ins during longer talking segments to reach <=4.0s elite pacing.", asl, cuts),
                        3.5,
                    )
                } else if asl <= 10.0 {
                    let needed_cuts = (10usize).saturating_sub(cuts).max(2);
                    (
                        target,
                        65.0,
                        VectorStatus::MarginalDrag,
                        format!("Static lecture drag ({:.1}s/cut, {} cuts/min). Insert ~{} diagram/screen cutaways to hit <=6.0s/cut target.", asl, cuts, needed_cuts),
                        6.0,
                    )
                } else if asl <= 20.0 {
                    let needed_cuts = (10usize).saturating_sub(cuts).max(4);
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Extended screen-share pacing ({:.1}s/cut, {} cuts/min). Add ~{} code punch-ins or terminal cutaways to keep visual rhythm engaging.", asl, cuts, needed_cuts),
                        7.5,
                    )
                } else {
                    let needed_cuts = (10usize).saturating_sub(cuts).max(5);
                    (
                        target,
                        45.0,
                        VectorStatus::CriticalFriction,
                        format!("Continuous screencast walkthrough ({:.1}s/cut, {} cuts/min). Add ~{} screen cutaways, slide splits, or zoom punch-ins to maintain viewer alertness.", asl, cuts, needed_cuts),
                        9.0,
                    )
                }
            }
            NicheToneProfile::NarrativeEmotionalPitch => {
                let target = 3.0;
                if asl <= 1.8 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite narrative dramatic pacing ({:.1}s/cut, {} cuts/min); sharp emotional momentum without visual lag.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 3.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Strong cinematic pacing ({:.1}s/cut, {} cuts/min); within narrative target (<=3.0s/cut).", asl, cuts),
                        2.5,
                    )
                } else if asl <= 4.5 {
                    let needed_cuts = (20usize).saturating_sub(cuts).max(2);
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Narrative pacing drag ({:.1}s/cut, {} cuts/min). Insert ~{} scene/angle cuts to reach <=3.0s/cut target.", asl, cuts, needed_cuts),
                        6.0,
                    )
                } else {
                    let needed_cuts = (20usize).saturating_sub(cuts).max(4);
                    (
                        target,
                        30.0,
                        VectorStatus::CriticalFriction,
                        format!("Sluggish dramatic pacing ({:.1}s/cut, {} cuts/min). Add ~{} visual cutaways or reaction shots to maintain dramatic tension.", asl, cuts, needed_cuts),
                        11.0,
                    )
                }
            }
            NicheToneProfile::ConversationalPodcast => {
                let target = 20.0;
                if asl <= 10.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite multi-camera interview pacing ({:.1}s/cut, {} cuts/min); dynamic switching between host and guest.", asl, cuts),
                        0.0,
                    )
                } else if asl <= 20.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Broadcast conversational cutting rhythm ({:.1}s/cut, {} cuts/min); compliant with multi-camera studio standards.", asl, cuts),
                        2.0,
                    )
                } else if asl <= 30.0 {
                    (
                        target,
                        75.0,
                        VectorStatus::Optimal,
                        format!("Measured longform talk show pace ({:.1}s/cut, {} cuts/min); safe for relaxed longform discussions.", asl, cuts),
                        4.0,
                    )
                } else if asl <= 45.0 {
                    let needed_cuts = (3usize).saturating_sub(cuts).max(2);
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Conversational pacing drag ({:.1}s/cut, {} cuts/min). Switch camera angles or insert reaction shots ~{} times in the first 60s to stay within <=20.0s ASL.", asl, cuts, needed_cuts),
                        6.0,
                    )
                } else {
                    let needed_cuts = (3usize).saturating_sub(cuts).max(3);
                    (
                        target,
                        35.0,
                        VectorStatus::CriticalFriction,
                        format!("Static single-camera lock ({:.1}s/cut, {} cuts/min). Longform talk shows require multi-camera angle switches: insert ~{} cuts/min to reach <=20.0s ASL.", asl, cuts, needed_cuts),
                        10.0,
                    )
                }
            }
        }
    };

        TelemetryVector {
            name: "Opening 60s Visual Rhythm (ASL & Cuts)".to_string(),
            measured_value: (asl * 10.0).round() / 10.0,
            target_benchmark,
            unit: "s/cut".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 2: Opening 60s Visual Rhythm (ASL & Cuts) conditioned on NicheToneProfile.
    pub fn evaluate_visual_rhythm_with_tone(
        asl_60s: f64,
        cuts_60s: usize,
        tone: NicheToneProfile,
    ) -> TelemetryVector {
        Self::evaluate_visual_rhythm_with_context(asl_60s, cuts_60s, tone, false)
    }

    /// Vector 2: Opening 60s Visual Rhythm (ASL & Cuts) - defaults to HighStakesPanic for backward compatibility.
    pub fn evaluate_visual_rhythm(asl_60s: f64, cuts_60s: usize) -> TelemetryVector {
        Self::evaluate_visual_rhythm_with_tone(asl_60s, cuts_60s, NicheToneProfile::HighStakesPanic)
    }

    /// Backward-compatible alias for ASL evaluation.
    pub fn evaluate_asl(asl_60s: f64) -> TelemetryVector {
        let implied_cuts = (60.0 / asl_60s.max(0.1)).round() as usize;
        let mut vec = Self::evaluate_visual_rhythm(asl_60s, implied_cuts);
        vec.name = "60s Trailer Visual Velocity (ASL)".to_string();
        vec
    }

    /// Backward-compatible alias for cut density evaluation.
    pub fn evaluate_cut_density(cuts_60s: usize) -> TelemetryVector {
        let asl = 60.0 / (cuts_60s.max(1) as f64);
        let mut vec = Self::evaluate_visual_rhythm(asl, cuts_60s);
        vec.name = "60s Trailer Cut Density".to_string();
        vec.measured_value = cuts_60s as f64;
        vec.unit = "cuts".to_string();
        vec.target_benchmark = 30.0;
        vec
    }

    /// Vector 3: Longest Static Shot Freeze conditioned on NicheToneProfile and Audio-First status.
    pub fn evaluate_static_hold_with_context(
        longest_shot_60s: f64,
        tone: NicheToneProfile,
        is_audio_first: bool,
    ) -> TelemetryVector {
        let hold = longest_shot_60s.max(0.1);
        let (target_benchmark, score, status, fix, lift) = if is_audio_first && hold > 8.0 {
            let target = match tone {
                NicheToneProfile::EntertainmentSpectacle => 15.0,
                NicheToneProfile::HighStakesPanic | NicheToneProfile::ConsumerReviewExplainer => 7.0,
                NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => 10.0,
                NicheToneProfile::EducationalBreakdown => 14.0,
                NicheToneProfile::NarrativeEmotionalPitch => 8.0,
                NicheToneProfile::ConversationalPodcast => 45.0,
            };
            (
                target,
                85.0,
                VectorStatus::Optimal,
                "Static audio-first canvas detected—visuals serve as ambient accompaniment; listener retention is driven by audio fidelity.".to_string(),
                2.0,
            )
        } else {
            match tone {
            NicheToneProfile::EntertainmentSpectacle => {
                let target = 15.0;
                if hold <= 8.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite high-energy spectacle pacing (longest hold {:.1}s <= 8.0s); zero visual fatigue.", hold),
                        0.0,
                    )
                } else if hold <= 15.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Establishing spectacle hold ({:.1}s <= 15.0s target); successfully establishes scale and visual grandeur without dragging.", hold),
                        2.0,
                    )
                } else if hold <= 20.0 {
                    (
                        target,
                        65.0,
                        VectorStatus::MarginalDrag,
                        format!("Spectacle stagnation risk: shot lingers for {:.1}s (>15.0s benchmark). Insert an alternate angle or reaction punch-in.", hold),
                        6.0,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Expectation Mismatch: Severe hold freeze in spectacle format: {:.1}s uninterrupted hold! Split with an action cut or punch-in.", hold),
                        12.0,
                    )
                }
            }
            NicheToneProfile::ConsumerReviewExplainer => {
                let target = 7.0;
                if hold <= 5.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        "Excellent dynamic hold discipline; all camera angles transition within 5.0s.".to_string(),
                        0.0,
                    )
                } else if hold <= 7.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Hold duration ({:.1}s) is within the <=7.0s consumer review threshold.", hold),
                        2.0,
                    )
                } else if hold <= 10.0 {
                    (
                        target,
                        65.0,
                        VectorStatus::MarginalDrag,
                        format!("Longest hold is {:.1}s (target: <=7.0s). Apply 115% digital punch-in or cut to product close-up at midpoint.", hold),
                        6.0,
                    )
                } else {
                    (
                        target,
                        35.0,
                        VectorStatus::CriticalFriction,
                        format!("Static camera freeze ({:.1}s vs <=7.0s target). Split extended talking-head hold with B-roll cutaways.", hold),
                        10.0,
                    )
                }
            }
            NicheToneProfile::HighStakesPanic => {
                let target = 7.0;
                if hold <= 5.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        "No static shot drag; all shots transition within 5.0s.".to_string(),
                        0.0,
                    )
                } else if hold <= 7.5 {
                    (
                        target,
                        85.0,
                        VectorStatus::Optimal,
                        format!("Longest hold is {:.1}s; split this shot with an alternate angle at the 4.0s mark.", hold),
                        4.0,
                    )
                } else if hold <= 16.0 {
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Attention drop risk: longest shot lingers for {:.1}s. Cut or insert action B-roll at 5.0s.", hold),
                        9.0,
                    )
                } else {
                    (
                        target,
                        20.0,
                        VectorStatus::CriticalFriction,
                        format!("Expectation Mismatch: Severe talking-head freeze: {:.1}s static lock! Insert at least 2 camera punch-ins/cutaways.", hold),
                        16.0,
                    )
                }
            }
            NicheToneProfile::DocumentaryBuildProcess => {
                let target = 12.0;
                if hold <= 8.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Immersive shot duration ({:.1}s); excellent balance of ambient craft atmosphere and visual progression.", hold),
                        0.0,
                    )
                } else if hold <= 12.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Longest ambient shot is {:.1}s (within cozy/craft <=12.0s benchmark); safe for slow-living/build immersion.", hold),
                        2.0,
                    )
                } else if hold <= 18.0 {
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Static stagnation risk: craft shot lingers for {:.1}s (>12.0s benchmark). Insert a close-up tool/hands cutaway at ~8.0s.", hold),
                        6.0,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Expectation Mismatch: Severe craft freeze: {:.1}s uninterrupted hold! Split with an alternate angle or detail macro shot.", hold),
                        12.0,
                    )
                }
            }
            NicheToneProfile::AestheticLifestyleCozy => {
                let target = 14.0;
                if hold <= 10.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Immersive scenic shot duration ({:.1}s); excellent balance of natural atmosphere and progression.", hold),
                        0.0,
                    )
                } else if hold <= 14.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Longest ambient shot is {:.1}s (within scenic/outdoor <=14.0s benchmark); safe for travel/lifestyle immersion.", hold),
                        2.0,
                    )
                } else if hold <= 20.0 {
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Static stagnation risk: scenic shot lingers for {:.1}s (>14.0s benchmark). Insert a close-up cutaway or angle shift.", hold),
                        6.0,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Expectation Mismatch: Severe freeze: {:.1}s uninterrupted hold! Split with an alternate angle or scenic macro shot.", hold),
                        12.0,
                    )
                }
            }
            NicheToneProfile::EducationalBreakdown => {
                let target = 14.0;
                if hold <= 8.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Dynamic visual breakdown ({:.1}s longest hold); avoids screen-share stagnation.", hold),
                        0.0,
                    )
                } else if hold <= 14.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Longest explanation/slide hold is {:.1}s (within educational <=14.0s benchmark).", hold),
                        2.5,
                    )
                } else if hold <= 24.0 {
                    (
                        target,
                        65.0,
                        VectorStatus::MarginalDrag,
                        format!("Slide fatigue risk: static screen held for {:.1}s (>14.0s benchmark). Add a cursor highlight, code zoom, or speaker insert.", hold),
                        5.5,
                    )
                } else if hold <= 45.0 {
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("Extended IDE screen hold ({:.1}s). Punch in 1.5x on active syntax or add visual callouts to break visual inertia.", hold),
                        7.0,
                    )
                } else {
                    (
                        target,
                        45.0,
                        VectorStatus::CriticalFriction,
                        format!("Continuous screen walkthrough ({:.1}s static hold). Punch in on active code lines or split terminal view to refresh frame.", hold),
                        8.5,
                    )
                }
            }
            NicheToneProfile::NarrativeEmotionalPitch => {
                let target = 8.0;
                if hold <= 5.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite dramatic scene rhythm (longest camera hold {:.1}s <= 5.0s); zero static visual fatigue.", hold),
                        0.0,
                    )
                } else if hold <= 8.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Cinematic hold discipline (longest camera hold {:.1}s <= 8.0s); holds emotional gaze without dragging.", hold),
                        2.5,
                    )
                } else if hold <= 14.0 {
                    (
                        target,
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Static shot drag: camera held on single frame for {:.1}s (>8.0s target). Cut to character reaction, close-up, or scene detail.", hold),
                        6.5,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("Severe narrative freeze: camera held static for {:.1}s. Break into reverse shots or dynamic push-ins to maintain audience immersion.", hold),
                        12.0,
                    )
                }
            }
            NicheToneProfile::ConversationalPodcast => {
                let target = 45.0;
                if hold <= 25.0 {
                    (
                        target,
                        100.0,
                        VectorStatus::Elite,
                        format!("Elite multi-camera switching discipline (longest hold {:.1}s <= 25.0s); seamless speaker coverage.", hold),
                        0.0,
                    )
                } else if hold <= 45.0 {
                    (
                        target,
                        90.0,
                        VectorStatus::Optimal,
                        format!("Longest hold is {:.1}s (within talk show <=45.0s target); natural listening/reaction hold duration.", hold),
                        2.0,
                    )
                } else if hold <= 60.0 {
                    (
                        target,
                        80.0,
                        VectorStatus::Optimal,
                        format!("Extended conversational hold ({:.1}s <= 60.0s ceiling); compliant with broadcast talk show standards.", hold),
                        4.0,
                    )
                } else if hold <= 75.0 {
                    (
                        target,
                        55.0,
                        VectorStatus::MarginalDrag,
                        format!("SlowVisualPacing warning: static camera hold persists for {:.1}s (>60.0s broadcast ceiling). Cut to wide shot or listening host angle at midpoint.", hold),
                        7.0,
                    )
                } else {
                    (
                        target,
                        25.0,
                        VectorStatus::CriticalFriction,
                        format!("CameraFreeze warning: unbroken single camera angle locked for {:.1}s (exceeds 60.0s broadcast limit). Switch to alternate angle or insert reaction cutaway.", hold),
                        12.0,
                    )
                }
            }
        }
    };

        TelemetryVector {
            name: "Longest Static Hold (Anti-Stagnation)".to_string(),
            measured_value: (hold * 10.0).round() / 10.0,
            target_benchmark,
            unit: "s".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 3: Longest Static Shot Freeze conditioned on NicheToneProfile.
    pub fn evaluate_static_hold_with_tone(longest_shot_60s: f64, tone: NicheToneProfile) -> TelemetryVector {
        Self::evaluate_static_hold_with_context(longest_shot_60s, tone, false)
    }

    /// Vector 3: Longest Static Shot Freeze - defaults to HighStakesPanic for backward compatibility.
    pub fn evaluate_static_hold(longest_shot_60s: f64) -> TelemetryVector {
        Self::evaluate_static_hold_with_tone(longest_shot_60s, NicheToneProfile::HighStakesPanic)
    }

    /// Vector 4: Mid-Roll Retention Stability (R'(t) Drop Cliff)
    /// Target Benchmark: <= 1.5 %/s drop rate (Elite: <= 1.0 %/s)
    pub fn evaluate_retention_stability(steepest_drop_rate_pct_per_sec: Option<f64>) -> TelemetryVector {
        Self::evaluate_retention_stability_with_timestamp(steepest_drop_rate_pct_per_sec, None)
    }

    /// Vector 4 with explicit drop cliff timestamp mm:ss.
    pub fn evaluate_retention_stability_with_timestamp(
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
    ) -> TelemetryVector {
        // Default to -1.5 %/s (Optimal) if heatmap/retention curve is absent
        let drop = steepest_drop_rate_pct_per_sec.unwrap_or(-1.5);
        let abs_drop = drop.abs();

        let ts_str = match worst_drop_timestamp_sec {
            Some(sec) if sec > 0.0 => {
                let total_sec = sec.round() as u64;
                let mm = total_sec / 60;
                let ss = total_sec % 60;
                format!(" at {:02}:{:02}", mm, ss)
            }
            _ => String::new(),
        };

        let (score, status, fix, lift) = if abs_drop <= 1.0 {
            (
                100.0,
                VectorStatus::Elite,
                "Zero retention drop cliffs detected; mid-roll audience engagement is exceptionally stable.".to_string(),
                0.0,
            )
        } else if abs_drop <= 1.8 {
            (
                88.0,
                VectorStatus::Optimal,
                if ts_str.is_empty() {
                    format!("Audience decay is steady (-{:.1}%/s); place a forward narrative question or open loop before the steepest dip.", abs_drop)
                } else {
                    format!("Audience decay is steady (-{:.1}%/s); place a forward narrative question or open loop{} before the steepest dip.", abs_drop, ts_str)
                },
                3.0,
            )
        } else if abs_drop <= 3.0 {
            (
                60.0,
                VectorStatus::MarginalDrag,
                if ts_str.is_empty() {
                    format!("Mid-roll drop cliff detected (-{:.1}%/s drop rate): insert a visual pattern interrupt or re-hook at this timestamp to prevent viewer abandonment.", abs_drop)
                } else {
                    format!("Mid-roll drop cliff detected{} (-{:.1}%/s drop rate): insert a visual pattern interrupt or trim exposition{} to prevent viewer abandonment.", ts_str, abs_drop, ts_str)
                },
                8.0,
            )
        } else {
            (
                25.0,
                VectorStatus::CriticalFriction,
                if ts_str.is_empty() {
                    format!("Catastrophic retention bleed (-{:.1}%/s drop rate): trim slow exposition, remove dead air, or restructure the narrative climax.", abs_drop)
                } else {
                    format!("Catastrophic retention bleed{} (-{:.1}%/s drop rate): trim slow exposition, remove dead air, or restructure the narrative climax{}.", ts_str, abs_drop, ts_str)
                },
                14.0,
            )
        };

        TelemetryVector {
            name: "Mid-Roll Retention Stability (R'(t) Cliff)".to_string(),
            measured_value: -(abs_drop * 10.0).round() / 10.0,
            target_benchmark: -1.5,
            unit: "%/s".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 5: Timeline Audio Density (Breath/Pause Trimming)
    /// Target Benchmark: 150.0 - 220.0 WPM (or 115.0 WPM in audio-first/ASMR mode, or 125.0 - 185.0 WPM in podcast mode)
    pub fn evaluate_audio_density_with_tone_and_context(
        active_wpm: f64,
        is_audio_first: bool,
        tone: Option<NicheToneProfile>,
    ) -> TelemetryVector {
        let wpm = active_wpm.max(0.0);
        let is_podcast = tone == Some(NicheToneProfile::ConversationalPodcast);
        let (score, status, fix, lift) = if is_audio_first {
            if (85.0..=145.0).contains(&wpm) {
                (
                    95.0,
                    VectorStatus::Elite,
                    format!(
                        "Intentional ambient/ASMR delivery ({:.0} WPM)—natural intimate pacing; do not trim breath pauses or accelerate speech.",
                        wpm
                    ),
                    0.0,
                )
            } else if wpm < 85.0 {
                (
                    85.0,
                    VectorStatus::Optimal,
                    format!(
                        "Live-action / ambient scene detected ({:.0} WPM)—natural on-location audio; no artificial speech trimming needed.",
                        wpm
                    ),
                    0.0,
                )
            } else if wpm <= 180.0 {
                (
                    90.0,
                    VectorStatus::Optimal,
                    format!("Clear conversational audio delivery ({:.0} WPM)—intelligible listening cadence.", wpm),
                    2.0,
                )
            } else {
                (
                    65.0,
                    VectorStatus::MarginalDrag,
                    format!("Delivery is slightly fast for ambient listening ({:.0} WPM); let pauses linger.", wpm),
                    5.0,
                )
            }
        } else if is_podcast {
            if (125.0..=185.0).contains(&wpm) {
                (
                    100.0,
                    VectorStatus::Optimal,
                    format!("Broadcast conversational cadence ({:.0} WPM)—intelligible longform dialogue; natural speech pacing.", wpm),
                    0.0,
                )
            } else if wpm < 95.0 {
                (
                    85.0,
                    VectorStatus::Optimal,
                    format!("Guest contemplation / thoughtful listening detected ({:.0} WPM)—natural conversational dynamic; no speech trimming needed.", wpm),
                    0.0,
                )
            } else if wpm < 125.0 {
                let _trim_sec = ((125.0 - wpm) / 125.0 * 2.0).clamp(0.4, 2.0);
                (
                    70.0,
                    VectorStatus::MarginalDrag,
                    format!("Pacing is slightly deliberate ({:.0} WPM). Trim unprompted dead-air silences >3.0s between topics in your edit.", wpm),
                    5.0,
                )
            } else {
                // wpm > 185.0
                if wpm <= 220.0 {
                    (
                        85.0,
                        VectorStatus::Optimal,
                        format!("Brisk conversational tempo ({:.0} WPM)—high energy banter within broadcast tolerance.", wpm),
                        2.0,
                    )
                } else {
                    (
                        60.0,
                        VectorStatus::MarginalDrag,
                        format!("Dialogue is rushed for a longform podcast ({:.0} WPM); let conversational points land before pivoting.", wpm),
                        6.0,
                    )
                }
            }
        } else if (150.0..=220.0).contains(&wpm) {
            (
                100.0,
                VectorStatus::Optimal,
                format!("Natural conversational cadence ({:.0} WPM)—no vocal changes needed; keep natural breathing rhythm.", wpm),
                0.0,
            )
        } else if wpm < 95.0 {
            (
                85.0,
                VectorStatus::Optimal,
                format!("Live-action / ambient scene detected ({:.0} WPM)—natural on-location audio; no artificial speech trimming needed.", wpm),
                0.0,
            )
        } else if wpm < 150.0 {
            let trim_sec = ((150.0 - wpm) / 150.0 * 2.0).clamp(0.4, 1.8);
            if wpm >= 120.0 {
                (
                    65.0,
                    VectorStatus::MarginalDrag,
                    format!("Do NOT try to talk faster on camera. Instead, trim ~{:.1}s of dead-air breath pauses between sentences in your NLE audio track to tighten pacing.", trim_sec),
                    7.0,
                )
            } else {
                (
                    30.0,
                    VectorStatus::CriticalFriction,
                    format!("Do NOT try to talk faster on camera. Instead, trim ~{:.1}s of dead-air breath pauses between sentences in your NLE audio track to tighten pacing.", trim_sec),
                    12.0,
                )
            }
        } else {
            // wpm > 220.0
            if wpm <= 250.0 {
                (
                    65.0,
                    VectorStatus::MarginalDrag,
                    format!("Delivery is slightly rushed ({:.0} WPM); let sentences breathe with 0.3s natural pauses in the edit.", wpm),
                    6.0,
                )
            } else {
                (
                    35.0,
                    VectorStatus::CriticalFriction,
                    format!("Delivery is slightly rushed ({:.0} WPM); let sentences breathe with 0.3s natural pauses in the edit.", wpm),
                    10.0,
                )
            }
        };

        let target_benchmark = if is_audio_first {
            115.0
        } else if is_podcast {
            155.0
        } else {
            185.0
        };

        TelemetryVector {
            name: "Timeline Audio Density (Breath/Pause Trimming)".to_string(),
            measured_value: (wpm * 10.0).round() / 10.0,
            target_benchmark,
            unit: "WPM".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 5: Timeline Audio Density (Breath/Pause Trimming) - backward-compatible context wrapper.
    pub fn evaluate_audio_density_with_context(active_wpm: f64, is_audio_first: bool) -> TelemetryVector {
        Self::evaluate_audio_density_with_tone_and_context(active_wpm, is_audio_first, None)
    }

    /// Vector 5: Timeline Audio Density (Breath/Pause Trimming) - default non-audio-first wrapper.
    pub fn evaluate_audio_density(active_wpm: f64) -> TelemetryVector {
        Self::evaluate_audio_density_with_tone_and_context(active_wpm, false, None)
    }

    /// Backward-compatible alias for evaluate_audio_density.
    pub fn evaluate_active_wpm(active_wpm: f64) -> TelemetryVector {
        Self::evaluate_audio_density(active_wpm)
    }

    /// Vector 6: Title Stakes Evaluator (alias for evaluate_stakes_lexicon_with_tone).
    pub fn evaluate_title_stakes(
        title: &str,
        hook_text: Option<&str>,
        tone: Option<NicheToneProfile>,
    ) -> TelemetryVector {
        Self::evaluate_stakes_lexicon_with_tone(title, hook_text, tone)
    }

    /// Vector 6: Physical Stakes & Framing Lexicon
    /// Target Benchmark: >= 70.0 / 100
    pub fn evaluate_stakes_lexicon(title: &str, hook_text: Option<&str>) -> TelemetryVector {
        let auto_tone = NicheToneProfile::detect(title, "", hook_text.unwrap_or(""));
        Self::evaluate_stakes_lexicon_with_tone(title, hook_text, Some(auto_tone))
    }

    /// Vector 6: Physical Stakes & Framing Lexicon conditioned on NicheToneProfile.
    pub fn evaluate_stakes_lexicon_with_tone(
        title: &str,
        hook_text: Option<&str>,
        tone: Option<NicheToneProfile>,
    ) -> TelemetryVector {
        let combined = format!("{} {}", title, hook_text.unwrap_or("")).to_lowercase();
        let effective_tone = tone.unwrap_or_else(|| NicheToneProfile::detect(title, "", hook_text.unwrap_or("")));
        
        // Physical craft, quantifiable units, and engineering keywords
        let high_stakes_tokens = [
            "excavator", "dump truck", "machine", "thousand", "10,000", "50,000", "100k", "110k",
            "lbs", "gallon", "pond", "rock", "boulder", "build", "concrete", "steel", "engine",
            "broken", "failed", "danger", "disaster", "seized", "water", "plumbing", "electrical",
            "wiring", "off-grid", "shower", "tiny home", "van life", "timber", "framing",
        ];

        // Narrative tropes, POV tension markers, and irreversible consequences
        let narrative_tokens = [
            "enemies to lovers", "he fell first", "moral gray", "morally gray",
            "touch her and you", "fake dating", "forced proximity", "villain gets the girl",
            "grumpy x sunshine", "who did this to you",
            "pov:", "pov ", "when the villain", "the villain", "villain",
            "betrayal", "traitor", "secret", "sacrifice", "curse", "blade", "dagger",
            "dark romance", "burn the world", "burn it all", "kill", "death",
            "die", "deadly", "destroy", "ruin", "execution", "blood", "vengeance",
            "revenge", "war", "poison",
        ];

        let generic_vlog_tokens = [
            "rough week", "day in the life", "vlog", "catch up", "hanging out", "chill",
            "relaxing", "update", "as two females", "girls build", "q&a",
        ];

        let mut points = 50.0f64;

        for token in &high_stakes_tokens {
            if combined.contains(token) {
                points += 12.5;
            }
        }

        for token in &narrative_tokens {
            if combined.contains(token) {
                points += 15.0;
            }
        }

        for token in &generic_vlog_tokens {
            if combined.contains(token) {
                points -= 10.0;
            }
        }

        // Bonus for explicit numbers / measurements in title
        if title.chars().any(|c| c.is_ascii_digit()) {
            points += 10.0;
        }

        let score = points.clamp(10.0, 100.0);
        let (status, fix, lift) = match effective_tone {
            NicheToneProfile::NarrativeEmotionalPitch => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong narrative stakes: high-tension trope framing, character conflict, or irreversible consequences detected.".to_string(),
                        0.0,
                    )
                } else if score >= 70.0 {
                    (
                        VectorStatus::Optimal,
                        "Dramatic tension is present; heighten emotional stakes with an explicit trope juxtaposition (e.g. 'enemies to lovers') or high-consequence dilemma.".to_string(),
                        4.0,
                    )
                } else {
                    let status = if score >= 40.0 {
                        VectorStatus::MarginalDrag
                    } else {
                        VectorStatus::CriticalFriction
                    };
                    let lift = if score >= 40.0 { 8.0 } else { 15.0 };
                    (
                        status,
                        "Title relies on passive entity naming; add high-stakes trope modifiers, character status inversions, or POV conflict hooks (e.g., 'enemies to lovers', 'when the villain', 'moral gray') to lift CTR.".to_string(),
                        lift,
                    )
                }
            }
            NicheToneProfile::DocumentaryBuildProcess => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong physical stakes: title communicates tangible scale, machinery, or engineering jeopardy.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Stakes are clear; consider adding an explicit number, weight, or cost to maximize CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Vlog tilt detected: swap emotional labels (e.g. 'rough week') for physical milestones or obstacle nouns.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Zero physical stakes: title sounds like a passive personal diary. Reframe around conflict, weight, or machinery.".to_string(),
                        15.0,
                    )
                }
            }
            NicheToneProfile::EducationalBreakdown => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong educational problem/solution framing: communicates clear technical outcome, mistake avoidance, or skill unlock.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Educational problem/solution framing is clear; add concrete metric or urgency to maximize CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Title lacks curiosity gap or concrete outcome; frame around a specific problem, mistake, or technical payoff.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Passive educational title: reframe around a high-stakes mistake, architectural decision, or concrete capability.".to_string(),
                        15.0,
                    )
                }
            }
            NicheToneProfile::AestheticLifestyleCozy => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong aspirational craft framing: evocative sensory destination or cozy milestone.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Lifestyle framing is clear; add a specific aesthetic milestone or sensory payoff to lift CTR.".to_string(),
                        4.0,
                    )
                } else {
                    let status = if score >= 40.0 {
                        VectorStatus::MarginalDrag
                    } else {
                        VectorStatus::CriticalFriction
                    };
                    let lift = if score >= 40.0 { 8.0 } else { 15.0 };
                    (
                        status,
                        "Title lacks curiosity or aspirational hook; frame around a specific lifestyle milestone, transformation, or scenic journey.".to_string(),
                        lift,
                    )
                }
            }
            NicheToneProfile::HighStakesPanic => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong physical stakes: title communicates tangible scale, machinery, or engineering jeopardy.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Stakes are clear; consider adding an explicit number, weight, or cost to maximize CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Vlog tilt detected: swap emotional labels (e.g. 'rough week') for physical milestones or obstacle nouns.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Zero physical stakes: title sounds like a passive personal diary. Reframe around conflict, weight, or machinery.".to_string(),
                        15.0,
                    )
                }
            }
            NicheToneProfile::EntertainmentSpectacle => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Elite spectacle framing: title communicates extreme contrast, high-stakes competition, or irresistible curiosity gap.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Spectacle stakes are clear; consider heightening the numerical contrast or physical scale to maximize CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Title lacks extreme contrast or high-stakes challenge hook; introduce clear stakes or comparative scale.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Zero spectacle stakes: title fails to communicate challenge or reward scale.".to_string(),
                        15.0,
                    )
                }
            }
            NicheToneProfile::ConsumerReviewExplainer => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Strong consumer review framing: clear product clarity, buyer warning, or financial stakes detected.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Consumer advisory stakes are clear; consider adding an explicit price anchor or ownership duration to maximize CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Generic product title: add specific cautionary trigger ('don't buy', 'buyer beware', 'the real cost') to heighten decision stakes.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Passive product mention without buyer stakes. Reframe with an explicit warning, financial risk, or long-term verdict.".to_string(),
                        15.0,
                    )
                }
            }
            NicheToneProfile::ConversationalPodcast => {
                if score >= 80.0 {
                    (
                        VectorStatus::Elite,
                        "Elite conversational packaging: compelling quote framing, high-stakes debate angle, or insider disclosure.".to_string(),
                        0.0,
                    )
                } else if score >= 60.0 {
                    (
                        VectorStatus::Optimal,
                        "Podcast topic framing is clear; consider adding a provocative quote or specific guest tension to heighten CTR.".to_string(),
                        4.0,
                    )
                } else if score >= 40.0 {
                    (
                        VectorStatus::MarginalDrag,
                        "Generic episode title: introduce a curiosity gap, controversial debate topic, or direct quote from the guest.".to_string(),
                        8.0,
                    )
                } else {
                    (
                        VectorStatus::CriticalFriction,
                        "Passive episode label without conversation stakes. Reframe around the central conflict, revelation, or debate of the episode.".to_string(),
                        15.0,
                    )
                }
            }
        };

        TelemetryVector {
            name: "Physical Stakes & Framing Lexicon".to_string(),
            measured_value: score,
            target_benchmark: 70.0,
            unit: "score".to_string(),
            score_0_to_100: score,
            status,
            one_percent_fix: fix,
            estimated_retention_lift_pct: lift,
        }
    }

    /// Vector 6: Packaging & Visual Synergy Evaluator incorporating ThumbnailForensicReport.
    pub fn evaluate_title_stakes_with_thumbnail(
        title: &str,
        hook_text: Option<&str>,
        tone: Option<NicheToneProfile>,
        thumbnail_report: Option<&ThumbnailForensicReport>,
        early_drop_rate: Option<f64>,
    ) -> TelemetryVector {
        let mut base_vec = Self::evaluate_stakes_lexicon_with_tone(title, hook_text, tone);

        if let Some(thumb) = thumbnail_report {
            let is_clean_authentic = thumb.primary_archetype == VisualClickbaitArchetype::CleanAuthentic
                || thumb.packaging_integrity_index >= 0.80;

            let is_suggestive = thumb.suggestive_metadata_score >= 0.70;
            let is_deceptive = thumb.primary_archetype.is_deceptive();
            let is_exogenous = (is_suggestive || is_deceptive) && !is_clean_authentic;

            let exogenous_badge = if is_suggestive && !is_clean_authentic {
                "[SUGGESTIVE PACKAGING]"
            } else if is_deceptive && !is_clean_authentic {
                "[DECEPTIVE CLICKBAIT]"
            } else {
                thumb.primary_archetype.badge()
            };

            // Expectation Mismatch check: steep early retention drop (e.g. <= -3.0%/s or |drop| >= 3.0)
            // If primary_archetype == VisualClickbaitArchetype::CleanAuthentic (or packaging_integrity_index >= 0.80),
            // strictly forbid applying a deceptive clickbait penalty, even if retention slope has a dip.
            let severe_early_drop = early_drop_rate.map(|r| r.abs() >= 3.0).unwrap_or(false);
            let expectation_mismatch = !is_clean_authentic
                && (is_suggestive || is_deceptive || thumb.packaging_integrity_index < 0.50)
                && severe_early_drop;

            if expectation_mismatch {
                // Apply -15 pts penalty on Vector 6
                let penalized_score = (base_vec.score_0_to_100 - 15.0).clamp(10.0, 100.0);
                base_vec.score_0_to_100 = penalized_score;
                base_vec.measured_value = penalized_score;
                let drop_val = early_drop_rate.map(|r| r.abs()).unwrap_or(3.0);
                base_vec.status = if penalized_score >= 80.0 {
                    VectorStatus::Elite
                } else if penalized_score >= 60.0 {
                    VectorStatus::Optimal
                } else if penalized_score >= 40.0 {
                    VectorStatus::MarginalDrag
                } else {
                    VectorStatus::CriticalFriction
                };
                if base_vec.status == VectorStatus::Optimal || base_vec.status == VectorStatus::Elite {
                    base_vec.status = VectorStatus::MarginalDrag;
                }
                base_vec.one_percent_fix = format!(
                    "Expectation Mismatch (-15 pts): High clickbait packaging ({}) clashes with steep early retention bleed (-{:.1}%/s). Align frame-one visual payoff with thumbnail hook promise to prevent immediate abandonment. {}",
                    exogenous_badge, drop_val, base_vec.one_percent_fix
                );
                base_vec.estimated_retention_lift_pct = (base_vec.estimated_retention_lift_pct + 5.0).max(8.0);
            } else if is_exogenous {
                let note = if is_suggestive {
                    format!("Exogenous CTR Driver ({} | Score: {:.2}): Viewership heavily CTR-frontloaded via provocative packaging; content kinematics lag behind click promise.", exogenous_badge, thumb.suggestive_metadata_score)
                } else {
                    format!("Exogenous CTR Driver ({} | Modifier: {:.2}x): Deceptive visual archetype detected; ensure early frames resolve clickbait premise.", exogenous_badge, thumb.exogenous_ctr_modifier)
                };
                base_vec.one_percent_fix = format!("{} {}", note, base_vec.one_percent_fix);
            }
        }

        base_vec
    }

    /// Vector 6: Packaging & Visual Synergy Evaluator incorporating ThumbnailForensicReport and PromiseGapReport.
    pub fn evaluate_title_stakes_with_thumbnail_and_gap(
        title: &str,
        hook_text: Option<&str>,
        tone: Option<NicheToneProfile>,
        thumbnail_report: Option<&ThumbnailForensicReport>,
        early_drop_rate: Option<f64>,
        promise_gap: Option<&PromiseGapReport>,
    ) -> TelemetryVector {
        let mut base_vec = Self::evaluate_title_stakes_with_thumbnail(
            title,
            hook_text,
            tone,
            thumbnail_report,
            early_drop_rate,
        );

        if let Some(gap) = promise_gap {
            // Suppress false PromiseGapMismatch penalties when dialogue begins immediately (premise confirmation <= 0.3s)
            // and verbal handshake passed, or when captions are pending
            let suppress_penalty = (gap.premise_confirmation_second.map(|s| s <= 0.3).unwrap_or(false) && gap.verbal_handshake_passed)
                || gap.verbal_handshake_detail.contains("captions pending")
                || gap.verbal_handshake_detail.contains("caption processing");

            if gap.is_breach && !suppress_penalty {
                let penalized_score = (base_vec.score_0_to_100 - 15.0).clamp(10.0, 100.0);
                base_vec.score_0_to_100 = penalized_score;
                base_vec.status = if penalized_score >= 80.0 {
                    VectorStatus::Elite
                } else if penalized_score >= 60.0 {
                    VectorStatus::Optimal
                } else if penalized_score >= 40.0 {
                    VectorStatus::MarginalDrag
                } else {
                    VectorStatus::CriticalFriction
                };
                if base_vec.status == VectorStatus::Optimal || base_vec.status == VectorStatus::Elite {
                    base_vec.status = VectorStatus::MarginalDrag;
                }
                base_vec.one_percent_fix = format!(
                    "PromiseGapMismatch (-15 pts): {} {}",
                    PromiseGapReport::BREACH_ALERT,
                    base_vec.one_percent_fix
                );
                base_vec.estimated_retention_lift_pct = (base_vec.estimated_retention_lift_pct + 5.0).max(8.0);
            }
        }

        base_vec
    }

    /// Alias for evaluate_title_stakes_with_thumbnail.
    pub fn evaluate_packaging_with_thumbnail(
        title: &str,
        hook_text: Option<&str>,
        tone: Option<NicheToneProfile>,
        thumbnail_report: Option<&ThumbnailForensicReport>,
        early_drop_rate: Option<f64>,
    ) -> TelemetryVector {
        Self::evaluate_title_stakes_with_thumbnail(title, hook_text, tone, thumbnail_report, early_drop_rate)
    }

    /// Synthesizes the full 6-point Brailsford Scorecard and selects the Top 3 "1% Tweaks".
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        t_silence_sec: f64,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        active_wpm: f64,
        hook_text: Option<&str>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_with_retention(
            target_id,
            title,
            niche_profile,
            t_silence_sec,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            None,
            active_wpm,
            hook_text,
        )
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with explicit retention curve drop rate.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_with_retention(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        t_silence_sec: f64,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_full(
            target_id,
            title,
            niche_profile,
            t_silence_sec,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            steepest_drop_rate_pct_per_sec,
            None,
            active_wpm,
            hook_text,
            None,
        )
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with Series Reliability, SPOF,
    /// drop cliff timestamps, viewer satisfaction index, and tone profile expectation alignment.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_with_tone(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        tone: Option<NicheToneProfile>,
        t_silence_sec: f64,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
        satisfaction: Option<ViewerSatisfactionMetrics>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_with_tone_and_ocr(
            target_id,
            title,
            niche_profile,
            tone,
            t_silence_sec,
            None,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            steepest_drop_rate_pct_per_sec,
            worst_drop_timestamp_sec,
            active_wpm,
            hook_text,
            satisfaction,
        )
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with OCR Text Hook awareness,
    /// Series Reliability, SPOF, drop cliff timestamps, viewer satisfaction index, and tone profile alignment.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_with_tone_and_ocr(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        tone: Option<NicheToneProfile>,
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
        satisfaction: Option<ViewerSatisfactionMetrics>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_with_thumbnail(
            target_id,
            title,
            niche_profile,
            tone,
            t_silence_sec,
            ocr_text_hook,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            steepest_drop_rate_pct_per_sec,
            worst_drop_timestamp_sec,
            active_wpm,
            hook_text,
            satisfaction,
            None,
        )
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with Two-Part Handshake & Promise Gap awareness,
    /// Thumbnail Forensics, OCR Text Hook awareness, Series Reliability, SPOF, drop cliff timestamps,
    /// viewer satisfaction index, and tone profile alignment.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_with_handshake(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        tone: Option<NicheToneProfile>,
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
        satisfaction: Option<ViewerSatisfactionMetrics>,
        thumbnail_report: Option<ThumbnailForensicReport>,
        promise_gap: Option<PromiseGapReport>,
    ) -> MarginalGainsScorecard {
        let active_tone = tone.unwrap_or_else(|| NicheToneProfile::detect(title, niche_profile, ""));
        let audio_first = Self::is_audio_first(title, &format!("{} {}", niche_profile, hook_text.unwrap_or("")));
        let v1 = Self::evaluate_cold_open_with_promise_gap(
            t_silence_sec,
            ocr_text_hook,
            Some(active_tone),
            audio_first,
            promise_gap.as_ref(),
        );
        let v2 = Self::evaluate_visual_rhythm_with_context(asl_60s, cuts_60s, active_tone, audio_first);
        let v3 = Self::evaluate_static_hold_with_context(longest_shot_60s, active_tone, audio_first);
        let v4 = Self::evaluate_retention_stability_with_timestamp(
            steepest_drop_rate_pct_per_sec,
            worst_drop_timestamp_sec,
        );
        let mut v5 = Self::evaluate_audio_density_with_tone_and_context(active_wpm, audio_first, Some(active_tone));
        if hook_text.is_some_and(|h| h.contains("captions pending")) && active_wpm > 0.0 {
            v5.one_percent_fix = format!(
                "Natural conversational cadence (~{:.0} WPM estimated from acoustic energy; captions pending)—no vocal changes needed; keep natural breathing rhythm.",
                active_wpm
            );
        }
        let v6 = Self::evaluate_title_stakes_with_thumbnail_and_gap(
            title,
            hook_text,
            Some(active_tone),
            thumbnail_report.as_ref(),
            steepest_drop_rate_pct_per_sec,
            promise_gap.as_ref(),
        );

        let vectors = vec![v1, v2, v3, v4, v5, v6];

        // 1. Composite Weighted Score (Brailsford 0..100)
        // Weights: Cold Open (0.15), Visual Rhythm (0.20), Static Hold (0.20), Retention Stability (0.15), Audio Density (0.15), Lexicon (0.15)
        let weights = [0.15, 0.20, 0.20, 0.15, 0.15, 0.15];
        let mut composite_score = 0.0;
        for (i, vec) in vectors.iter().enumerate() {
            composite_score += vec.score_0_to_100 * weights[i];
        }
        let composite_brailsford_score = (composite_score * 10.0).round() / 10.0;

        // 2. Series System Reliability (Weakest-Link Weighted Geometric Mean)
        let mut series_prob_product = 1.0f64;
        let mut lowest_vector: Option<&TelemetryVector> = None;

        for vec in &vectors {
            let p_i = (vec.score_0_to_100 / 100.0).clamp(0.15, 1.0);
            series_prob_product *= p_i;

            match lowest_vector {
                None => lowest_vector = Some(vec),
                Some(curr_min) if vec.score_0_to_100 < curr_min.score_0_to_100 => {
                    lowest_vector = Some(vec);
                }
                _ => {}
            }
        }
        let geom_mean = series_prob_product.powf(1.0 / vectors.len() as f64) * 100.0;
        let min_score = lowest_vector.map(|v| v.score_0_to_100).unwrap_or(100.0);
        let series_system_reliability_pct = (((0.5 * geom_mean + 0.5 * min_score).clamp(0.0, 100.0)) * 10.0).round() / 10.0;

        // 3. Single Point of Failure (SPOF): Lowest vector if score < 70.0
        let single_point_of_failure = match lowest_vector {
            Some(v) if v.score_0_to_100 < 70.0 => Some(format!(
                "{} @ {:.0}/100 — bottlenecking total series reliability",
                v.human_label(), v.score_0_to_100
            )),
            _ => None,
        };

        // 4. Select Top 3 Marginal Gains (highest estimated lift percentage, ONLY lift > 0.0%)
        let mut candidates = vectors.clone();
        candidates.retain(|v| v.estimated_retention_lift_pct > 0.0);
        candidates.sort_by(|a, b| {
            b.estimated_retention_lift_pct
                .partial_cmp(&a.estimated_retention_lift_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(3);

        // Compounded multiplicative lift: Product(1 + lift_i)
        let mut multiplier_prod = 1.0f64;
        for vec in &candidates {
            multiplier_prod *= 1.0 + (vec.estimated_retention_lift_pct / 100.0);
        }
        let predicted_multiplier_lift = ((multiplier_prod - 1.0) * 100.0 * 10.0).round() / 10.0;

        // 5. Series Funnel Insight for sequel episodes (#2+) with high satisfaction
        let series_funnel_insight = if let Some(ep_num) = Self::detect_series_episode_number(title) {
            if ep_num >= 2 {
                if let Some(sat) = &satisfaction {
                    if sat.comments_per_1k_views >= 0.8 || sat.like_to_view_pct >= 2.5 {
                        Some(format!(
                            "High-Retention Sequel (Episode #{ep_num}): Lower raw views are normal multi-part course funnel decay, but Viewer Satisfaction is elite ({:.2} comments/1k)—retitle as a standalone problem without '#{ep_num}' to unlock search traffic.",
                            sat.comments_per_1k_views
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        MarginalGainsScorecard {
            target_id: target_id.to_string(),
            title: title.to_string(),
            niche_profile: niche_profile.to_string(),
            tone_profile: tone,
            composite_brailsford_score,
            series_system_reliability_pct,
            single_point_of_failure,
            satisfaction_index: satisfaction,
            predicted_multiplier_lift,
            vectors,
            top_3_marginal_gains: candidates,
            series_funnel_insight,
            outro_exit_note: None,
            duration_seconds: None,
            is_local_draft: false,
            thumbnail_report,
            promise_gap_report: promise_gap,
            actual_spoken_hook: hook_text.map(|s| s.to_string()),
            loudness_metrics: None,
            cuttable_fat: None,
            comment_clusters: Vec::new(),
            suitability_report: None,
            shorts_metrics: None,
            re_engagement_checkpoints: Vec::new(),
            stair_stepping: None,
            outro_report: None,
            retention_curve: Vec::new(),
            attention_breaches: Vec::new(),
        }
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with Thumbnail Forensics, OCR Text Hook awareness,
    /// Series Reliability, SPOF, drop cliff timestamps, viewer satisfaction index, and tone profile alignment.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_with_thumbnail(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        tone: Option<NicheToneProfile>,
        t_silence_sec: f64,
        ocr_text_hook: Option<&str>,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
        satisfaction: Option<ViewerSatisfactionMetrics>,
        thumbnail_report: Option<ThumbnailForensicReport>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_with_handshake(
            target_id,
            title,
            niche_profile,
            tone,
            t_silence_sec,
            ocr_text_hook,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            steepest_drop_rate_pct_per_sec,
            worst_drop_timestamp_sec,
            active_wpm,
            hook_text,
            satisfaction,
            thumbnail_report,
            None,
        )
    }

    /// Extracts sequential episode number from titles (e.g. "#2", "Ep 2", "Part 2", "Episode 3").
    pub fn detect_series_episode_number(title: &str) -> Option<u32> {
        let lower = title.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let len = chars.len();

        for (i, &ch) in chars.iter().enumerate() {
            if ch == '#' {
                let mut j = i + 1;
                while j < len && chars[j].is_whitespace() {
                    j += 1;
                }
                let start_digits = j;
                while j < len && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start_digits {
                    let slice: String = chars[start_digits..j].iter().collect();
                    if let Ok(num) = slice.parse::<u32>() {
                        return Some(num);
                    }
                }
            }
        }

        let words: Vec<&str> = lower.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            if clean == "ep" || clean == "episode" || clean == "part" || clean == "pt" {
                if let Some(next_word) = words.get(i + 1) {
                    let next_clean = next_word.trim_matches(|c: char| !c.is_ascii_digit());
                    if let Ok(num) = next_clean.parse::<u32>() {
                        return Some(num);
                    }
                }
            }
        }

        None
    }

    /// Synthesizes the full 6-point Brailsford Scorecard with Series Reliability, SPOF,
    /// drop cliff timestamps, and viewer satisfaction index (defaults to legacy profile).
    #[allow(clippy::too_many_arguments)]
    pub fn generate_scorecard_full(
        target_id: &str,
        title: &str,
        niche_profile: &str,
        t_silence_sec: f64,
        asl_60s: f64,
        cuts_60s: usize,
        longest_shot_60s: f64,
        steepest_drop_rate_pct_per_sec: Option<f64>,
        worst_drop_timestamp_sec: Option<f64>,
        active_wpm: f64,
        hook_text: Option<&str>,
        satisfaction: Option<ViewerSatisfactionMetrics>,
    ) -> MarginalGainsScorecard {
        Self::generate_scorecard_with_tone(
            target_id,
            title,
            niche_profile,
            None,
            t_silence_sec,
            asl_60s,
            cuts_60s,
            longest_shot_60s,
            steepest_drop_rate_pct_per_sec,
            worst_drop_timestamp_sec,
            active_wpm,
            hook_text,
            satisfaction,
        )
    }

    /// Renders a beautiful creator markdown report for the Brailsford Scorecard.
    pub fn render_markdown_scorecard(sc: &MarginalGainsScorecard) -> String {
        let mut out = String::new();
        out.push_str("# 🚴 The Brailsford Marginal Gains Scorecard\n\n");
        out.push_str(&format!("> **Video**: **\"{}\"**\n> **Channel / Creator**: `{}` | Target ID: `{}`\n", sc.title, sc.niche_profile, sc.target_id));
        if let Some(tone) = sc.tone_profile {
            out.push_str(&format!("> **Tone Expectation**: `{}` | **Composite Score**: **{:.1} / 100**\n", tone.label(), sc.composite_brailsford_score));
        } else {
            out.push_str(&format!("> **Composite Score**: **{:.1} / 100**\n", sc.composite_brailsford_score));
        }
        out.push_str(&format!("> **Overall Video Health (Weakest-Link Grade)**: **{:.1}%**\n", sc.series_system_reliability_pct));

        if let Some(spof) = &sc.single_point_of_failure {
            out.push_str(&format!("> 🚨 **#1 Bottleneck Holding This Video Back**: **{}**\n", spof));
        } else {
            out.push_str("> ✅ **#1 Bottleneck Holding This Video Back**: None detected (all vectors ≥ 70/100)\n");
        }

        if let Some(sat) = &sc.satisfaction_index {
            out.push_str(&format!(
                "> **Viewer Satisfaction**: **{}** | Likes/Views: **{:.2}%** | Comments/1k Views: **{:.2}**\n",
                sat.satisfaction_tier, sat.like_to_view_pct, sat.comments_per_1k_views
            ));
        }

        if let Some(insight) = &sc.series_funnel_insight {
            out.push_str(&format!("> 💡 **Series Funnel Insight**: {}\n", insight));
        }

        if let Some(outro) = &sc.outro_exit_note {
            out.push_str(&format!("> 🚪 **Outro Drop Note**: {}\n", outro));
        }

        out.push_str(&format!("> **Compounded Marginal ROI**: **+{:.1}% Viewership Lift** across the Top {} micro-tweaks.\n\n", sc.predicted_multiplier_lift, sc.top_3_marginal_gains.len()));

        out.push_str("### 📊 6-Point Telemetry Vector Audit\n\n");
        out.push_str("| Telemetry Vector | Measured | Editor Target | Score | Status | 1% Actionable Tweak | Est. Lift |\n");
        out.push_str("|:---|:---:|:---:|:---:|:---|:---|:---:|\n");

        for v in &sc.vectors {
            out.push_str(&format!(
                "| **{}** | {:.1}{} | {} | {:.0} | {} | {} | **+{:.1}%** |\n",
                v.human_label(),
                v.measured_value, v.unit,
                v.editor_target(),
                v.score_0_to_100,
                v.status.badge(),
                v.one_percent_fix,
                v.estimated_retention_lift_pct
            ));
        }

        let tweak_count = sc.top_3_marginal_gains.len();
        if tweak_count == 0 {
            out.push_str("\n### 🏆 Top Marginal Tweaks (Highest ROI View-Multiplier Lift)\n\n");
            out.push_str("*All 6 telemetry vectors are performing at Elite/Optimal benchmarks; zero friction points detected.*\n");
        } else {
            let header_title = if tweak_count == 3 {
                "Top 3 \"1% Tweaks\"".to_string()
            } else if tweak_count == 1 {
                "Top 1 \"1% Tweak\"".to_string()
            } else {
                format!("Top {} \"1% Tweaks\"", tweak_count)
            };
            out.push_str(&format!("\n### 🏆 {} (Highest ROI View-Multiplier Lift)\n\n", header_title));
            for (idx, tweak) in sc.top_3_marginal_gains.iter().enumerate() {
                out.push_str(&format!(
                    "{}. **{}** (+{:.1}% lift)\n   - *Action*: {}\n   - *Current Metric*: {:.1}{} (Target: {})\n\n",
                    idx + 1,
                    tweak.human_label(),
                    tweak.estimated_retention_lift_pct,
                    tweak.one_percent_fix,
                    tweak.measured_value, tweak.unit,
                    tweak.editor_target()
                ));
            }
        }

        if let Some(suit) = &sc.suitability_report {
            out.push_str("\n### 🛡️ Monetization & Ad-Suitability Pre-Flight Gate\n\n");
            let badge = if suit.rating == "GREEN" {
                "🟢 **MONETIZATION SAFE (Full Ad Distribution / Immediate Public Scheduling)**"
            } else {
                "🟡 **ADVERTISER RISK (Limited Ads / Unlisted 24h Review Recommended)**"
            };
            out.push_str(&format!("- **Overall Rating**: {}\n", badge));
            out.push_str(&format!("- **Workflow Directive**: `{}`\n", suit.recommended_workflow));
            out.push_str(&format!("- **Hook Window (0:00–0:15)**: {}\n", if suit.hook_window_safe { "✅ Safe (0 flags)" } else { "🚨 Violations detected!" }));
            out.push_str(&format!("- **Total Violations**: {}\n", suit.violations_count));
            out.push_str(&format!("- **Summary**: {}\n", suit.executive_summary));
            if !suit.violations.is_empty() {
                out.push_str("\n| Timestamp | Category | Flagged Token | Context | Severity |\n");
                out.push_str("|:---:|:---|:---|:---|:---:|\n");
                for v in &suit.violations {
                    out.push_str(&format!(
                        "| `{:.1}s` | {} | `{}` | \"{}\" | {:?} |\n",
                        v.timestamp_sec, v.category.label(), v.token, v.context, v.severity
                    ));
                }
            }
            out.push('\n');
        }

        if let Some(shorts) = &sc.shorts_metrics {
            out.push_str("\n### 📱 YouTube Shorts Mobile Retention Radar\n\n");
            let hook_tag = if shorts.first_frame_hook_safe { "✅ Safe (≤ 1.5s)" } else { "⚠️ High Swipe-Away Risk (> 1.5s)" };
            let loop_tag = if shorts.loop_seamless { "🟢 Seamless Infinite Loop" } else { "⚠️ Abrupt Outro Cutoff" };
            out.push_str(&format!("- **First-Frame Hook Delay**: {:.2}s ({})\n", shorts.first_frame_hook_delay, hook_tag));
            out.push_str(&format!("- **Duration Category**: {:?} ({:.1}s)\n", shorts.duration_safety_status, shorts.duration_seconds));
            out.push_str(&format!("- **Loop Continuity Score**: {:.1} / 100 ({})\n", shorts.loop_continuity_score, loop_tag));
            out.push_str(&format!("- **Stay vs Swipe Risk**: {} (Est. Stay Ratio: {:.1}%)\n", shorts.stay_vs_swipe_risk, shorts.estimated_stay_ratio_pct));
            out.push_str(&format!("- **Loop Bridge Insight**: {}\n", shorts.loop_bridge_insight));
            out.push_str(&format!("- **Executive Action**: {}\n\n", shorts.executive_summary));
        }

        out
    }

    /// Aggregates breath pauses (>0.4s), static camera holds exceeding niche thresholds,
    /// and pre-speech cold-open lag into an exact editorial runtime shaving budget.
    pub fn calculate_cuttable_fat(
        total_duration_sec: f64,
        t_silence_sec: f64,
        tone: Option<NicheToneProfile>,
        cuts: &[f64],
        breath_pauses: &[f64],
    ) -> CuttableFatReport {
        // 1. Cold-open lag (excess above 0.3s)
        let cold_open_lag_seconds = if t_silence_sec > 0.4 {
            ((t_silence_sec - 0.3) * 10.0).round() / 10.0
        } else {
            0.0
        };

        // 2. Dead air breath pauses (pauses > 0.4s trimmed to 0.25s, or > 3.0s trimmed to 1.5s for podcasts)
        let pause_threshold = if tone == Some(NicheToneProfile::ConversationalPodcast) {
            3.0
        } else {
            0.4
        };
        let trim_target = if tone == Some(NicheToneProfile::ConversationalPodcast) {
            1.5
        } else {
            0.25
        };
        let mut dead_air_breath_seconds = 0.0;
        let mut breath_edits_count = 0;
        for &pause in breath_pauses {
            if pause > pause_threshold {
                dead_air_breath_seconds += (pause - trim_target).max(0.0);
                breath_edits_count += 1;
            }
        }
        dead_air_breath_seconds = (dead_air_breath_seconds * 10.0).round() / 10.0;

        // 3. Camera holds & 115% Punch-In cues
        let hold_benchmark = match tone {
            Some(NicheToneProfile::HighStakesPanic) | Some(NicheToneProfile::NarrativeEmotionalPitch) | Some(NicheToneProfile::ConsumerReviewExplainer) => 7.0,
            Some(NicheToneProfile::DocumentaryBuildProcess) => 12.0,
            Some(NicheToneProfile::AestheticLifestyleCozy) | Some(NicheToneProfile::EducationalBreakdown) => 14.0,
            Some(NicheToneProfile::EntertainmentSpectacle) => 15.0,
            Some(NicheToneProfile::ConversationalPodcast) => 45.0,
            None => 8.0,
        };

        let mut punch_in_cues = Vec::new();
        let mut camera_hold_fat_seconds = 0.0;
        let mut streaming_dropouts_count = 0;

        let mut sorted_cuts: Vec<f64> = cuts.iter().copied().filter(|&c| c > 0.05 && c < total_duration_sec).collect();
        sorted_cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted_cuts.dedup();

        let mut prev_cut = 0.0;
        let mut shot_idx = 1;

        let mut evaluate_shot = |start: f64, end: f64, s_idx: usize| {
            let dur = (end - start).max(0.0);
            if dur > 120.0 {
                // Streaming Cut-Detection Dropout: on an edited YouTube video, an unbroken interval > 120s
                // indicates FFmpeg dropped video frames during scene detection rather than a true single-take hold.
                streaming_dropouts_count += 1;
                // Clamp hold fat contribution to a reasonable ceiling (at most 20s total take, i.e. 20.0 - hold_benchmark excess)
                let clamped_dur = 20.0f64;
                if clamped_dur > hold_benchmark {
                    camera_hold_fat_seconds += clamped_dur - hold_benchmark;
                }
                // Suppress punch-in cues across dropped frames
            } else if dur > hold_benchmark {
                let excess = dur - hold_benchmark;
                camera_hold_fat_seconds += excess;

                if tone == Some(NicheToneProfile::ConversationalPodcast) {
                    // For conversational podcasts, unbroken takes exceeding 45.0s get a midpoint punch-in
                    if dur <= 60.0 {
                        let midpoint = start + (dur / 2.0);
                        let mm = (midpoint / 60.0).floor() as u64;
                        let ss = (midpoint % 60.0).floor() as u64;
                        let tc = format!("{:02}:{:02}", mm, ss);
                        punch_in_cues.push(PunchInCue {
                            timecode: tc,
                            timecode_seconds: (midpoint * 10.0).round() / 10.0,
                            hold_duration_seconds: (dur * 10.0).round() / 10.0,
                            crop_scale_pct: 115,
                            edit_instruction: format!(
                                "Shot {:02} static hold ({:.1}s exceeds {:.1}s limit). Apply 115% digital punch-in or cut to alternate angle at midpoint to reset visual decay.",
                                s_idx, dur, hold_benchmark
                            ),
                        });
                    } else {
                        let mut cue_time = start + 30.0;
                        let mut p_idx = 1;
                        while cue_time < end - 5.0 {
                            let mm = (cue_time / 60.0).floor() as u64;
                            let ss = (cue_time % 60.0).floor() as u64;
                            let tc = format!("{:02}:{:02}", mm, ss);
                            punch_in_cues.push(PunchInCue {
                                timecode: tc,
                                timecode_seconds: (cue_time * 10.0).round() / 10.0,
                                hold_duration_seconds: (dur * 10.0).round() / 10.0,
                                crop_scale_pct: 115,
                                edit_instruction: format!(
                                    "Shot {:02} extended interview hold cue #{} ({:.1}s hold). Apply 115% digital punch-in to reset visual decay (30s cadence).",
                                    s_idx, p_idx, dur
                                ),
                            });
                            cue_time += 30.0;
                            p_idx += 1;
                        }
                    }
                } else if dur <= 25.0 {
                    let midpoint = start + (dur / 2.0);
                    let mm = (midpoint / 60.0).floor() as u64;
                    let ss = (midpoint % 60.0).floor() as u64;
                    let tc = format!("{:02}:{:02}", mm, ss);
                    punch_in_cues.push(PunchInCue {
                        timecode: tc,
                        timecode_seconds: (midpoint * 10.0).round() / 10.0,
                        hold_duration_seconds: (dur * 10.0).round() / 10.0,
                        crop_scale_pct: 115,
                        edit_instruction: format!(
                            "Shot {:02} static hold ({:.1}s exceeds {:.1}s limit). Apply 115% digital punch-in or cut to alternate angle at midpoint to reset visual decay.",
                            s_idx, dur, hold_benchmark
                        ),
                    });
                } else {
                    let mut cue_time = start + 18.0;
                    let mut p_idx = 1;
                    while cue_time < end - 4.0 {
                        let mm = (cue_time / 60.0).floor() as u64;
                        let ss = (cue_time % 60.0).floor() as u64;
                        let tc = format!("{:02}:{:02}", mm, ss);
                        punch_in_cues.push(PunchInCue {
                            timecode: tc,
                            timecode_seconds: (cue_time * 10.0).round() / 10.0,
                            hold_duration_seconds: (dur * 10.0).round() / 10.0,
                            crop_scale_pct: 115,
                            edit_instruction: format!(
                                "Shot {:02} extended monologue cue #{} ({:.1}s hold). Apply 115% digital punch-in to reset visual decay (18s cadence).",
                                s_idx, p_idx, dur
                            ),
                        });
                        cue_time += 18.0;
                        p_idx += 1;
                    }
                }
            }
        };

        for &c in &sorted_cuts {
            evaluate_shot(prev_cut, c, shot_idx);
            prev_cut = c;
            shot_idx += 1;
        }
        if prev_cut < total_duration_sec {
            evaluate_shot(prev_cut, total_duration_sec, shot_idx);
        }

        camera_hold_fat_seconds = (camera_hold_fat_seconds * 10.0).round() / 10.0;

        // Cumulative cuttable seconds (combining breath pauses, pre-speech lag, and partial hold trimming)
        let total_cuttable_seconds = ((cold_open_lag_seconds + dead_air_breath_seconds + (camera_hold_fat_seconds * 0.4)) * 10.0).round() / 10.0;

        let micro_edits_count = breath_edits_count
            + (if cold_open_lag_seconds > 0.0 { 1 } else { 0 })
            + punch_in_cues.len();

        let format_runtime = |sec: f64| -> String {
            let total = sec.max(0.0).round() as u64;
            let m = total / 60;
            let s = total % 60;
            format!("{:02}:{:02}", m, s)
        };

        let format_runtime_spoken = |sec: f64| -> String {
            let total = sec.max(0.0).round() as u64;
            let m = total / 60;
            let s = total % 60;
            if m > 0 {
                format!("{}m {:02}s", m, s)
            } else {
                format!("{}s", s)
            }
        };

        let original_runtime_formatted = format_runtime(total_duration_sec);
        let streamlined_duration = (total_duration_sec - total_cuttable_seconds).max(0.0);
        let streamlined_runtime_formatted = format_runtime(streamlined_duration);

        let runtime_reduction_pct = if total_duration_sec > 0.0 {
            ((total_cuttable_seconds / total_duration_sec) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        let executive_summary = if total_cuttable_seconds > 0.0 && (micro_edits_count > 0 || streaming_dropouts_count > 0) {
            let dropout_suffix = if streaming_dropouts_count > 0 {
                format!(" (clamped {} streaming cut dropout(s) >120s)", streaming_dropouts_count)
            } else {
                String::new()
            };
            format!(
                "You can shave {} across {} micro-edits ({} -> {}, -{:.1}%){} without deleting story beats.",
                format_runtime_spoken(total_cuttable_seconds),
                micro_edits_count,
                original_runtime_formatted,
                streamlined_runtime_formatted,
                runtime_reduction_pct,
                dropout_suffix
            )
        } else if streaming_dropouts_count > 0 {
            format!("Timeline pacing is lean (clamped {} streaming cut dropout(s) >120s).", streaming_dropouts_count)
        } else {
            "Timeline pacing is lean. No micro-fat detected across breath pauses or camera holds.".to_string()
        };

        CuttableFatReport {
            dead_air_breath_seconds,
            camera_hold_fat_seconds,
            cold_open_lag_seconds,
            total_cuttable_seconds,
            micro_edits_count,
            original_runtime_formatted,
            streamlined_runtime_formatted,
            runtime_reduction_pct,
            punch_in_cues,
            executive_summary,
            streaming_dropouts_count,
        }
    }

    /// Evaluates a target video, automatically detecting whether it is a remote URL or a local video draft on disk.
    pub fn evaluate_target(target: &str) -> Result<MarginalGainsScorecard, Box<dyn std::error::Error + Send + Sync>> {
        if Self::is_local_file(target) {
            Self::evaluate_local_draft(target)
        } else {
            Self::evaluate_video_url(target)
        }
    }

    /// Determines whether the target string is an existing local video file path.
    pub fn is_local_file(target: &str) -> bool {
        let p = std::path::Path::new(target);
        if p.exists() && p.is_file() {
            return true;
        }
        let lower = target.to_lowercase();
        (lower.ends_with(".mp4") || lower.ends_with(".mov") || lower.ends_with(".mkv") || lower.ends_with(".webm") || lower.ends_with(".avi"))
            && p.exists()
    }

    /// Extracts live telemetry for a video URL and synthesizes a complete MarginalGainsScorecard.
    pub fn evaluate_video_url(video_url: &str) -> Result<MarginalGainsScorecard, Box<dyn std::error::Error + Send + Sync>> {
        if Self::is_local_file(video_url) {
            return Self::evaluate_local_draft(video_url);
        }
        let meta = crate::ingest::MediaResolver::extract_with_ytdlp(video_url)?;
        let cues = match &meta.vtt_subtitles {
            Some(vtt) => crate::math::RetentionEngine::parse_vtt(vtt),
            None => Vec::new(),
        };

        let clean_speech_text = |text: &str| -> String {
            let mut cleaned = String::new();
            let mut in_bracket = false;
            for ch in text.chars() {
                if ch == '[' || ch == '(' {
                    in_bracket = true;
                } else if ch == ']' || ch == ')' {
                    in_bracket = false;
                } else if !in_bracket && ch != '♪' {
                    cleaned.push(ch);
                }
            }
            cleaned.trim().to_string()
        };

        let spoken_cues: Vec<(&crate::math::retention::VttCue, String)> = cues
            .iter()
            .filter_map(|c| {
                let cleaned = clean_speech_text(&c.text);
                if cleaned.chars().any(|ch| ch.is_alphanumeric()) {
                    Some((c, cleaned))
                } else {
                    None
                }
            })
            .collect();

        // Distinguish theme song singing / bumper lyrics from creator spoken narrative dialogue
        let first_narrative_cue = spoken_cues
            .iter()
            .find(|(_, text)| !crate::analyzer::VadClassifier::is_theme_song_or_singing_cue(text, Some(&meta.title)))
            .or_else(|| spoken_cues.first());

        let raw_t_silence = first_narrative_cue.map(|(c, _)| c.start_sec).unwrap_or(2.5);

        // Frame-zero raw PCM audio VAD check:
        // In videos like Outdoor Tom (96-Oc0Y2S8k), dialogue begins at 00:00:00.00 ("Nate here with..."),
        // but VTT subtitle cues were offset to 2.5s.
        // Inspect the raw PCM audio energy within the opening window (6.0s). If acoustic energy exceeds the RMS
        // speech threshold at t < 0.3s AND is not a harmonic music track (theme song), lock t_silence = 0.0s (Score: 100 ELITE).
        let opening_pcm = crate::ingest::MediaSceneDetector::extract_audio_pcm(video_url, Some(6.0)).ok();
        let vad = crate::analyzer::VadClassifier::default();
        let t_silence = vad.calibrate_t_silence(opening_pcm.as_deref(), Some(raw_t_silence));

        // Assemble opening spoken hook across rolling VTT subtitle cues starting from the first narrative dialogue
        let narrative_start_sec = first_narrative_cue.map(|(c, _)| c.start_sec).unwrap_or(0.0);
        let mut opening_words = Vec::new();
        for (cue, text) in &spoken_cues {
            if cue.start_sec >= narrative_start_sec {
                if cue.start_sec <= narrative_start_sec + 7.0 || opening_words.len() < 20 {
                    opening_words.push(text.as_str());
                }
                if cue.start_sec > narrative_start_sec + 7.0 && opening_words.len() >= 12 {
                    break;
                }
            }
        }
        let first_spoken_hook = if !opening_words.is_empty() {
            Some(opening_words.join(" "))
        } else {
            first_narrative_cue.map(|(_, text)| text.clone())
        };

        // If VTT subtitles are empty or contain 0 spoken words (e.g. fresh upload where auto-captions
        // have not yet been processed by YouTube), extract a 6.0s PCM audio stream to evaluate speech activity via VAD:
        let speech_pcm = if spoken_cues.is_empty() {
            crate::ingest::MediaSceneDetector::extract_audio_pcm(video_url, Some(6.0)).ok()
        } else {
            opening_pcm.clone()
        };

        let active_wpm = crate::math::RetentionEngine::calculate_active_window_wpm_with_pcm(
            &cues,
            Some(t_silence),
            5.0,
            speech_pcm.as_deref().or(opening_pcm.as_deref()),
        );

        let early_cue_hook = cues.iter()
            .find(|c| c.start_sec <= 0.8 && clean_speech_text(&c.text).chars().any(|ch| ch.is_alphanumeric()))
            .map(|c| c.text.as_str());

        let detected_ocr_snippet: Option<&str> = early_cue_hook.or_else(|| {
            let lower = meta.title.to_lowercase();
            if lower.contains("pov")
                || lower.contains("when ")
                || lower.contains("trailer")
                || lower.contains("teaser")
                || lower.contains("trope")
                || lower.contains("short")
                || lower.contains("edit")
                || (lower.contains("book") && !lower.contains("facebook")) {
                Some(meta.title.as_str())
            } else {
                None
            }
        });

        let effective_hook = first_spoken_hook.as_deref()
            .or(detected_ocr_snippet)
            .or(if active_wpm > 0.0 && spoken_cues.is_empty() { Some("[Spoken dialogue detected - captions pending]") } else { None });
        let detected_tone = NicheToneProfile::detect_with_duration(&meta.title, &meta.uploader, effective_hook.unwrap_or(""), Some(meta.duration_seconds));
        let scene_threshold = match detected_tone {
            NicheToneProfile::EducationalBreakdown => Some(0.12),
            NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => Some(0.45),
            NicheToneProfile::ConversationalPodcast => Some(0.35),
            _ => Some(0.35),
        };

        let sample_sec = Some(60.0);
        let pacing = crate::ingest::MediaSceneDetector::analyze_video_visual_pacing_with_threshold(video_url, sample_sec, scene_threshold).ok();
        let asl = pacing.as_ref().map(|p| p.average_shot_length_sec).unwrap_or(2.5);
        let cuts = pacing.as_ref().map(|p| p.cut_timestamps.as_slice()).unwrap_or(&[]);
        let total_cuts = cuts.len();

        let mut longest_shot = 0.0f64;
        let mut prev_cut = 0.0f64;
        for &c in cuts {
            if c <= 60.0 {
                longest_shot = longest_shot.max(c - prev_cut);
                prev_cut = c;
            }
        }
        if prev_cut < 60.0 {
            longest_shot = longest_shot.max(60.0 - prev_cut);
        }
        if cuts.is_empty() {
            longest_shot = 60.0;
        }

        // Bounded Mid-Video Retention Window scan:
        // For long-form (duration >= 180.0s), scan strictly between max(60.0s, 0.12 * duration) and 0.82 * duration
        // to isolate genuine mid-video drop-offs from intro hooks and outro sign-offs.
        let (min_t, max_t) = if meta.duration_seconds >= 180.0 {
            (60.0f64.max(0.12 * meta.duration_seconds), 0.82 * meta.duration_seconds)
        } else {
            (6.0f64.min(0.15 * meta.duration_seconds), (0.85 * meta.duration_seconds).max(6.0))
        };

        let (steepest_drop_rate, worst_drop_ts, outro_exit_note) = if !meta.heatmap.is_empty() {
            let derivatives = crate::math::RetentionEngine::compute_retention_derivative(&meta.heatmap);
            let worst_mid = crate::math::RetentionEngine::find_steepest_drop_in_window(&derivatives, min_t, max_t);
            let (drop_rate, drop_ts) = match worst_mid {
                Some((t, r_prime)) => (Some(r_prime * 100.0), Some(t)),
                None => (None, None),
            };

            // Detect early outro exit in the final 18% (> max_t) on long-form videos
            let outro_note = if meta.duration_seconds >= 180.0 {
                derivatives
                    .iter()
                    .filter(|(t, _)| *t > max_t)
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .and_then(|&(t, r_prime)| {
                        if r_prime * 100.0 <= -1.8 {
                            let total_sec = t.round() as u64;
                            let mm = total_sec / 60;
                            let ss = total_sec % 60;
                            Some(format!("Early Outro Exit detected at {:02}:{:02}—skip verbal 'wrapping up' cues", mm, ss))
                        } else {
                            None
                        }
                    })
            } else {
                None
            };

            (drop_rate, drop_ts, outro_note)
        } else {
            (None, None, None)
        };

        let satisfaction = match meta.view_count {
            Some(views) if views > 0 => {
                let likes = meta.like_count.unwrap_or(0);
                let comments = meta.comment_count.unwrap_or(0);
                Some(ViewerSatisfactionMetrics::new(views, likes, comments))
            }
            _ => None,
        };

        let thumb_report = meta.thumbnail.as_deref().map(|t_url| {
            if std::env::var("GEMINI_API_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false) {
                ThumbnailVisionEngine::analyze_url_blocking(t_url)
            } else {
                ThumbnailVisionEngine::fallback_report(t_url)
            }
        });

        let promise_gap = Some(PromiseGapEvaluator::evaluate(
            &meta.title,
            thumb_report.as_ref().map(|r| r.detected_focal_elements.join(" ")).as_deref(),
            detected_ocr_snippet,
            effective_hook,
            t_silence,
        ));

        let mut scorecard = Self::generate_scorecard_with_handshake(
            &meta.video_id,
            &meta.title,
            &meta.uploader,
            Some(detected_tone),
            t_silence,
            detected_ocr_snippet,
            asl,
            total_cuts,
            longest_shot,
            steepest_drop_rate,
            worst_drop_ts,
            active_wpm,
            effective_hook,
            satisfaction,
            thumb_report,
            promise_gap,
        );
        scorecard.outro_exit_note = outro_exit_note;
        scorecard.duration_seconds = Some(meta.duration_seconds);

        let mut breath_pauses = Vec::new();
        for i in 0..spoken_cues.len().saturating_sub(1) {
            let gap_sec = spoken_cues[i + 1].0.start_sec - spoken_cues[i].0.end_sec;
            if gap_sec > 0.4 {
                breath_pauses.push(gap_sec);
            }
        }
        let total_dur = meta.duration_seconds.max(10.0);
        let cuttable_fat = Some(Self::calculate_cuttable_fat(
            total_dur,
            t_silence,
            Some(detected_tone),
            cuts,
            &breath_pauses,
        ));
        let loudness = crate::ingest::MediaSceneDetector::extract_loudness_metrics(video_url, false).ok();
        scorecard.loudness_metrics = loudness;
        scorecard.cuttable_fat = cuttable_fat;

        let retention_report = if !meta.heatmap.is_empty() {
            Some(crate::math::RetentionEngine::analyze_curve_multimodal(
                &meta.video_id,
                &meta.title,
                meta.duration_seconds,
                &meta.heatmap,
                &cues,
                cuts,
            ))
        } else {
            None
        };
        let raw_comments = crate::analyzer::CommentFrictionMiner::extract_comments_with_ytdlp(video_url, 100).unwrap_or_default();
        let comment_clusters = crate::analyzer::CommentFrictionMiner::mine_comment_clusters(&raw_comments, retention_report.as_ref());
        scorecard.comment_clusters = comment_clusters;

        scorecard.retention_curve = meta.heatmap.clone();
        if let Some(ref report) = retention_report {
            scorecard.attention_breaches = report.attention_breaches.clone();
            scorecard.re_engagement_checkpoints = report.re_engagement_checkpoints.clone();
        } else if meta.duration_seconds >= 480.0 {
            scorecard.re_engagement_checkpoints = crate::math::RetentionEngine::evaluate_re_engagement_checkpoints(meta.duration_seconds, &meta.heatmap, cuts);
        }

        // Advertiser-friendly suitability pre-flight gate (v0.5.0)
        let suitability = Some(SuitabilityAuditor::audit_cues(&cues));
        scorecard.suitability_report = suitability;

        // Dedicated YouTube Shorts mode (v0.5.0)
        if meta.duration_seconds <= 60.0 {
            let shorts_metrics = Some(ShortsEngine::evaluate(&cues, meta.duration_seconds, t_silence));
            scorecard.shorts_metrics = shorts_metrics;
        }

        // Dedicated Stair-Stepping Escalation Radar (v0.5.3)
        let mut chapters_or_cues: Vec<String> = meta
            .chapters
            .iter()
            .map(|c| {
                let mm = (c.start_time / 60.0).floor() as u64;
                let ss = (c.start_time % 60.0).round() as u64;
                format!("{:02}:{:02} - {}", mm, ss, c.title)
            })
            .collect();
        if chapters_or_cues.is_empty() {
            chapters_or_cues = cues
                .iter()
                .map(|c| {
                    let mm = (c.start_sec / 60.0).floor() as u64;
                    let ss = (c.start_sec % 60.0).round() as u64;
                    format!("{:02}:{:02} - {}", mm, ss, c.text)
                })
                .collect();
        }

        if crate::pipeline::HookMachineEngine::detect_stair_stepping_format(&meta.title) {
            scorecard.stair_stepping = Some(crate::pipeline::HookMachineEngine::evaluate_stair_stepping_integrity(
                &meta.title,
                meta.duration_seconds,
                &chapters_or_cues,
            ));
        }

        // Dedicated Abrupt Ending & Outro Runway Radar (v0.5.4)
        let heatmap_curve: Vec<(f64, f64)> = meta.heatmap.iter().map(|p| (p.start_time, p.value)).collect();
        let outro_report = crate::math::RetentionEngine::evaluate_outro_runway(
            meta.duration_seconds,
            &cues,
            if heatmap_curve.is_empty() { None } else { Some(&heatmap_curve) },
        );
        scorecard.outro_report = Some(outro_report);

        Ok(scorecard)
    }

    /// Evaluates an unreleased local video draft (.mp4/.mov) directly from NLE export.
    /// Bypasses network extraction, runs in-memory FFmpeg scene detection & VAD,
    /// and simulates Predictive Retention Risk without needing a public YouTube heatmap.
    pub fn evaluate_local_draft(file_path: &str) -> Result<MarginalGainsScorecard, Box<dyn std::error::Error + Send + Sync>> {
        let path = std::path::Path::new(file_path);
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("local_draft");
        let clean_title_str = file_stem.replace(['_', '-'], " ");

        let duration_seconds = crate::ingest::MediaSceneDetector::probe_local_duration_seconds(file_path)
            .unwrap_or(60.0)
            .max(5.0);

        let pcm = crate::ingest::MediaSceneDetector::extract_local_audio_pcm(file_path, Some(60.0)).unwrap_or_default();
        let vad = crate::analyzer::VadClassifier::default();
        let onset_ms = if !pcm.is_empty() {
            vad.detect_speech_onset_ms(&pcm)
        } else {
            None
        };
        let t_silence = onset_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);

        let segments = if !pcm.is_empty() {
            vad.detect_speech_segments(&pcm)
        } else {
            Vec::new()
        };
        let total_speech_ms: u32 = segments.iter().map(|(start, end)| end.saturating_sub(*start)).sum();
        let active_wpm = if total_speech_ms > 500 {
            let speech_sec = total_speech_ms as f64 / 1000.0;
            let estimated_words = (speech_sec * (190.0 / 60.0)).round();
            ((estimated_words / (speech_sec / 60.0)).clamp(120.0, 240.0) * 10.0).round() / 10.0
        } else if pcm.is_empty() || segments.is_empty() {
            0.0
        } else {
            185.0
        };

        let detected_tone = NicheToneProfile::detect_with_duration(&clean_title_str, "Local Editor Draft", "", Some(duration_seconds));
        let scene_threshold = match detected_tone {
            NicheToneProfile::EducationalBreakdown => Some(0.12),
            NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => Some(0.45),
            NicheToneProfile::ConversationalPodcast => Some(0.35),
            _ => Some(0.35),
        };

        let sample_sec = Some(duration_seconds.min(180.0));
        let pacing = crate::ingest::MediaSceneDetector::analyze_video_visual_pacing_with_threshold(file_path, sample_sec, scene_threshold).ok();
        let asl = pacing.as_ref().map(|p| p.average_shot_length_sec).unwrap_or(2.5);
        let cuts = pacing.as_ref().map(|p| p.cut_timestamps.as_slice()).unwrap_or(&[]);
        let total_cuts = cuts.len();

        let mut longest_shot = 0.0f64;
        let mut longest_shot_start = 0.0f64;
        let mut prev_cut = 0.0f64;
        for &c in cuts {
            if c <= 60.0 {
                let hold = c - prev_cut;
                if hold > longest_shot {
                    longest_shot = hold;
                    longest_shot_start = prev_cut;
                }
                prev_cut = c;
            }
        }
        if prev_cut < 60.0 {
            let hold = 60.0 - prev_cut;
            if hold > longest_shot {
                longest_shot = hold;
                longest_shot_start = prev_cut;
            }
        }
        if cuts.is_empty() {
            longest_shot = 60.0;
            longest_shot_start = 0.0;
        }

        // Predictive Retention Modeling for unreleased drafts:
        let hold_benchmark = match detected_tone {
            NicheToneProfile::ConversationalPodcast => 45.0,
            NicheToneProfile::EntertainmentSpectacle => 15.0,
            NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => 12.0,
            NicheToneProfile::EducationalBreakdown => 15.0,
            NicheToneProfile::HighStakesPanic | NicheToneProfile::NarrativeEmotionalPitch | NicheToneProfile::ConsumerReviewExplainer => 7.0,
        };

        let (steepest_drop_rate, worst_drop_ts) = if longest_shot > hold_benchmark {
            let excess = longest_shot - hold_benchmark;
            let predicted_drop = (-1.5 - (excess * 0.25)).max(-6.0);
            (Some(predicted_drop), Some(longest_shot_start + hold_benchmark))
        } else if t_silence > 0.4 {
            (Some(-2.5), Some(t_silence))
        } else {
            (Some(-1.4), None)
        };

        let outro_exit_note = if duration_seconds >= 180.0 {
            let exit_sec = (duration_seconds * 0.85).round() as u64;
            let mm = exit_sec / 60;
            let ss = exit_sec % 60;
            Some(format!("Pre-publish review: Check final 15% (after {:02}:{:02}) for premature wrap-up cues.", mm, ss))
        } else {
            None
        };

        let mut scorecard = Self::generate_scorecard_with_tone_and_ocr(
            file_stem,
            &clean_title_str,
            "Local Editor Draft",
            Some(detected_tone),
            t_silence,
            None,
            asl,
            total_cuts,
            longest_shot,
            steepest_drop_rate,
            worst_drop_ts,
            active_wpm,
            Some(&clean_title_str),
            None,
        );
        scorecard.outro_exit_note = outro_exit_note;
        scorecard.duration_seconds = Some(duration_seconds);
        scorecard.is_local_draft = true;

        let mut breath_pauses = Vec::new();
        for i in 0..segments.len().saturating_sub(1) {
            let gap_ms = segments[i + 1].0.saturating_sub(segments[i].1);
            if gap_ms > 400 {
                breath_pauses.push(gap_ms as f64 / 1000.0);
            }
        }
        let cuttable_fat = Some(Self::calculate_cuttable_fat(
            duration_seconds,
            t_silence,
            Some(detected_tone),
            cuts,
            &breath_pauses,
        ));
        let loudness = crate::ingest::MediaSceneDetector::extract_loudness_metrics(file_path, true).ok();
        scorecard.loudness_metrics = loudness;
        scorecard.cuttable_fat = cuttable_fat;

        // Dedicated YouTube Shorts mode for local drafts (v0.5.0)
        if duration_seconds <= 60.0 {
            let shorts_metrics = Some(ShortsEngine::evaluate(&[], duration_seconds, t_silence));
            scorecard.shorts_metrics = shorts_metrics;
        }

        if duration_seconds >= 480.0 {
            scorecard.re_engagement_checkpoints = crate::math::RetentionEngine::evaluate_re_engagement_checkpoints(duration_seconds, &[], cuts);
        }

        if crate::pipeline::HookMachineEngine::detect_stair_stepping_format(&clean_title_str) {
            scorecard.stair_stepping = Some(crate::pipeline::HookMachineEngine::evaluate_stair_stepping_integrity(
                &clean_title_str,
                duration_seconds,
                &[],
            ));
        }

        // Dedicated Abrupt Ending & Outro Runway Radar (v0.5.4)
        let outro_report = crate::math::RetentionEngine::evaluate_outro_runway(
            duration_seconds,
            &[],
            None,
        );
        scorecard.outro_report = Some(outro_report);

        // Synthesize predictive retention curve for unreleased local draft
        let mut sim_curve = Vec::new();
        let step = (duration_seconds / 100.0).max(1.0);
        let mut current_t = 0.0;
        let mut current_ret = 1.0;
        while current_t <= duration_seconds {
            let decay_rate = 0.0015 * step;
            let hold_penalty = if longest_shot > hold_benchmark && current_t >= longest_shot_start && current_t <= longest_shot_start + longest_shot {
                0.0035 * step
            } else {
                0.0
            };
            current_ret = (current_ret - decay_rate - hold_penalty).max(0.20);
            sim_curve.push(crate::math::retention::HeatmapPoint {
                start_time: (current_t * 10.0).round() / 10.0,
                end_time: ((current_t + step) * 10.0).round() / 10.0,
                value: (current_ret * 1000.0).round() / 1000.0,
            });
            current_t += step;
        }
        scorecard.retention_curve = sim_curve;

        let mut sim_breaches = Vec::new();
        if longest_shot > hold_benchmark {
            let breach_ts = (longest_shot_start + hold_benchmark).min(duration_seconds);
            sim_breaches.push(crate::math::retention::AttentionBreach {
                timestamp_sec: (breach_ts * 10.0).round() / 10.0,
                formatted_timestamp: crate::math::RetentionEngine::format_timestamp(breach_ts),
                intensity: 0.65,
                r_prime: -2.8,
                root_cause: crate::math::retention::BreachRootCause::SlowVisualPacing { shot_length_sec: longest_shot },
                transcript_context: "Extended static camera take without B-roll or visual reset.".to_string(),
                tactical_remedy: format!("Insert 115% digital punch-in or B-roll cutaway at {:.1}s.", breach_ts),
            });
        }
        if t_silence > 0.4 {
            sim_breaches.push(crate::math::retention::AttentionBreach {
                timestamp_sec: 0.0,
                formatted_timestamp: "00:00".to_string(),
                intensity: 0.85,
                r_prime: -3.5,
                root_cause: crate::math::retention::BreachRootCause::DeadAirSilence { silence_sec: t_silence },
                transcript_context: format!("Delayed verbal hook onset ({:.2}s cold-open silence).", t_silence),
                tactical_remedy: "Ripple trim opening dead air so first word hits on frame zero.".to_string(),
            });
        }
        scorecard.attention_breaches = sim_breaches;

        Ok(scorecard)
    }
}

/// Vector delta comparison between two evaluated video targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorDeltaComparison {
    pub vector_name: String,
    pub measured_a: String,
    pub score_a: f64,
    pub measured_b: String,
    pub score_b: f64,
    pub delta_score: f64,
    pub advantage: String,
}

/// Automated Head-to-Head Comparative Audit Report contrasting two videos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetComparisonReport {
    pub target_a_id: String,
    pub target_a_title: String,
    pub target_b_id: String,
    pub target_b_title: String,
    pub health_grade_a: f64,
    pub health_grade_b: f64,
    pub delta_health_grade: f64,
    pub composite_score_a: f64,
    pub composite_score_b: f64,
    pub delta_composite_score: f64,
    pub vector_comparisons: Vec<VectorDeltaComparison>,
    pub structural_advantages_a: Vec<String>,
    pub what_to_change_playbook: Vec<String>,
}

impl TargetComparisonReport {
    /// Generates a head-to-head comparison between Target A and Target B.
    pub fn generate(scorecard_a: &MarginalGainsScorecard, scorecard_b: &MarginalGainsScorecard) -> Self {
        let health_grade_a = scorecard_a.series_system_reliability_pct.round();
        let health_grade_b = scorecard_b.series_system_reliability_pct.round();
        let delta_health_grade = health_grade_a - health_grade_b;

        let composite_score_a = scorecard_a.composite_brailsford_score;
        let composite_score_b = scorecard_b.composite_brailsford_score;
        let delta_composite_score = ((composite_score_a - composite_score_b) * 10.0).round() / 10.0;

        let mut vector_comparisons = Vec::new();
        let mut structural_advantages_a = Vec::new();
        let mut what_to_change_playbook = Vec::new();

        for vec_a in &scorecard_a.vectors {
            if let Some(vec_b) = scorecard_b.vectors.iter().find(|v| v.name == vec_a.name) {
                let delta = ((vec_a.score_0_to_100 - vec_b.score_0_to_100) * 10.0).round() / 10.0;
                let advantage = if delta > 0.0 {
                    "Target A".to_string()
                } else if delta < 0.0 {
                    "Target B".to_string()
                } else {
                    "Tied".to_string()
                };

                let measured_a = format!("{:.1}{}", vec_a.measured_value, vec_a.unit);
                let measured_b = format!("{:.1}{}", vec_b.measured_value, vec_b.unit);

                vector_comparisons.push(VectorDeltaComparison {
                    vector_name: vec_a.human_label().to_string(),
                    measured_a,
                    score_a: vec_a.score_0_to_100,
                    measured_b,
                    score_b: vec_b.score_0_to_100,
                    delta_score: delta,
                    advantage,
                });

                if delta >= 15.0 {
                    structural_advantages_a.push(format!(
                        "{}: Target A achieved {:.1}{} vs. {:.1}{} on Target B (+{:.1} pts). {}",
                        vec_a.human_label(),
                        vec_a.measured_value, vec_a.unit,
                        vec_b.measured_value, vec_b.unit,
                        delta,
                        vec_a.one_percent_fix
                    ));
                    what_to_change_playbook.push(format!(
                        "Fix {}: {}",
                        vec_b.human_label(),
                        vec_b.one_percent_fix
                    ));
                }
            }
        }

        if structural_advantages_a.is_empty() {
            structural_advantages_a.push("Both targets exhibit comparable pacing kinematics.".to_string());
        }
        if what_to_change_playbook.is_empty() {
            what_to_change_playbook.push("Target B aligns closely with Target A's editing benchmarks.".to_string());
        }

        Self {
            target_a_id: scorecard_a.target_id.clone(),
            target_a_title: scorecard_a.title.clone(),
            target_b_id: scorecard_b.target_id.clone(),
            target_b_title: scorecard_b.title.clone(),
            health_grade_a,
            health_grade_b,
            delta_health_grade,
            composite_score_a,
            composite_score_b,
            delta_composite_score,
            vector_comparisons,
            structural_advantages_a,
            what_to_change_playbook,
        }
    }

    /// Renders terminal output table.
    pub fn render_terminal(&self) -> String {
        let mut out = String::new();
        out.push_str("\n╔════════════════════════════════════════════════════════════════════════════════════════════════════╗\n");
        out.push_str("║                          ⚔️  HEAD-TO-HEAD COMPARATIVE AUDIT                                         ║\n");
        out.push_str("╚════════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
        out.push_str(&format!(" 🏆 Target A (Baseline) : {} ({})\n", self.target_a_title, self.target_a_id));
        out.push_str(&format!(" 🎯 Target B (Challenger): {} ({})\n", self.target_b_title, self.target_b_id));
        out.push_str(&format!(" 📊 Health Grade Delta  : {:.0}% vs. {:.0}% ({:+.0}%)\n", self.health_grade_a, self.health_grade_b, self.delta_health_grade));
        out.push_str(&format!(" 🚀 Composite Score     : {:.1} vs. {:.1} ({:+.1} pts)\n", self.composite_score_a, self.composite_score_b, self.delta_composite_score));
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 📋 SIDE-BY-SIDE VECTOR DELTA MATRIX\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        for comp in &self.vector_comparisons {
            out.push_str(&format!(
                " • {:<42} | A: {:<8} (Score: {:>3.0}) | B: {:<8} (Score: {:>3.0}) | Delta: {:>+4.0} [{}]\n",
                comp.vector_name, comp.measured_a, comp.score_a, comp.measured_b, comp.score_b, comp.delta_score, comp.advantage
            ));
        }
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 💡 STRUCTURAL ADVANTAGES OF TARGET A\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        for (i, adv) in self.structural_advantages_a.iter().enumerate() {
            out.push_str(&format!(" {}. {}\n", i + 1, adv));
        }
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 🛠️ WHAT TO CHANGE IN TARGET B (Editing Playbook)\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        for (i, fix) in self.what_to_change_playbook.iter().enumerate() {
            out.push_str(&format!(" {}. {}\n", i + 1, fix));
        }
        out.push_str("══════════════════════════════════════════════════════════════════════════════════════════════════════\n");
        out
    }

    /// Renders full formatted markdown comparison.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# ⚔️ Head-to-Head Comparative Audit: Target A vs. Target B\n\n");
        out.push_str(&format!("> **Target A (Baseline)**: **\"{}\"** (`{}`) — Health: **{:.0}%** | Composite: **{:.1}**\n", self.target_a_title, self.target_a_id, self.health_grade_a, self.composite_score_a));
        out.push_str(&format!("> **Target B (Challenger)**: **\"{}\"** (`{}`) — Health: **{:.0}%** | Composite: **{:.1}**\n", self.target_b_title, self.target_b_id, self.health_grade_b, self.composite_score_b));
        out.push_str(&format!("> **Net Variance**: **{:+.0}% Health Grade** | **{:+.1} Composite Score Delta**\n\n", self.delta_health_grade, self.delta_composite_score));
        out.push_str("---\n\n");
        out.push_str("## 1. Side-by-Side Telemetry Delta Matrix\n\n");
        out.push_str("| Telemetry Vector | Target A (`Baseline`) | Target B (`Challenger`) | Score A | Score B | Score Delta | Advantage |\n");
        out.push_str("|:---|:---:|:---:|:---:|:---:|:---:|:---:|\n");
        for comp in &self.vector_comparisons {
            out.push_str(&format!(
                "| **{}** | {} | {} | {:.0} | {:.0} | {:+.0} | **{}** |\n",
                comp.vector_name, comp.measured_a, comp.measured_b, comp.score_a, comp.score_b, comp.delta_score, comp.advantage
            ));
        }
        out.push_str("\n---\n\n");
        out.push_str("## 2. Key Structural Advantages of Target A\n\n");
        for (i, adv) in self.structural_advantages_a.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, adv));
        }
        out.push_str("\n---\n\n");
        out.push_str("## 3. What to Change in Target B (Actionable NLE Playbook)\n\n");
        for (i, fix) in self.what_to_change_playbook.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, fix));
        }
        out.push('\n');
        out
    }

    /// Saves comparative analysis to output/comparison_<slug_a>_vs_<slug_b>.md.
    pub fn save_to_markdown(&self, output_dir: &Path) -> Result<PathBuf, std::io::Error> {
        let slug_a = crate::export::flight_sheet::slugify(&self.target_a_title);
        let slug_b = crate::export::flight_sheet::slugify(&self.target_b_title);
        let filename = format!("comparison_{}_vs_{}.md", slug_a, slug_b);
        let path = output_dir.join(filename);
        std::fs::write(&path, self.to_markdown())?;
        Ok(path)
    }
}
