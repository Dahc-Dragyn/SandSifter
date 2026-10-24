use serde::{Deserialize, Serialize};
use crate::math::retention::VttCue;

/// The 7 official YouTube advertiser-friendly content violation categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuitabilityCategory {
    /// Strong profanity, vulgarities, or obscenities (severely weighted in 0:00–0:15 hook window)
    InappropriateLanguage,
    /// Explicit sexual themes, nudity, or adult humor
    AdultContent,
    /// Graphic violence, severe physical trauma, open injuries, blood, or gore
    Violence,
    /// Dangerous stunts, suicide, self-harm, or hazardous machinery without safety gear
    HarmfulActs,
    /// Recreational drugs, drug consumption, manufacturing, or paraphernalia
    DrugContent,
    /// Firearms, automatic weapons, bump stocks, conversion switches, or ammunition modifications
    Firearms,
    /// Terrorism, mass casualties, active war, school violence, or tragic deaths
    SensitiveEvents,
}

impl SuitabilityCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::InappropriateLanguage => "Inappropriate Language",
            Self::AdultContent => "Adult & Sensual Content",
            Self::Violence => "Violence & Graphic Imagery",
            Self::HarmfulActs => "Harmful or Dangerous Acts",
            Self::DrugContent => "Recreational Drugs & Substances",
            Self::Firearms => "Firearms & Weapons",
            Self::SensitiveEvents => "Sensitive Events & Mass Tragedy",
        }
    }
}

/// Severity classification of a detected suitability violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuitabilitySeverity {
    /// Minor slang or colloquial term in body text (safe for standard ad distribution)
    Low,
    /// Moderate vulgarity or sensitive discussion after the hook window (t > 15s)
    Moderate,
    /// Severe violation: Any violation in the 0:00–0:15 Hook Window, or severe prohibited terms anywhere
    Severe,
}

/// An individual detected suitability violation with timestamp and token context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuitabilityViolation {
    pub category: SuitabilityCategory,
    pub timestamp_sec: f64,
    pub token: String,
    pub context: String,
    pub is_hook_window: bool, // t <= 15.0s
    pub severity: SuitabilitySeverity,
}

/// Comprehensive Advertiser-Suitability Pre-Flight Report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSuitabilityReport {
    /// Overall monetization rating: "GREEN" (Safe for full ads) or "YELLOW" (Limited or no ads)
    pub rating: String,
    /// Recommended publication workflow: "IMMEDIATE_PUBLIC" or "UNLISTED_MANUAL_REVIEW"
    pub recommended_workflow: String,
    /// True if zero violations detected in the critical 0:00–0:15 Hook Window
    pub hook_window_safe: bool,
    /// Total count of detected suitability flags
    pub violations_count: usize,
    /// Count of violations located inside the 0:00–0:15 Hook Window
    pub hook_violations_count: usize,
    /// Granular list of detected violations with timecodes
    pub violations: Vec<SuitabilityViolation>,
    /// Plain-English executive action statement for creators
    pub executive_summary: String,
}

impl VideoSuitabilityReport {
    /// Constructs a clean, safe report when zero violations are found.
    pub fn clean() -> Self {
        Self {
            rating: "GREEN".to_string(),
            recommended_workflow: "IMMEDIATE_PUBLIC".to_string(),
            hook_window_safe: true,
            violations_count: 0,
            hook_violations_count: 0,
            violations: Vec::new(),
            executive_summary: "🟢 Monetization Safe: Zero profanity or advertiser-sensitive terms detected in the 0:00–0:15 Hook Window. Cleared for immediate public monetization.".to_string(),
        }
    }
}

/// High-performance rule-based auditor enforcing YouTube's Advertiser-Friendly Guidelines.
pub struct SuitabilityAuditor;

impl SuitabilityAuditor {
    /// Audits a collection of WebVTT subtitle cues.
    pub fn audit_cues(cues: &[VttCue]) -> VideoSuitabilityReport {
        let mut timed_lines = Vec::with_capacity(cues.len());
        for cue in cues {
            timed_lines.push((cue.start_sec, cue.text.as_str()));
        }
        Self::audit_timed_text(&timed_lines)
    }

