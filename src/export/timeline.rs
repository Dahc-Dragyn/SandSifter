use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

use crate::math::forensics::AcousticDeliveryMetrics;
use crate::math::retention::{RetentionCurveReport, RetentionEventType};
use crate::math::visual::VisualPacingMetrics;

/// XML Escaper for safe text embedding in FCPXML attributes and bodies.
pub fn xml_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Minimal valid 1x1 black PNG image (67 bytes).
/// Used as an embedded lightweight slug asset for DaVinci Resolve & FCPXML timeline guide tracks.
pub const BLACK_SLUG_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    0x00, 0x00, 0x00, 0x0D, // IHDR length (13)
    0x49, 0x48, 0x44, 0x52, // "IHDR"
    0x00, 0x00, 0x00, 0x01, // Width: 1
    0x00, 0x00, 0x00, 0x01, // Height: 1
    0x08, 0x00, 0x00, 0x00, 0x00, // 8-bit Grayscale, compression 0, filter 0, interlace 0
    0x3A, 0x7E, 0x9B, 0x55, // IHDR CRC
    0x00, 0x00, 0x00, 0x0A, // IDAT length (10)
    0x49, 0x48, 0x41, 0x54, // "IDAT"
    0x78, 0x9C, 0x63, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, // zlib stream
    0x48, 0xAF, 0xA4, 0x71, // IDAT CRC
    0x00, 0x00, 0x00, 0x00, // IEND length (0)
    0x49, 0x45, 0x4E, 0x44, // "IEND"
    0xAE, 0x42, 0x60, 0x82, // IEND CRC
];

/// Converts a local filesystem Path into a RFC 3986 compliant `file:///...` URL.
pub fn path_to_file_url(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let cleaned = raw.replace('\\', "/");
    let trimmed = cleaned.trim_start_matches("//?/");
    let encoded = trimmed.replace(' ', "%20");
    if encoded.starts_with('/') {
        format!("file://{}", encoded)
    } else {
        format!("file:///{}", encoded)
    }
}

/// Ensures that the lightweight 1x1 black slug asset exists in the target directory.
/// Writes `output_dir/.sandsifter_slug.png` if it does not already exist.
pub fn ensure_slug_asset(output_dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let slug_path = output_dir.join(".sandsifter_slug.png");
    if !slug_path.exists() {
        std::fs::write(&slug_path, BLACK_SLUG_PNG)?;
    }
    let abs_path = std::fs::canonicalize(&slug_path).unwrap_or(slug_path);
    Ok(abs_path)
}

/// Rational time representation matching the Apple FCPXML spec: value/timescale seconds.
/// Guarantees floating-point rounding errors never introduce drop-frame timecode drift in NLEs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RationalTime {
    pub value: i64,
    pub timescale: i64,
}

impl RationalTime {
    pub const fn new(value: i64, timescale: i64) -> Self {
        Self { value, timescale }
    }

    pub const fn zero() -> Self {
        Self {
            value: 0,
            timescale: 30000,
        }
    }

    /// Converts seconds to a RationalTime aligned to the given timescale (e.g., 30000 for 29.97fps, 24 for 24fps).
    pub fn from_seconds(seconds: f64, timescale: i64) -> Self {
        let value = (seconds * timescale as f64).round() as i64;
        Self { value, timescale }
    }

    /// Returns the decimal seconds representation.
    pub fn to_seconds(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.value as f64 / self.timescale as f64
        }
    }

    /// Formats as an FCPXML time attribute string (e.g., "0s", "5s", or "1001/30000s").
    pub fn to_fcpxml_str(&self) -> String {
        if self.value == 0 {
            "0s".to_string()
        } else if self.timescale == 1 {
            format!("{}s", self.value)
        } else if self.value % self.timescale == 0 {
            format!("{}s", self.value / self.timescale)
        } else {
            format!("{}/{}s", self.value, self.timescale)
        }
    }

    /// Converts into SMPTE timecode string ("HH:MM:SS:FF") for standard EDL generation.
    pub fn to_timecode_str(&self, fps: f64) -> String {
        let total_seconds = self.to_seconds().max(0.0);
        let hours = (total_seconds / 3600.0).floor() as u32;
        let rem_seconds = total_seconds - (hours as f64 * 3600.0);
        let minutes = (rem_seconds / 60.0).floor() as u32;
        let seconds = (rem_seconds - (minutes as f64 * 60.0)).floor() as u32;
        let sub_second = total_seconds - total_seconds.floor();
        let frames = (sub_second * fps).floor() as u32;

        format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, seconds, frames)
    }

    /// Converts time into a common timescale to prevent drift when mixing time bases.
    pub fn rescaled(&self, new_timescale: i64) -> Self {
        if self.timescale == new_timescale || self.timescale == 0 {
            return *self;
        }
        let new_value = ((self.value as f64 / self.timescale as f64) * new_timescale as f64).round() as i64;
        Self {
            value: new_value,
            timescale: new_timescale,
        }
    }
}

