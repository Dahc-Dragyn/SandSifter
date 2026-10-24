use serde::{Deserialize, Serialize};
use crate::math::retention::VttCue;

/// YouTube Shorts duration classification based on mobile swipe retention mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortsDurationStatus {
    /// Under 25 seconds: Micro-short with high completion but lower watch-time accumulation
    Compact,
    /// 25–45 seconds: The algorithmic sweet spot balancing retention rate and session time
    Optimal,
    /// 46–58 seconds: Extended duration where mobile swipe-away risk accelerates
    Extended,
    /// > 58 seconds: Severe mobile scrubber clipping risk (YouTube UI cutoffs)
    TruncationRisk,
}

impl ShortsDurationStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Compact => "Compact (<25s)",
            Self::Optimal => "Optimal Sweet Spot (25–45s)",
            Self::Extended => "Extended Duration (46–58s)",
            Self::TruncationRisk => "Truncation Risk (>58s)",
        }
    }

    pub fn is_optimal(&self) -> bool {
        matches!(self, Self::Optimal)
    }
}

/// Comprehensive Short-Form Retention & Loop Continuity Metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortsRetentionMetrics {
    /// Total duration in seconds
    pub duration_seconds: f64,
    /// First-frame hook delay in seconds (silence or ambient lag before hook entry)
    pub first_frame_hook_delay: f64,
    /// True if verbal/visual hook triggers within the <= 1.5s mobile window
    pub first_frame_hook_safe: bool,
    /// Duration safety tier
    pub duration_safety_status: ShortsDurationStatus,
    /// Loop continuity score (0.0 to 100.0) measuring seamless rewatch bridging
    pub loop_continuity_score: f64,
    /// True if closing beat bridges into opening hook to promote >=100% loop completion
    pub loop_seamless: bool,
    /// Mobile View-vs-Swiped-Away risk category ("LOW", "MODERATE", "HIGH")
    pub stay_vs_swipe_risk: String,
    /// Projected percentage of feed impressions that stay rather than swiping (Benchmark >= 75%)
    pub estimated_stay_ratio_pct: f64,
    /// Detected opening spoken phrase (for loop matching)
    pub loop_opening_phrase: Option<String>,
    /// Detected closing spoken phrase (for loop matching)
    pub loop_closing_phrase: Option<String>,
    /// Plain-English analysis of the transition between the ending and beginning
    pub loop_bridge_insight: String,
    /// Actionable executive directive for short-form mobile pacing
    pub executive_summary: String,
}

/// Algorithmic evaluator calibrating short-form retention mechanics.
pub struct ShortsEngine;

impl ShortsEngine {
    /// Returns true if a video qualifies as a YouTube Short based on duration (<= 60.0s).
    pub fn is_short(duration_sec: f64) -> bool {
        duration_sec <= 60.0
    }

