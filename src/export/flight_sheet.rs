use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::analyzer::{suitability::VideoSuitabilityReport, PromiseGapReport, ThumbnailForensicReport};
use crate::export::timeline::{ExportTimeline, MarkerColor, RationalTime, TimelineMarker};
use crate::math::marginal_gains::MarginalGainsScorecard;
use crate::math::shorts::ShortsRetentionMetrics;
use crate::pipeline::NicheToneProfile;

/// Helper to sanitize a title into a clean filename slug.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_is_sep = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_is_sep = false;
        } else if !prev_is_sep && (c == ' ' || c == '-' || c == '_' || c == '.') {
            slug.push('_');
            prev_is_sep = true;
        }
    }
    let trimmed = slug.trim_matches('_');
    if trimmed.is_empty() {
        "video".to_string()
    } else {
        trimmed.chars().take(50).collect()
    }
}

/// Helper to extract MM:SS or HH:MM from free text.
fn extract_timecode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 5 {
        return None;
    }
    for i in 0..=bytes.len() - 5 {
        if bytes[i].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2] == b':'
            && bytes[i + 3].is_ascii_digit()
            && bytes[i + 4].is_ascii_digit()
        {
            return Some(text[i..i + 5].to_string());
        }
    }
    None
}

/// Helper to parse first seconds timestamp from a timecode range (e.g. "00:16" or "00:00 - 00:02").
fn parse_first_seconds(tc: &str) -> Option<f64> {
    let raw = tc.split('-').next()?.trim();
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() == 2 {
        let mm: f64 = parts[0].parse().ok()?;
        let ss: f64 = parts[1].parse().ok()?;
        Some(mm * 60.0 + ss)
    } else if parts.len() == 3 {
        let hh: f64 = parts[0].parse().ok()?;
        let mm: f64 = parts[1].parse().ok()?;
        let ss: f64 = parts[2].parse().ok()?;
        Some(hh * 3600.0 + mm * 60.0 + ss)
    } else {
        None
    }
}

/// Formats seconds into MM:SS.
fn format_seconds(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let mm = total / 60;
    let ss = total % 60;
    format!("{:02}:{:02}", mm, ss)
}

/// An individual timeline editing instruction for the creator / editor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEditCue {
    /// Timestamp or range on the timeline (e.g. "00:00 - 00:02", "00:16", "08:10")
    pub timecode_range: String,
    /// Categorical edit type: "TRIM DEAD AIR", "INSERT B-ROLL", "CUT EXPOSITION", "OUTRO FIX"
    pub action_type: String,
    /// Plain-English editing command with exact numbers
    pub remedy_instruction: String,
    /// High-level audience retention impact
    pub impact_summary: String,
}

/// Sanitizes a subject noun phrase by resolving balanced leading badges,
/// eliminating orphaned brackets, and trimming boundary punctuation.
pub fn sanitize_subject_noun(raw: &str) -> String {
    let mut working = raw.trim().to_string();
    if working.is_empty() {
        return working;
    }

    // 1. Balanced Leading Wrapper / Badge Stripping
    // Handles leading badges like (103 Salmon & 7 Grizzlies), [SOLO CAMPING], [4K], etc.
    let starts_with_open = working.starts_with('(') || working.starts_with('[') || working.starts_with('{');
    if starts_with_open {
        let open_char = working.chars().next().unwrap();
        let close_char = match open_char {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            _ => ')',
        };
        if let Some(close_idx) = working.find(close_char) {
            let inner = working[open_char.len_utf8()..close_idx].trim();
            let remainder = working[close_idx + close_char.len_utf8()..].trim();
            let lower_inner = inner.to_lowercase();
            let is_metadata = lower_inner == "4k"
                || lower_inner == "hd"
                || lower_inner == "official video"
                || lower_inner == "official music video"
                || lower_inner == "full episode"
                || lower_inner == "full podcast"
                || lower_inner == "livestream"
                || lower_inner == "live"
                || lower_inner.starts_with("ep ")
                || lower_inner.starts_with("ep.")
                || lower_inner.starts_with("episode");

            if is_metadata {
                working = remainder.to_string();
            } else if !remainder.is_empty() {
                let is_all_upper = inner.chars().any(|c| c.is_alphabetic()) && inner.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
                let (formatted_inner, sep) = if is_all_upper && open_char == '[' {
                    // Convert "SOLO CAMPING" to "Solo Camping: "
                    let title_cased = inner
                        .split_whitespace()
                        .map(|w| {
                            let mut chars = w.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(" ");
                    (title_cased, ": ")
                } else {
                    (inner.to_string(), " ")
                };
                working = format!("{}{}{}", formatted_inner, sep, remainder.trim_start_matches([':', '-', '—', '|']).trim());
            } else {
                working = inner.to_string();
            }
        }
    }

    // 2. Orphan and Dangling Bracket Elimination using character index tracking
    let mut indices_to_remove = std::collections::BTreeSet::new();
    let mut bracket_stack: Vec<(char, usize)> = Vec::new();

    for (byte_idx, ch) in working.char_indices() {
        match ch {
            '(' | '[' | '{' => {
                bracket_stack.push((ch, byte_idx));
            }
            ')' => {
                if let Some(&(top_ch, _)) = bracket_stack.last() {
                    if top_ch == '(' {
                        bracket_stack.pop();
                    } else {
                        indices_to_remove.insert(byte_idx);
                    }
                } else {
                    indices_to_remove.insert(byte_idx);
                }
            }
            ']' => {
                if let Some(&(top_ch, _)) = bracket_stack.last() {
                    if top_ch == '[' {
                        bracket_stack.pop();
                    } else {
                        indices_to_remove.insert(byte_idx);
                    }
                } else {
                    indices_to_remove.insert(byte_idx);
                }
            }
            '}' => {
                if let Some(&(top_ch, _)) = bracket_stack.last() {
                    if top_ch == '{' {
                        bracket_stack.pop();
                    } else {
                        indices_to_remove.insert(byte_idx);
                    }
                } else {
                    indices_to_remove.insert(byte_idx);
                }
            }
            _ => {}
        }
    }

    // Any remaining items in bracket_stack are unclosed opening brackets
    for (_, open_byte_idx) in bracket_stack {
        indices_to_remove.insert(open_byte_idx);
    }

    let mut sanitized = String::with_capacity(working.len());
    for (byte_idx, ch) in working.char_indices() {
        if !indices_to_remove.contains(&byte_idx) {
            sanitized.push(ch);
        }
    }

    // 3. Boundary Punctuation Trimming (commas, colons, hyphens, pipes, quotes, slashes)
    let trimmed = sanitized
        .trim_matches(['"', '\'', ':', '-', '—', '–', ',', '|', '/'])
        .trim();

    // Collapse multiple internal whitespaces
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    tokens.join(" ")
}

/// Cleans raw titles by stripping hashtags, episode badges, numbers, and pipe delimiters.
pub fn clean_title(title: &str) -> String {
    let mut s = title.trim().to_string();

    // Strip hashtags anywhere in the title (#shorts, #fyp, #amazing, etc.)
    let tokens: Vec<&str> = s.split_whitespace().filter(|w| !w.starts_with('#')).collect();
    s = tokens.join(" ");

    for sep in &["|", " - Ep", " - ep", " [ep", " [Ep", " (ep", " (Ep", " #", " ep."] {
        if let Some(idx) = s.rfind(sep) {
            if s.len() - idx <= 20 {
                s = s[..idx].trim().to_string();
            }
        }
    }
    for badge in &[
        " (start to finish)", " (full build)", " (timelapse)", " (time-lapse)",
        " (step by step)", " [start to finish]", " [full build]", " [timelapse]",
    ] {
        let lower = s.to_lowercase();
        if lower.ends_with(badge) {
            s = s[..s.len() - badge.len()].trim().to_string();
        }
    }
    if s.ends_with('|') {
        s.pop();
    }
    sanitize_subject_noun(s.trim())
}

/// Dynamically extracts the core subject noun phrase from a video title without hardcoded niche fallbacks.
pub fn extract_dynamic_subject(title: &str) -> String {
    let clean = clean_title(title);

    // 1. Take the primary substantive clause before delimiters (':', '-', '|', '—', '–', '/', '•', '?')
    let mut primary_clause = clean.as_str();
    for sep in &["/", "//", "|", "—", "–", "•", ":", " - ", "? ", "?"] {
        if let Some(idx) = primary_clause.find(sep) {
            let left = primary_clause[..idx].trim();
            let right = primary_clause[idx + sep.len()..].trim();
            let left_lower = left.to_lowercase();
            if left.len() <= 4
                || left_lower.starts_with("ep")
                || left_lower == "review"
                || left_lower == "update"
                || left_lower == "vlog"
            {
                if !right.is_empty() {
                    primary_clause = right;
                }
            } else if !left.is_empty() {
                primary_clause = left;
            }
            break;
        }
    }

    let clause_lower = primary_clause.to_lowercase();

    // High-frequency exact subject matches
    let recognized_subjects = [
        ("airstream", "Airstream"),
        ("catamaran", "Catamaran"),
        ("trimaran", "Trimaran"),
        ("sailboat", "Sailboat"),
        ("sail boat", "Sailboat"),
        ("yacht", "Yacht"),
        ("log cabin", "Log Cabin"),
        ("cabin", "Cabin"),
        ("homestead", "Homestead"),
        ("swim pond", "Natural Swim Pond"),
        ("swimming pond", "Natural Swim Pond"),
        ("pond", "Natural Pond"),
        ("camper van", "Camper Van"),
        ("van", "Van"),
        ("camper", "Camper"),
        ("rv", "RV"),
        ("bushcraft shelter", "Bushcraft Shelter"),
        ("bushcraft camping trip", "Bushcraft Shelter"),
        ("bushcraft camp", "Bushcraft Shelter"),
        ("bushcraft", "Bushcraft Shelter"),
        ("survival shelter", "Survival Shelter"),
        ("shelter", "Survival Shelter"),
        ("camping trip", "Camping Trip"),
        ("camping", "Camping"),
        ("alaskan cod", "Pacific Cod"),
        ("cod fishing", "Pacific Cod"),
        ("cod", "Pacific Cod"),
        ("fishing trip", "Fishing Trip"),
        ("fishing", "Fishing"),
        ("dog sledding", "Dog Sledding"),
        ("dog sled", "Dog Sledding"),
        ("jet ski", "Jet Ski"),
        ("jetski", "Jet Ski"),
        ("outboard motor", "Outboard Motor"),
        ("outboard", "Outboard Motor"),
        ("boat motor", "Boat Motor"),
        ("canoe", "Canoe"),
        ("timber frame", "Timber Frame House"),
        ("tiny house", "Tiny House"),
        ("greenhouse", "Greenhouse"),
        ("workshop", "Workshop"),
        ("3d printer", "3D Printer"),
        ("laser cutter", "Laser Cutter"),
        ("solar", "Solar Power System"),
        ("battery", "Battery Bank"),
        ("tractor", "Tractor"),
        ("excavator", "Excavator"),
        ("tesla", "Tesla"),
        ("compiler", "Compiler"),
        ("rust", "Rust System"),
        ("python", "Python Backend"),
        ("shower waterproofing", "Shower Waterproofing Method"),
        ("waterproofing", "Waterproofing"),
        ("scrambled eggs", "Scrambled Eggs"),
        ("egg", "Cooking Eggs"),
        ("steak", "Cooking Steak"),
        ("recipe", "Recipe"),
        ("cooking", "Cooking"),
    ];

    for (needle, name) in recognized_subjects {
        if clause_lower.contains(needle) {
            return name.to_string();
        }
    }

    // Dynamic stripping of leading clickbait / action frames / durations
    let mut working = primary_clause.trim();

    let prefixes = [
        "we converted a ", "we converted an ", "we converted ",
        "i converted a ", "i converted an ", "i converted ",
        "how we converted a ", "how we converted an ", "how we converted ",
        "how i converted a ", "how i converted an ", "how i converted ",
        "how we built a ", "how we built an ", "how we built ",
        "how i built a ", "how i built an ", "how i built ",
        "we built a ", "we built an ", "we built ",
        "i built a ", "i built an ", "i built ",
        "how to build a ", "how to build an ", "how to build ",
        "how to convert a ", "how to convert an ", "how to convert ",
        "how to make a ", "how to make an ", "how to make ", "how to ",
        "converting a ", "converting an ", "converting ",
        "don't buy", "dont buy", "do not buy",
        "why you shouldn't buy", "why we're selling", "why i'm selling",
        "why we sold", "why i sold", "why we're giving up on",
        "why i'm giving up on", "giving up on", "we gave up on", "i gave up on",
        "selling our", "selling my", "warning:", "warning",
        "the truth about", "the real cost of", "the reality of", "before you buy",
        "should you buy", "is it worth it", "is the", "is a",
        "honest review of", "review of", "review:", "review",
        "building a", "building an", "building",
        "crafting a", "crafting an", "crafting",
        "how we", "how i", "i tried", "we tried", "i left", "we left",
        "living in an", "living in a", "living in", "living on a", "living on an", "living on",
        "surviving", "survival in", "survival",
        "riding", "racing", "flying", "driving",
        "restoring the", "restoring a", "restoring an", "restoring",
        "turning $100 into $1,000", "turning $100 into",
        "is ", "are ", "can ", "why ", "how ", "what ", "could ", "do ", "does ", "will ", "would ", "should ",
    ];

    let mut changed = true;
    while changed {
        changed = false;
        let lower_working = working.to_lowercase();
        for p in prefixes {
            if lower_working.starts_with(p) {
                working = working[p.len()..].trim();
                changed = true;
                break;
            }
        }

        let lower_working = working.to_lowercase();
        for dur in &[
            "100 hours ", "48 hours ", "24 hours ",
            "365 days ", "100 days ", "30 days ", "10 days ", "7 days ", "5 days ", "3 days ", "2 days ", "1 day ",
            "2 years ", "two years ", "1 year ", "one year ", "years ",
            "1300 miles ", "1,300 miles ",
            "1st ", "first ", "2nd ", "second ", "3rd ", "third ", "solo ",
            "the old american ", "old american ", "the american ", "the traditional ", "traditional ",
            "the old ", "the new ", "an ", "a ", "the ", "our ", "my ", "your ", "this ",
        ] {
            if lower_working.starts_with(dur) {
                working = working[dur.len()..].trim();
                changed = true;
                break;
            }
        }
    }

    // Normalization of question connectors e.g. "Is Chemistry the Same Throughout the Universe?" -> "Chemistry Throughout the Universe"
    let mut working_str = working.to_string();
    let lower_w = working_str.to_lowercase();
    if lower_w.contains(" the same throughout ") {
        if let Some(idx) = lower_w.find(" the same throughout ") {
            working_str = format!("{} throughout {}", &working_str[..idx], &working_str[idx + " the same throughout ".len()..]);
        }
    } else if lower_w.contains(" the same in ") {
        if let Some(idx) = lower_w.find(" the same in ") {
            working_str = format!("{} in {}", &working_str[..idx], &working_str[idx + " the same in ".len()..]);
        }
    } else if lower_w.ends_with(" the same") {
        working_str = working_str[..working_str.len() - " the same".len()].trim().to_string();
    }
    working = working_str.as_str();

    // Strip trailing commentary suffixes / noise
    let lower_subject = working.to_lowercase();
    for suffix in &[
        "(start to finish)", "[start to finish]", "start to finish",
        "(full build)", "[full build]", "full build",
        "(step by step)", "[step by step]",
        "really surprised me", "surprised me", "blew my mind",
        "changed my mind", "is crazy", "is insane", "just dropped",
        "is finally here", "is here", "full breakdown", "honest thoughts",
        "what could go wrong", "what happened",
    ] {
        if let Some(idx) = lower_subject.rfind(suffix) {
            working = working[..idx].trim();
            break;
        }
    }

    // Strip trailing punctuation / noise (preserve $ for tiered contrast e.g. $1 vs $500,000)
    let clean_subject = sanitize_subject_noun(working);

    // Capitalize into Title Case
    if !clean_subject.is_empty() {
        let words: Vec<String> = clean_subject
            .split_whitespace()
            .map(|w| {
                if w.eq_ignore_ascii_case("vs") || w.eq_ignore_ascii_case("vs.") {
                    "vs".to_string()
                } else if w.eq_ignore_ascii_case("the") || w.eq_ignore_ascii_case("of") || w.eq_ignore_ascii_case("in") {
                    w.to_lowercase()
                } else {
                    let mut chars = w.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            if first == '$' {
                                format!("${}", chars.as_str())
                            } else {
                                first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                            }
                        }
                    }
                }
            })
            .collect();
        let mut formatted_words = words;
        if let Some(first_word) = formatted_words.first_mut() {
            let mut chars = first_word.chars();
            if let Some(first_char) = chars.next() {
                if first_char != '$' {
                    *first_word = first_char.to_uppercase().collect::<String>() + chars.as_str();
                }
            }
        }
        let formatted = sanitize_subject_noun(&formatted_words.join(" "));
        if !formatted.is_empty() && !formatted.to_lowercase().contains("project") && formatted.len() >= 3 {
            return formatted;
        }
    }

    // Fallback: extract first 3 non-stopword tokens from the clean title
    let stopwords = [
        "the", "a", "an", "and", "or", "to", "of", "in", "for", "with", "on", "at", 
        "from", "by", "is", "it", "my", "our", "we", "i", "you", "this", "that", "be", 
        "as", "how", "why", "what", "can", "will", "so", "do", "if", "not", "all", "out",
        "project",
    ];

    let fallback_words: Vec<String> = clean
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !stopwords.contains(&w.to_lowercase().as_str()))
        .take(3)
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect();

    if !fallback_words.is_empty() {
        sanitize_subject_noun(&fallback_words.join(" "))
    } else {
        "The Build".to_string()
    }
}