impl std::ops::Add for RationalTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        if self.timescale == rhs.timescale {
            Self {
                value: self.value + rhs.value,
                timescale: self.timescale,
            }
        } else {
            let common_ts = self.timescale * rhs.timescale;
            let val = (self.value * rhs.timescale) + (rhs.value * self.timescale);
            Self {
                value: val,
                timescale: common_ts,
            }
        }
    }
}

impl std::ops::Sub for RationalTime {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        if self.timescale == rhs.timescale {
            Self {
                value: self.value - rhs.value,
                timescale: self.timescale,
            }
        } else {
            let common_ts = self.timescale * rhs.timescale;
            let val = (self.value * rhs.timescale) - (rhs.value * self.timescale);
            Self {
                value: val,
                timescale: common_ts,
            }
        }
    }
}

/// Color classification for timeline markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkerColor {
    Green,   // Replay spikes / viral retention surges
    Red,     // Retention dips / drop-offs
    Blue,    // Cut pacing / visual rhythm
    Cyan,    // Hook window / acoustic speech onset
    Orange,  // Peak moment across entire video
    Purple,  // Linguistic / open-loop marker
}

impl MarkerColor {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Green => "GREEN",
            Self::Red => "RED",
            Self::Blue => "BLUE",
            Self::Cyan => "CYAN",
            Self::Orange => "ORANGE",
            Self::Purple => "PURPLE",
        }
    }

    /// Returns the exact DaVinci Resolve marker color name recognized by CMX 3600 EDL import.
    pub fn resolve_color_name(&self) -> &'static str {
        match self {
            Self::Green => "ResolveColorGreen",
            Self::Red => "ResolveColorRed",
            Self::Blue => "ResolveColorBlue",
            Self::Cyan => "ResolveColorCyan",
            Self::Orange => "ResolveColorOrange",
            Self::Purple => "ResolveColorPurple",
        }
    }
}

/// A timeline marker placed at a specific point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineMarker {
    pub start: RationalTime,
    pub duration: RationalTime,
    pub name: String,
    pub note: String,
    pub color: MarkerColor,
}

impl TimelineMarker {
    pub fn new(
        start: RationalTime,
        duration: RationalTime,
        name: impl Into<String>,
        note: impl Into<String>,
        color: MarkerColor,
    ) -> Self {
        Self {
            start,
            duration,
            name: name.into(),
            note: note.into(),
            color,
        }
    }
}

/// A placeholder visual slice corresponding to an ASL shot placed on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShotClipPlaceholder {
    pub shot_index: usize,
    /// Absolute start offset on the master sequence timeline.
    pub offset: RationalTime,
    /// Duration of this shot.
    pub duration: RationalTime,
    pub label: String,
    /// Markers attached to this specific shot (with local start relative to shot offset).
    pub markers: Vec<TimelineMarker>,
}

/// Master timeline container ready for FCPXML or EDL export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportTimeline {
    pub title: String,
    pub duration: RationalTime,
    pub frame_rate_numerator: i64,
    pub frame_rate_denominator: i64,
    pub width: u32,
    pub height: u32,
    pub shots: Vec<ShotClipPlaceholder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug_path: Option<PathBuf>,
}

impl ExportTimeline {
    /// Builds a new timeline with vertical 9:16 default resolution (1080x1920) at 29.97fps.
    pub fn new_vertical_short(title: impl Into<String>, duration_seconds: f64) -> Self {
        let timescale = 30000;
        let duration = RationalTime::from_seconds(duration_seconds, timescale);
        Self {
            title: title.into(),
            duration,
            frame_rate_numerator: 30000,
            frame_rate_denominator: 1001,
            width: 1080,
            height: 1920,
            shots: Vec::new(),
            slug_path: None,
        }
    }

    /// Builds a new timeline with standard landscape 16:9 resolution (1920x1080) at 29.97fps.
    pub fn new_landscape(title: impl Into<String>, duration_seconds: f64) -> Self {
        let timescale = 30000;
        let duration = RationalTime::from_seconds(duration_seconds, timescale);
        Self {
            title: title.into(),
            duration,
            frame_rate_numerator: 30000,
            frame_rate_denominator: 1001,
            width: 1920,
            height: 1080,
            shots: Vec::new(),
            slug_path: None,
        }
    }