    /// Audits timed text slices (timestamp_sec, text).
    pub fn audit_timed_text(items: &[(f64, &str)]) -> VideoSuitabilityReport {
        let mut violations = Vec::new();

        // 7 Category Lexicons
        let profanity_terms = [
            "fuck", "fucking", "fucked", "fucker", "motherfucker", "shit", "bullshit",
            "bitch", "cunt", "asshole", "bastard", "nigger", "nigga", "faggot", "dickhead",
            "f***", "s***", "b***", "c***",
        ];

        let adult_terms = [
            "porn", "porno", "pornography", "nsfw", "onlyfans", "naked", "sex tape",
            "orgasm", "dildo", "vibrator", "erotic", "blowjob", "handjob", "masturbat",
        ];

        let violence_terms = [
            "bloodbath", "decapitat", "behead", "torture", "mutilat", "slaughter",
            "massacre", "dismember", "corpse",
        ];

        let harmful_terms = [
            "suicide", "kill myself", "hang myself", "slit wrists", "choking game",
            "drink bleach", "drinking bleach", "self harm", "overdose",
        ];

        let drug_terms = [
            "cocaine", "heroin", "methamphetamine", "crystal meth", "fentanyl",
            "crack pipe", "injecting drugs",
        ];

        let firearm_terms = [
            "ghost gun", "auto sear", "glock switch", "bump stock", "silencer build",
            "unregistered firearm", "sawed off shotgun",
        ];

        let sensitive_terms = [
            "mass shooter", "mass shooting", "terrorist attack", "school shooter",
            "school shooting", "genocide",
        ];

        for &(timestamp_sec, text) in items {
            let lower = text.to_lowercase();
            let is_hook = timestamp_sec <= 15.0;

            let check_category = |category: SuitabilityCategory, terms: &[&str], violations: &mut Vec<SuitabilityViolation>| {
                for &term in terms {
                    if Self::contains_word_or_phrase(&lower, term) {
                        let severity = if is_hook
                            || category == SuitabilityCategory::HarmfulActs
                            || category == SuitabilityCategory::SensitiveEvents
                        {
                            SuitabilitySeverity::Severe
                        } else {
                            SuitabilitySeverity::Moderate
                        };

                        violations.push(SuitabilityViolation {
                            category,
                            timestamp_sec: (timestamp_sec * 10.0).round() / 10.0,
                            token: term.to_string(),
                            context: text.trim().to_string(),
                            is_hook_window: is_hook,
                            severity,
                        });
                    }
                }
            };

            check_category(SuitabilityCategory::InappropriateLanguage, &profanity_terms, &mut violations);
            check_category(SuitabilityCategory::AdultContent, &adult_terms, &mut violations);
            check_category(SuitabilityCategory::Violence, &violence_terms, &mut violations);
            check_category(SuitabilityCategory::HarmfulActs, &harmful_terms, &mut violations);
            check_category(SuitabilityCategory::DrugContent, &drug_terms, &mut violations);
            check_category(SuitabilityCategory::Firearms, &firearm_terms, &mut violations);
            check_category(SuitabilityCategory::SensitiveEvents, &sensitive_terms, &mut violations);
        }

        let hook_violations_count = violations.iter().filter(|v| v.is_hook_window).count();
        let total_violations = violations.len();
        let has_severe_violations = violations.iter().any(|v| v.severity == SuitabilitySeverity::Severe);

        let (rating, workflow, summary) = if hook_violations_count > 0 || has_severe_violations || total_violations >= 3 {
            let first_violation = violations.first().map(|v| v.token.as_str()).unwrap_or("sensitive term");
            (
                "YELLOW".to_string(),
                "UNLISTED_MANUAL_REVIEW".to_string(),
                format!(
                    "⚠️ Advertiser Risk Detected ({} flag(s), {} in 0:00–0:15 Hook Window): Triggered by '{}'. Upload as Unlisted ≥ 24 hours prior to public release and request Human Review to protect channel trust score.",
                    total_violations, hook_violations_count, first_violation
                )
            )
        } else if total_violations > 0 {
            (
                "GREEN".to_string(),
                "IMMEDIATE_PUBLIC".to_string(),
                format!(
                    "🟢 Monetization Safe (Minor Body Slang): {} non-hook term(s) detected after 15s. Cleared for standard public ad distribution.",
                    total_violations
                )
            )
        } else {
            (
                "GREEN".to_string(),
                "IMMEDIATE_PUBLIC".to_string(),
                "🟢 Monetization Safe: Zero profanity or advertiser-sensitive terms detected in the 0:00–0:15 Hook Window. Cleared for immediate public monetization.".to_string(),
            )
        };

        VideoSuitabilityReport {
            rating,
            recommended_workflow: workflow,
            hook_window_safe: hook_violations_count == 0,
            violations_count: total_violations,
            hook_violations_count,
            violations,
            executive_summary: summary,
        }
    }

    /// Checks if a text contains a word or phrase, enforcing alphanumeric word boundaries for single words.
    fn contains_word_or_phrase(text: &str, term: &str) -> bool {
        if term.contains(' ') || term.contains('*') {
            return text.contains(term);
        }

        for word in text.split(|c: char| !c.is_alphanumeric()) {
            if word == term {
                return true;
            }
        }
        false
    }
}