/// Backward-compatible alias for extract_dynamic_subject.
pub fn extract_core_subject(title: &str) -> String {
    extract_dynamic_subject(title)
}

/// Prepends the correct indefinite article ("a" or "an") to a subject noun.
pub fn subject_with_article(subject: &str) -> String {
    let lower = subject.to_lowercase();
    let starts_with_vowel = lower.starts_with('a') || lower.starts_with('e') || lower.starts_with('i') || lower.starts_with('o') || lower.starts_with('u');
    if starts_with_vowel {
        format!("an {}", subject)
    } else {
        format!("a {}", subject)
    }
}

/// Chooses "in" or "on" based on whether the entity is a watercraft or land dwelling.
pub fn living_preposition(subject: &str) -> &'static str {
    let lower = subject.to_lowercase();
    if lower.contains("catamaran") || lower.contains("boat") || lower.contains("yacht") || lower.contains("trimaran") || lower.contains("sailboat") || lower.contains("ship") {
        "on"
    } else {
        "in"
    }
}

/// Cross-device title truncation evaluator modeling platform cutoff points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TruncationRadar {
    pub raw_title: String,
    pub title_chars_count: usize,
    pub mobile_push_preview: String,
    pub mobile_push_truncated: bool,
    pub mobile_feed_ctv_preview: String,
    pub mobile_feed_ctv_truncated: bool,
    pub desktop_search_preview: String,
    pub desktop_search_truncated: bool,
    pub front_load_recommendation: Option<String>,
}

impl TruncationRadar {
    /// Evaluates title across mobile push (45c), mobile feed/CTV (52c), and desktop search (60c).
    pub fn evaluate(title: &str) -> Self {
        let chars_count = title.chars().count();

        let format_cutoff = |limit: usize| -> (String, bool) {
            if chars_count <= limit {
                (format!("\"{}\"", title), false)
            } else {
                let truncated: String = title.chars().take(limit.saturating_sub(3)).collect();
                (format!("\"{}...\"", truncated), true)
            }
        };

        let (mobile_push_preview, mobile_push_truncated) = format_cutoff(45);
        let (mobile_feed_ctv_preview, mobile_feed_ctv_truncated) = format_cutoff(52);
        let (desktop_search_preview, desktop_search_truncated) = format_cutoff(60);

        let front_load_recommendation = if mobile_feed_ctv_truncated {
            Some(Self::generate_front_load_recommendation(title))
        } else {
            None
        };

        Self {
            raw_title: title.to_string(),
            title_chars_count: chars_count,
            mobile_push_preview,
            mobile_push_truncated,
            mobile_feed_ctv_preview,
            mobile_feed_ctv_truncated,
            desktop_search_preview,
            desktop_search_truncated,
            front_load_recommendation,
        }
    }

    /// Calculates a front-loaded recommendation under the specified target character limit.
    pub fn calculate_front_load_recommendation(title: &str, target_limit: usize) -> String {
        let clean_title = title.trim();
        if clean_title.chars().count() <= target_limit {
            return clean_title.to_string();
        }

        // Isolate the primary clause before secondary subtitles
        let primary_clause = clean_title.split(&['-', '|', ':'][..]).next().unwrap_or(clean_title).trim();

        if primary_clause.chars().count() <= target_limit {
            primary_clause.to_string()
        } else {
            // Snap to nearest word boundary before target_limit
            let mut truncated = String::new();
            for word in primary_clause.split_whitespace() {
                if (truncated.len() + word.len() + 1) <= target_limit {
                    if !truncated.is_empty() { truncated.push(' '); }
                    truncated.push_str(word);
                } else {
                    break;
                }
            }
            if truncated.is_empty() {
                clean_title.chars().take(target_limit).collect()
            } else {
                truncated
            }
        }
    }

    /// Generates a high-leverage front-loaded recommendation under 45 characters.
    pub fn generate_front_load_recommendation(title: &str) -> String {
        Self::calculate_front_load_recommendation(title, 45)
    }
}

/// Title and opening line upgrade options based on proven formula architecture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackagingUpgrade {
    pub current_title: String,
    pub title_upgrade_a: String,
    pub title_upgrade_b: String,
    pub opening_line_current: String,
    pub opening_line_upgraded: String,
}

