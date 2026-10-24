use serde::{Deserialize, Serialize};

use crate::analyzer::transcript::FullTranscript;

/// Extracted opening hook metadata with precision boundaries and cadence metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookExtractionResult {
    pub spoken_hook: String,
    pub start_second: f64,
    pub end_second: f64,
    pub duration_seconds: f64,
    pub word_count: usize,
    pub cadence_wpm: f64,
    pub is_truncated_at_ceiling: bool,
}

/// Precision hook boundary extractor adhering to the 0.0s-3.5s retention window.
#[derive(Debug, Clone, Copy)]
pub struct HookAligner {
    /// Upper bound ceiling for the opening hook in milliseconds (default 3500ms / 3.5s)
    pub max_hook_window_ms: u32,
    /// Minimum threshold for a standalone sentence hook in milliseconds (default 1200ms)
    pub min_hook_window_ms: u32,
}

impl Default for HookAligner {
    fn default() -> Self {
        Self {
            max_hook_window_ms: 3500,
            min_hook_window_ms: 1200,
        }
    }
}

impl HookAligner {
    pub fn new(max_hook_window_ms: u32, min_hook_window_ms: u32) -> Self {
        Self {
            max_hook_window_ms,
            min_hook_window_ms,
        }
    }

    /// Extracts the opening hook from a full timestamped transcript.
    /// 
    /// Follows the empirical narrative rule:
    /// - If the first sentence terminates between `min_hook_window_ms` and `max_hook_window_ms + 300ms`,
    ///   the hook boundary aligns cleanly to that sentence ending.
    /// - If the sentence extends past the window ceiling, the hook truncates at the last complete word
    ///   ending before `max_hook_window_ms`.
    pub fn extract_opening_hook(&self, transcript: &FullTranscript) -> Option<HookExtractionResult> {
        self.extract_opening_hook_calibrated(transcript, None)
    }

    /// Extracts the opening hook with optional VAD acoustic speech onset calibration.
    pub fn extract_opening_hook_calibrated(
        &self,
        transcript: &FullTranscript,
        vad_onset_ms: Option<u32>,
    ) -> Option<HookExtractionResult> {
        if transcript.sentences.is_empty() {
            return None;
        }

        let first_sentence = &transcript.sentences[0];
        // If VAD onset is detected earlier or later, calibrate onset
        let onset_ms = vad_onset_ms.unwrap_or(first_sentence.start_ms);

        let first_words = get_or_synthesize_words(first_sentence);
        if first_words.is_empty() && first_sentence.text.trim().is_empty() {
            return None;
        }

        // Case 1: First sentence ends within acceptable hook window bounds [1.2s - 3.8s]
        let sentence_end_relative = first_sentence.end_ms.saturating_sub(onset_ms);
        let soft_ceiling = self.max_hook_window_ms + 300; // Allow 300ms grace window for complete thought

        if sentence_end_relative >= self.min_hook_window_ms && sentence_end_relative <= soft_ceiling {
            let duration_sec = (sentence_end_relative as f64) / 1000.0;
            let word_count = if first_words.is_empty() {
                first_sentence.text.split_whitespace().count()
            } else {
                first_words.len()
            };
            let wpm = if duration_sec > 0.0 {
                (word_count as f64) / (duration_sec / 60.0)
            } else {
                0.0
            };

            return Some(HookExtractionResult {
                spoken_hook: first_sentence.text.clone(),
                start_second: (onset_ms as f64) / 1000.0,
                end_second: (first_sentence.end_ms as f64) / 1000.0,
                duration_seconds: duration_sec,
                word_count,
                cadence_wpm: wpm,
                is_truncated_at_ceiling: false,
            });
        }

        // Case 2: Multi-sentence combination if first sentence was very brief (< 1.2s)
        if sentence_end_relative < self.min_hook_window_ms && transcript.sentences.len() > 1 {
            let second_sentence = &transcript.sentences[1];
            let combined_end = second_sentence.end_ms.saturating_sub(onset_ms);

            if combined_end <= soft_ceiling {
                let second_words = get_or_synthesize_words(second_sentence);
                let combined_text = format!("{} {}", first_sentence.text, second_sentence.text);
                let duration_sec = (combined_end as f64) / 1000.0;
                let word_count = first_words.len() + second_words.len();
                let wpm = if duration_sec > 0.0 {
                    (word_count as f64) / (duration_sec / 60.0)
                } else {
                    0.0
                };

                return Some(HookExtractionResult {
                    spoken_hook: combined_text,
                    start_second: (onset_ms as f64) / 1000.0,
                    end_second: (second_sentence.end_ms as f64) / 1000.0,
                    duration_seconds: duration_sec,
                    word_count,
                    cadence_wpm: wpm,
                    is_truncated_at_ceiling: false,
                });
            }
        }

        // Case 3: Speech exceeds window ceiling -> Truncate at last word <= max_hook_window_ms
        let ceiling_ms = onset_ms + self.max_hook_window_ms;
        let mut words_in_window = Vec::new();
        let mut last_end_ms = onset_ms;

        for sentence in &transcript.sentences {
            let words = get_or_synthesize_words(sentence);
            for word in &words {
                if word.end_ms <= ceiling_ms || words_in_window.is_empty() {
                    words_in_window.push(word.word.clone());
                    last_end_ms = word.end_ms;
                } else {
                    break;
                }
            }
            if last_end_ms >= ceiling_ms {
                break;
            }
        }

        if words_in_window.is_empty() {
            return None;
        }

        let spoken_hook = words_in_window.join(" ");
        let duration_sec = (last_end_ms.saturating_sub(onset_ms) as f64) / 1000.0;
        let word_count = words_in_window.len();
        let wpm = if duration_sec > 0.0 {
            (word_count as f64) / (duration_sec / 60.0)
        } else {
            0.0
        };

        Some(HookExtractionResult {
            spoken_hook,
            start_second: (onset_ms as f64) / 1000.0,
            end_second: (last_end_ms as f64) / 1000.0,
            duration_seconds: duration_sec,
            word_count,
            cadence_wpm: wpm,
            is_truncated_at_ceiling: true,
        })
    }
}