    /// Configures the embedded slug asset path for single-click DaVinci Resolve import.
    pub fn with_slug_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.slug_path = Some(path.into());
        self
    }

    /// Synthesizes an ExportTimeline from empirical forensics:
    /// - ASL cuts from `visual`
    /// - Retention spikes and dips from `retention`
    /// - Speech onset & hook WPM from `acoustic`
    pub fn from_forensics(
        title: &str,
        total_duration_sec: f64,
        is_vertical: bool,
        pacing: Option<&VisualPacingMetrics>,
        retention: Option<&RetentionCurveReport>,
        acoustic: Option<&AcousticDeliveryMetrics>,
    ) -> Self {
        let mut timeline = if is_vertical {
            Self::new_vertical_short(title, total_duration_sec)
        } else {
            Self::new_landscape(title, total_duration_sec)
        };

        let timescale = timeline.frame_rate_numerator;
        let frame_duration = RationalTime::new(timeline.frame_rate_denominator, timescale);

        // 1. Build Contiguous Shot Placeholders from Cut Timestamps
        let mut cut_points: Vec<f64> = match pacing {
            Some(p) if !p.cut_timestamps.is_empty() => p.cut_timestamps.clone(),
            _ => Vec::new(),
        };

        // Ensure cuts are sorted and within (0.0, total_duration_sec)
        cut_points.retain(|&t| t > 0.05 && t < (total_duration_sec - 0.05));
        cut_points.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        cut_points.dedup();

        if cut_points.is_empty() {
            // Single shot video: one contiguous gap covering the entire duration
            timeline.shots.push(ShotClipPlaceholder {
                shot_index: 1,
                offset: RationalTime::zero(),
                duration: timeline.duration,
                label: "Shot 1 (Continuous Take)".to_string(),
                markers: Vec::new(),
            });
        } else {
            let mut prev_time = 0.0;
            for (idx, &cut) in cut_points.iter().enumerate() {
                let shot_dur_sec = (cut - prev_time).max(0.1);
                let offset = RationalTime::from_seconds(prev_time, timescale);
                let duration = RationalTime::from_seconds(shot_dur_sec, timescale);
                timeline.shots.push(ShotClipPlaceholder {
                    shot_index: idx + 1,
                    offset,
                    duration,
                    label: format!("Shot {:02} ({:.1}s)", idx + 1, shot_dur_sec),
                    markers: Vec::new(),
                });
                prev_time = cut;
            }

            // Final trailing shot
            if prev_time < total_duration_sec {
                let tail_dur = total_duration_sec - prev_time;
                let offset = RationalTime::from_seconds(prev_time, timescale);
                let duration = RationalTime::from_seconds(tail_dur, timescale);
                let idx = timeline.shots.len() + 1;
                timeline.shots.push(ShotClipPlaceholder {
                    shot_index: idx,
                    offset,
                    duration,
                    label: format!("Shot {:02} ({:.1}s)", idx, tail_dur),
                    markers: Vec::new(),
                });
            }
        }

        // Helper closure to attach a marker to the appropriate shot placeholder
        let attach_marker = |shots: &mut Vec<ShotClipPlaceholder>, marker_time_sec: f64, name: &str, note: &str, color: MarkerColor| {
            if shots.is_empty() {
                return;
            }
            let marker_rat = RationalTime::from_seconds(marker_time_sec, timescale);

            // Find the shot that contains this timestamp
            let mut target_idx = 0;
            for (i, shot) in shots.iter().enumerate() {
                let shot_end = shot.offset + shot.duration;
                if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                    target_idx = i;
                    break;
                }
                if i == shots.len() - 1 {
                    target_idx = i;
                }
            }

            let local_start = marker_rat - shots[target_idx].offset;
            shots[target_idx].markers.push(TimelineMarker::new(
                local_start,
                frame_duration,
                format!("[{}] {}", color.label(), name),
                note,
                color,
            ));
        };

        // 2. Map Acoustic Delivery Data (Speech Onset & Hook Pace)
        if let Some(ac) = acoustic {
            let onset_sec = ac.t_silence_ms.unwrap_or(ac.pre_speech_silence_ms) as f64 / 1000.0;
            attach_marker(
                &mut timeline.shots,
                onset_sec,
                &format!("Speech Onset ({:.2}s)", onset_sec),
                "First spoken vocal onset; maintain audio ducking here.",
                MarkerColor::Cyan,
            );

            let pace_label = format!("Hook Pace: {:.0} WPM", ac.wpm_0_to_35.max(ac.wpm_opening));
            let pace_note = if ac.is_rapid_delivery {
                "High-velocity delivery (>=165 WPM). Strong hook retention rhythm."
            } else if ac.is_sluggish_delivery {
                "Sluggish delivery (<=110 WPM). Consider tightening pauses."
            } else {
                "Balanced conversational pace."
            };
            attach_marker(&mut timeline.shots, 0.0, &pace_label, pace_note, MarkerColor::Purple);
        }

        // 3. Map Retention Curve Inflections (Dips and Spikes)
        if let Some(ret) = retention {
            // Map Top Replay Spike into Green <marker>
            if let Some(top_spike) = ret.major_spikes.iter().max_by(|a, b| a.delta_intensity.partial_cmp(&b.delta_intensity).unwrap_or(std::cmp::Ordering::Equal)) {
                let label = format!("Top Replay Spike: Retention Spike (+{:.1}%)", top_spike.delta_intensity * 100.0);
                let note = format!("Transcript: \"{}\" | Payoff: {}", top_spike.transcript_context, top_spike.tactical_diagnosis);
                attach_marker(&mut timeline.shots, top_spike.timestamp_sec, &label, &note, MarkerColor::Green);

                // Map any other secondary replay spikes
                for spike in &ret.major_spikes {
                    if (spike.timestamp_sec - top_spike.timestamp_sec).abs() > 0.25 {
                        let sub_label = format!("Retention Spike (+{:.1}%)", spike.delta_intensity * 100.0);
                        let sub_note = format!("Transcript: \"{}\" | Note: {}", spike.transcript_context, spike.tactical_diagnosis);
                        attach_marker(&mut timeline.shots, spike.timestamp_sec, &sub_label, &sub_note, MarkerColor::Green);
                    }
                }
            }

            // Map Top Attention Breaches into Red <marker> tags with Root Cause diagnosis
            if !ret.attention_breaches.is_empty() {
                let mut sorted_breaches = ret.attention_breaches.clone();
                sorted_breaches.sort_by(|a, b| a.r_prime.partial_cmp(&b.r_prime).unwrap_or(std::cmp::Ordering::Equal));

                // Take top attention breaches (up to 5 steepest drop-offs)
                for breach in sorted_breaches.iter().take(5) {
                    let drop_rate = (breach.r_prime * 100.0).abs();
                    let label = format!("Attention Breach / Drop (-{:.1}%/s)", drop_rate);
                    let note = format!(
                        "Root Cause: {} | Remedy: {} | Transcript: \"{}\"",
                        breach.root_cause.label(),
                        breach.tactical_remedy,
                        breach.transcript_context
                    );
                    attach_marker(&mut timeline.shots, breach.timestamp_sec, &label, &note, MarkerColor::Red);
                }
            } else {
                for dip in &ret.major_dips {
                    let drop_pct = (dip.delta_intensity * 100.0).abs();
                    let label = match dip.event_type {
                        RetentionEventType::CatastrophicDrop => format!("CRITICAL DROP (-{:.1}%)", drop_pct),
                        _ => format!("Retention Dip (-{:.1}%)", drop_pct),
                    };
                    let note = format!("Transcript: \"{}\" | Diagnosis: {}", dip.transcript_context, dip.tactical_diagnosis);
                    attach_marker(&mut timeline.shots, dip.timestamp_sec, &label, &note, MarkerColor::Red);
                }
            }

            for checkpoint in &ret.re_engagement_checkpoints {
                if !checkpoint.passed {
                    let exact_sec = if checkpoint.window_start_sec < 250.0 { 180.0 } else { 360.0 };
                    let mm_ss = if exact_sec == 180.0 { "03:00" } else { "06:00" };
                    let label = format!("[RE-ENGAGEMENT CUE] Pacing fatigue at {}. Insert visual pattern interrupt or narrative complication.", mm_ss);
                    attach_marker(&mut timeline.shots, exact_sec, &label, &checkpoint.diagnostic_note, MarkerColor::Red);
                }
            }
        }

        timeline
    }

    /// Injects dedicated Red timeline markers for failed 3-Minute and 6-Minute Re-Engagement Gates.
    pub fn inject_re_engagement_checkpoints(&mut self, checkpoints: &[crate::math::retention::ReEngagementCheckpoint]) {
        let timescale = self.frame_rate_numerator;
        let frame_duration = RationalTime::new(self.frame_rate_denominator, timescale);

        for checkpoint in checkpoints {
            if !checkpoint.passed {
                let exact_sec = if checkpoint.window_start_sec < 250.0 { 180.0 } else { 360.0 };
                let mm_ss = if exact_sec == 180.0 { "03:00" } else { "06:00" };
                let marker_rat = RationalTime::from_seconds(exact_sec, timescale);

                let mut target_idx = 0;
                for (i, shot) in self.shots.iter().enumerate() {
                    let shot_end = shot.offset + shot.duration;
                    if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                        target_idx = i;
                        break;
                    }
                    if i == self.shots.len() - 1 {
                        target_idx = i;
                    }
                }

                if let Some(shot) = self.shots.get_mut(target_idx) {
                    let local_start = marker_rat - shot.offset;
                    shot.markers.push(TimelineMarker::new(
                        local_start,
                        frame_duration,
                        format!("[RE-ENGAGEMENT CUE] Pacing fatigue at {}. Insert visual pattern interrupt or narrative complication.", mm_ss),
                        &checkpoint.diagnostic_note,
                        MarkerColor::Red,
                    ));
                }
            }
        }
    }

    /// Injects green timeline markers for early micro-milestones.
    pub fn inject_micro_milestones(&mut self, milestones: &[crate::pipeline::script_generator::MicroMilestone]) {
        let timescale = self.frame_rate_numerator;
        let frame_duration = RationalTime::new(self.frame_rate_denominator, timescale);

        for ms in milestones {
            let marker_rat = RationalTime::from_seconds(ms.timecode_seconds, timescale);
            let mut target_idx = 0;
            for (i, shot) in self.shots.iter().enumerate() {
                let shot_end = shot.offset + shot.duration;
                if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                    target_idx = i;
                    break;
                }
                if i == self.shots.len() - 1 {
                    target_idx = i;
                }
            }

            if let Some(shot) = self.shots.get_mut(target_idx) {
                let local_start = marker_rat - shot.offset;
                shot.markers.push(TimelineMarker::new(
                    local_start,
                    frame_duration,
                    format!("[MILESTONE #{}] {}", ms.milestone_index, ms.achievement_description),
                    &ms.progression_cue,
                    MarkerColor::Green,
                ));
            }
        }
    }

    /// Injects dedicated Cyan timeline markers for 115% Digital Punch-Ins at the midpoint
    /// of extended single-camera takes where no B-roll exists.
    pub fn inject_punch_in_cues(&mut self, cues: &[crate::math::marginal_gains::PunchInCue]) {
        let timescale = self.frame_rate_numerator;
        let frame_duration = RationalTime::new(self.frame_rate_denominator, timescale);

        for cue in cues {
            if self.shots.is_empty() {
                continue;
            }
            let marker_rat = RationalTime::from_seconds(cue.timecode_seconds, timescale);

            let mut target_idx = 0;
            for (i, shot) in self.shots.iter().enumerate() {
                let shot_end = shot.offset + shot.duration;
                if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                    target_idx = i;
                    break;
                }
                if i == self.shots.len() - 1 {
                    target_idx = i;
                }
            }

            let local_start = marker_rat - self.shots[target_idx].offset;
            self.shots[target_idx].markers.push(TimelineMarker::new(
                local_start,
                frame_duration,
                "[PUNCH-IN 115%]",
                &cue.edit_instruction,
                MarkerColor::Cyan,
            ));
        }
    }

    /// Injects punch-in cues from an evaluated CuttableFatReport.
    pub fn with_cuttable_fat(mut self, fat: Option<&crate::math::marginal_gains::CuttableFatReport>) -> Self {
        if let Some(f) = fat {
            self.inject_punch_in_cues(&f.punch_in_cues);
        }
        self
    }

    /// Injects crowd-sourced audience comment markers:
    /// - Red markers for Confusion / Friction / Pacing drag clusters
    /// - Green markers for Praise / Humor / Replay clusters
    /// - Purple markers for Story Engagement
    /// - Cyan markers for Neutral Sequence references
    pub fn inject_comment_clusters(&mut self, clusters: &[crate::analyzer::comments::CommentTimestampCluster]) {
        let timescale = self.frame_rate_numerator;
        let frame_duration = RationalTime::new(self.frame_rate_denominator, timescale);

        for cluster in clusters {
            if self.shots.is_empty() {
                continue;
            }
            let marker_rat = RationalTime::from_seconds(cluster.center_second, timescale);

            let mut target_idx = 0;
            for (i, shot) in self.shots.iter().enumerate() {
                let shot_end = shot.offset + shot.duration;
                if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                    target_idx = i;
                    break;
                }
                if i == self.shots.len() - 1 {
                    target_idx = i;
                }
            }

            let local_start = marker_rat - self.shots[target_idx].offset;
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

            self.shots[target_idx].markers.push(TimelineMarker::new(
                local_start,
                frame_duration,
                format!("{} {}", prefix, cluster.timecode_formatted),
                &note,
                marker_color,
            ));
        }
    }

    /// Builder helper to inject comment clusters into timeline.
    pub fn with_comment_clusters(mut self, clusters: &[crate::analyzer::comments::CommentTimestampCluster]) -> Self {
        self.inject_comment_clusters(clusters);
        self
    }

    /// Injects an Orange timeline marker for an abrupt ending violation.
    pub fn inject_outro_report(&mut self, outro_report: Option<&crate::math::retention::OutroRunwayReport>) {
        if let Some(outro) = outro_report {
            if !outro.is_abrupt_ending {
                if let Some(ts_sec) = outro.signoff_cue_timestamp_seconds {
                    if self.shots.is_empty() {
                        return;
                    }
                    let timescale = self.frame_rate_numerator;
                    let frame_duration = RationalTime::new(self.frame_rate_denominator, timescale);
                    let marker_rat = RationalTime::from_seconds(ts_sec, timescale);

                    let mut target_idx = 0;
                    for (i, shot) in self.shots.iter().enumerate() {
                        let shot_end = shot.offset + shot.duration;
                        if marker_rat.value >= shot.offset.value && marker_rat.value < shot_end.value {
                            target_idx = i;
                            break;
                        }
                        if i == self.shots.len() - 1 {
                            target_idx = i;
                        }
                    }

                    let local_start = marker_rat - self.shots[target_idx].offset;
                    let note = format!(
                        "Cut video here. Eliminates {:.0}s of verbal wind-down and halts premature audience exit.",
                        outro.outro_runway_seconds
                    );
                    self.shots[target_idx].markers.push(TimelineMarker::new(
                        local_start,
                        frame_duration,
                        "[BLADE TIMELINE HERE: ABRUPT ENDING]".to_string(),
                        &note,
                        MarkerColor::Orange,
                    ));
                }
            }
        }
    }

    /// Injects outro abrupt ending marker from an evaluated OutroRunwayReport.
    pub fn with_outro_report(mut self, outro: Option<&crate::math::retention::OutroRunwayReport>) -> Self {
        self.inject_outro_report(outro);
        self
    }

    /// Serializes the timeline into a 100% compliant Apple FCPXML v1.8 document.
    /// If slug_path was configured on the timeline, uses the embedded slug asset;
    /// otherwise uses native `<gap>` elements on the primary `<spine>`.
    pub fn to_fcpxml_string(&self) -> String {
        self.to_fcpxml_string_with_slug(self.slug_path.as_deref())
    }

    /// Serializes the timeline into Apple FCPXML v1.8 with optional embedded black slug asset.
    /// When `slug_path` is provided, an `<asset>` is defined in `<resources>` and `<video ref="r2">`
    /// wraps the shots on `<spine>`, allowing single-click native marker import in DaVinci Resolve.
    pub fn to_fcpxml_string_with_slug(&self, slug_path: Option<&Path>) -> String {
        let mut out = String::with_capacity(16 * 1024);

        let format_duration_str = format!("{}/{}s", self.frame_rate_denominator, self.frame_rate_numerator);
        let total_duration_str = self.duration.to_fcpxml_str();
        let safe_title = xml_escape(&self.title);

        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<!DOCTYPE fcpxml>\n");
        out.push_str("<fcpxml version=\"1.8\">\n");
        out.push_str("    <resources>\n");
        let _ = writeln!(
            out,
            "        <format id=\"r1\" name=\"SandsifterForensicFormat\" frameDuration=\"{}\" width=\"{}\" height=\"{}\"/>",
            format_duration_str, self.width, self.height
        );
        if let Some(slug) = slug_path {
            let file_url = xml_escape(&path_to_file_url(slug));
            let _ = writeln!(
                out,
                "        <asset id=\"r2\" name=\"Sandsifter_Guide_Track\" src=\"{}\" duration=\"{}\" hasVideo=\"1\" format=\"r1\"/>",
                file_url, total_duration_str
            );
        }
        out.push_str("    </resources>\n");
        out.push_str("    <library>\n");
        out.push_str("        <event name=\"Sandsifter Intelligence\">\n");
        let _ = writeln!(out, "            <project name=\"{}\">", safe_title);
        let _ = writeln!(
            out,
            "                <sequence format=\"r1\" duration=\"{}\" tcStart=\"0s\">",
            total_duration_str
        );
        out.push_str("                    <spine>\n");

        for shot in &self.shots {
            let offset_str = shot.offset.to_fcpxml_str();
            let duration_str = shot.duration.to_fcpxml_str();
            let safe_label = xml_escape(&shot.label);

            let tag = if slug_path.is_some() { "video ref=\"r2\"" } else { "gap" };
            let closing_tag = if slug_path.is_some() { "video" } else { "gap" };

            if shot.markers.is_empty() {
                let _ = writeln!(
                    out,
                    "                        <{} name=\"{}\" offset=\"{}\" duration=\"{}\" start=\"0s\"/>",
                    tag, safe_label, offset_str, duration_str
                );
            } else {
                let _ = writeln!(
                    out,
                    "                        <{} name=\"{}\" offset=\"{}\" duration=\"{}\" start=\"0s\">",
                    tag, safe_label, offset_str, duration_str
                );
                for marker in &shot.markers {
                    let m_start_str = marker.start.to_fcpxml_str();
                    let m_dur_str = marker.duration.to_fcpxml_str();
                    let m_name = xml_escape(&marker.name);
                    let m_note = xml_escape(&marker.note);

                    let _ = writeln!(
                        out,
                        "                            <marker start=\"{}\" duration=\"{}\" value=\"{}\" note=\"{}\" completed=\"0\"/>",
                        m_start_str, m_dur_str, m_name, m_note
                    );
                }
                let _ = writeln!(out, "                        </{}>", closing_tag);
            }
        }

        out.push_str("                    </spine>\n");
        out.push_str("                </sequence>\n");
        out.push_str("            </project>\n");
        out.push_str("        </event>\n");
        out.push_str("    </library>\n");
        out.push_str("</fcpxml>\n");

        out
    }

    /// Saves the FCPXML to disk.
    pub fn save_fcpxml<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let content = self.to_fcpxml_string();
        fs::write(path, content)
    }

    /// Saves the FCPXML to disk with the specified slug asset path.
    pub fn save_fcpxml_with_slug<P: AsRef<Path>>(&self, path: P, slug_path: Option<&Path>) -> std::io::Result<()> {
        let content = self.to_fcpxml_string_with_slug(slug_path);
        fs::write(path, content)
    }

    /// Serializes the timeline into a standard CMX 3600 Edit Decision List (EDL)
    /// compatible with DaVinci Resolve timeline marker import (`Timelines -> Import -> Timeline Markers from EDL...`).
    pub fn to_edl_string(&self) -> String {
        let mut out = String::with_capacity(8 * 1024);
        let fps = (self.frame_rate_numerator as f64) / (self.frame_rate_denominator as f64);

        let _ = writeln!(out, "TITLE: {}", self.title.to_uppercase());
        out.push_str("FCM: NON-DROP FRAME\n\n");

        let mut event_idx = 1;

        // 1. Output Shot Cuts (for standard NLE timeline track import)
        for shot in &self.shots {
            let src_in = "00:00:00:00";
            let src_out = shot.duration.to_timecode_str(fps);
            let rec_in = shot.offset.to_timecode_str(fps);
            let rec_out = (shot.offset + shot.duration).to_timecode_str(fps);

            let _ = writeln!(
                out,
                "{:03}  AX       V     C        {} {} {} {}",
                shot.shot_index, src_in, src_out, rec_in, rec_out
            );
            let _ = writeln!(out, "* FROM CLIP: {}", shot.label);

            for marker in &shot.markers {
                let marker_abs = shot.offset + marker.start;
                let _ = writeln!(
                    out,
                    "* MARKER: {} @ {} | {}",
                    marker.name,
                    marker_abs.to_timecode_str(fps),
                    marker.note
                );
            }
            out.push('\n');
            if shot.shot_index >= event_idx {
                event_idx = shot.shot_index + 1;
            }
        }

        // 2. Output Dedicated DaVinci Resolve Marker Events
        // DaVinci Resolve's "Import Timeline Markers from EDL" assigns timeline markers based on
        // the Record In (`rec_in`) of events containing pipe-delimited `|C:ResolveColor...` comments.
        let frame_duration = RationalTime::new(self.frame_rate_denominator, self.frame_rate_numerator);
        for shot in &self.shots {
            for marker in &shot.markers {
                let marker_abs = shot.offset + marker.start;
                let dur = if marker.duration.value > 0 {
                    marker.duration
                } else {
                    frame_duration
                };
                let rec_in = marker_abs.to_timecode_str(fps);
                let rec_out = (marker_abs + dur).to_timecode_str(fps);
                let dur_frames = ((dur.to_seconds() * fps).round() as u32).max(1);
                let color_str = marker.color.resolve_color_name();
                let clean_text = if marker.note.is_empty() {
                    marker.name.clone()
                } else {
                    format!("{} - {}", marker.name, marker.note)
                };

                let _ = writeln!(
                    out,
                    "{:03}  001      V     C        {} {} {} {}",
                    event_idx, rec_in, rec_out, rec_in, rec_out
                );
                let _ = writeln!(out, " |C:{} |M:{} |D:{}", color_str, clean_text, dur_frames);
                let _ = writeln!(
                    out,
                    "* MARKER: {} @ {} | {}",
                    marker.name,
                    marker_abs.to_timecode_str(fps),
                    marker.note
                );
                out.push('\n');
                event_idx += 1;
            }
        }

        out
    }

    /// Saves the EDL to disk.
    pub fn save_edl<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let content = self.to_edl_string();
        fs::write(path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rational_time_framerate_math() {
        // 29.97 fps (timescale 30000, 1001/30000s)
        let rt_2997 = RationalTime::new(1001, 30000);
        assert_eq!(rt_2997.to_fcpxml_str(), "1001/30000s");
        let seconds = rt_2997.to_seconds();
        assert!((seconds - (1.0 / 29.97002997)).abs() < 0.0001);

        // 24.0 fps
        let rt_24 = RationalTime::new(1, 24);
        assert_eq!(rt_24.to_fcpxml_str(), "1/24s");
        assert_eq!(rt_24.to_timecode_str(24.0), "00:00:00:01");

        // Whole seconds reduction
        let rt_whole = RationalTime::new(90000, 30000);
        assert_eq!(rt_whole.to_fcpxml_str(), "3s");

        // Zero seconds
        let rt_zero = RationalTime::zero();
        assert_eq!(rt_zero.to_fcpxml_str(), "0s");

        // Arithmetic addition
        let a = RationalTime::from_seconds(1.5, 30000);
        let b = RationalTime::from_seconds(2.5, 30000);
        let c = a + b;
        assert_eq!(c.to_fcpxml_str(), "4s");
    }

    #[test]
    fn test_contiguous_gap_spines() {
        let mut timeline = ExportTimeline::new_vertical_short("Pacing Template", 10.0);
        timeline.shots.push(ShotClipPlaceholder {
            shot_index: 1,
            offset: RationalTime::from_seconds(0.0, 30000),
            duration: RationalTime::from_seconds(3.0, 30000),
            label: "Shot 1".to_string(),
            markers: vec![],
        });
        timeline.shots.push(ShotClipPlaceholder {
            shot_index: 2,
            offset: RationalTime::from_seconds(3.0, 30000),
            duration: RationalTime::from_seconds(7.0, 30000),
            label: "Shot 2".to_string(),
            markers: vec![],
        });

        let xml = timeline.to_fcpxml_string();
        assert!(xml.contains("<gap name=\"Shot 1\" offset=\"0s\" duration=\"3s\" start=\"0s\"/>"));
        assert!(xml.contains("<gap name=\"Shot 2\" offset=\"3s\" duration=\"7s\" start=\"0s\"/>"));
    }

    #[test]
    fn test_xml_escaping() {
        assert_eq!(xml_escape("Tom & Jerry <Danger>"), "Tom &amp; Jerry &lt;Danger&gt;");
        assert_eq!(xml_escape("\"Quote's\""), "&quot;Quote&apos;s&quot;");
    }

    #[test]
    fn test_edl_timecode_formatting() {
        let mut timeline = ExportTimeline::new_vertical_short("Sample EDL", 5.0);
        timeline.shots.push(ShotClipPlaceholder {
            shot_index: 1,
            offset: RationalTime::zero(),
            duration: RationalTime::from_seconds(5.0, 30000),
            label: "Intro Shot".to_string(),
            markers: vec![TimelineMarker::new(
                RationalTime::from_seconds(1.5, 30000),
                RationalTime::new(1001, 30000),
                "Speech Onset",
                "Hook spoken",
                MarkerColor::Cyan,
            )],
        });

        let edl = timeline.to_edl_string();
        assert!(edl.contains("001  AX       V     C        00:00:00:00 00:00:05:00 00:00:00:00 00:00:05:00"));
        assert!(edl.contains("* FROM CLIP: Intro Shot"));
        assert!(edl.contains("* MARKER: Speech Onset @ 00:00:01:14 | Hook spoken"));
        assert!(edl.contains("|C:ResolveColorCyan |M:Speech Onset - Hook spoken |D:1"));
    }

    #[test]
    fn test_slug_asset_and_video_spine_export() {
        let mut timeline = ExportTimeline::new_vertical_short("Pacing Template with Slug", 10.0);
        timeline.shots.push(ShotClipPlaceholder {
            shot_index: 1,
            offset: RationalTime::from_seconds(0.0, 30000),
            duration: RationalTime::from_seconds(3.0, 30000),
            label: "Shot 1".to_string(),
            markers: vec![TimelineMarker::new(
                RationalTime::from_seconds(1.0, 30000),
                RationalTime::new(1001, 30000),
                "Test Marker",
                "Marker Note",
                MarkerColor::Cyan,
            )],
        });

        let slug = Path::new("output/.sandsifter_slug.png");
        let xml = timeline.to_fcpxml_string_with_slug(Some(slug));
        assert!(xml.contains("<asset id=\"r2\" name=\"Sandsifter_Guide_Track\""));
        assert!(xml.contains("<video ref=\"r2\" name=\"Shot 1\" offset=\"0s\" duration=\"3s\" start=\"0s\">"));
        assert!(xml.contains("</video>"));
        assert!(xml.contains("<marker start=\"1s\""));
    }
}