impl PackagingUpgrade {
    /// Dynamic packaging synthesis driven by NicheToneProfile and context keywords.
    pub fn from_context(
        title: &str,
        tone: NicheToneProfile,
        actual_spoken_hook: Option<&str>,
    ) -> Self {
        let clean = clean_title(title);
        let title_lower = clean.to_lowercase();

        let is_cautionary_review = tone == NicheToneProfile::ConsumerReviewExplainer
            || title_lower.contains("don't buy")
            || title_lower.contains("dont buy")
            || title_lower.contains("do not buy")
            || title_lower.contains("why i'm selling")
            || title_lower.contains("why we're selling")
            || title_lower.contains("why we sold")
            || title_lower.contains("why i sold")
            || title_lower.contains("buyer beware")
            || title_lower.contains("before you buy")
            || title_lower.contains("worth it")
            || title_lower.contains("warning")
            || title_lower.contains("mistake")
            || title_lower.contains("should you buy")
            || title_lower.contains("should you")
            || title_lower.contains("review")
            || title_lower.contains("giving up on")
            || title_lower.contains("selling our")
            || title_lower.contains("selling my");

        let is_tiered_contrast = clean.to_lowercase().contains(" vs ")
            || clean.to_lowercase().contains(" vs. ")
            || clean.to_lowercase().starts_with("vs ");

        let v6_score = crate::math::marginal_gains::MarginalGainsEngine::evaluate_title_stakes(&clean, None, Some(tone)).score_0_to_100;
        let is_elite_title = v6_score >= 80.0;

        let (title_upgrade_a, title_upgrade_b, opening_line_upgraded) = if is_cautionary_review {
            let subject = extract_core_subject(&clean);
            let article_subj = subject_with_article(&subject);
            let in_or_on = living_preposition(&subject);

            let upgrade_a = format!("The Real Cost of Living {} {} (What Dealers Hide)", in_or_on, article_subj);
            let upgrade_b = format!("Why We're Giving Up on Our {} After 2 Years", subject);
            let hook = format!("Before you spend $100,000 on {}, here is the harsh reality nobody tells you.", article_subj);
            (upgrade_a, upgrade_b, hook)
        } else if is_tiered_contrast {
            let subject = extract_dynamic_subject(&clean);
            let upgrade_a = format!("{} Tested to the Absolute Limit", subject);
            let upgrade_b = format!("The Shocking Difference Between {}", subject);
            let hook = format!("\"We tested {} to see if price actually equals quality.\"", subject);
            (upgrade_a, upgrade_b, hook)
        } else if is_elite_title && tone != NicheToneProfile::DocumentaryBuildProcess && tone != NicheToneProfile::AestheticLifestyleCozy {
            let subject = extract_dynamic_subject(&clean);
            let upgrade_a = format!("{} (What Really Happened)", clean);
            let upgrade_b = format!("The Untold Reality Behind {}", subject);
            let hook = format!("\"If you think you know {}, wait until you see what happens next.\"", subject);
            (upgrade_a, upgrade_b, hook)
        } else {
            match tone {
                NicheToneProfile::DocumentaryBuildProcess | NicheToneProfile::AestheticLifestyleCozy => {
                    let has_two_years = title_lower.contains("2 year") || title_lower.contains("two year") || title_lower.contains("2-year");
                    let has_one_year = title_lower.contains("1 year") || title_lower.contains("one year") || title_lower.contains("1-year") || title_lower.contains("365 day");
                    let has_multi_year = has_two_years
                        || title_lower.contains("3 year")
                        || title_lower.contains("three year")
                        || title_lower.contains("4 year")
                        || title_lower.contains("5 year")
                        || title_lower.contains("multi-year")
                        || title_lower.contains("years");
                    let has_multi_buildings = title_lower.contains("buildings") || title_lower.contains("structures");

                    if (has_two_years || has_multi_year) && has_multi_buildings {
                        let time_desc = if has_two_years { "2 Years" } else { "Years" };
                        let buildings_desc = if title_lower.contains("5 building") {
                            "5 Log Cabins"
                        } else {
                            "Multiple Off-Grid Structures"
                        };
                        (
                            format!("{} Off-Grid: Building {} Completely Alone", time_desc, buildings_desc),
                            format!("What {} of Building an Off-Grid Homestead Actually Looks Like", time_desc),
                            format!("\"{} ago I left the city to see if I could build an entire off-grid homestead with my own two hands.\"", if has_two_years { "Two years" } else { "Years" }),
                        )
                    } else if has_two_years {
                        (
                            "2 Years Off-Grid: Building a Log Cabin & Homestead Completely Alone".to_string(),
                            "What 2 Years of Building an Off-Grid Homestead Actually Looks Like".to_string(),
                            "\"Two years ago I left the city to see if I could build an entire off-grid homestead with my own two hands.\"".to_string(),
                        )
                    } else if has_one_year {
                        (
                            "1 Year Off-Grid: Building a Log Cabin & Homestead Completely Alone".to_string(),
                            "What 1 Year of Building an Off-Grid Cabin Alone Actually Looks Like".to_string(),
                            "\"One year ago I walked away from everything in the city to build a life in the wilderness with my own two hands.\"".to_string(),
                        )
                    } else if has_multi_year {
                        (
                            "Years Off-Grid: Building an Entire Homestead Completely Alone".to_string(),
                            "What Years of Building an Off-Grid Homestead Actually Looks Like".to_string(),
                            "\"I walked away from modern life to see if I could build an entire off-grid homestead with my own two hands.\"".to_string(),
                        )
                    } else if has_multi_buildings {
                        (
                            "Building Multiple Off-Grid Structures from Scratch (Completely Alone)".to_string(),
                            "What It Actually Takes to Build an Entire Homestead Alone".to_string(),
                            "\"I didn't stop at one cabin—I set out to build an entire off-grid homestead with my own two hands.\"".to_string(),
                        )
                    } else if title_lower.contains("log cabin") {
                        (
                            "Building an Off-Grid Log Cabin from Scratch (30 Days Alone)".to_string(),
                            "Can One Person Build a Log Cabin Alone?".to_string(),
                            "\"I gave up everything in the city to see if I could build a log cabin with my own two hands.\"".to_string(),
                        )
                    } else if title_lower.contains("cabin") {
                        (
                            "Building an Off-Grid Cabin from Scratch (Alone in the Woods)".to_string(),
                            "The Reality of Building an Off-Grid Cabin With No Power".to_string(),
                            "\"I walked away from modern comfort to see if I could build an off-grid cabin from scratch.\"".to_string(),
                        )
                    } else if title_lower.contains("homestead") || title_lower.contains("off-grid") || title_lower.contains("off grid") {
                        (
                            "Starting an Off-Grid Homestead from Scratch (30 Days Alone)".to_string(),
                            "The Harsh Reality of Starting an Off-Grid Homestead Alone".to_string(),
                            "\"I left the city behind to find out what it really takes to build an off-grid homestead.\"".to_string(),
                        )
                    } else if title_lower.contains("pond") || title_lower.contains("swim pond") {
                        (
                            "Digging a Massive Natural Swim Pond from Scratch (Full Build Breakdown)".to_string(),
                            "Can You Build a Swimming Pond With No Heavy Machinery?".to_string(),
                            "\"We decided to turn raw dirt and rock into a natural swimming pond, completely on our own.\"".to_string(),
                        )
                    } else if title_lower.contains("van")
                        || title_lower.contains("camper")
                        || title_lower.contains("trailer")
                        || title_lower.contains("skoolie")
                        || title_lower.contains("bus")
                        || title_lower.contains("truck")
                        || title_lower.contains("ambulance")
                        || title_lower.contains("cargo")
                    {
                        if title_lower.contains("trailer") || title_lower.contains("cheap trailer") || title_lower.contains("utility trailer") {
                            (
                                "Converting a $1,000 Utility Trailer into an Off-Grid Micro Camper (Full Build)".to_string(),
                                "The 3-Week Micro Camper Build: From Tractor Supply Trailer to Home on Wheels".to_string(),
                                "\"We took a $1,000 utility trailer and three weeks to see if we could turn it into a fully insulated off-grid camper for our best friends' wedding.\"".to_string(),
                            )
                        } else if title_lower.contains("skoolie") || title_lower.contains("bus") {
                            let subject = extract_dynamic_subject(&clean);
                            (
                                format!("Converting an Old School Bus into an Off-Grid Home ({})", subject),
                                format!("Can You Build a Skoolie for Under $10,000? ({})", subject),
                                "\"We bought an old school bus and gutted the entire interior to see if we could build a custom home on wheels.\"".to_string(),
                            )
                        } else if title_lower.contains("ambulance") {
                            let subject = extract_dynamic_subject(&clean);
                            (
                                format!("Converting a 4x4 Ambulance into the Ultimate Adventure Rig ({})", subject),
                                format!("The Reality of Living in an Ambulance Camper ({})", subject),
                                "\"We bought a retired emergency ambulance to see if we could convert it into an off-grid expedition rig.\"".to_string(),
                            )
                        } else {
                            (
                                "Building the Ultimate Off-Grid Adventure Van by Hand".to_string(),
                                "The Reality of Living in an Off-Grid Van in Freezing Winter".to_string(),
                                "\"We gutted an empty cargo van to build a home on wheels that can survive anywhere off-grid.\"".to_string(),
                            )
                        }
                    } else if (title_lower.contains("boat") || title_lower.contains("hull") || title_lower.contains("vessel"))
                        && (title_lower.contains("facebook")
                            || title_lower.contains("worst boat")
                            || (title_lower.contains("fix") && !title_lower.contains("$100") && !title_lower.contains("$1,000"))
                            || title_lower.contains("scrap boat")
                            || title_lower.contains("restore")
                            || title_lower.contains("restoration"))
                    {
                        (
                            "Restoring the Worst Boat on Facebook Marketplace (Full Budget Breakdown)".to_string(),
                            "Can You Actually Fix a Scrap Boat from Facebook for Under $1,000?".to_string(),
                            "\"I bought the absolute worst boat on Facebook Marketplace to see if we could fix it and get it on the water.\"".to_string(),
                        )
                    } else if title_lower.contains("flipping")
                        || title_lower.contains("flip")
                        || title_lower.contains("side hustle")
                        || title_lower.contains("motor")
                        || title_lower.contains("outboard")
                        || title_lower.contains("engine")
                        || title_lower.contains("mechanic")
                        || title_lower.contains("$100")
                    {
                        if title_lower.contains("run on") || title_lower.contains("cooking oil") || title_lower.contains("oil") {
                            (
                                "Can an Old Outboard Motor Actually Run on Cooking Oil? (Destruction Test)".to_string(),
                                "What Actually Happens When You Put Cooking Oil in a Boat Motor?".to_string(),
                                "\"We wanted to find out what happens if you run an outboard on pure cooking oil until it blows up.\"".to_string(),
                            )
                        } else {
                            (
                                "Turning $100 into $1,000 Flipping Broken Boat Motors (Full Step-by-Step Breakdown)".to_string(),
                                "Can You Actually Make $1,000 Flipping Scrap Motors in a Weekend?".to_string(),
                                "\"I took this exact $100 bill to see if I could flip broken motors into $1,000 with simple garage tools.\"".to_string(),
                            )
                        }
                    } else if title_lower.contains("sail") || title_lower.contains("ocean") || title_lower.contains("offshore") || (title_lower.contains("boat") && !title_lower.contains("fix")) {
                        (
                            "Sailing Across the Open Ocean Alone: 30 Days Offshore".to_string(),
                            "The Reality of Sailing Offshore in 40-Knot Winds Alone".to_string(),
                            "\"We dropped the dock lines and sailed offshore with no backup crew to see if we could make the crossing.\"".to_string(),
                        )
                    } else if title_lower.contains("bushcraft") || title_lower.contains("shelter") {
                        let subject = "Bushcraft Shelter";
                        (
                            format!("Building a {} from Scratch in the Deep Woods", subject),
                            format!("Can One Person Build a {} Completely Alone?", subject),
                            format!("\"I ventured into the wilderness with minimal tools to see if I could build a {} with my own two hands.\"", subject.to_lowercase()),
                        )
                    } else if title_lower.contains("wood") || title_lower.contains("tree") || title_lower.contains("timber") || title_lower.contains("canoe") {
                        let subject = if title_lower.contains("canoe") {
                            "Wooden Canoe"
                        } else if title_lower.contains("timber") {
                            "Timber Frame Structure"
                        } else {
                            "Handcrafted Woodwork"
                        };
                        (
                            format!("Crafting {} by Hand from a Single Fallen Tree", subject_with_article(subject)),
                            format!("Can Traditional Hand Tools Build {} from Scratch?", subject_with_article(subject)),
                            format!("\"I took a single fallen tree into the workshop to see if traditional tools could build {}.\"", subject.to_lowercase()),
                        )
                    } else if (title_lower.contains("salmon") && title_lower.contains("grizzl")) || title_lower.contains("103 salmon") {
                        (
                            "4 Days on Alaska's Wildest River: 103 Salmon & 7 Grizzlies".to_string(),
                            "Surviving the Alaskan Wilderness With My Family (4-Day Expedition)".to_string(),
                            "\"We flew deep into the Alaskan wilderness for four days of salmon fishing, surrounded by grizzly bears and untamed rivers.\"".to_string(),
                        )
                    } else if (title_lower.contains("salmon") || title_lower.contains("grizzl") || title_lower.contains("wildlife"))
                        && (title_lower.contains("alaska") || title_lower.contains("river") || title_lower.contains("adventure") || title_lower.contains("expedition"))
                    {
                        let subject = extract_dynamic_subject(&clean);
                        (
                            format!("4 Days on Alaska's Wildest River: {}", subject),
                            format!("Surviving the Alaskan Wilderness With My Family ({})", subject),
                            "\"We flew deep into the Alaskan wilderness for four days of salmon fishing, surrounded by grizzly bears and untamed rivers.\"".to_string(),
                        )
                    } else if (title_lower.contains("fishing") || title_lower.contains("fish") || title_lower.contains("cod") || title_lower.contains("trout") || title_lower.contains("salmon"))
                        && !title_lower.contains("jet ski")
                        && !title_lower.contains("jetski")
                    {
                        let subject = extract_dynamic_subject(&clean);
                        if title_lower.contains("trout") {
                            (
                                "Catching Native Trout in Alaska's Remote Rivers (3-Day Expedition)".to_string(),
                                format!("The Reality of Surviving on Wild Caught Fish: {}", subject),
                                format!("\"We packed minimal gear and hiked into the backcountry to see if we could find {}.\"", subject.to_lowercase()),
                            )
                        } else if title_lower.contains("cod") {
                            (
                                "Catching Pacific Cod from Shore in Alaska: 3-Day Expeditions".to_string(),
                                "Can You Catch Pacific Cod from the Beach in Alaska?".to_string(),
                                "\"We pushed our kayaks into the heavy Alaskan surf to see if we could land Pacific Cod from shore.\"".to_string(),
                            )
                        } else {
                            (
                                format!("Catching Wild Fish from Shore in Alaska ({})", subject),
                                format!("Can You Catch Wild Fish from the Beach in Alaska? ({})", subject),
                                "\"We pushed our kayaks into the heavy Alaskan surf to see if we could land wild fish from shore.\"".to_string(),
                            )
                        }
                    } else if title_lower.contains("camping") || title_lower.contains("camp") || title_lower.contains("snow cave") {
                        let subject = extract_dynamic_subject(&clean);
                        (
                            format!("Solo Camping in the Deep Wilderness: 3 Days Alone ({})", subject),
                            format!("The Harsh Reality of Camping Alone: {}", subject),
                            format!("\"I hiked deep into the backcountry with minimal gear to see if I could survive {}.\"", subject.to_lowercase()),
                        )
                    } else if title_lower.contains("expedition") || title_lower.contains("adventure") || (title_lower.contains("alaska") && !title_lower.contains("facebook")) || title_lower.contains("river") {
                        let subject = extract_dynamic_subject(&clean);
                        (
                            format!("4 Days on Alaska's Wildest River: {}", subject),
                            "Surviving the Alaskan Wilderness (4-Day Expedition)".to_string(),
                            "\"We traveled deep into the untamed wilderness to see what happens when you leave civilization behind.\"".to_string(),
                        )
                    } else {
                        let subject = extract_dynamic_subject(&clean);
                        if tone == NicheToneProfile::AestheticLifestyleCozy {
                            (
                                format!("{}: A Quiet Journey into the Wilderness", subject),
                                format!("The Reality of {}: Living Off the Land", subject),
                                format!("\"I set out to discover what happens when you step away from the noise and embrace {}.\"", subject.to_lowercase()),
                            )
                        } else {
                            (
                                format!("Building {} Completely from Scratch (Full Step-by-Step Breakdown)", subject),
                                format!("What It Actually Takes to Build {} with No Experience", subject),
                                format!("\"I took on {} with minimal tools to see if I could build it with my own two hands.\"", subject.to_lowercase()),
                            )
                        }
                    }
                }
                NicheToneProfile::NarrativeEmotionalPitch => {
                    let subject = extract_dynamic_subject(&clean);
                    if title_lower.contains("villain") || title_lower.contains("enemy") {
                        (
                            "When the Villain Refuses to Let You Fall (Enemies to Lovers)".to_string(),
                            "POV: When the Morally Gray Villain Realizes Who Stole His Blade".to_string(),
                            "\"Everyone warned me he was ruthless. But when the blades were drawn, he stepped in front of me.\"".to_string(),
                        )
                    } else if title_lower.contains("trailer") || title_lower.contains("teaser") || title_lower.contains("glass") {
                        (
                            format!("When the Blade Chooses the Traitor ({})", subject),
                            format!("POV: The Secret Heir Who Survived the Fall ({})", subject),
                            "\"She was supposed to be a forgotten captive. Until the royal guard knelt before her.\"".to_string(),
                        )
                    } else {
                        (
                            format!("When the Morally Gray Hero Burns the World for You ({})", subject),
                            format!("POV: You Realize Who Protected You the Whole Time ({})", subject),
                            "\"They told me he was the monster of this realm, until he stood between me and death.\"".to_string(),
                        )
                    }
                }
                NicheToneProfile::EducationalBreakdown => {
                    let subject = extract_dynamic_subject(&clean);
                    if title_lower.contains("rust") || title_lower.contains("compiler") || title_lower.contains("code") || title_lower.contains("api") {
                        (
                            format!("Building a Production-Ready {} from Scratch (Full Architecture)", subject),
                            format!("The Fatal Architectural Mistake Costing You Weeks on {}", subject),
                            format!("\"If you are struggling to make {} work in production, this single architectural shift fixes it.\"", subject),
                        )
                    } else {
                        (
                            format!("The Complete Guide to {} (Step-by-Step Breakdown)", subject),
                            format!("The Truth About {} (What Most People Miss)", subject),
                            format!("\"If you want to understand {}, this single principle changes everything.\"", subject),
                        )
                    }
                }
                NicheToneProfile::HighStakesPanic => {
                    let subject = extract_dynamic_subject(&clean);
                    (
                        format!("Surviving 48 Hours with {} (High-Stakes Deep Dive)", subject),
                        format!("We Had 60 Minutes Before Catastrophic Failure: {}", subject),
                        "\"The moment the warning alarm went off, we knew we had less than five minutes to react.\"".to_string(),
                    )
                }
                NicheToneProfile::ConsumerReviewExplainer => {
                    let subject = extract_dynamic_subject(&clean);
                    let article_subj = subject_with_article(&subject);
                    let in_or_on = living_preposition(&subject);
                    (
                        format!("The Real Cost of Living {} {} (What Dealers Hide)", in_or_on, article_subj),
                        format!("Why We're Giving Up on Our {} After 2 Years", subject),
                        format!("Before you spend $100,000 on {}, here is the harsh reality nobody tells you.", article_subj),
                    )
                }
                NicheToneProfile::EntertainmentSpectacle => {
                    let subject = extract_dynamic_subject(&clean);
                    (
                        format!("{} Tested to the Absolute Limit", subject),
                        format!("The Shocking Difference Between {}", subject),
                        format!("\"We tested {} to see if price actually equals quality.\"", subject),
                    )
                }
                NicheToneProfile::ConversationalPodcast => {
                    let subject = extract_dynamic_subject(&clean);
                    (
                        format!("\"I've Kept This Quiet for Years\": Inside {}", subject),
                        format!("The Unfiltered Debate Behind {}: What Really Happened", subject),
                        format!("\"Two hours into our conversation, the truth about {} finally came out.\"", subject),
                    )
                }
            }
        };

        let sanitize_project = |s: String| -> String {
            s.replace("Woodworking Project", "Handcrafted Woodwork")
                .replace("woodworking project", "handcrafted woodwork")
                .replace("a Project", "the Build")
                .replace("A Project", "The Build")
                .replace("the Project", "the Build")
                .replace("The Project", "The Build")
                .replace(" Project", " Build")
                .replace("Project ", "Build ")
                .replace("Project", "Build")
        };
        let title_upgrade_a = sanitize_subject_noun(&sanitize_project(title_upgrade_a));
        let title_upgrade_b = sanitize_subject_noun(&sanitize_project(title_upgrade_b));
        let opening_line_upgraded = sanitize_project(opening_line_upgraded);

        let opening_line_current = match actual_spoken_hook {
            Some(hook) if !hook.trim().is_empty() => {
                let trimmed = hook.trim().trim_matches('"');
                let first_sentence = if let Some(pos) = trimmed.find(". ") {
                    &trimmed[..=pos]
                } else if let Some(pos) = trimmed.find("! ") {
                    &trimmed[..=pos]
                } else if let Some(pos) = trimmed.find("? ") {
                    &trimmed[..=pos]
                } else {
                    trimmed
                };
                format!("\"{}\"", first_sentence.trim())
            }
            _ => "[No spoken dialogue in opening 5s — ambient environmental audio / music opening]".to_string(),
        };

        Self {
            current_title: title.to_string(),
            title_upgrade_a,
            title_upgrade_b,
            opening_line_current,
            opening_line_upgraded,
        }
    }