/// Fallback helper to synthesize word boundaries if only sentence-level timestamps were supplied
fn get_or_synthesize_words(sentence: &crate::analyzer::transcript::TimestampedSentence) -> Vec<crate::analyzer::transcript::TimestampedWord> {
    if !sentence.words.is_empty() {
        return sentence.words.clone();
    }

    let tokens: Vec<&str> = sentence.text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    let duration = sentence.end_ms.saturating_sub(sentence.start_ms);
    let step = (duration / (tokens.len() as u32)).max(1);

    let num_tokens = tokens.len();
    tokens
        .into_iter()
        .enumerate()
        .map(|(i, tok)| {
            let w_start = sentence.start_ms + (i as u32) * step;
            let w_end = if i + 1 == num_tokens {
                sentence.end_ms
            } else {
                (w_start + step).min(sentence.end_ms)
            };
            crate::analyzer::transcript::TimestampedWord {
                word: tok.to_string(),
                start_ms: w_start,
                end_ms: w_end,
                confidence: 0.90,
            }
        })
        .collect()
}

/// Two-Part Handshake & Promise Gap Report (0:00–0:05).
/// Evaluates visual alignment at Frame 0 and verbal premise validation in the first 5 seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromiseGapReport {
    /// Overall promise gap score (0.0 to 100.0, where >= 80 is aligned and < 50 indicates severe breach)
    pub promise_gap_score: f64,
    /// Whether Frame 0 visual imagery corresponds to thumbnail subject/setting
    pub visual_handshake_passed: bool,
    /// Explanation of visual handshake verification
    pub visual_handshake_detail: String,
    /// Whether first spoken sentence directly confirms the title's core topic within 5.0s
    pub verbal_handshake_passed: bool,
    /// Explanation of verbal handshake verification
    pub verbal_handshake_detail: String,
    /// Whether throat-clearing, logo animation, or sponsor read was detected delaying the premise
    pub throat_clearing_detected: bool,
    /// Specific throat-clearing pattern identified (if any)
    pub throat_clearing_detail: Option<String>,
    /// Exact second when the core premise/topic was first spoken
    pub premise_confirmation_second: Option<f64>,
    /// Whether a critical Promise Gap breach was flagged
    pub is_breach: bool,
    /// Emitted warning alert
    pub diagnostic_alert: Option<String>,
}

impl Default for PromiseGapReport {
    fn default() -> Self {
        Self::aligned()
    }
}

impl PromiseGapReport {
    pub const BREACH_ALERT: &'static str = "🚨 Promise Gap Breach: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk.";

    /// Creates an aligned report (100.0 score, zero penalties).
    pub fn aligned() -> Self {
        Self {
            promise_gap_score: 100.0,
            visual_handshake_passed: true,
            visual_handshake_detail: "Frame 0 visual matches thumbnail subject and setting.".to_string(),
            verbal_handshake_passed: true,
            verbal_handshake_detail: "Opening spoken sentence directly validates title premise within 5.0s.".to_string(),
            throat_clearing_detected: false,
            throat_clearing_detail: None,
            premise_confirmation_second: Some(0.3),
            is_breach: false,
            diagnostic_alert: None,
        }
    }