    /// Evaluates subtitle cues and audio timing against YouTube Shorts mobile algorithms.
    pub fn evaluate(cues: &[VttCue], duration_sec: f64, t_silence: f64) -> ShortsRetentionMetrics {
        let hook_delay = (t_silence.max(0.0) * 100.0).round() / 100.0;
        let first_frame_hook_safe = hook_delay <= 1.5;

        let duration_safety_status = if duration_sec > 58.0 {
            ShortsDurationStatus::TruncationRisk
        } else if duration_sec > 45.0 {
            ShortsDurationStatus::Extended
        } else if duration_sec >= 25.0 {
            ShortsDurationStatus::Optimal
        } else {
            ShortsDurationStatus::Compact
        };

        // Extract spoken dialogue text (skipping music notes and empty brackets)
        let clean_cue_text = |text: &str| -> String {
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

        let spoken_cues: Vec<String> = cues
            .iter()
            .map(|c| clean_cue_text(&c.text))
            .filter(|t| t.chars().any(|ch| ch.is_alphanumeric()))
            .collect();

        let (loop_score, is_seamless, open_phrase, close_phrase, bridge_insight) =
            Self::evaluate_loop_continuity(&spoken_cues);

        // Calculate Stay-vs-Swipe risk and estimated stay ratio
        let mut stay_ratio = 78.0_f64;

        if hook_delay <= 1.0 {
            stay_ratio += 5.0;
        } else if hook_delay <= 1.5 {
            stay_ratio += 0.0;
        } else {
            stay_ratio -= 18.0;
        }

        match duration_safety_status {
            ShortsDurationStatus::Optimal => stay_ratio += 4.0,
            ShortsDurationStatus::Compact => stay_ratio += 2.0,
            ShortsDurationStatus::Extended => stay_ratio -= 6.0,
            ShortsDurationStatus::TruncationRisk => stay_ratio -= 15.0,
        }

        if is_seamless {
            stay_ratio += 6.0;
        } else if loop_score <= 30.0 {
            stay_ratio -= 10.0;
        }

        let estimated_stay_ratio_pct = (stay_ratio.clamp(35.0, 95.0) * 10.0).round() / 10.0;

        let stay_vs_swipe_risk = if !first_frame_hook_safe || estimated_stay_ratio_pct < 60.0 {
            "HIGH".to_string()
        } else if estimated_stay_ratio_pct >= 75.0 {
            "LOW".to_string()
        } else {
            "MODERATE".to_string()
        };

        let executive_summary = if !first_frame_hook_safe {
            format!(
                "⚠️ High Swipe-Away Risk: Verbal hook delayed by {:.2}s (exceeds 1.5s mobile threshold). Trim opening ambient lag to frame one to prevent immediate swipe.",
                hook_delay
            )
        } else if duration_safety_status == ShortsDurationStatus::TruncationRisk {
            format!(
                "⚠️ Truncation Warning: Runtime of {:.1}s exceeds 58s safe threshold. Mobile interface overlays and scrubbers risk clipping your climax.",
                duration_sec
            )
        } else if is_seamless {
            format!(
                "🟢 Elite Short Pacing: {:.2}s hook onset with seamless loop bridging ({:.0}/100). Engineered for >75% stay ratio and infinite rewatch loops.",
                hook_delay, loop_score
            )
        } else if loop_score <= 30.0 {
            "⚠️ Abrupt Outro Detected: Sign-off or CTA signals video completion before the final frame, prompting viewers to swipe away early.".to_string()
        } else {
            format!(
                "🟢 Solid Baseline: {:.2}s hook onset inside the 25–45s sweet spot ({:.1}s). Strengthen closing connector to unlock 100%+ loop rewatch.",
                hook_delay, duration_sec
            )
        };

        ShortsRetentionMetrics {
            duration_seconds: (duration_sec * 10.0).round() / 10.0,
            first_frame_hook_delay: hook_delay,
            first_frame_hook_safe,
            duration_safety_status,
            loop_continuity_score: loop_score,
            loop_seamless: is_seamless,
            stay_vs_swipe_risk,
            estimated_stay_ratio_pct,
            loop_opening_phrase: open_phrase,
            loop_closing_phrase: close_phrase,
            loop_bridge_insight: bridge_insight,
            executive_summary,
        }
    }

    /// Evaluates spoken cues for syntactic loop bridging vs abrupt sign-offs.
    fn evaluate_loop_continuity(
        spoken_cues: &[String],
    ) -> (f64, bool, Option<String>, Option<String>, String) {
        if spoken_cues.is_empty() {
            return (
                60.0,
                false,
                None,
                None,
                "No verbal subtitle cues detected to evaluate syntactic loop continuity.".to_string(),
            );
        }

        let first_text = spoken_cues.first().unwrap().trim();
        let last_text = spoken_cues.last().unwrap().trim();

        let open_phrase = Some(first_text.to_string());
        let close_phrase = Some(last_text.to_string());

        let last_lower = last_text.to_lowercase();

        // 1. Check for abrupt sign-offs / CTAs (anti-patterns that ruin loops)
        let abrupt_signoffs = [
            "subscribe",
            "like and subscribe",
            "like and follow",
            "thanks for watching",
            "thank you for watching",
            "see you in the next",
            "see you next time",
            "peace out",
            "bye",
            "link in bio",
            "comment below",
            "follow for more",
            "check out my channel",
            "leave a like",
            "don't forget to like",
        ];

        for &signoff in &abrupt_signoffs {
            if last_lower.contains(signoff) {
                return (
                    25.0,
                    false,
                    open_phrase,
                    close_phrase,
                    format!(
                        "Abrupt sign-off detected ('{}'). Viewers anticipate video termination and swipe away before the loop connects.",
                        signoff
                    ),
                );
            }
        }

        // 2. Check for seamless syntactic loop connectors at the end of the final cue
        let loop_connectors = [
            "which is why",
            "and that's why",
            "and that's how",
            "all because",
            "which means",
            "the reason is",
            "so next time",
            "if you ever",
            "turns out",
            "leading to",
            "because",
            "which is",
            "so that's",
            "and that is",
        ];

        for &connector in &loop_connectors {
            if last_lower.ends_with(connector)
                || last_lower.contains(&format!("{}...", connector))
                || last_lower.ends_with(&format!("{}:", connector))
            {
                return (
                    92.0,
                    true,
                    open_phrase,
                    close_phrase,
                    format!(
                        "Seamless syntactic connector detected ('{}'). The final phrase bridges directly into the opening hook to trigger automatic rewatch loops.",
                        connector
                    ),
                );
            }
        }

        // 3. Check for open punctuation (ellipsis or dangling connector)
        if last_text.ends_with("...") || last_text.ends_with('—') || last_text.ends_with(':') {
            return (
                85.0,
                true,
                open_phrase,
                close_phrase,
                "Open sentence ending detected (trailing punctuation). Creates an unresolved narrative gap that rolls smoothly into frame one.".to_string(),
            );
        }

        // 4. Default natural ending
        (
            65.0,
            false,
            open_phrase,
            close_phrase,
            "Standard narrative conclusion. Video concludes cleanly but does not actively bridge syntactically back into the opening hook.".to_string(),
        )
    }
}