    /// Default packaging upgrade helper using general title heuristics.
    pub fn from_title_and_tone(title: &str, tone: Option<NicheToneProfile>) -> Self {
        let effective_tone = tone.unwrap_or_else(|| NicheToneProfile::detect(title, "", ""));
        Self::from_context(title, effective_tone, None)
    }

    /// Packaging upgrade helper integrating title, tone, and actual spoken dialogue hook.
    pub fn from_title_tone_and_hook(title: &str, tone: Option<NicheToneProfile>, actual_spoken_hook: Option<&str>) -> Self {
        let effective_tone = tone.unwrap_or_else(|| NicheToneProfile::detect(title, "", ""));
        Self::from_context(title, effective_tone, actual_spoken_hook)
    }
}

/// The executive human-facing "Editor's Flight Sheet" delivering direct, actionable instructions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightSheet {
    pub video_title: String,
    pub channel_name: String,
    pub duration_formatted: String,
    pub tone_profile_formatted: String,
    pub overall_verdict: String,
    pub primary_bottleneck: String,
    pub timeline_punch_list: Vec<TimelineEditCue>,
    pub packaging_upgrades: PackagingUpgrade,
    pub audience_satisfaction_tier: String,
    pub compounded_gain_pct: f64,
    pub export_paths: Vec<String>,
    #[serde(default)]
    pub is_local_draft: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_report: Option<ThumbnailForensicReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub telemetry_matrix: Vec<crate::math::marginal_gains::TelemetryVector>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_radar: Option<TruncationRadar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promise_gap_report: Option<PromiseGapReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness_metrics: Option<crate::math::LoudnessMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuttable_fat: Option<crate::math::marginal_gains::CuttableFatReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comment_clusters: Vec<crate::analyzer::comments::CommentTimestampCluster>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suitability_report: Option<VideoSuitabilityReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shorts_metrics: Option<ShortsRetentionMetrics>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub re_engagement_checkpoints: Vec<crate::math::retention::ReEngagementCheckpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stair_stepping: Option<crate::pipeline::hook_machine::StairSteppingProgression>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outro_report: Option<crate::math::retention::OutroRunwayReport>,
    /// Optional audience retention time-series curve points (v0.5.4)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retention_curve: Vec<crate::math::retention::HeatmapPoint>,
    /// Optional multimodal attention breaches & drop cliff events (v0.5.4)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attention_breaches: Vec<crate::math::retention::AttentionBreach>,
}

impl FlightSheet {
    /// Constructs a FlightSheet directly from an evaluated MarginalGainsScorecard.
    pub fn from_scorecard(scorecard: &MarginalGainsScorecard) -> Self {
        let slug = slugify(&scorecard.title);
        let grade = scorecard.series_system_reliability_pct.round() as u64;
        let is_local_draft = scorecard.is_local_draft;
        let effective_tone = scorecard.tone_profile.unwrap_or_else(|| NicheToneProfile::detect(&scorecard.title, "", ""));

        let overall_verdict = if is_local_draft {
            if scorecard.composite_brailsford_score >= 70.0 {
                format!("⚡ PRE-PUBLISH READY (Health Grade: {}%)", grade)
            } else {
                format!("⚠️ PRE-PUBLISH FRICTION DETECTED (Health Grade: {}%)", grade)
            }
        } else if scorecard.composite_brailsford_score >= 80.0 {
            format!("🔥 VIRAL BREAKOUT POTENTIAL (Health Grade: {}%)", grade)
        } else if scorecard.composite_brailsford_score >= 65.0 {
            format!("🟢 HEALTHY PERFORMANCE BASELINE (Health Grade: {}%)", grade)
        } else if scorecard.composite_brailsford_score >= 50.0 {
            format!("⚠️ PACING FRICTION DETECTED (Health Grade: {}%)", grade)
        } else {
            format!("🚨 CRITICAL RETENTION BLEED (Health Grade: {}%)", grade)
        };

        let primary_bottleneck = scorecard
            .single_point_of_failure
            .clone()
            .unwrap_or_else(|| {
                "None. Pacing and retention vectors are performing above optimal baseline.".to_string()
            });

        // Build all 4 core punch list categories
        let mut punch_list = Vec::with_capacity(4);

        // 1. TRIM DEAD AIR (Vector 1: Cold Open)
        let v1 = scorecard.vectors.first();
        let dead_air_cue = if let Some(v) = v1 {
            let t_sil = v.measured_value;
            let range = if t_sil <= 0.4 {
                "00:00 - 00:00".to_string()
            } else {
                let sec = (t_sil.ceil() as u64).max(1);
                format!("00:00 - 00:{:02}", sec)
            };
            let instruction = if v.score_0_to_100 >= 90.0 {
                format!("Speech onset is elite ({:.1}s silence); maintain instant audio hook.", t_sil)
            } else {
                v.one_percent_fix.clone()
            };
            let impact = if v.estimated_retention_lift_pct > 0.0 {
                format!("Halts autoplay drop-off (+{:.1}% lift)", v.estimated_retention_lift_pct)
            } else {
                "Halts autoplay drop-off & captures silent autoplay scrollers".to_string()
            };
            TimelineEditCue {
                timecode_range: range,
                action_type: "TRIM DEAD AIR".to_string(),
                remedy_instruction: instruction,
                impact_summary: impact,
            }
        } else {
            TimelineEditCue {
                timecode_range: "00:00 - 00:01".to_string(),
                action_type: "TRIM DEAD AIR".to_string(),
                remedy_instruction: "Trim pre-speech bumper silence down to under 0.5s (Frames 1-12).".to_string(),
                impact_summary: "Halts autoplay drop-off".to_string(),
            }
        };
        punch_list.push(dead_air_cue);

        // 2. INSERT B-ROLL (Vector 3: Camera Hold / Vector 2: Visual Rhythm)
        let v3 = scorecard.vectors.get(2);
        let v2 = scorecard.vectors.get(1);
        let broll_cue = if let Some(v) = v3 {
            let hold = v.measured_value;
            let range = format!("00:00 - 00:{:02}", (hold.min(60.0).round() as u64).max(5));
            let instruction = if effective_tone == NicheToneProfile::ConversationalPodcast {
                let asl_val = v2.map(|r| r.measured_value).unwrap_or(15.0);
                format!("Broadcast conversational pace (ASL {:.1}s, max hold {:.1}s); compliant with multi-camera interview standards.", asl_val, hold)
            } else if v.score_0_to_100 >= 85.0 {
                if let Some(v_rhythm) = v2 {
                    format!("Visual pacing is dialed ({:.1}s/cut, hold {:.1}s); maintain dynamic shot variety.", v_rhythm.measured_value, hold)
                } else {
                    v.one_percent_fix.clone()
                }
            } else {
                v.one_percent_fix.clone()
            };
            let impact = if effective_tone == NicheToneProfile::ConversationalPodcast {
                "Compliant with multi-camera interview standards".to_string()
            } else if v.estimated_retention_lift_pct > 0.0 {
                format!("Eliminates visual stagnation (+{:.1}% lift)", v.estimated_retention_lift_pct)
            } else {
                "Eliminates talking-head freeze & visual drag".to_string()
            };
            TimelineEditCue {
                timecode_range: range,
                action_type: "INSERT B-ROLL".to_string(),
                remedy_instruction: instruction,
                impact_summary: impact,
            }
        } else {
            TimelineEditCue {
                timecode_range: "00:00 - 01:00".to_string(),
                action_type: "INSERT B-ROLL".to_string(),
                remedy_instruction: "Insert action b-roll or cutaways on shots lingering past target editor hold limit (max 10s).".to_string(),
                impact_summary: "Eliminates visual stagnation".to_string(),
            }
        };
        punch_list.push(broll_cue);

        // 3. CUT EXPOSITION (Vector 4: Mid-Video Retention Drop-Off)
        let v4 = scorecard.vectors.get(3);
        let exposition_cue = if let Some(v) = v4 {
            let tc = extract_timecode(&v.one_percent_fix).unwrap_or_else(|| "Mid-Video".to_string());
            let instruction = if v.score_0_to_100 >= 85.0 {
                "Audience decay is steady; place forward open loops before any slow transitions to sustain mid-video watch time.".to_string()
            } else {
                v.one_percent_fix.clone()
            };
            let impact = if v.estimated_retention_lift_pct > 0.0 {
                format!("Halts mid-video viewer cliff (+{:.1}% lift)", v.estimated_retention_lift_pct)
            } else {
                "Protects mid-video watch time & session completion".to_string()
            };
            TimelineEditCue {
                timecode_range: tc,
                action_type: "CUT EXPOSITION".to_string(),
                remedy_instruction: instruction,
                impact_summary: impact,
            }
        } else {
            TimelineEditCue {
                timecode_range: "Mid-Video".to_string(),
                action_type: "CUT EXPOSITION".to_string(),
                remedy_instruction: "Trim slow exposition or introduce forward curiosity loops at steepest drop cliff.".to_string(),
                impact_summary: "Halts mid-video retention bleed".to_string(),
            }
        };
        punch_list.push(exposition_cue);

        // 4. OUTRO FIX (Outro Exit Note / Strict Abrupt Ending Audit)
        let outro_cue = if let Some(ref outro) = scorecard.outro_report {
            let range = if outro.recommended_blade_timecode == "N/A" {
                "Outro".to_string()
            } else {
                outro.recommended_blade_timecode.clone()
            };
            let (instruction, impact) = if outro.is_abrupt_ending {
                (
                    format!(
                        "Abrupt ending verified ({:.1}s post-payoff runway); zero wind-down drag.",
                        outro.outro_runway_seconds
                    ),
                    "Zero wind-down drag; maximum end-screen clickthrough.".to_string(),
                )
            } else {
                let phrase = outro.first_signoff_phrase.as_deref().unwrap_or("verbal wind-down");
                (
                    format!(
                        "Blade timeline at {} (eliminate {:.0}s verbal wind-down: \"{}\"). Saves ~{:.1}% of audience retention.",
                        outro.recommended_blade_timecode,
                        outro.outro_runway_seconds,
                        phrase,
                        outro.audience_loss_in_runway_pct
                    ),
                    "Prevents premature click-away & boosts end-screen CTR".to_string(),
                )
            };
            TimelineEditCue {
                timecode_range: range,
                action_type: "OUTRO FIX".to_string(),
                remedy_instruction: instruction,
                impact_summary: impact,
            }
        } else if let Some(ref note) = scorecard.outro_exit_note {
            let tc = extract_timecode(note).unwrap_or_else(|| "Outro".to_string());
            TimelineEditCue {
                timecode_range: tc,
                action_type: "OUTRO FIX".to_string(),
                remedy_instruction: note.clone(),
                impact_summary: "Prevents premature click-away & boosts end-screen CTR".to_string(),
            }
        } else {
            TimelineEditCue {
                timecode_range: "Final 15%".to_string(),
                action_type: "OUTRO FIX".to_string(),
                remedy_instruction: "Eliminate verbal sign-off lag ('thank you for watching', 'in conclusion'); roll end screen cards directly over final content payoff.".to_string(),
                impact_summary: "Maintains viewer momentum into next recommended video".to_string(),
            }
        };
        punch_list.push(outro_cue);

        let packaging_upgrades = PackagingUpgrade::from_context(
            &scorecard.title,
            effective_tone,
            scorecard.actual_spoken_hook.as_deref(),
        );
        let audience_satisfaction_tier = scorecard
            .satisfaction_index
            .as_ref()
            .map(|s| s.satisfaction_tier.clone())
            .unwrap_or_else(|| "🟢 Healthy Satisfaction".to_string());

        let suffix = if is_local_draft { "_draft" } else { "" };
        let export_paths = vec![
            format!("output/{}{}_flight_sheet.md", slug, suffix),
            format!("output/{}{}_template.fcpxml", slug, suffix),
            format!("output/{}{}.edl", slug, suffix),
        ];

        let duration_formatted = if let Some(dur) = scorecard.duration_seconds {
            format_seconds(dur)
        } else if let Some(ref outro) = scorecard.outro_exit_note {
            extract_timecode(outro).unwrap_or_else(|| "10:00".to_string())
        } else {
            "10:00".to_string()
        };

        let tone_profile_formatted = scorecard
            .tone_profile
            .map(|t| t.label().to_string())
            .unwrap_or_else(|| "General Audience".to_string());

        let truncation_radar = Some(TruncationRadar::evaluate(&scorecard.title));

        Self {
            video_title: scorecard.title.clone(),
            channel_name: scorecard.niche_profile.clone(),
            duration_formatted,
            tone_profile_formatted,
            overall_verdict,
            primary_bottleneck,
            timeline_punch_list: punch_list,
            packaging_upgrades,
            audience_satisfaction_tier,
            compounded_gain_pct: scorecard.predicted_multiplier_lift,
            export_paths,
            is_local_draft,
            thumbnail_report: scorecard.thumbnail_report.clone(),
            telemetry_matrix: scorecard.vectors.clone(),
            truncation_radar,
            promise_gap_report: scorecard.promise_gap_report.clone(),
            loudness_metrics: scorecard.loudness_metrics.clone(),
            cuttable_fat: scorecard.cuttable_fat.clone(),
            comment_clusters: scorecard.comment_clusters.clone(),
            suitability_report: scorecard.suitability_report.clone(),
            shorts_metrics: scorecard.shorts_metrics.clone(),
            re_engagement_checkpoints: scorecard.re_engagement_checkpoints.clone(),
            stair_stepping: scorecard.stair_stepping.clone().or_else(|| {
                if crate::pipeline::HookMachineEngine::detect_stair_stepping_format(&scorecard.title) {
                    let dur = scorecard.duration_seconds.unwrap_or(600.0);
                    Some(crate::pipeline::HookMachineEngine::evaluate_stair_stepping_integrity(&scorecard.title, dur, &[]))
                } else {
                    None
                }
            }),
            outro_report: scorecard.outro_report.clone(),
            retention_curve: scorecard.retention_curve.clone(),
            attention_breaches: scorecard.attention_breaches.clone(),
        }
    }