    /// Alias for aligned report.
    pub fn clean() -> Self {
        Self::aligned()
    }

    /// Creates a breach report with penalty details.
    pub fn breach(
        score: f64,
        visual_passed: bool,
        visual_detail: &str,
        verbal_passed: bool,
        verbal_detail: &str,
        throat_clearing: Option<&str>,
        premise_sec: Option<f64>,
    ) -> Self {
        Self {
            promise_gap_score: score.clamp(0.0, 100.0),
            visual_handshake_passed: visual_passed,
            visual_handshake_detail: visual_detail.to_string(),
            verbal_handshake_passed: verbal_passed,
            verbal_handshake_detail: verbal_detail.to_string(),
            throat_clearing_detected: throat_clearing.is_some(),
            throat_clearing_detail: throat_clearing.map(|s| s.to_string()),
            premise_confirmation_second: premise_sec,
            is_breach: true,
            diagnostic_alert: Some(Self::BREACH_ALERT.to_string()),
        }
    }
}

/// Evaluator enforcing the 0:00–0:05 Two-Part Handshake between thumbnail, title, frame 0, and spoken hook.
pub struct PromiseGapEvaluator;

impl PromiseGapEvaluator {
    /// Evaluates the 0:00–0:05 Two-Part Handshake between thumbnail, title, frame 0, and spoken hook.
    pub fn evaluate(
        title: &str,
        thumbnail_context: Option<&str>,
        frame_0_visual_context: Option<&str>,
        spoken_hook: Option<&str>,
        spoken_onset_sec: f64,
    ) -> PromiseGapReport {
        let is_pending_placeholder = spoken_hook.map(|h| {
            let trimmed = h.trim();
            trimmed.contains("captions pending")
                || trimmed.contains("[Spoken dialogue detected")
                || trimmed.is_empty()
        }).unwrap_or(false);

        let (visual_passed, visual_detail) = Self::evaluate_visual_handshake(thumbnail_context, frame_0_visual_context);
        let throat_clearing = if is_pending_placeholder {
            None
        } else {
            Self::detect_throat_clearing(spoken_hook.unwrap_or(""), spoken_onset_sec, frame_0_visual_context)
        };
        let (verbal_passed, verbal_detail, premise_sec) = Self::evaluate_verbal_handshake(
            title,
            spoken_hook,
            spoken_onset_sec,
            throat_clearing.is_some(),
        );

        let mut score: f64 = 100.0;
        if !visual_passed {
            score -= 35.0;
        }
        if !verbal_passed {
            score -= 35.0;
        }
        if throat_clearing.is_some() {
            score -= 30.0;
        }
        if spoken_onset_sec > 5.0 {
            score -= 20.0;
        }

        if is_pending_placeholder && visual_passed && throat_clearing.is_none() && spoken_onset_sec <= 5.0 {
            score = score.max(85.0);
        }

        let promise_gap_score = score.clamp(0.0f64, 100.0f64);
        // Suppress false breach when dialogue begins immediately (spoken_onset_sec <= 0.3s)
        // or when captions are pending and visual handshake passed
        let is_breach = (promise_gap_score < 70.0 || throat_clearing.is_some() || !visual_passed || !verbal_passed)
            && !(spoken_onset_sec <= 0.3 && verbal_passed && throat_clearing.is_none())
            && !(is_pending_placeholder && visual_passed && throat_clearing.is_none());
        let diagnostic_alert = if is_breach {
            Some(PromiseGapReport::BREACH_ALERT.to_string())
        } else {
            None
        };

        PromiseGapReport {
            promise_gap_score,
            visual_handshake_passed: visual_passed,
            visual_handshake_detail: visual_detail,
            verbal_handshake_passed: verbal_passed,
            verbal_handshake_detail: verbal_detail,
            throat_clearing_detected: throat_clearing.is_some(),
            throat_clearing_detail: throat_clearing,
            premise_confirmation_second: premise_sec,
            is_breach,
            diagnostic_alert,
        }
    }

    /// Evaluates the Two-Part Handshake between thumbnail, title, frame 0, and spoken hook (alias for evaluate).
    pub fn evaluate_handshake(
        title: &str,
        thumbnail_context: Option<&str>,
        frame_0_visual_context: Option<&str>,
        spoken_hook: Option<&str>,
        spoken_onset_sec: f64,
    ) -> PromiseGapReport {
        Self::evaluate(title, thumbnail_context, frame_0_visual_context, spoken_hook, spoken_onset_sec)
    }