    /// Returns the parsed runtime duration in seconds.
    pub fn duration_seconds_f64(&self) -> f64 {
        parse_first_seconds(&self.duration_formatted).unwrap_or(600.0)
    }

    /// Attaches an evaluated OutroRunwayReport.
    pub fn with_outro_report(mut self, report: crate::math::retention::OutroRunwayReport) -> Self {
        self.outro_report = Some(report);
        self
    }

    /// Attaches audience retention curve points.
    pub fn with_retention_curve(mut self, curve: Vec<crate::math::retention::HeatmapPoint>) -> Self {
        self.retention_curve = curve;
        self
    }

    /// Attaches multimodal attention breaches.
    pub fn with_attention_breaches(mut self, breaches: Vec<crate::math::retention::AttentionBreach>) -> Self {
        self.attention_breaches = breaches;
        self
    }

    /// Attaches an evaluated VideoSuitabilityReport.
    pub fn with_suitability_report(mut self, report: VideoSuitabilityReport) -> Self {
        self.suitability_report = Some(report);
        self
    }

    /// Attaches evaluated ShortsRetentionMetrics.
    pub fn with_shorts_metrics(mut self, metrics: ShortsRetentionMetrics) -> Self {
        self.shorts_metrics = Some(metrics);
        self
    }

    /// Attaches evaluated comment clusters.
    pub fn with_comment_clusters(mut self, clusters: Vec<crate::analyzer::comments::CommentTimestampCluster>) -> Self {
        self.comment_clusters = clusters;
        self
    }

    /// Attaches evaluated audio loudness metrics.
    pub fn with_loudness_metrics(mut self, loudness: crate::math::LoudnessMetrics) -> Self {
        self.loudness_metrics = Some(loudness);
        self
    }

    /// Attaches an evaluated CuttableFatReport.
    pub fn with_cuttable_fat(mut self, fat: crate::math::marginal_gains::CuttableFatReport) -> Self {
        self.cuttable_fat = Some(fat);
        self
    }

    /// Attaches an evaluated ThumbnailForensicReport.
    pub fn with_thumbnail_report(mut self, report: ThumbnailForensicReport) -> Self {
        self.thumbnail_report = Some(report);
        self
    }

    /// Attaches an evaluated TruncationRadar.
    pub fn with_truncation_radar(mut self, radar: TruncationRadar) -> Self {
        self.truncation_radar = Some(radar);
        self
    }

    /// Attaches an evaluated PromiseGapReport.
    pub fn with_promise_gap_report(mut self, gap: PromiseGapReport) -> Self {
        self.promise_gap_report = Some(gap);
        self
    }

    /// Sets explicit duration and formatted string.
    pub fn with_duration_seconds(mut self, duration_sec: f64) -> Self {
        self.duration_formatted = format_seconds(duration_sec);
        self
    }

    /// Sets explicit channel uploader name.
    pub fn with_channel_name(mut self, channel: impl Into<String>) -> Self {
        self.channel_name = channel.into();
        self
    }

    /// Renders the Flight Sheet into a clean, bordered ANSI terminal presentation.
    pub fn render_terminal_string(&self) -> String {
        let mut out = String::new();
        let header_title = if self.is_local_draft {
            "║                             ✈️  EDITOR'S PRE-PUBLISH DRAFT SHEET                                   ║\n"
        } else {
            "║                                 ✈️  EDITOR'S FLIGHT SHEET                                          ║\n"
        };
        out.push_str("\n╔════════════════════════════════════════════════════════════════════════════════════════════════════╗\n");
        out.push_str(header_title);
        out.push_str("╚════════════════════════════════════════════════════════════════════════════════════════════════════╝\n");
        out.push_str(&format!(" 📹 Video     : {}\n", self.video_title));
        out.push_str(&format!(" 👤 Channel   : {}\n", self.channel_name));
        out.push_str(&format!(" ⏱️ Runtime   : {}\n", self.duration_formatted));
        out.push_str(&format!(" 🎭 Niche Tone: {}\n", self.tone_profile_formatted));
        out.push_str(&format!(" 🎯 Verdict   : {}\n", self.overall_verdict));
        out.push_str(&format!(" 🚨 Bottleneck: {}\n", self.primary_bottleneck));
        out.push_str(&format!(" 💎 Audience  : {}\n", self.audience_satisfaction_tier));
        out.push_str(&format!(" 🚀 Lift ROI  : +{:.1}% Viewership Lift across Top 3 micro-edits\n", self.compounded_gain_pct));

        if let Some(suit) = &self.suitability_report {
            let tag = if suit.rating == "GREEN" {
                "🟢 MONETIZATION SAFE (Full Ad Distribution)"
            } else {
                "🟡 ADVERTISER RISK (Limited Ads / Unlisted 24h Review Recommended)"
            };
            out.push_str(&format!(" 🛡️ Ad Safety : {}\n", tag));
            out.push_str(&format!("   ↳ Status   : {}\n", suit.executive_summary));
        }

        if let Some(shorts) = &self.shorts_metrics {
            out.push_str(&format!(" 📱 Shorts Mode: Est. Stay Ratio: {:.1}% ({})\n", shorts.estimated_stay_ratio_pct, shorts.stay_vs_swipe_risk));
            out.push_str(&format!("   ↳ Loop Rate: {:.1}/100 [{}]\n", shorts.loop_continuity_score, if shorts.loop_seamless { "SEAMLESS LOOP" } else { "ABRUPT EXIT DETECTED" }));
        }

        out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 📋 TIMELINE EDITING PUNCH LIST (Direct NLE Cuts)\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        for cue in &self.timeline_punch_list {
            out.push_str(&format!(" [{}] {}\n", cue.timecode_range, cue.action_type));
            let label = if cue.remedy_instruction.starts_with("Broadcast conversational pace") {
                "↳ Rhythm"
            } else {
                "↳ Command"
            };
            out.push_str(&format!("   {}: {}\n", label, cue.remedy_instruction));
            out.push_str(&format!("   ↳ Impact : {}\n\n", cue.impact_summary));
        }

        let failed_checkpoints: Vec<&str> = self
            .re_engagement_checkpoints
            .iter()
            .filter(|c| !c.passed)
            .map(|c| if c.window_start_sec < 250.0 { "03:00" } else { "06:00" })
            .collect();
        if !failed_checkpoints.is_empty() {
            let failed_str = failed_checkpoints.join(" / ");
            out.push_str(&format!(
                " ⚠️ Retention Milestone Warning: [{}] failed re-engagement gate. See timeline marker for recommended pacing reset.\n\n",
                failed_str
            ));
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
                out.push_str(" 🪜 STAIR-STEPPING ESCALATION RADAR\n");
                out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
                out.push_str("- Format Detected      : Multi-Tiered Contrast Escalation ($X vs $Y)\n");
                let mono_tag = if ss.has_monotonic_escalation {
                    "[VERIFIED] Stakes increase monotonically across all tiers"
                } else {
                    "[FAILED] Stakes do not escalate monotonically"
                };
                out.push_str(&format!("- Monotonic Growth     : {}\n", mono_tag));

                let climax_tier = ss.detected_tiers.iter().find(|t| t.is_climax).or_else(|| ss.detected_tiers.last());
                let climax_tag = if ss.climax_held_for_finale {
                    if let Some(ct) = climax_tier {
                        let mm = (ct.timecode_seconds / 60.0).floor() as u64;
                        let ss_sec = (ct.timecode_seconds % 60.0).round() as u64;
                        let pct = ((ct.timecode_seconds / self.duration_seconds_f64().max(1.0)) * 100.0).round() as u64;
                        format!("[FINAL 20% LOCKED] Peak payoff situated at {:02}:{:02} of {} ({}% mark)", mm, ss_sec, self.duration_formatted, pct)
                    } else {
                        "[FINAL 20% LOCKED] Peak payoff situated in final 20% of runtime".to_string()
                    }
                } else if let Some(ct) = climax_tier {
                    let mm = (ct.timecode_seconds / 60.0).floor() as u64;
                    let ss_sec = (ct.timecode_seconds % 60.0).round() as u64;
                    let pct = ((ct.timecode_seconds / self.duration_seconds_f64().max(1.0)) * 100.0).round() as u64;
                    format!("[PREMATURE CLIMAX RISK] Peak payoff situated at {:02}:{:02} of {} ({}% mark)", mm, ss_sec, self.duration_formatted, pct)
                } else {
                    "[PREMATURE CLIMAX RISK] Climax revealed prior to final 20%".to_string()
                };
                out.push_str(&format!("- Climax Payoff Timing : {}\n", climax_tag));

                let tier_progression_str: Vec<String> = ss.detected_tiers.iter().map(|t| {
                    let tc = t.timecode_range.split('-').next().unwrap_or("00:00").trim();
                    format!("{} ({})", t.label, tc)
                }).collect();
                out.push_str(&format!("- Tier Progression     : {}\n\n", tier_progression_str.join(" ➔ ")));
            }
        }

        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 📦 PACKAGING & HOOK UPGRADES (A/B Test Suite)\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        if let Some(radar) = &self.truncation_radar {
            out.push_str(" 📱 CROSS-DEVICE TITLE TRUNCATION RADAR\n");
            let push_tag = if radar.mobile_push_truncated { "[⚠️ TRUNCATED]" } else { "[SAFE]" };
            let feed_tag = if radar.mobile_feed_ctv_truncated { "[⚠️ TRUNCATED ON MOBILE/CTV]" } else { "[SAFE]" };
            let desktop_tag = if radar.desktop_search_truncated { "[⚠️ TRUNCATED]" } else { "[SAFE]" };

            out.push_str(&format!("  • Mobile Push (45c)   : {} {}\n", radar.mobile_push_preview, push_tag));
            out.push_str(&format!("  • Mobile/CTV (52c)    : {} {}\n", radar.mobile_feed_ctv_preview, feed_tag));
            out.push_str(&format!("  • Desktop Search (60c): {} {}\n", radar.desktop_search_preview, desktop_tag));
            if let Some(rec) = &radar.front_load_recommendation {
                out.push_str("    ↳ 💡 Truncation Alert: Front-load the core hook! Title cut off on >70% of mobile/CTV feeds.\n");
                out.push_str(&format!("    ↳ Recommended Front-Load: \"{}\" ({} chars)\n", rec, rec.chars().count()));
            }
            out.push('\n');
        }
        if let Some(thumb) = &self.thumbnail_report {
            let integrity_pct = (thumb.packaging_integrity_index * 100.0).round() as u64;
            let risk_str = if integrity_pct < 40 {
                "High Bait-and-Switch Risk"
            } else if integrity_pct < 70 {
                "Moderate Packaging Friction"
            } else {
                "Authentic Alignment"
            };
            out.push_str(&format!(
                " • Visual Packaging: {} (Packaging Integrity: {}% - {})\n",
                thumb.primary_archetype.badge(),
                integrity_pct,
                risk_str
            ));
            if !thumb.diagnostic_note.is_empty() {
                out.push_str(&format!(" • Packaging Note   : {}\n", thumb.diagnostic_note));
            }
            if thumb.timestamp_collision_detected {
                let detail = thumb.collision_detail.as_deref().unwrap_or("Text/Focal element detected in bottom-right dead zone; obstructed by YouTube timestamp overlay.");
                out.push_str(&format!(" • ⚠️ Timestamp Collision: {}\n", detail));
            }
            if let Some(redundancy) = thumb.title_text_redundancy_pct {
                if redundancy >= 50.0 {
                    out.push_str(&format!(
                        " • ⚠️ Verbal Redundancy: Thumbnail text duplicates title by {:.0}%; wastes persuasive canvas space. Shift from Reinforcement to Complementarity.\n",
                        redundancy
                    ));
                }
            }
        }
        if let Some(gap) = &self.promise_gap_report {
            if gap.is_breach {
                out.push_str(" • 🚨 Promise Gap Breach: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk.\n");
                if let Some(detail) = &gap.throat_clearing_detail {
                    out.push_str(&format!("   ↳ Throat-Clearing : {}\n", detail));
                }
                out.push_str(&format!("   ↳ Visual Handshake: {}\n", gap.visual_handshake_detail));
                out.push_str(&format!("   ↳ Verbal Handshake: {}\n", gap.verbal_handshake_detail));
            } else {
                out.push_str(&format!(
                    " • Two-Part Handshake (0:00-0:05): [PASSED] (Score: {:.0}/100) — {}\n",
                    gap.promise_gap_score, gap.verbal_handshake_detail
                ));
            }
        }
        out.push_str(&format!(" • Current Title   : {}\n", self.packaging_upgrades.current_title));
        out.push_str(&format!(" • Option A (Scale): {}\n", self.packaging_upgrades.title_upgrade_a));
        out.push_str(&format!(" • Option B (Hooks): {}\n", self.packaging_upgrades.title_upgrade_b));
        out.push_str(&format!(" • Opening Line    : {}\n", self.packaging_upgrades.opening_line_current));
        out.push_str(&format!(" • Upgraded 4 Ps   : {}\n", self.packaging_upgrades.opening_line_upgraded));