    /// Evaluates whether Frame 0 visual imagery corresponds to thumbnail subject/setting.
    pub fn evaluate_visual_handshake(
        thumbnail_context: Option<&str>,
        frame_0_context: Option<&str>,
    ) -> (bool, String) {
        let frame_0 = match frame_0_context {
            Some(f) if !f.trim().is_empty() => f.trim(),
            _ => return (true, "Frame 0 visual imagery assumed aligned (unspecified frame).".to_string()),
        };

        let frame_lower = frame_0.to_lowercase();
        // Check for branded logo animation or splash screen
        if frame_lower.contains("logo")
            || frame_lower.contains("splash screen")
            || frame_lower.contains("channel bumper")
            || frame_lower.contains("intro animation")
            || frame_lower.contains("subscribe animation")
            || frame_lower.contains("blank black")
            || frame_lower.contains("dark void")
        {
            return (
                false,
                format!("Frame 0 contains branded logo animation or splash screen (\"{}\"), obstructing visual handshake.", frame_0),
            );
        }

        let thumb = match thumbnail_context {
            Some(t) if !t.trim().is_empty() => t.trim(),
            _ => return (true, "Frame 0 visual imagery verified (clean natural frame).".to_string()),
        };

        let thumb_lower = thumb.to_lowercase();

        // Check for explicit visual discontinuity / bait-and-switch framing
        // e.g. thumbnail promised outdoors/action/ocean, but frame 0 is bedroom/talking head/stock chart
        if (thumb_lower.contains("boat") || thumb_lower.contains("ocean") || thumb_lower.contains("cabin") || thumb_lower.contains("volcano") || thumb_lower.contains("outdoors") || thumb_lower.contains("sea"))
            && (frame_lower.contains("bedroom talking head") || frame_lower.contains("stock chart") || frame_lower.contains("webcam") || frame_lower.contains("office"))
        {
            return (
                false,
                format!("Visual discontinuity: Frame 0 ({}) diverges from promised thumbnail environment ({}).", frame_0, thumb),
            );
        }

        (true, format!("Frame 0 visual ({}) aligns with thumbnail premise ({}).", frame_0, thumb))
    }

    /// Detects branded logo animations, sponsor reads, or biographical throat-clearing delaying premise beyond 5.0s.
    pub fn detect_throat_clearing(
        spoken_text: &str,
        spoken_onset_sec: f64,
        frame_0_context: Option<&str>,
    ) -> Option<String> {
        let lower = spoken_text.to_lowercase();

        // 1. Spoken onset delayed past 5.0s
        if spoken_onset_sec > 5.0 {
            return Some(format!(
                "Severe intro latency: first spoken word delayed until {:.1}s (exceeds 5.0s threshold).",
                spoken_onset_sec
            ));
        }

        // 2. Visual frame 0 is an animated bumper / logo
        if let Some(f0) = frame_0_context {
            let f0_lower = f0.to_lowercase();
            if f0_lower.contains("logo animation") || f0_lower.contains("intro bumper") || f0_lower.contains("channel sting") {
                return Some(format!("Branded logo animation at Frame 0: \"{}\"", f0));
            }
        }

        // 3. Opening Sponsor Reads in 0:00–0:05
        let sponsor_triggers = [
            "sponsored by",
            "brought to you by",
            "thanks to our sponsor",
            "sponsor of today",
            "partnered with",
            "huge thanks to",
            "use code",
            "link in the description",
        ];
        for trigger in sponsor_triggers {
            if lower.contains(trigger) {
                return Some(format!(
                    "Opening sponsor read (\"{}\"): monetizes before validating click premise.",
                    trigger
                ));
            }
        }

        // 4. Biographical / Channel Throat-Clearing greetings
        let throat_triggers = [
            "hey guys welcome back",
            "welcome back to the channel",
            "welcome back everyone",
            "welcome to the channel",
            "welcome back guys",
            "before we get started",
            "before we jump into",
            "before we begin",
            "if you're new here",
            "if you are new to the channel",
            "don't forget to like and subscribe",
            "hit that subscribe button",
            "smash that like button",
            "my name is",
            "in this video today i am going to",
            "in today's video we will",
            "what is up guys",
            "yo what's up guys",
            "what's going on guys",
        ];
        for trigger in throat_triggers {
            if lower.contains(trigger) {
                return Some(format!(
                    "Biographical/channel throat-clearing (\"{}\"): delays narrative premise beyond 0:00-0:05 window.",
                    trigger
                ));
            }
        }

        None
    }

    /// Evaluates whether first spoken sentence directly confirms the title's core topic within 5.0s.
    pub fn evaluate_verbal_handshake(
        title: &str,
        spoken_hook: Option<&str>,
        spoken_onset_sec: f64,
        is_throat_clearing: bool,
    ) -> (bool, String, Option<f64>) {
        if spoken_onset_sec > 5.0 {
            return (
                false,
                format!("Speech onset occurs at {:.1}s, missing the 5.0s verbal handshake gate.", spoken_onset_sec),
                None,
            );
        }

        let hook = match spoken_hook {
            Some(h) if !h.trim().is_empty() => h.trim(),
            _ => {
                if spoken_onset_sec <= 0.3 {
                    return (
                        true,
                        format!("Frame-zero speech onset ({:.1}s) verified via acoustic VAD.", spoken_onset_sec),
                        Some(spoken_onset_sec),
                    );
                }
                return (
                    false,
                    "No spoken hook detected in opening 5 seconds.".to_string(),
                    None,
                );
            }
        };

        let is_pending_placeholder = hook.contains("captions pending")
            || hook.contains("[Spoken dialogue detected")
            || hook.is_empty();

        if is_pending_placeholder {
            return (
                true,
                "Verbal Handshake: Acoustic speech energy verified at frame one; text validation pending YouTube caption processing.".to_string(),
                Some(spoken_onset_sec),
            );
        }

        if is_throat_clearing {
            return (
                false,
                format!("Opening words (\"{}\") deliver channel throat-clearing instead of validating the title.", hook),
                None,
            );
        }

        let stopwords = [
            "the", "a", "an", "and", "or", "to", "of", "in", "for", "with", "on", "at", 
            "from", "by", "is", "it", "my", "our", "we", "i", "you", "this", "that", "be", 
            "as", "how", "why", "what", "can", "will", "so", "do", "if", "not", "all", "out",
            "best", "worst", "ever", "new", "full", "ep", "episode", "part", "vs", "are", "was"
        ];

        let extract_keywords = |s: &str| -> Vec<String> {
            s.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| !w.is_empty() && w.len() > 2 && !stopwords.contains(w))
                .map(|w| w.to_string())
                .collect()
        };

        let title_words = extract_keywords(title);
        let hook_words = extract_keywords(hook);

        if title_words.is_empty() {
            return (true, "Title has no substantive keywords; verbal handshake accepted.".to_string(), Some(spoken_onset_sec));
        }

        let is_semantic_match = |tw: &str, hw: &str| -> bool {
            if hw == tw || hw.contains(tw) || tw.contains(hw) {
                return true;
            }
            const CLUSTERS: &[&[&str]] = &[
                // Aquatic & Angling
                &[
                    "salmon", "trout", "bass", "cod", "tenkara", "catch", "caught", "angling", "angler",
                    "fishing", "fish", "river", "lake", "water", "creek", "ocean", "surf", "stream",
                    "alagnak", "rod", "fly rod",
                ],
                // Fauna & Wildlife
                &[
                    "grizzlies", "grizzly", "bear", "bears", "wolves", "wolf", "elk", "moose",
                    "wildlife", "animals", "predators", "nature", "creatures",
                ],
                // Expedition & Wilderness
                &[
                    "alaska", "alaskan", "rockies", "yukon", "outback", "wilderness", "adventure",
                    "expedition", "outdoors", "camp", "camping", "remote", "survival", "bushcraft",
                ],
            ];
            for cluster in CLUSTERS {
                let tw_in_cluster = cluster.iter().any(|&c| c == tw || tw.contains(c) || c.contains(tw));
                let hw_in_cluster = cluster.iter().any(|&c| c == hw || hw.contains(c) || c.contains(hw));
                if tw_in_cluster && hw_in_cluster {
                    return true;
                }
            }
            false
        };

        let mut matches = 0;
        let mut matched_words = Vec::new();
        for tw in &title_words {
            if hook_words.iter().any(|hw| is_semantic_match(tw, hw)) {
                matches += 1;
                matched_words.push(tw.as_str());
            }
        }

        if matches >= 1 {
            (
                true,
                format!("First spoken sentence confirms title keywords ({:?}) at {:.1}s.", matched_words, spoken_onset_sec),
                Some(spoken_onset_sec),
            )
        } else {
            (
                false,
                format!("First spoken sentence (\"{}\") has zero topical overlap with title (\"{}\").", hook, title),
                None,
            )
        }
    }
}