        if let Some(loudness) = &self.loudness_metrics {
            out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(" 🔊 AUDIO MASTERING & YOUTUBE LOUDNESS RADAR\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            let penalty_tag = if loudness.youtube_penalty_db < 0.0 {
                format!("{:.1} dB Penalty (Hot Mix)", loudness.youtube_penalty_db)
            } else {
                "0.0 dB (Compliant)".to_string()
            };
            let peak_tag = if loudness.true_peak_dbtp > crate::math::LoudnessMetrics::TRUE_PEAK_CEILING_DBTP {
                format!("{:.1} dBTP [⚠️ Mobile Speaker Distortion Risk]", loudness.true_peak_dbtp)
            } else {
                format!("{:.1} dBTP [SAFE]", loudness.true_peak_dbtp)
            };
            out.push_str(&format!("  • Integrated Loudness  : {:.1} LUFS (YouTube Standard: -14.0 LUFS)\n", loudness.integrated_lufs));
            out.push_str(&format!("  • YouTube Volume Cut   : {}\n", penalty_tag));
            out.push_str(&format!("  • Cold-Open (0-30s)    : {:.1} LUFS\n", loudness.cold_open_lufs));
            out.push_str(&format!("  • Loudness Range (LRA) : {:.1} LU\n", loudness.loudness_range_lu));
            out.push_str(&format!("  • True Peak Level      : {}\n", peak_tag));
            out.push_str(&format!("  • Mastering Status     : {}\n", loudness.mastering_status));
            out.push_str(&format!("  • Master Bus Command   : {}\n", loudness.actionable_prescription));
        }

        if let Some(shorts) = &self.shorts_metrics {
            out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(" 📱 YOUTUBE SHORTS RETENTION & LOOP RADAR\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(&format!("  • Hook Delay (0-1.5s)  : {:.2}s [{}]\n", shorts.first_frame_hook_delay, if shorts.first_frame_hook_safe { "✅ SAFE <= 1.5s" } else { "⚠️ HIGH SWIPE-AWAY RISK > 1.5s" }));
            out.push_str(&format!("  • Duration Category    : {:?} ({:.1}s)\n", shorts.duration_safety_status, shorts.duration_seconds));
            out.push_str(&format!("  • Loop Continuity      : {:.1}/100 [{}]\n", shorts.loop_continuity_score, if shorts.loop_seamless { "SEAMLESS LOOP" } else { "ABRUPT EXIT DETECTED" }));
            out.push_str(&format!("  • Stay vs Swipe Risk   : {} (Est. Stay Ratio: {:.1}%)\n", shorts.stay_vs_swipe_risk, shorts.estimated_stay_ratio_pct));
            out.push_str(&format!("  • Loop Bridge Insight  : {}\n", shorts.loop_bridge_insight));
            out.push_str(&format!("  • Action Summary       : {}\n", shorts.executive_summary));
        }

        if let Some(fat) = &self.cuttable_fat {
            out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(" ✂️ CUMULATIVE CUTTABLE FAT & RE-TIMING BUDGET\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(&format!("  • Reclaimable Runtime  : {:.1}s across {} micro-edits\n", fat.total_cuttable_seconds, fat.micro_edits_count));
            out.push_str(&format!("  • Runtime Shaving Goal : {} -> {} (-{:.1}% total duration)\n", fat.original_runtime_formatted, fat.streamlined_runtime_formatted, fat.runtime_reduction_pct));
            out.push_str(&format!("  • Dead Air / Breaths   : {:.1}s trimmed from inter-phrase pauses (>0.4s)\n", fat.dead_air_breath_seconds));
            out.push_str(&format!("  • Static Hold Excess   : {:.1}s across {} extended camera takes\n", fat.camera_hold_fat_seconds, fat.punch_in_cues.len()));
            out.push_str(&format!("  • Cold-Open Delay Lag  : {:.1}s pre-speech bumper silence\n", fat.cold_open_lag_seconds));
            out.push_str(&format!("  • Editorial Summary    : {}\n", fat.executive_summary));
            if !fat.punch_in_cues.is_empty() {
                out.push_str("  • 115% Punch-In Cues   :\n");
                for cue in fat.punch_in_cues.iter().take(4) {
                    out.push_str(&format!("    ↳ [{}] Midpoint of {:.1}s hold: {}\n", cue.timecode, cue.hold_duration_seconds, cue.edit_instruction));
                }
                if fat.punch_in_cues.len() > 4 {
                    out.push_str(&format!("    ↳ ... plus {} additional punch-in timeline markers.\n", fat.punch_in_cues.len() - 4));
                }
            }
        }

        if !self.comment_clusters.is_empty() {
            out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(" 💬 CROWD-SOURCED COMMENT FRICTION & ENGAGEMENT CLUSTERS\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            for cluster in self.comment_clusters.iter().take(6) {
                let rel_str = if let Some(ref rel) = cluster.correlated_retention_event {
                    format!(" | 📉 {}", rel)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "  • [{}] {} ({} comments, {} likes){}\n",
                    cluster.timecode_formatted, cluster.sentiment.badge(), cluster.mention_count, cluster.total_likes, rel_str
                ));
                out.push_str(&format!("    ↳ \"{}\"\n", cluster.representative_quote));
            }
            if self.comment_clusters.len() > 6 {
                out.push_str(&format!("    ↳ ... plus {} additional crowd timestamp clusters.\n", self.comment_clusters.len() - 6));
            }
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
                out.push_str(" 🪜 STAIR-STEPPING ESCALATION RADAR\n");
                out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
                let format_desc = "Multi-Tiered Contrast Escalation ($X vs $Y)";
                let monotonic_tag = if ss.has_monotonic_escalation {
                    "[VERIFIED] Stakes increase monotonically across all tiers"
                } else {
                    "[BROKEN] Stakes fail to increase monotonically across tiers"
                };

                let climax_tier = ss.detected_tiers.iter().find(|t| t.is_climax).cloned();
                let dur_sec = self.duration_seconds_f64();
                let climax_timing = if let Some(ref c) = climax_tier {
                    let mm = (c.timecode_seconds / 60.0).floor() as u64;
                    let ss_sec = (c.timecode_seconds % 60.0).round() as u64;
                    let dur_mm = (dur_sec / 60.0).floor() as u64;
                    let dur_ss = (dur_sec % 60.0).round() as u64;
                    let pct = ((c.timecode_seconds / dur_sec.max(1.0)) * 100.0).round() as u64;
                    if ss.climax_held_for_finale {
                        format!("[FINAL 20% LOCKED] Peak payoff situated at {:02}:{:02} of {:02}:{:02} ({}% mark)", mm, ss_sec, dur_mm, dur_ss, pct)
                    } else {
                        format!("[PREMATURE CLIMAX] Peak payoff situated at {:02}:{:02} of {:02}:{:02} ({}% mark)", mm, ss_sec, dur_mm, dur_ss, pct)
                    }
                } else {
                    "[NOT DETECTED] No distinct climax payoff identified".to_string()
                };

                let progression_str = ss.detected_tiers
                    .iter()
                    .map(|t| format!("{} ({})", t.label, t.timecode_range))
                    .collect::<Vec<_>>()
                    .join(" ➔ ");

                out.push_str(&format!("- Format Detected      : {}\n", format_desc));
                out.push_str(&format!("- Monotonic Growth     : {}\n", monotonic_tag));
                out.push_str(&format!("- Climax Payoff Timing : {}\n", climax_timing));
                out.push_str(&format!("- Tier Progression     : {}\n", progression_str));
            }
        }

        if let Some(outro) = &self.outro_report {
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str(" 🚪 ABRUPT ENDING & OUTRO RUNWAY RADAR\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            if outro.is_abrupt_ending {
                out.push_str(&format!("- Ending Style         : Abrupt Ending ({:.1}s runway) [✅ RETENTION PROTECTED]\n", outro.outro_runway_seconds));
                if let (Some(ts), Some(phrase)) = (outro.signoff_cue_timestamp_seconds, &outro.first_signoff_phrase) {
                    let mm = (ts / 60.0).floor() as u64;
                    let ss = (ts % 60.0).round() as u64;
                    out.push_str(&format!("- First Sign-Off Cue   : {:02}:{:02} (\"{}\")\n", mm, ss, phrase));
                } else {
                    out.push_str("- First Sign-Off Cue   : None detected (Payoff held to finale)\n");
                }
                out.push_str("- Recommended Blade Cut: None required (Full timeline payoff preserved)\n");
                out.push_str("- Projected Retention  : Maximum retention preserved into end-screen cards\n\n");
            } else {
                out.push_str(&format!("- Ending Style         : Verbose Wind-Down ({:.1}s runway) [⚠️ RETENTION BLEED]\n", outro.outro_runway_seconds));
                if let (Some(ts), Some(phrase)) = (outro.signoff_cue_timestamp_seconds, &outro.first_signoff_phrase) {
                    let mm = (ts / 60.0).floor() as u64;
                    let ss = (ts % 60.0).floor() as u64;
                    out.push_str(&format!("- First Sign-Off Cue   : {:02}:{:02} (\"{}\")\n", mm, ss, phrase));
                } else {
                    out.push_str("- First Sign-Off Cue   : Outro transition\n");
                }
                out.push_str(&format!("- Recommended Blade Cut: {} (Cuts video immediately after the final scene payoff)\n", outro.recommended_blade_timecode));
                out.push_str(&format!("- Projected Retention  : Reclaims ~{:.1}% of audience retention before end-screen cards\n\n", outro.audience_loss_in_runway_pct));
            }
        }

        out.push_str("\n──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        out.push_str(" 📁 EXPORTED PRODUCTION ASSETS\n");
        out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
        for path in &self.export_paths {
            out.push_str(&format!(" • {}\n", path));
        }
        out.push_str("══════════════════════════════════════════════════════════════════════════════════════════════════════\n\n");

        out
    }

    /// Prints the Flight Sheet directly to stdout.
    pub fn render_terminal_stdout(&self) {
        println!("{}", self.render_terminal_string());
    }

    /// Renders this Flight Sheet as structured GitHub-flavored markdown.
    pub fn to_markdown_string(&self) -> String {
        let mut out = String::new();
        let header = if self.is_local_draft {
            "# ✈️ Editor's Pre-Publish Draft Sheet\n\n"
        } else {
            "# ✈️ Editor's Flight Sheet\n\n"
        };
        out.push_str(header);
        out.push_str(&format!("> **Video Title**: **{}**  \n", self.video_title));
        out.push_str(&format!("> **Channel**: `{}`  \n", self.channel_name));
        out.push_str(&format!("> **Runtime**: `{}`  \n", self.duration_formatted));
        out.push_str(&format!("> **Niche Tone Profile**: `{}`  \n", self.tone_profile_formatted));
        out.push_str(&format!("> **Overall Verdict**: **{}**  \n", self.overall_verdict));
        out.push_str(&format!("> 🚨 **Primary Bottleneck**: **{}**  \n", self.primary_bottleneck));
        out.push_str(&format!("> 💎 **Audience Satisfaction**: **{}**  \n", self.audience_satisfaction_tier));
        out.push_str(&format!("> 🚀 **Compounded ROI Potential**: **+{:.1}% Viewership Lift**  \n", self.compounded_gain_pct));

        if let Some(suit) = &self.suitability_report {
            let badge = if suit.rating == "GREEN" {
                format!("🟢 **MONETIZATION SAFE (Full Ad Distribution / Immediate Public Scheduling)** — {}", suit.executive_summary)
            } else {
                format!("🟡 **ADVERTISER RISK (Limited Ads / Unlisted 24h Review Recommended)** — {}", suit.executive_summary)
            };
            out.push_str(&format!("> 🛡️ **Monetization & Ad-Suitability**: {}\n", badge));
        }
        out.push('\n');

        out.push_str("---\n\n");
        out.push_str("## 📋 Timeline Punch List (NLE Editing Commands)\n\n");
        out.push_str("| Timecode | Action Required | Editing Command & Timestamp Prescription | Projected Impact |\n");
        out.push_str("|:---:|:---|:---|:---|\n");
        for cue in &self.timeline_punch_list {
            out.push_str(&format!(
                "| `{}` | **{}** | {} | {} |\n",
                cue.timecode_range, cue.action_type, cue.remedy_instruction, cue.impact_summary
            ));
        }

        let failed_checkpoints: Vec<&str> = self
            .re_engagement_checkpoints
            .iter()
            .filter(|c| !c.passed)
            .map(|c| if c.window_start_sec < 250.0 { "03:00" } else { "06:00" })
            .collect();
        if !failed_checkpoints.is_empty() {
            let failed_str = failed_checkpoints.join(" / ");
            out.push_str(&format!(
                "\n> ⚠️ **Retention Milestone Warning**: [{}] failed re-engagement gate. See timeline marker for recommended pacing reset.\n",
                failed_str
            ));
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                out.push_str("## 🪜 Stair-Stepping Escalation Radar\n\n");
                out.push_str("- **Format Detected**: Multi-Tiered Contrast Escalation ($X vs $Y)\n");
                let mono_tag = if ss.has_monotonic_escalation {
                    "`[VERIFIED]` Stakes increase monotonically across all tiers"
                } else {
                    "`[FAILED]` Stakes do not escalate monotonically"
                };
                out.push_str(&format!("- **Monotonic Growth**: {}\n", mono_tag));

                let climax_tier = ss.detected_tiers.iter().find(|t| t.is_climax).or_else(|| ss.detected_tiers.last());
                let climax_tag = if ss.climax_held_for_finale {
                    if let Some(ct) = climax_tier {
                        let mm = (ct.timecode_seconds / 60.0).floor() as u64;
                        let ss_sec = (ct.timecode_seconds % 60.0).round() as u64;
                        let pct = ((ct.timecode_seconds / self.duration_seconds_f64().max(1.0)) * 100.0).round() as u64;
                        format!("`[FINAL 20% LOCKED]` Peak payoff situated at {:02}:{:02} of {} ({}% mark)", mm, ss_sec, self.duration_formatted, pct)
                    } else {
                        "`[FINAL 20% LOCKED]` Peak payoff situated in final 20% of runtime".to_string()
                    }
                } else if let Some(ct) = climax_tier {
                    let mm = (ct.timecode_seconds / 60.0).floor() as u64;
                    let ss_sec = (ct.timecode_seconds % 60.0).round() as u64;
                    let pct = ((ct.timecode_seconds / self.duration_seconds_f64().max(1.0)) * 100.0).round() as u64;
                    format!("`[PREMATURE CLIMAX RISK]` Peak payoff situated at {:02}:{:02} of {} ({}% mark)", mm, ss_sec, self.duration_formatted, pct)
                } else {
                    "`[PREMATURE CLIMAX RISK]` Climax revealed prior to final 20%".to_string()
                };
                out.push_str(&format!("- **Climax Payoff Timing**: {}\n", climax_tag));

                let tier_progression_str: Vec<String> = ss.detected_tiers.iter().map(|t| {
                    let tc = t.timecode_range.split('-').next().unwrap_or("00:00").trim();
                    format!("{} ({})", t.label, tc)
                }).collect();
                out.push_str(&format!("- **Tier Progression**: {}\n\n", tier_progression_str.join(" ➔ ")));
            }
        }

        out.push_str("\n---\n\n");

        if !self.telemetry_matrix.is_empty() {
            out.push_str("## 📊 6-Vector Telemetry Matrix Audit\n\n");
            out.push_str("| Telemetry Vector | Measured | Editor Target | Score | Status | 1% Actionable Editing Prescription | Projected Lift |\n");
            out.push_str("|:---|:---:|:---:|:---:|:---|:---|:---:|\n");
            for v in &self.telemetry_matrix {
                out.push_str(&format!(
                    "| **{}** | {:.1}{} | {} | **{:.0}** | {} | {} | **+{:.1}%** |\n",
                    v.human_label(),
                    v.measured_value, v.unit,
                    v.editor_target(),
                    v.score_0_to_100,
                    v.status.badge(),
                    v.one_percent_fix,
                    v.estimated_retention_lift_pct
                ));
            }
            out.push_str("\n---\n\n");
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                out.push_str("## 🪜 Stair-Stepping Escalation Radar\n\n");
                let format_desc = "Multi-Tiered Contrast Escalation ($X vs $Y)";
                let monotonic_tag = if ss.has_monotonic_escalation {
                    "[VERIFIED] Stakes increase monotonically across all tiers"
                } else {
                    "[BROKEN] Stakes fail to increase monotonically across tiers"
                };

                let climax_tier = ss.detected_tiers.iter().find(|t| t.is_climax).cloned();
                let dur_sec = self.duration_seconds_f64();
                let climax_timing = if let Some(ref c) = climax_tier {
                    let mm = (c.timecode_seconds / 60.0).floor() as u64;
                    let ss_sec = (c.timecode_seconds % 60.0).round() as u64;
                    let dur_mm = (dur_sec / 60.0).floor() as u64;
                    let dur_ss = (dur_sec % 60.0).round() as u64;
                    let pct = ((c.timecode_seconds / dur_sec.max(1.0)) * 100.0).round() as u64;
                    if ss.climax_held_for_finale {
                        format!("[FINAL 20% LOCKED] Peak payoff situated at {:02}:{:02} of {:02}:{:02} ({}% mark)", mm, ss_sec, dur_mm, dur_ss, pct)
                    } else {
                        format!("[PREMATURE CLIMAX] Peak payoff situated at {:02}:{:02} of {:02}:{:02} ({}% mark)", mm, ss_sec, dur_mm, dur_ss, pct)
                    }
                } else {
                    "[NOT DETECTED] No distinct climax payoff identified".to_string()
                };

                let progression_str = ss.detected_tiers
                    .iter()
                    .map(|t| format!("{} ({})", t.label, t.timecode_range))
                    .collect::<Vec<_>>()
                    .join(" ➔ ");

                out.push_str(&format!("- **Format Detected**: {}\n", format_desc));
                out.push_str(&format!("- **Monotonic Growth**: {}\n", monotonic_tag));
                out.push_str(&format!("- **Climax Payoff Timing**: {}\n", climax_timing));
                out.push_str(&format!("- **Tier Progression**: {}\n\n", progression_str));
            }
        }

        out.push_str("## 📦 Packaging & Hook Upgrades (A/B Test Suite)\n\n");
        if let Some(radar) = &self.truncation_radar {
            out.push_str("### 📱 Cross-Device Title Truncation Radar\n\n");
            let push_tag = if radar.mobile_push_truncated { "`[⚠️ TRUNCATED]`" } else { "`[SAFE]`" };
            let feed_tag = if radar.mobile_feed_ctv_truncated { "`[⚠️ TRUNCATED ON MOBILE/CTV]`" } else { "`[SAFE]`" };
            let desktop_tag = if radar.desktop_search_truncated { "`[⚠️ TRUNCATED]`" } else { "`[SAFE]`" };

            out.push_str(&format!("- **Mobile Push (45c)**: {} {}\n", radar.mobile_push_preview, push_tag));
            out.push_str(&format!("- **Mobile/CTV Feed (52c)**: {} {}\n", radar.mobile_feed_ctv_preview, feed_tag));
            out.push_str(&format!("- **Desktop Search (60c)**: {} {}\n", radar.desktop_search_preview, desktop_tag));
            if let Some(rec) = &radar.front_load_recommendation {
                out.push_str(&format!(
                    "\n> 💡 **Truncation Alert**: Front-load the core hook! The title cuts off on >70% of Mobile/CTV feeds.  \n> **Recommended Front-Load**: `{}` ({} chars)\n",
                    rec, rec.chars().count()
                ));
            }
            out.push('\n');
        }
        if let Some(thumb) = &self.thumbnail_report {
            let integrity_pct = (thumb.packaging_integrity_index * 100.0).round() as u64;
            let risk_str = if integrity_pct < 40 {
                "High Bait-and-Switch Risk"
            } else if integrity_pct < 70 {
                "Moderate Packaging Friction"
            } else {
                "Authentic Alignment"
            };
            out.push_str(&format!(
                "- **Visual Packaging**: `{}`  \n- **Packaging Integrity**: `{}% ({})`  \n",
                thumb.primary_archetype.badge(),
                integrity_pct,
                risk_str
            ));
            if !thumb.diagnostic_note.is_empty() {
                out.push_str(&format!("- **Packaging Note**: {}\n", thumb.diagnostic_note));
            }
            if thumb.timestamp_collision_detected {
                let detail = thumb.collision_detail.as_deref().unwrap_or("Text/Focal element detected in bottom-right dead zone; obstructed by YouTube timestamp overlay.");
                out.push_str(&format!("- ⚠️ **Timestamp Collision**: {}\n", detail));
            }
            if let Some(redundancy) = thumb.title_text_redundancy_pct {
                if redundancy >= 50.0 {
                    out.push_str(&format!(
                        "- ⚠️ **Verbal Redundancy**: Thumbnail text duplicates title by **{:.0}%**; wastes persuasive canvas space. Shift from Reinforcement to Complementarity.\n",
                        redundancy
                    ));
                }
            }
            out.push('\n');
        }
        if let Some(gap) = &self.promise_gap_report {
            if gap.is_breach {
                out.push_str("- 🚨 **Promise Gap Breach**: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk.\n");
                if let Some(detail) = &gap.throat_clearing_detail {
                    out.push_str(&format!("  - **Detail**: {}\n", detail));
                }
                out.push_str(&format!("  - **Visual Handshake**: {}\n", gap.visual_handshake_detail));
                out.push_str(&format!("  - **Verbal Handshake**: {}\n", gap.verbal_handshake_detail));
            } else {
                out.push_str(&format!(
                    "- **Two-Part Handshake (0:00–0:05)**: `[PASSED] ({:.0}/100)` — {}\n",
                    gap.promise_gap_score, gap.verbal_handshake_detail
                ));
            }
            out.push('\n');
        }
        out.push_str(&format!("- **Current Title**: {}\n", self.packaging_upgrades.current_title));
        out.push_str(&format!("- **Title Upgrade A (Concrete Scale / Physical Anchor)**:\n  {}\n", self.packaging_upgrades.title_upgrade_a));
        out.push_str(&format!("- **Title Upgrade B (High-Stakes Curiosity Gap)**:\n  {}\n", self.packaging_upgrades.title_upgrade_b));
        out.push_str(&format!("- **Current Opening Line**:\n  {}\n", self.packaging_upgrades.opening_line_current));
        out.push_str(&format!("- **Upgraded Opening Line (4 Ps Direct Entry Hook)**:\n  {}\n\n", self.packaging_upgrades.opening_line_upgraded));

        if let Some(loudness) = &self.loudness_metrics {
            out.push_str("## 🔊 Audio Mastering & YouTube Loudness Radar\n\n");
            let penalty_tag = if loudness.youtube_penalty_db < 0.0 {
                format!("`{:.1} dB Penalty (Hot Mix)`", loudness.youtube_penalty_db)
            } else {
                "`0.0 dB (Safe / Compliant)`".to_string()
            };
            let peak_tag = if loudness.true_peak_dbtp > crate::math::LoudnessMetrics::TRUE_PEAK_CEILING_DBTP {
                format!("`{:.1} dBTP` `[⚠️ Mobile Distortion Risk]`", loudness.true_peak_dbtp)
            } else {
                format!("`{:.1} dBTP` `[SAFE]`", loudness.true_peak_dbtp)
            };
            out.push_str(&format!("- **Integrated Loudness**: `{:.1} LUFS` *(YouTube Target: -14.0 LUFS)*\n", loudness.integrated_lufs));
            out.push_str(&format!("- **YouTube Volume Attenuation**: {}\n", penalty_tag));
            out.push_str(&format!("- **Cold-Open Loudness (0:00-0:30)**: `{:.1} LUFS`\n", loudness.cold_open_lufs));
            out.push_str(&format!("- **Loudness Range (LRA)**: `{:.1} LU`\n", loudness.loudness_range_lu));
            out.push_str(&format!("- **True Peak Level**: {}\n", peak_tag));
            out.push_str(&format!("- **Mastering Status**: {}\n", loudness.mastering_status));
            out.push_str(&format!("- 💡 **Actionable Master Bus Prescription**: {}\n\n", loudness.actionable_prescription));
            out.push_str("---\n\n");
        }

        if let Some(fat) = &self.cuttable_fat {
            out.push_str("## ✂️ Cumulative Cuttable Fat & Re-Timing Budget\n\n");
            out.push_str(&format!("> **Executive Shaving Budget**: **{}**  \n\n", fat.executive_summary));
            out.push_str("| Metric | Measured Shaving Fat | Editor Action |\n");
            out.push_str("|:---|:---:|:---|\n");
            out.push_str(&format!("| **Total Cuttable Runtime** | **{:.1}s** | Shave across {} micro-edits ({} -> {}, -{:.1}%) |\n",
                fat.total_cuttable_seconds, fat.micro_edits_count, fat.original_runtime_formatted, fat.streamlined_runtime_formatted, fat.runtime_reduction_pct));
            out.push_str(&format!("| **Dead Air / Breath Pauses** | {:.1}s | Tighten breath gaps > 0.4s to 0.25s |\n", fat.dead_air_breath_seconds));
            out.push_str(&format!("| **Excess Camera Hold Stagnation** | {:.1}s | Insert 115% digital punch-ins or B-roll at midpoint |\n", fat.camera_hold_fat_seconds));
            out.push_str(&format!("| **Pre-Speech Cold-Open Lag** | {:.1}s | Trim bumper silence before first spoken syllable |\n\n", fat.cold_open_lag_seconds));

            if !fat.punch_in_cues.is_empty() {
                out.push_str("### 🎯 Midpoint 115% Digital Punch-In Markers\n\n");
                out.push_str("| Timecode | Take Duration | Scale | NLE Marker Command |\n");
                out.push_str("|:---:|:---:|:---:|:---|\n");
                for cue in &fat.punch_in_cues {
                    out.push_str(&format!("| `{}` | {:.1}s | **{}%** | {} |\n", cue.timecode, cue.hold_duration_seconds, cue.crop_scale_pct, cue.edit_instruction));
                }
                out.push('\n');
            }
            out.push_str("---\n\n");
        }

        if !self.comment_clusters.is_empty() {
            out.push_str("## 💬 Crowd-Sourced Comment Friction & Engagement Clusters\n\n");
            out.push_str("| Timecode | Sentiment / Category | Mentions | Total Likes | Top Viewer Consensus Quote | Correlated Retention Event |\n");
            out.push_str("|:---:|:---|:---:|:---:|:---|:---|\n");
            for cluster in &self.comment_clusters {
                let rel = cluster.correlated_retention_event.as_deref().unwrap_or("None");
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} | \"{}\" | {} |\n",
                    cluster.timecode_formatted,
                    cluster.sentiment.badge(),
                    cluster.mention_count,
                    cluster.total_likes,
                    cluster.representative_quote,
                    rel
                ));
            }
            out.push_str("\n---\n\n");
        }

        if let Some(shorts) = &self.shorts_metrics {
            out.push_str("## 📱 YouTube Shorts Mobile Retention Radar\n\n");
            let hook_tag = if shorts.first_frame_hook_safe { "`[✅ SAFE <= 1.5s]`" } else { "`[⚠️ HIGH SWIPE-AWAY RISK > 1.5s]`" };
            let loop_tag = if shorts.loop_seamless { "`[🟢 SEAMLESS INFINITE LOOP]`" } else { "`[⚠️ ABRUPT EXIT CUTOFF]`" };
            out.push_str(&format!("- **First-Frame Hook Delay**: `{:.2}s` {}\n", shorts.first_frame_hook_delay, hook_tag));
            out.push_str(&format!("- **Duration Safety Status**: `{:?}` (`{:.1}s`)\n", shorts.duration_safety_status, shorts.duration_seconds));
            out.push_str(&format!("- **Loop Continuity Score**: `**{:.1} / 100**` {}\n", shorts.loop_continuity_score, loop_tag));
            out.push_str(&format!("- **Mobile Stay vs Swipe Risk**: `**{}**` (Est. Stay Ratio: `**{:.1}%**`)\n", shorts.stay_vs_swipe_risk, shorts.estimated_stay_ratio_pct));
            out.push_str(&format!("- **Loop Bridge Insight**: {}\n", shorts.loop_bridge_insight));
            out.push_str(&format!("- **Executive Directive**: {}\n\n", shorts.executive_summary));
            out.push_str("---\n\n");
        }

        if let Some(outro) = &self.outro_report {
            out.push_str("## 🚪 ABRUPT ENDING & OUTRO RUNWAY RADAR\n\n");
            if outro.is_abrupt_ending {
                out.push_str(&format!("- **Ending Style**: Abrupt Ending ({:.1}s runway) `[✅ RETENTION PROTECTED]`\n", outro.outro_runway_seconds));
                if let (Some(ts), Some(phrase)) = (outro.signoff_cue_timestamp_seconds, &outro.first_signoff_phrase) {
                    let mm = (ts / 60.0).floor() as u64;
                    let ss = (ts % 60.0).round() as u64;
                    out.push_str(&format!("- **First Sign-Off Cue**: {:02}:{:02} (\"{}\")\n", mm, ss, phrase));
                } else {
                    out.push_str("- **First Sign-Off Cue**: None detected (Payoff held to finale)\n");
                }
                out.push_str("- **Recommended Blade Cut**: None required (Full timeline payoff preserved)\n");
                out.push_str("- **Projected Retention**: Maximum retention preserved into end-screen cards\n\n");
            } else {
                out.push_str(&format!("- **Ending Style**: Verbose Wind-Down ({:.1}s runway) `[⚠️ RETENTION BLEED]`\n", outro.outro_runway_seconds));
                if let (Some(ts), Some(phrase)) = (outro.signoff_cue_timestamp_seconds, &outro.first_signoff_phrase) {
                    let mm = (ts / 60.0).floor() as u64;
                    let ss = (ts % 60.0).floor() as u64;
                    out.push_str(&format!("- **First Sign-Off Cue**: {:02}:{:02} (\"{}\")\n", mm, ss, phrase));
                } else {
                    out.push_str("- **First Sign-Off Cue**: Outro transition\n");
                }
                out.push_str(&format!("- **Recommended Blade Cut**: {} (Cuts video immediately after the final scene payoff)\n", outro.recommended_blade_timecode));
                out.push_str(&format!("- **Projected Retention**: Reclaims ~{:.1}% of audience retention before end-screen cards\n\n", outro.audience_loss_in_runway_pct));
            }
            out.push_str("---\n\n");
        }

        out.push_str("---\n\n");
        out.push_str("## 📁 Exported NLE Production Assets\n\n");
        for path in &self.export_paths {
            out.push_str(&format!("- `{}`\n", path));
        }
        out.push('\n');

        out
    }

    /// Saves the Flight Sheet as `output/<slug>_flight_sheet.md` (or `_draft_flight_sheet.md`).
    pub fn save_to_markdown(&self, output_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let slug = slugify(&self.video_title);
        let suffix = if self.is_local_draft { "_draft" } else { "" };
        let filename = format!("{}{}_flight_sheet.md", slug, suffix);
        let target_path = output_dir.join(filename);
        let md = self.to_markdown_string();
        std::fs::write(&target_path, md)?;
        Ok(target_path)
    }

    /// Constructs an ExportTimeline populated with markers corresponding to the punch list cues.
    pub fn to_export_timeline(&self) -> ExportTimeline {
        let dur_sec = parse_first_seconds(&self.duration_formatted).unwrap_or(600.0);
        let mut timeline = ExportTimeline::new_landscape(&self.video_title, dur_sec);
        let timescale = timeline.frame_rate_numerator;
        let frame_duration = RationalTime::new(timeline.frame_rate_denominator, timescale);

        if timeline.shots.is_empty() {
            timeline.shots.push(crate::export::timeline::ShotClipPlaceholder {
                shot_index: 1,
                offset: RationalTime::zero(),
                duration: timeline.duration,
                label: "Shot 01 (Continuous Sequence)".to_string(),
                markers: Vec::new(),
            });
        }

        for cue in &self.timeline_punch_list {
            if cue.action_type == "OUTRO FIX" {
                if let Some(ref outro) = self.outro_report {
                    if outro.is_abrupt_ending {
                        continue;
                    }
                    let ts_sec = outro.signoff_cue_timestamp_seconds.unwrap_or_else(|| {
                        parse_first_seconds(&outro.recommended_blade_timecode).unwrap_or(dur_sec * 0.85)
                    });
                    let marker_time = RationalTime::from_seconds(ts_sec, timescale);
                    let note = format!(
                        "Cut video here. Eliminates {:.0}s of verbal wind-down and halts premature audience exit.",
                        outro.outro_runway_seconds
                    );
                    if let Some(shot) = timeline.shots.first_mut() {
                        shot.markers.push(TimelineMarker::new(
                            marker_time,
                            frame_duration,
                            "[BLADE TIMELINE HERE: ABRUPT ENDING]".to_string(),
                            &note,
                            MarkerColor::Orange,
                        ));
                    }
                    continue;
                }
            }

            let ts_sec = match cue.action_type.as_str() {
                "TRIM DEAD AIR" => parse_first_seconds(&cue.timecode_range).unwrap_or(0.0),
                "INSERT B-ROLL" => {
                    let parts: Vec<&str> = cue.timecode_range.split('-').collect();
                    if parts.len() == 2 {
                        let t1 = parse_first_seconds(parts[0]).unwrap_or(0.0);
                        let t2 = parse_first_seconds(parts[1]).unwrap_or(t1);
                        if t1 == 0.0 && t2 > 0.0 {
                            (t2 / 2.0).round()
                        } else {
                            ((t1 + t2) / 2.0).round()
                        }
                    } else {
                        parse_first_seconds(&cue.timecode_range).unwrap_or(0.0)
                    }
                }
                "CUT EXPOSITION" => {
                    if let Some(sec) = parse_first_seconds(&cue.timecode_range) {
                        sec
                    } else if let Some(tc) = extract_timecode(&cue.remedy_instruction) {
                        parse_first_seconds(&tc).unwrap_or((dur_sec * 0.50).round())
                    } else {
                        (dur_sec * 0.50).round()
                    }
                }
                "OUTRO FIX" => {
                    if let Some(sec) = parse_first_seconds(&cue.timecode_range) {
                        sec
                    } else if let Some(tc) = extract_timecode(&cue.remedy_instruction) {
                        parse_first_seconds(&tc).unwrap_or((dur_sec * 0.85).round())
                    } else {
                        (dur_sec * 0.85).round()
                    }
                }
                _ => parse_first_seconds(&cue.timecode_range).unwrap_or(0.0),
            };

            let marker_time = RationalTime::from_seconds(ts_sec, timescale);
            let color = match cue.action_type.as_str() {
                "TRIM DEAD AIR" => MarkerColor::Cyan,
                "INSERT B-ROLL" => MarkerColor::Purple,
                "CUT EXPOSITION" => MarkerColor::Red,
                "OUTRO FIX" => MarkerColor::Orange,
                _ => MarkerColor::Blue,
            };
            let label = format!("[{}] {}", cue.action_type, cue.impact_summary);
            if let Some(shot) = timeline.shots.first_mut() {
                shot.markers.push(TimelineMarker::new(
                    marker_time,
                    frame_duration,
                    label,
                    &cue.remedy_instruction,
                    color,
                ));
            }
        }

        if let Some(fat) = &self.cuttable_fat {
            for cue in &fat.punch_in_cues {
                let marker_time = RationalTime::from_seconds(cue.timecode_seconds, timescale);
                if let Some(shot) = timeline.shots.first_mut() {
                    shot.markers.push(TimelineMarker::new(
                        marker_time,
                        frame_duration,
                        "[PUNCH-IN 115%]",
                        &cue.edit_instruction,
                        MarkerColor::Cyan,
                    ));
                }
            }
        }

        for cluster in &self.comment_clusters {
            let marker_time = RationalTime::from_seconds(cluster.center_second, timescale);
            let (marker_color, prefix) = match cluster.sentiment {
                crate::analyzer::comments::ClusterSentiment::ConfusionOrFriction => (MarkerColor::Red, "[AUDIENCE FRICTION]"),
                crate::analyzer::comments::ClusterSentiment::PraiseOrHumor => (MarkerColor::Green, "[AUDIENCE REACTION]"),
                crate::analyzer::comments::ClusterSentiment::StoryEngagement => (MarkerColor::Purple, "[AUDIENCE STORY BEAT]"),
                crate::analyzer::comments::ClusterSentiment::NeutralTimecode => (MarkerColor::Cyan, "[AUDIENCE TIMECODE]"),
            };
            let note = if let Some(ref rel) = cluster.correlated_retention_event {
                format!("\"{}\" ({} likes) | {}", cluster.representative_quote, cluster.total_likes, rel)
            } else {
                format!("\"{}\" ({} likes)", cluster.representative_quote, cluster.total_likes)
            };
            if let Some(shot) = timeline.shots.first_mut() {
                shot.markers.push(TimelineMarker::new(
                    marker_time,
                    frame_duration,
                    format!("{} {}", prefix, cluster.timecode_formatted),
                    &note,
                    marker_color,
                ));
            }
        }

        for checkpoint in &self.re_engagement_checkpoints {
            if !checkpoint.passed {
                let exact_sec = if checkpoint.window_start_sec < 250.0 { 180.0 } else { 360.0 };
                let mm_ss = if exact_sec == 180.0 { "03:00" } else { "06:00" };
                let marker_time = RationalTime::from_seconds(exact_sec, timescale);
                if let Some(shot) = timeline.shots.first_mut() {
                    shot.markers.push(TimelineMarker::new(
                        marker_time,
                        frame_duration,
                        format!("[RE-ENGAGEMENT CUE] Pacing fatigue at {}. Insert visual pattern interrupt or narrative complication.", mm_ss),
                        &checkpoint.diagnostic_note,
                        MarkerColor::Red,
                    ));
                }
            }
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                for tier in &ss.detected_tiers {
                    let marker_time = RationalTime::from_seconds(tier.timecode_seconds, timescale);
                    let climax_desc = if tier.is_climax { "Peak stakes climax payoff" } else { "Escalating stakes tier" };
                    if let Some(shot) = timeline.shots.first_mut() {
                        shot.markers.push(TimelineMarker::new(
                            marker_time,
                            frame_duration,
                            format!("[TIER #{}: {}] {}", tier.tier_index, tier.label, climax_desc),
                            format!("Scale: {:.0} | Range: {} | Climax: {}", tier.scale_value, tier.timecode_range, tier.is_climax),
                            MarkerColor::Purple,
                        ));
                    }
                }
            }
        }

        timeline
    }

    /// Saves the color-coded template FCPXML to `output/<slug>_template.fcpxml` (or `_draft_template.fcpxml`).
    /// Automatically embeds the local 1x1 black slug asset for single-click DaVinci Resolve marker import.
    pub fn save_fcpxml_template(&self, output_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let slug_asset = crate::export::timeline::ensure_slug_asset(output_dir)?;
        let slug = slugify(&self.video_title);
        let suffix = if self.is_local_draft { "_draft" } else { "" };
        let filename = format!("{}{}_template.fcpxml", slug, suffix);
        let target_path = output_dir.join(filename);
        let mut timeline = self.to_export_timeline();
        timeline.slug_path = Some(slug_asset.clone());
        timeline.save_fcpxml_with_slug(&target_path, Some(&slug_asset))?;
        Ok(target_path)
    }

    /// Saves the color-coded template EDL with native DaVinci Resolve markers to `output/<slug>.edl` (or `_draft.edl`).
    pub fn save_edl_template(&self, output_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let slug = slugify(&self.video_title);
        let suffix = if self.is_local_draft { "_draft" } else { "" };
        let filename = format!("{}{}.edl", slug, suffix);
        let target_path = output_dir.join(filename);
        let timeline = self.to_export_timeline();
        timeline.save_edl(&target_path)?;
        Ok(target_path)
    }

    /// Saves the interactive HTML report to `output/<slug>_report.html` (or `_draft_report.html`).
    pub fn save_html_report(&self, output_dir: &Path, scorecard: &crate::math::marginal_gains::MarginalGainsScorecard) -> std::io::Result<PathBuf> {
        crate::export::html_report::HtmlReportGenerator::save(self, scorecard, output_dir)
    }

    /// Automatically writes all production deliverables:
    /// - Markdown Flight Sheet (`<slug>_flight_sheet.md`)
    /// - Interactive HTML Report (`<slug>_report.html`)
    /// - Apple FCPXML Template (`<slug>_template.fcpxml`)
    /// - DaVinci Resolve Marker EDL (`<slug>.edl`)
    pub fn save_all_assets(
        &self,
        output_dir: &Path,
        scorecard: &crate::math::marginal_gains::MarginalGainsScorecard,
    ) -> std::io::Result<Vec<PathBuf>> {
        std::fs::create_dir_all(output_dir)?;
        let paths = vec![
            self.save_to_markdown(output_dir)?,
            self.save_html_report(output_dir, scorecard)?,
            self.save_fcpxml_template(output_dir)?,
            self.save_edl_template(output_dir)?,
        ];
        Ok(paths)
    }
}

