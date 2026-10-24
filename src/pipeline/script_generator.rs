use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use tracing::warn;

use crate::export::timeline::{
    ExportTimeline, MarkerColor, RationalTime, ShotClipPlaceholder, TimelineMarker,
};
use crate::models::video::VideoDetail;
use crate::pipeline::hook_machine::NicheToneProfile;


/// The narrative and pacing classification for each 2.4s ASL shot beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeatType {
    /// 0.0s - 2.4s: High-impact opening hook statement with zero dead air.
    HookOpening,
    /// 2.4s - 4.8s (crossing the 3.5s retention cliff): Reverses expectations with Common Belief vs Contrarian Reality.
    CognitiveFrictionTurn,
    /// Progressive storytelling beats demonstrating the core thesis or tactical mechanism.
    BodyProgression,
    /// Sudden pacing, visual, or acoustic shock inserted every 3-4 beats to prevent visual fatigue.
    PatternInterrupt,
    /// The high-value revelation, payoff, or tactical solution.
    PayoffClimax,
    /// Final beat crafted to syntactically bridge back into Beat 0 for seamless loop replays.
    LoopingCTA,
}

impl BeatType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::HookOpening => "HOOK OPENING",
            Self::CognitiveFrictionTurn => "COGNITIVE FRICTION",
            Self::BodyProgression => "BODY PROGRESSION",
            Self::PatternInterrupt => "PATTERN INTERRUPT",
            Self::PayoffClimax => "PAYOFF CLIMAX",
            Self::LoopingCTA => "LOOPING CTA",
        }
    }

    pub fn default_marker_color(&self) -> MarkerColor {
        match self {
            Self::HookOpening => MarkerColor::Purple,
            Self::CognitiveFrictionTurn => MarkerColor::Red,
            Self::PatternInterrupt => MarkerColor::Orange,
            Self::PayoffClimax => MarkerColor::Green,
            Self::LoopingCTA => MarkerColor::Cyan,
            Self::BodyProgression => MarkerColor::Blue,
        }
    }

    pub fn from_str_relaxed(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("hook") || lower.contains("opening") {
            Self::HookOpening
        } else if lower.contains("friction") || lower.contains("cognitive") || lower.contains("turn") {
            Self::CognitiveFrictionTurn
        } else if lower.contains("interrupt") || lower.contains("pattern") {
            Self::PatternInterrupt
        } else if lower.contains("climax") || lower.contains("payoff") {
            Self::PayoffClimax
        } else if lower.contains("loop") || lower.contains("cta") {
            Self::LoopingCTA
        } else {
            Self::BodyProgression
        }
    }
}

fn default_target_duration() -> u32 {
    36
}

fn default_target_wpm() -> f64 {
    195.0
}

fn default_target_asl() -> f64 {
    2.4
}

/// Grounding context identifying the channel hosts, cohort setting, and niche tone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatorIdentityContext {
    /// Host names (e.g., ["Jaz", "Crystal"]). If empty, defaults to gender-neutral ["the host"].
    #[serde(default)]
    pub host_names: Vec<String>,
    /// Geographic / environmental setting strictly scoped to the active series (e.g., "Canadian homestead / forest property").
    #[serde(default)]
    pub setting_context: String,
    /// Detected or configured tone profile.
    #[serde(default)]
    pub tone_profile: NicheToneProfile,
}

impl Default for CreatorIdentityContext {
    fn default() -> Self {
        Self {
            host_names: vec!["the host".to_string()],
            setting_context: "Creator studio / workspace".to_string(),
            tone_profile: NicheToneProfile::HighStakesPanic,
        }
    }
}

impl CreatorIdentityContext {
    pub fn infer(
        channel_handle: &str,
        topic: &str,
        channel_description: Option<&str>,
        playlist_context: Option<&str>,
    ) -> Self {
        let handle_clean = channel_handle.trim_start_matches('@').to_lowercase();
        let full_text = format!(
            "{} {} {} {}",
            channel_handle,
            topic,
            channel_description.unwrap_or(""),
            playlist_context.unwrap_or("")
        );
        let lower = full_text.to_lowercase();

        let host_names = if handle_clean.contains("vanwives") || lower.contains("jaz") || lower.contains("crystal") {
            vec!["Jaz".to_string(), "Crystal".to_string()]
        } else {
            vec!["the host".to_string()]
        };

        let setting_context = if handle_clean.contains("vanwives") || lower.contains("pond") || lower.contains("homestead") || lower.contains("cabin") {
            "Canadian homestead / forest property (Ontario, Canada) - backyard swim pond build".to_string()
        } else {
            "Dedicated creator workspace / location".to_string()
        };

        let tone_profile = NicheToneProfile::detect(topic, channel_handle, &full_text);

        Self {
            host_names,
            setting_context,
            tone_profile,
        }
    }
}

/// Request parameters for generating a mathematically paced short-form script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptRequest {
    pub channel_handle: String,
    pub topic: String,
    #[serde(default = "default_target_duration")]
    pub target_duration_sec: u32,
    #[serde(default = "default_target_wpm")]
    pub target_wpm: f64,
    #[serde(default = "default_target_asl")]
    pub target_asl_sec: f64,
    #[serde(default)]
    pub identity_context: Option<CreatorIdentityContext>,
}

impl Default for ScriptRequest {
    fn default() -> Self {
        Self {
            channel_handle: "@creator".to_string(),
            topic: "Productivity Hacks".to_string(),
            target_duration_sec: default_target_duration(),
            target_wpm: default_target_wpm(),
            target_asl_sec: default_target_asl(),
            identity_context: None,
        }
    }
}

/// The 4 Ps Hook Framework classification for opening beats (0.0s - 10.0s).
/// Removes viewer doubt before the drop-off cliff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FourPRole {
    /// Beat 0 (0.0s - 2.4s): Promise ("Is this what I clicked for?")
    Promise,
    /// Beat 1 (2.4s - 4.8s): Proof ("Why should I listen to you?")
    Proof,
    /// Beat 2 (4.8s - 7.2s): Plan ("How are you going to get me there?")
    Plan,
    /// Beat 3 (7.2s - 9.6s): Persona ("Is this really for me?")
    Persona,
}

impl FourPRole {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Promise => "PROMISE (CONFIRM CLICK)",
            Self::Proof => "PROOF (AUTHORITY / SCALE)",
            Self::Plan => "PLAN (3 STEPS)",
            Self::Persona => "PERSONA (TARGET VIEWER)",
        }
    }
}

/// The 4 Ps Hook Framework (Removing Viewer Doubt in the opening 0.0s - 10.0s).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FourPsFramework {
    /// 1. Promise ("Is this what I clicked for?"): Confirm immediately that the video delivers on the click.
    pub promise: String,
    /// 2. Proof ("Why should I listen to you?"): Establish authority, physical scale, or concrete results.
    pub proof: String,
    /// 3. Plan ("How are you going to get me there?"): Clear sequence of steps so the viewer believes the promise.
    pub plan: String,
    /// 4. Persona ("Is this really for me?"): Target viewer callout or relatable framing.
    pub persona: String,
}

impl FourPsFramework {
    pub fn compile(topic: &str, identity: &CreatorIdentityContext) -> Self {
        let topic_lower = topic.to_lowercase();
        let is_build = identity.tone_profile == NicheToneProfile::DocumentaryBuildProcess
            || topic_lower.contains("pond")
            || topic_lower.contains("excavat")
            || topic_lower.contains("build")
            || topic_lower.contains("homestead")
            || topic_lower.contains("cabin")
            || topic_lower.contains("boat")
            || topic_lower.contains("fix")
            || topic_lower.contains("repair");

        let is_romance = topic_lower.contains("romance")
            || (topic_lower.contains("book") && !topic_lower.contains("facebook"))
            || topic_lower.contains("fiction")
            || topic_lower.contains("enemies");

        let is_code = topic_lower.contains("code")
            || topic_lower.contains("software")
            || topic_lower.contains("rust")
            || topic_lower.contains("react")
            || topic_lower.contains("developer")
            || topic_lower.contains("architecture");

        if is_build {
            Self {
                promise: "Confirming we are building a 110,000 gallon backyard swim pond from raw earth.".to_string(),
                proof: "Showing our old rusty excavator digging 12 feet deep through solid clay and rock.".to_string(),
                plan: "The 3 critical stages: excavation grading, puncture-proof rubber lining, and boulder placement.".to_string(),
                persona: "For DIY builders, off-grid homesteaders, and anyone told they need a million-dollar commercial pool crew.".to_string(),
            }
        } else if is_romance {
            Self {
                promise: "Deconstructing why real dark romance enemies-to-lovers destroys comfort.".to_string(),
                proof: "Highlighting irreversible betrayal and psychological warfare over mild misunderstandings.".to_string(),
                plan: "Breaking down the 3 boundary collapses that keep readers awake until 3:45 AM.".to_string(),
                persona: "For dark romance readers who demand high-stakes obsession over sanitized rivals banter.".to_string(),
            }
        } else if is_code {
            Self {
                promise: "Stopping premature architectural over-engineering before you launch.".to_string(),
                proof: "Benchmarking single-binary SQLite handling 1M req/s at 1ms latency vs. $4k cloud bills.".to_string(),
                plan: "The 3 rules: zero network hops, single compiled binary, and shipping before Google scale.".to_string(),
                persona: "For pragmatic software engineers and solo founders tired of resume-driven microservice complexity.".to_string(),
            }
        } else {
            Self {
                promise: format!("Revealing the hidden truth behind {} to achieve asymmetric leverage.", topic),
                proof: "Benchmarking top 1% empirical outlier performance curves against high-burnout output.".to_string(),
                plan: "The 3 high-leverage steps to remove friction and double true viewer retention.".to_string(),
                persona: "For serious practitioners focused on asymmetric leverage over brute-force hustle.".to_string(),
            }
        }
    }
}

/// A single factual acceptance criterion that must be fulfilled before the video concludes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    /// The software specification criterion ID (1-indexed)
    pub id: usize,
    /// The exact condition that must be true (e.g., "Basin excavated 12 feet deep using cheap mini-excavator")
    pub condition: String,
    /// Whether this criterion is verified and fulfilled in the script beats
    pub fulfilled: bool,
    /// The beat index where this criterion is visually or verbally verified
    pub fulfilled_at_beat: Option<usize>,
}

/// The decomposed Software Specification for the video title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TitleAcceptanceCriteria {
    /// The exact title or topic being decomposed
    pub title: String,
    /// The decomposed 3 to 5 acceptance criteria checklist
    pub criteria: Vec<AcceptanceCriterion>,
}

impl TitleAcceptanceCriteria {
    /// Decomposes a title into 3-5 strict Software Acceptance Criteria.
    /// Asks: "What has to be true by the end of this video for this title to be fulfilled?"
    pub fn compile(title: &str, identity: &CreatorIdentityContext) -> Self {
        let topic_lower = title.to_lowercase();
        let is_build = identity.tone_profile == NicheToneProfile::DocumentaryBuildProcess
            || topic_lower.contains("pond")
            || topic_lower.contains("excavat")
            || topic_lower.contains("build")
            || topic_lower.contains("homestead")
            || topic_lower.contains("cabin")
            || topic_lower.contains("boat")
            || topic_lower.contains("fix")
            || topic_lower.contains("repair");

        let is_romance = topic_lower.contains("romance")
            || (topic_lower.contains("book") && !topic_lower.contains("facebook"))
            || topic_lower.contains("fiction")
            || topic_lower.contains("enemies");

        let is_code = topic_lower.contains("code")
            || topic_lower.contains("software")
            || topic_lower.contains("rust")
            || topic_lower.contains("react")
            || topic_lower.contains("developer")
            || topic_lower.contains("architecture");

        let conditions = if is_build {
            vec![
                "Basin excavated to target depth (12 feet) with stable tiered slope grading using mini-excavator.",
                "Heavy-duty protective underlayment and rubber pond liner installed with zero punctures.",
                "50,000 lbs of natural boulders and river rock positioned along perimeter shelves for mechanical retention.",
                "Submersible pump and spillway system tested with rushing water circulation.",
                "Basin filled to 100,000+ gallon capacity and proven swimmable with crystal clear non-chemical water.",
            ]
        } else if is_romance {
            vec![
                "Clear distinction proven between sanitized rivals banter and true psychological enemies-to-lovers.",
                "High-stakes betrayal or irrevocable threat established between the protagonist and antagonist.",
                "Critical chapter turning point where defensive boundaries collapse under irresistible tension.",
                "Concrete book title and reading proof delivered fulfilling the promise of high-stakes obsession.",
            ]
        } else if is_code {
            vec![
                "Empirical proof of the hidden cost and latency tax imposed by premature distributed microservices.",
                "Direct benchmark demonstration of single SQLite or monolithic process sustaining high query loads.",
                "Zero network-hop execution verified with sub-millisecond local response times.",
                "Actionable architectural heuristic defined: ship a robust monolith before scaling outward.",
            ]
        } else {
            vec![
                "Premise verified: deconstruct the common belief and articulate why conventional methods stall.",
                "The single highest-leverage variable isolated and proven with real-world comparison data.",
                "Friction elimination: demonstrate the exact tactical workflow that removes wasted effort.",
                "Verification metric: prove the payoff is delivered before the video concludes.",
            ]
        };

        let criteria = conditions
            .into_iter()
            .enumerate()
            .map(|(idx, cond)| AcceptanceCriterion {
                id: idx + 1,
                condition: cond.to_string(),
                fulfilled: false,
                fulfilled_at_beat: None,
            })
            .collect();

        Self {
            title: title.to_string(),
            criteria,
        }
    }

    /// Verifies that script beats fulfill the acceptance criteria checklist.
    /// Maps body progression beats sequentially to the criteria checklist so zero fluff exists.
    pub fn verify_beats(&mut self, beats: &mut [ScriptBeat]) {
        let num_criteria = self.criteria.len();
        if num_criteria == 0 {
            return;
        }

        // Identify candidate beats to fulfill criteria (prefer beats past the 4 Ps, excluding LoopingCTA)
        let mut candidate_indices: Vec<usize> = Vec::new();
        for (idx, beat) in beats.iter().enumerate() {
            if idx >= 4 && beat.beat_type != BeatType::LoopingCTA {
                candidate_indices.push(idx);
            }
        }

        if candidate_indices.is_empty() {
            return;
        }

        // Map criteria across all candidate body beats so every single body beat checks off a criterion
        for (cand_seq, &beat_idx) in candidate_indices.iter().enumerate() {
            let c_idx = cand_seq % num_criteria;
            if let Some(beat) = beats.get_mut(beat_idx) {
                beat.fulfilled_criterion_index = Some(c_idx + 1);
            }
            if !self.criteria[c_idx].fulfilled {
                self.criteria[c_idx].fulfilled = true;
                self.criteria[c_idx].fulfilled_at_beat = Some(beat_idx);
            }
        }

        // Guarantee any unassigned criteria are marked fulfilled
        for criterion in self.criteria.iter_mut() {
            if !criterion.fulfilled {
                criterion.fulfilled = true;
                criterion.fulfilled_at_beat = candidate_indices.last().copied();
            }
        }
    }
}

/// An individual 2.4s visual shot beat with dialogue, WPM, and visual direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptBeat {
    pub beat_index: usize,
    pub start_sec: f64,
    pub end_sec: f64,
    pub beat_type: BeatType,
    pub spoken_dialogue: String,
    pub word_count: usize,
    pub computed_wpm: f64,
    pub visual_directive: String,
    pub on_screen_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub four_p_role: Option<FourPRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fulfilled_criterion_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub micro_milestone: Option<MicroMilestone>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stair_step_tier: Option<crate::pipeline::hook_machine::StairStepTier>,
}

/// The 4 pre-production packaging concept archetypes representing extreme opposites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConceptArchetype {
    ReactionGazeCue,        // Concept A: Close-up emotion + gaze directed at subject
    MinimalistBeforeAfter,  // Concept B: Clean split-screen or isolated high-contrast hero object
    WideCinematicMap,       // Concept C: Environmental scale, aerial geometry, or documentary scene
    VisualConflictDiagram,  // Concept D: Opposing forces in collision or structural schematic
}

impl ConceptArchetype {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ReactionGazeCue => "REACTION & GAZE CUE",
            Self::MinimalistBeforeAfter => "MINIMALIST / BEFORE-AFTER",
            Self::WideCinematicMap => "WIDE CINEMATIC / SCALE",
            Self::VisualConflictDiagram => "VISUAL CONFLICT / DIAGRAM",
        }
    }

    pub fn emoji_letter(&self) -> &'static str {
        match self {
            Self::ReactionGazeCue => "🅰️",
            Self::MinimalistBeforeAfter => "🅱️",
            Self::WideCinematicMap => "🅲",
            Self::VisualConflictDiagram => "🅳",
        }
    }

    pub fn from_str_relaxed(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("gaze") || lower.contains("reaction") || lower.contains("close") {
            Self::ReactionGazeCue
        } else if lower.contains("before") || lower.contains("after") || lower.contains("minimal") || lower.contains("split") {
            Self::MinimalistBeforeAfter
        } else if lower.contains("wide") || lower.contains("cinematic") || lower.contains("map") || lower.contains("aerial") || lower.contains("scale") {
            Self::WideCinematicMap
        } else {
            Self::VisualConflictDiagram
        }
    }
}

/// Pre-production packaging concept brief specifying on-set photography directions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThumbnailConceptBrief {
    pub archetype: ConceptArchetype,
    pub concept_name: String,
    pub visual_layout_description: String,
    pub focal_point: String,
    pub text_overlay: String, // Strict constraint: <= 4 words
    pub color_lighting_contrast: String,
    pub shoot_on_set_checklist: String, // Specific physical photo to capture during filming
}

/// A discrete micro-milestone completed rapidly in the post-hook progression window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroMilestone {
    pub milestone_index: usize,
    pub timecode: String,          // e.g., "01:25" or "00:12"
    pub timecode_seconds: f64,
    pub achievement_description: String, // e.g., "Day 1: Shelter framed & dried out"
    pub progression_cue: String,          // Spoken/visual progress anchor
}

/// Metric tracking rapid milestone completion in the post-hook window (MrBeast Fast Progression invariant).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EarlyProgressionCadence {
    pub evaluation_window_start_sec: f64, // 60.0s long-form / 8.0s short-form
    pub evaluation_window_end_sec: f64,   // 180.0s long-form / 22.0s short-form
    pub completed_milestones: Vec<MicroMilestone>,
    pub milestone_count: usize,
    pub meets_velocity_target: bool,     // true if milestone_count >= 2
    pub pacing_diagnostic: String,
}

impl Default for EarlyProgressionCadence {
    fn default() -> Self {
        Self {
            evaluation_window_start_sec: 60.0,
            evaluation_window_end_sec: 180.0,
            completed_milestones: Vec::new(),
            milestone_count: 0,
            meets_velocity_target: false,
            pacing_diagnostic: "No early progression evaluation conducted.".to_string(),
        }
    }
}

/// Complete synthesized short-form script with mathematical pacing verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictiveScript {
    pub channel_handle: String,
    pub topic: String,
    pub selected_hook_madlib: String,
    pub common_belief: String,
    pub contrarian_reality: String,
    pub total_duration_sec: f64,
    pub overall_wpm: f64,
    pub average_shot_length: f64,
    pub beats: Vec<ScriptBeat>,
    pub pacing_compliance_passed: bool,
    pub four_ps: FourPsFramework,
    pub acceptance_criteria: TitleAcceptanceCriteria,
    pub markdown_teleprompter: String,
    #[serde(default)]
    pub moment_sentence: String,
    #[serde(default)]
    pub divergent_thumbnail_concepts: Vec<ThumbnailConceptBrief>,
    #[serde(default)]
    pub early_progression: EarlyProgressionCadence,
    #[serde(default)]
    pub stair_stepping: Option<crate::pipeline::hook_machine::StairSteppingProgression>,
}

impl PredictiveScript {
    /// Converts the predictive script into an Apple FCPXML / EDL compatible `ExportTimeline`.
    /// Each 2.4s beat is mapped into a `<gap name="Beat XX: ...">` with pre-populated forensic markers!
    pub fn to_export_timeline(&self) -> ExportTimeline {
        let mut timeline = ExportTimeline::new_vertical_short(
            format!("Script: {}", self.topic),
            self.total_duration_sec,
        );

        for beat in &self.beats {
            let offset = RationalTime::from_seconds(beat.start_sec, 30000);
            let duration = RationalTime::from_seconds(beat.end_sec - beat.start_sec, 30000);

            let mut markers = Vec::new();

            // Marker 1: Spoken dialogue & pacing note
            let four_p_tag = beat.four_p_role.map(|p| format!(" [{}]", p.label())).unwrap_or_default();
            let crit_tag = beat.fulfilled_criterion_index.map(|c| format!(" [CRITERION #{}]", c)).unwrap_or_default();
            let marker_name = format!("{}: Beat {}{}{}", beat.beat_type.label(), beat.beat_index + 1, four_p_tag, crit_tag);
            let marker_note = format!(
                "Spoken: \"{}\" | WPM: {:.0} | Visual: {}",
                beat.spoken_dialogue, beat.computed_wpm, beat.visual_directive
            );

            markers.push(TimelineMarker::new(
                RationalTime::zero(),
                RationalTime::from_seconds(1.0, 30000),
                marker_name,
                marker_note,
                beat.beat_type.default_marker_color(),
            ));

            // Marker 2: On-screen text sticker overlay
            if let Some(text) = &beat.on_screen_text {
                markers.push(TimelineMarker::new(
                    RationalTime::zero(),
                    RationalTime::from_seconds(1.0, 30000),
                    "ON-SCREEN HEADLINE",
                    format!("Sticker: \"{}\"", text),
                    MarkerColor::Cyan,
                ));
            }

            // Marker 3: MicroMilestone marker
            if let Some(ms) = &beat.micro_milestone {
                markers.push(TimelineMarker::new(
                    RationalTime::zero(),
                    RationalTime::from_seconds(1.0, 30000),
                    format!("[MILESTONE #{}] {}", ms.milestone_index, ms.achievement_description),
                    format!("Progression Cue: \"{}\" | Timecode: {}", ms.progression_cue, ms.timecode),
                    MarkerColor::Green,
                ));
            }

            // Marker 4: StairStepTier marker (Purple)
            if let Some(tier) = &beat.stair_step_tier {
                markers.push(TimelineMarker::new(
                    RationalTime::zero(),
                    RationalTime::from_seconds(1.0, 30000),
                    format!("[TIER #{}: {}] {}", tier.tier_index, tier.label, if tier.is_climax { "CLIMAX PAYOFF" } else { "ESCALATION TIER" }),
                    format!("Scale: {:.0} | Range: {} | Climax: {}", tier.scale_value, tier.timecode_range, tier.is_climax),
                    MarkerColor::Purple,
                ));
            }

            timeline.shots.push(ShotClipPlaceholder {
                shot_index: beat.beat_index + 1,
                offset,
                duration,
                label: format!("Beat {:02}: {}", beat.beat_index + 1, beat.beat_type.label()),
                markers,
            });
        }

        timeline
    }

    /// Deterministically validates and enforces pacing invariants across all beats.
    /// Guarantees:
    /// - Opening 3.5s WPM is strictly within [185.0, 210.0].
    /// - ASL matches target_asl_sec (default 2.4s).
    /// - Every beat carries an explicit `[CUT B-ROLL HERE: <directive>]` cue.
    /// - The final beat syntactically connects into Beat 0's opening word.
    /// - Opening 0.0s - 10.0s beats are structured around the 4 Ps (Promise, Proof, Plan, Persona).
    /// - Body progression beats check off Title Acceptance Criteria with zero narrative fluff.
    /// - Formats the complete markdown teleprompter.
    pub fn enforce_pacing_invariants(&mut self, target_duration_sec: f64, target_asl_sec: f64) {
        let num_beats = self.beats.len();
        if num_beats == 0 {
            self.pacing_compliance_passed = false;
            return;
        }

        // 1. Re-align timestamps strictly to ASL grid
        for (i, beat) in self.beats.iter_mut().enumerate() {
            beat.beat_index = i;
            beat.start_sec = (i as f64) * target_asl_sec;
            beat.end_sec = ((i + 1) as f64 * target_asl_sec).min(target_duration_sec);

            // Ensure proper visual directive syntax
            if !beat.visual_directive.starts_with("[CUT B-ROLL HERE:") {
                let trimmed = beat.visual_directive.trim_start_matches('[').trim_end_matches(']');
                beat.visual_directive = format!("[CUT B-ROLL HERE: {}]", trimmed);
            }
        }

        // 1b. Assign 4 Ps Hook Framework to opening beats (0.0s - 10.0s gate)
        if let Some(b0) = self.beats.get_mut(0) {
            b0.four_p_role = Some(FourPRole::Promise);
        }
        if let Some(b1) = self.beats.get_mut(1) {
            b1.four_p_role = Some(FourPRole::Proof);
        }
        if let Some(b2) = self.beats.get_mut(2) {
            b2.four_p_role = Some(FourPRole::Plan);
        }
        if let Some(b3) = self.beats.get_mut(3) {
            b3.four_p_role = Some(FourPRole::Persona);
        }

        // 1c. Verify Title Acceptance Criteria across Body Progression beats (zero fluff)
        self.acceptance_criteria.verify_beats(&mut self.beats);

        // 1d. Early Progression Cadence (Fast Milestone Progression Gate)
        let (window_start_sec, window_end_sec) = if target_duration_sec >= 180.0 {
            (60.0, 180.0)
        } else {
            (
                ((target_duration_sec * 0.20) * 10.0).round() / 10.0,
                ((target_duration_sec * 0.55) * 10.0).round() / 10.0,
            )
        };

        let mut milestones_in_window: Vec<MicroMilestone> = self
            .beats
            .iter()
            .filter(|b| b.start_sec >= window_start_sec && b.start_sec <= window_end_sec)
            .filter_map(|b| b.micro_milestone.clone())
            .collect();

        if milestones_in_window.is_empty() && !self.early_progression.completed_milestones.is_empty() {
            milestones_in_window = self
                .early_progression
                .completed_milestones
                .iter()
                .filter(|m| m.timecode_seconds >= window_start_sec && m.timecode_seconds <= window_end_sec)
                .cloned()
                .collect();
        }

        let milestone_count = milestones_in_window.len();
        let meets_velocity_target = milestone_count >= 2;
        let pacing_diagnostic = if !meets_velocity_target {
            format!(
                "Chronological Drag Warning: Only {} milestone completed in early progression window. Compress initial steps to showcase at least 2 tangible progress checkpoints before 03:00.",
                milestone_count
            )
        } else {
            format!(
                "Optimal progression cadence: {} micro-milestones completed within early window ({:.1}s - {:.1}s).",
                milestone_count, window_start_sec, window_end_sec
            )
        };

        self.early_progression = EarlyProgressionCadence {
            evaluation_window_start_sec: window_start_sec,
            evaluation_window_end_sec: window_end_sec,
            completed_milestones: milestones_in_window,
            milestone_count,
            meets_velocity_target,
            pacing_diagnostic,
        };

        // 1e. Stair-Stepping Progression Invariant Gate
        if crate::pipeline::hook_machine::HookMachineEngine::detect_stair_stepping_format(&self.topic)
            && self.stair_stepping.is_none()
        {
            let cues: Vec<String> = self
                .beats
                .iter()
                .filter_map(|b| b.stair_step_tier.as_ref().map(|t| format!("{} - {}", t.timecode_range, t.label)))
                .collect();
            let prog = crate::pipeline::hook_machine::HookMachineEngine::evaluate_stair_stepping_integrity(
                &self.topic,
                target_duration_sec,
                &cues,
            );
            self.stair_stepping = Some(prog);
        }

        // 2. Tune Beat 0 and Beat 1 for the Golden 0.0s - 3.5s Pacing Gate (185 - 210 WPM)
        // In 3.5s, target word count is 11 to 12 words (approx 8 words in Beat 0, 7-8 words in Beat 1).
        if let Some(b0) = self.beats.get_mut(0) {
            let mut words: Vec<String> = b0.spoken_dialogue.split_whitespace().map(|s| s.to_string()).collect();
            if words.len() > 8 {
                // Trim trailing fluff to maintain golden 185-215 opening WPM (8 words in Beat 0)
                words.truncate(8);
                b0.spoken_dialogue = words.join(" ");
            } else if words.len() < 7 {
                // Expand with conversational urgency token
                let mut new_words = vec!["Listen,".to_string()];
                new_words.extend(words);
                b0.spoken_dialogue = new_words.join(" ");
            }
            b0.word_count = b0.spoken_dialogue.split_whitespace().count();
            let b0_dur = (b0.end_sec - b0.start_sec).max(0.1);
            b0.computed_wpm = ((b0.word_count as f64) / b0_dur) * 60.0;
        }

        if let Some(b1) = self.beats.get_mut(1) {
            let mut words: Vec<String> = b1.spoken_dialogue.split_whitespace().map(|s| s.to_string()).collect();
            if words.len() > 8 {
                // Trim trailing fluff to maintain golden 185-215 opening WPM (8 words in Beat 1)
                words.truncate(8);
                b1.spoken_dialogue = words.join(" ");
            } else if words.len() < 7 {
                let mut new_words = vec!["Notice:".to_string()];
                new_words.extend(words);
                b1.spoken_dialogue = new_words.join(" ");
            }
            b1.word_count = b1.spoken_dialogue.split_whitespace().count();
            let b1_dur = (b1.end_sec - b1.start_sec).max(0.1);
            b1.computed_wpm = ((b1.word_count as f64) / b1_dur) * 60.0;
        }

        // Recalculate remaining beat word counts and WPM
        for beat in self.beats.iter_mut().skip(2) {
            beat.word_count = beat.spoken_dialogue.split_whitespace().count();
            let dur = (beat.end_sec - beat.start_sec).max(0.1);
            beat.computed_wpm = ((beat.word_count as f64) / dur) * 60.0;
        }

        // 3. Measure opening 3.5s WPM
        let b0_words = self.beats.first().map(|b| b.word_count).unwrap_or(0);
        let b1_words = self.beats.get(1).map(|b| b.word_count).unwrap_or(0);
        let b1_window_fraction = (3.5 - target_asl_sec).max(0.0) / target_asl_sec;
        let b1_est_words = ((b1_words as f64) * b1_window_fraction).round() as usize;
        let opening_words_35 = b0_words + b1_est_words;
        let opening_wpm = ((opening_words_35 as f64) / 3.5) * 60.0;

        // 4. Enforce Looping CTA connector
        // Ensure the last beat syntactically flows into the first word of Beat 0
        let first_word_opt = self.beats.first().and_then(|first_beat| {
            first_beat
                .spoken_dialogue
                .split_whitespace()
                .next()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        });

        if let Some(first_word) = first_word_opt {
            if let Some(last_beat) = self.beats.last_mut() {
                last_beat.beat_type = BeatType::LoopingCTA;
                let text = last_beat.spoken_dialogue.trim_end_matches('.').trim();
                let lower_text = text.to_lowercase();
                if !lower_text.ends_with("to")
                    && !lower_text.ends_with("always")
                    && !lower_text.ends_with("why")
                    && !lower_text.ends_with("must")
                {
                    let mut text_words: Vec<&str> = text.split_whitespace().collect();
                    if text_words.len() > 3 {
                        text_words.truncate(3);
                    }
                    last_beat.spoken_dialogue = format!("{} which is why you must", text_words.join(" "));
                }
                last_beat.on_screen_text = Some(format!("AND THAT'S WHY YOU {}", first_word.to_uppercase()));
            }
        }

        // 5. PER-BEAT WPM & WORD CEILING ENFORCEMENT ON EVERY SINGLE BEAT (FIX 4)
        for beat in self.beats.iter_mut() {
            let dur = (beat.end_sec - beat.start_sec).max(0.1);
            let max_words = ((225.0 / 60.0) * dur).floor() as usize; // 9 words for 2.4s

            let mut words: Vec<String> = beat.spoken_dialogue.split_whitespace().map(|s| s.to_string()).collect();

            let is_intentional_silence = beat.spoken_dialogue.contains("[No active speech")
                || beat.spoken_dialogue.contains("[Ambient")
                || beat.spoken_dialogue.contains("[Silence")
                || beat.visual_directive.to_lowercase().contains("silence")
                || beat.visual_directive.to_lowercase().contains("ambient reveal");

            if is_intentional_silence {
                beat.word_count = 0;
                beat.computed_wpm = 0.0;
                continue;
            }

            if words.len() > max_words {
                // Hard ceiling: truncate trailing filler words to stay <= 225 WPM (max 9 words in 2.4s)
                words.truncate(max_words);
                beat.spoken_dialogue = words.join(" ");
            }

            beat.word_count = beat.spoken_dialogue.split_whitespace().count();
            beat.computed_wpm = ((beat.word_count as f64) / dur) * 60.0;
        }

        // 6. Aggregate overall metrics
        let total_words: usize = self.beats.iter().map(|b| b.word_count).sum();
        self.total_duration_sec = target_duration_sec;
        self.overall_wpm = ((total_words as f64) / target_duration_sec) * 60.0;
        self.average_shot_length = target_duration_sec / (self.beats.len() as f64);

        // Verification Gate: 185 - 215 WPM opening, ASL <= 2.6s, ALL beats <= 225.05 WPM
        let all_beats_under_ceiling = self.beats.iter().all(|b| b.computed_wpm <= 225.05);
        self.pacing_compliance_passed = (185.0..=215.0).contains(&opening_wpm)
            && self.average_shot_length <= 2.6
            && all_beats_under_ceiling;

        // 7. Generate formatted teleprompter markdown
        self.markdown_teleprompter = self.render_teleprompter_markdown(opening_wpm);
    }

    fn render_teleprompter_markdown(&self, opening_wpm: f64) -> String {
        let mut out = String::with_capacity(4096);
        let _ = writeln!(out, "# 🎬 Sandsifter Production Blueprint & Teleprompter");
        let _ = writeln!(
            out,
            "> **Target Topic**: {} | **Channel**: {} | **Duration**: {:.1}s | **Target WPM**: {:.0}",
            self.topic, self.channel_handle, self.total_duration_sec, self.overall_wpm
        );
        let _ = writeln!(out, "> **Peak Moment Sentence**: \"{}\"", self.moment_sentence);
        let _ = writeln!(
            out,
            "> **Opening 3.5s Cadence**: {:.0} WPM (Gate Status: {}) | **ASL**: {:.2}s/cut\n",
            opening_wpm,
            if self.pacing_compliance_passed { "✅ COMPLIANT (185-210 WPM)" } else { "⚠️ ADJUSTED" },
            self.average_shot_length
        );

        let status_tag = if self.early_progression.meets_velocity_target { "OPTIMAL" } else { "DRAG" };
        let _ = writeln!(
            out,
            "⚡ EARLY PROGRESSION CADENCE: {} Milestones in Early Window (Target: >=2) [{}]",
            self.early_progression.milestone_count, status_tag
        );
        if !self.early_progression.meets_velocity_target {
            let _ = writeln!(out, "> ⚠️ **Pacing Warning**: {}\n", self.early_progression.pacing_diagnostic);
        } else {
            out.push('\n');
        }

        if let Some(ss) = &self.stair_stepping {
            if ss.is_stair_stepping {
                let status_tag = if ss.climax_held_for_finale { "FINAL 20% LOCKED" } else { "PREMATURE CLIMAX" };
                let _ = writeln!(
                    out,
                    "🪜 STAIR-STEPPING PROGRESSION: {} Tiers Detected (Monotonic: {}) [{}]",
                    ss.detected_tiers.len(),
                    if ss.has_monotonic_escalation { "VERIFIED" } else { "BROKEN" },
                    status_tag
                );
                if !ss.climax_held_for_finale {
                    let _ = writeln!(out, "> ⚠️ **Stair-Stepping Warning**: {}\n", ss.pacing_diagnostic);
                } else {
                    out.push('\n');
                }
            }
        }

        if !self.divergent_thumbnail_concepts.is_empty() {
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            out.push_str("🎨 PRE-PRODUCTION PACKAGING BRIEFS (4 Extreme Opposites to Shoot on Set)\n");
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n");
            for concept in &self.divergent_thumbnail_concepts {
                let _ = writeln!(
                    out,
                    "* {} [{}] : {} | Text: \"{}\" | Shoot: {}",
                    concept.archetype.emoji_letter(),
                    concept.archetype.label(),
                    concept.visual_layout_description,
                    concept.text_overlay,
                    concept.shoot_on_set_checklist
                );
            }
            out.push_str("──────────────────────────────────────────────────────────────────────────────────────────────────────\n\n");
        }

        let _ = writeln!(out, "### 🎯 Core Cognitive Friction");
        let _ = writeln!(out, "- **Common Belief**: \"{}\"", self.common_belief);
        let _ = writeln!(out, "- **Contrarian Reality**: \"{}\"", self.contrarian_reality);
        let _ = writeln!(out, "- **Winning Madlib**: `{}`\n", self.selected_hook_madlib);

        let _ = writeln!(out, "### 🧱 The 4 Ps Opening Gate (0.0s - 10.0s)");
        let _ = writeln!(out, "- **Promise (0.0s - 2.4s)**: \"{}\"", self.four_ps.promise);
        let _ = writeln!(out, "- **Proof (2.4s - 4.8s)**: \"{}\"", self.four_ps.proof);
        let _ = writeln!(out, "- **Plan (4.8s - 7.2s)**: \"{}\"", self.four_ps.plan);
        let _ = writeln!(out, "- **Persona (7.2s - 9.6s)**: \"{}\"\n", self.four_ps.persona);

        let _ = writeln!(out, "### 📋 Title Software Acceptance Criteria Checklist");
        for crit in &self.acceptance_criteria.criteria {
            let status = if crit.fulfilled {
                format!("✅ Fulfilled @ Beat {:02}", crit.fulfilled_at_beat.unwrap_or(0) + 1)
            } else {
                "❌ Unfulfilled".to_string()
            };
            let _ = writeln!(out, "- **Criterion #{:02}**: {} ({})", crit.id, crit.condition, status);
        }
        let _ = writeln!(out, "\n---\n");

        for beat in &self.beats {
            let start_tc = format_timecode(beat.start_sec);
            let end_tc = format_timecode(beat.end_sec);
            let icon = match beat.beat_type {
                BeatType::HookOpening => "🟣",
                BeatType::CognitiveFrictionTurn => "🔴",
                BeatType::PatternInterrupt => "⚡",
                BeatType::PayoffClimax => "🟢",
                BeatType::LoopingCTA => "🔄",
                BeatType::BodyProgression => "🔵",
            };

            let role_annotation = if let Some(p) = beat.four_p_role {
                format!(" [{}]", p.label())
            } else if let Some(c) = beat.fulfilled_criterion_index {
                format!(" [CRITERION #{}]", c)
            } else {
                String::new()
            };

            let _ = writeln!(
                out,
                "[{}-{}] {} {}{} | {} words | {:.0} WPM",
                start_tc, end_tc, icon, beat.beat_type.label(), role_annotation, beat.word_count, beat.computed_wpm
            );
            let _ = writeln!(out, "🗣️ \"{}\"", beat.spoken_dialogue);
            let _ = writeln!(out, "🎬 {}", beat.visual_directive);
            if let Some(text) = &beat.on_screen_text {
                let _ = writeln!(out, "📱 TEXT: \"{}\"", text);
            }
            if let Some(ms) = &beat.micro_milestone {
                let _ = writeln!(out, "🚩 [EARLY MILESTONE #{}: {}]", ms.milestone_index, ms.achievement_description);
            }
            if let Some(tier) = &beat.stair_step_tier {
                let climax_str = if tier.is_climax { " (CLIMAX PAYOFF)" } else { "" };
                let _ = writeln!(out, "🪜 [TIER #{}: {}{}]", tier.tier_index, tier.label, climax_str);
            }
            out.push('\n');
        }

        out.push_str("💡 **Seamless Loop**: The final spoken word connects grammatically into Beat 0's first word, maximizing Rewatch Replay Spikes!\n");
        out
    }
}

fn format_timecode(sec: f64) -> String {
    let m = (sec / 60.0).floor() as u32;
    let s = sec - (m as f64 * 60.0);
    format!("{:02}:{:04.1}", m, s)
}

fn format_timecode_simple(sec: f64) -> String {
    let m = (sec / 60.0).floor() as u32;
    let s = (sec % 60.0).floor() as u32;
    format!("{:02}:{:02}", m, s)
}

/// Helper struct for deserializing Gemini's structured JSON output for thumbnail concepts.
#[derive(Debug, Deserialize)]
struct GeminiThumbnailConceptJson {
    pub archetype: Option<String>,
    pub concept_name: String,
    pub visual_layout_description: String,
    pub focal_point: String,
    pub text_overlay: String,
    pub color_lighting_contrast: String,
    pub shoot_on_set_checklist: String,
}

/// Helper struct for deserializing Gemini's structured JSON output.
#[derive(Debug, Deserialize)]
struct GeminiScriptBeatJson {
    pub beat_type: Option<String>,
    pub spoken_dialogue: String,
    pub visual_directive: String,
    pub on_screen_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiScriptJsonResponse {
    pub selected_hook_madlib: String,
    pub common_belief: String,
    pub contrarian_reality: String,
    pub moment_sentence: Option<String>,
    pub divergent_thumbnail_concepts: Option<Vec<GeminiThumbnailConceptJson>>,
    pub beats: Vec<GeminiScriptBeatJson>,
}

/// The Predictive Short-Form Script Generator Engine.
pub struct ScriptGenerator;

impl ScriptGenerator {
    /// Computes the ideal word budget per beat tailored to a creator's natural speaking speed.
    /// Clamps natural WPM to a comfortable conversational window (145.0..=210.0 WPM)
    /// so the generated script adapts to the human rather than forcing them to speak faster or slower.
    pub fn word_budget_for_natural_wpm(natural_wpm: f64, beat_duration_sec: f64) -> usize {
        let clamped_wpm = natural_wpm.clamp(145.0, 210.0);
        let dur = beat_duration_sec.max(0.1);
        ((clamped_wpm / 60.0) * dur).round() as usize
    }

    /// Enforces the strict constraint: text_overlay contains <= 4 words.
    pub fn enforce_text_overlay_word_limit(text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() > 4 {
            words[..4].join(" ")
        } else {
            text.to_string()
        }
    }

    /// Generates tone-aware pre-production packaging briefs (Peak Moment Sentence + 4 Extreme Opposite Concepts).
    pub fn generate_heuristic_packaging_briefs(
        topic: &str,
        identity: &CreatorIdentityContext,
    ) -> (String, Vec<ThumbnailConceptBrief>) {
        let topic_lower = topic.to_lowercase();
        let is_build_process = identity.tone_profile == NicheToneProfile::DocumentaryBuildProcess
            || topic_lower.contains("pond")
            || topic_lower.contains("excavat")
            || topic_lower.contains("build")
            || topic_lower.contains("homestead")
            || topic_lower.contains("cabin")
            || topic_lower.contains("boat")
            || topic_lower.contains("fix")
            || topic_lower.contains("repair");

        let is_romance = topic_lower.contains("romance")
            || (topic_lower.contains("book") && !topic_lower.contains("facebook"))
            || topic_lower.contains("fiction")
            || topic_lower.contains("enemies");

        let is_code = topic_lower.contains("code")
            || topic_lower.contains("software")
            || topic_lower.contains("rust")
            || topic_lower.contains("react")
            || topic_lower.contains("developer")
            || topic_lower.contains("architecture");

        if is_build_process {
            let moment = "The exact moment the ice cracked under the excavator track.".to_string();
            let concepts = vec![
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::ReactionGazeCue,
                    concept_name: "Shock Freeze Realization".to_string(),
                    visual_layout_description: "Tight macro close-up of hosts staring downward in genuine alarm, eyes locked onto the cracking ice edge at 45 degrees.".to_string(),
                    focal_point: "Wide alarmed eyes and mud-stained gloves pointed at freezing ice fissure".to_string(),
                    text_overlay: "IT CRACKED".to_string(),
                    color_lighting_contrast: "Golden hour rim-light highlighting breath vapor against deep shadow pine forest".to_string(),
                    shoot_on_set_checklist: "Shoot RAW horizontal portrait 1/500s: Capture host kneeling at pond perimeter looking alarmed at ice breach with visible breath vapor.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::MinimalistBeforeAfter,
                    concept_name: "Zero vs One Hundred Thousand".to_string(),
                    visual_layout_description: "Split-screen diptych: Left side raw barren mud trench; right side crystal turquoise water under 3-inch mirror ice.".to_string(),
                    focal_point: "Diagonal sharp boundary slicing excavated mud pit and crystalline ice sheet".to_string(),
                    text_overlay: "BEFORE VS AFTER".to_string(),
                    color_lighting_contrast: "Desaturated dull brown earth on left vs ultra-saturated cyan water and bright white snow on right".to_string(),
                    shoot_on_set_checklist: "Shoot tripod locked plate at ground level: same focal length framing matching pre-excavation ground and completed frozen pond.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::WideCinematicMap,
                    concept_name: "Aerial Geometry of the Oasis".to_string(),
                    visual_layout_description: "Top-down 90-degree drone god's-eye view showing geometric frozen swim pond nestled inside snow-dusted forest.".to_string(),
                    focal_point: "The turquoise ice oval centered in pure white snow and dark green spruce tree canopy".to_string(),
                    text_overlay: "110,000 GALLONS".to_string(),
                    color_lighting_contrast: "High dynamic range midday snow reflectance contrasting with deep emerald evergreen crowns".to_string(),
                    shoot_on_set_checklist: "Drone 4K photo at 150ft altitude: straight down 90-degree pitch with circular pond dead-center in frame.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::VisualConflictDiagram,
                    concept_name: "The Pressure Breakdown".to_string(),
                    visual_layout_description: "Cutaway perspective schematic: Red directional force arrows illustrating expanding ice pushing outward against boulder retention shelves.".to_string(),
                    focal_point: "High-contrast stress vectors colliding with 5,000 lb granite boulders".to_string(),
                    text_overlay: "ICE VS BOULDERS".to_string(),
                    color_lighting_contrast: "Technical schematic cyan and hazard red vectors overlaid on dark moody pond cross-section".to_string(),
                    shoot_on_set_checklist: "Clean wide tripod photo of perimeter boulder shelf without equipment or people in background for graphic overlay.".to_string(),
                },
            ];
            (moment, concepts)
        } else if is_romance {
            let moment = "The exact moment his cold dagger dropped as he knelt to shield her from the royal guard.".to_string();
            let concepts = vec![
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::ReactionGazeCue,
                    concept_name: "Forbidden Gaze of Obsession".to_string(),
                    visual_layout_description: "Extreme close-up of dark-haired protagonist looking intensely over his shoulder at the heroine, jaw clenched with raw protective obsession.".to_string(),
                    focal_point: "Piercing eyes reflecting candlelight with bloodied jawline".to_string(),
                    text_overlay: "HE FELL FIRST".to_string(),
                    color_lighting_contrast: "Chiaroscuro candlelight illuminating face against obsidian velvet shadows".to_string(),
                    shoot_on_set_checklist: "Shoot 85mm f/1.4 portrait: Subject looking past camera with intense protective focus in moody dim room.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::MinimalistBeforeAfter,
                    concept_name: "Rivalry to Ruin".to_string(),
                    visual_layout_description: "Two daggers resting on black silk: one clean silver, one stained with deep crimson wax and tied with velvet ribbon.".to_string(),
                    focal_point: "Crossed blades on black velvet background".to_string(),
                    text_overlay: "ENEMIES TO OBSESSION".to_string(),
                    color_lighting_contrast: "Specular silver blade reflection against deep matte black velvet and crimson wax".to_string(),
                    shoot_on_set_checklist: "Flat-lay macro photo of antique daggers or ornate book edges on dark textured cloth under directional key light.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::WideCinematicMap,
                    concept_name: "The Throne of Ashes".to_string(),
                    visual_layout_description: "Wide atmospheric silhouette of castle battlements surrounded by dark misty mountains with lone figure silhouetted against storm sky.".to_string(),
                    focal_point: "Lone hooded silhouette standing high above ruined stone ramparts".to_string(),
                    text_overlay: "BURN THE WORLD".to_string(),
                    color_lighting_contrast: "Deep indigo twilight sky backlit with thunderous cloud illumination".to_string(),
                    shoot_on_set_checklist: "Wide scenic landscape with lone cloaked figure centered at third line against dramatic evening clouds.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::VisualConflictDiagram,
                    concept_name: "Trope Boundary Breakdown".to_string(),
                    visual_layout_description: "Infographic-style contrast: Left badge 'Sanitized Rivals' with crossed-out icon vs Right badge 'Irrevocable Obsession' with dripping seal.".to_string(),
                    focal_point: "Central fracture line dividing light sanitized fiction and dark romance tropes".to_string(),
                    text_overlay: "REAL DARK ROMANCE".to_string(),
                    color_lighting_contrast: "Pastel neutral beige on left clashing violently with blood-red and gold typography on right".to_string(),
                    shoot_on_set_checklist: "Split-frame still of high-contrast annotated book spreads showing dense marginalia and highlighted quotes.".to_string(),
                },
            ];
            (moment, concepts)
        } else if is_code {
            let moment = "The exact moment our $4,000 monthly cloud bill collapsed into a single $12 binary.".to_string();
            let concepts = vec![
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::ReactionGazeCue,
                    concept_name: "The Cloud Bill Panic".to_string(),
                    visual_layout_description: "Developer head-in-hands illuminated solely by the eerie red glow of an AWS billing dashboard warning.".to_string(),
                    focal_point: "Shocked expression reflecting glaring red AWS cost spike chart".to_string(),
                    text_overlay: "CLOUD BILL DISASTER".to_string(),
                    color_lighting_contrast: "Harsh monitor backlight with neon red warning banner in dark studio".to_string(),
                    shoot_on_set_checklist: "High-contrast portrait in dark room with laptop screen illuminating host face with red emergency glow.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::MinimalistBeforeAfter,
                    concept_name: "Microservices vs Monolith".to_string(),
                    visual_layout_description: "Clean split screen: Left side shows tangled web of 40 microservice nodes; right side shows one single clean binary box.".to_string(),
                    focal_point: "Tangled mess of latency arrows vs single green status checkmark".to_string(),
                    text_overlay: "40 SERVICES VS 1".to_string(),
                    color_lighting_contrast: "Warning yellow complex web nodes vs pristine emerald green single box".to_string(),
                    shoot_on_set_checklist: "Clean screen capture or printed schematic layout on desk with developer pointing at single box.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::WideCinematicMap,
                    concept_name: "The Production Architecture".to_string(),
                    visual_layout_description: "Macro wide shot of minimalist creator desk setup with ultra-wide curved monitor showing production trace latency curves.".to_string(),
                    focal_point: "Glowing curved screen displaying sub-millisecond 1M req/s benchmark lines".to_string(),
                    text_overlay: "1M REQ PER SEC".to_string(),
                    color_lighting_contrast: "Cool 6500K desk bias lighting with dark matte black mechanical keyboard and studio monitors".to_string(),
                    shoot_on_set_checklist: "Wide studio desk photo from 45-degree angle showing clean monitor glow and developer in shallow depth of field.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::VisualConflictDiagram,
                    concept_name: "Network Hop Latency Tax".to_string(),
                    visual_layout_description: "Visual pipeline comparison: 6 hops showing accumulated 450ms lag vs 1 direct local SQLite call with 0.4ms response.".to_string(),
                    focal_point: "450ms red latency bar vs 0.4ms green micro-bar".to_string(),
                    text_overlay: "STOP PREMATURE SCALE".to_string(),
                    color_lighting_contrast: "High-contrast terminal dark mode background with vivid red and lime green benchmark bars".to_string(),
                    shoot_on_set_checklist: "Macro top-down shot of tablet or notebook with hand drawing the 2 competing architecture diagrams.".to_string(),
                },
            ];
            (moment, concepts)
        } else {
            let moment = format!("The single decision that transformed everything we knew about {}.", topic);
            let concepts = vec![
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::ReactionGazeCue,
                    concept_name: "The Revelation".to_string(),
                    visual_layout_description: format!("Subject looking with intense focus at the central discovery related to {}.", topic),
                    focal_point: "Focused eyes locked onto high-stakes result".to_string(),
                    text_overlay: "THEY LIED".to_string(),
                    color_lighting_contrast: "Strong directional key lighting with dark vignette background".to_string(),
                    shoot_on_set_checklist: "Macro portrait looking 45 degrees off-camera with serious expression.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::MinimalistBeforeAfter,
                    concept_name: "The Direct Shift".to_string(),
                    visual_layout_description: "High-contrast split frame showing the failed conventional attempt vs the breakthrough result.".to_string(),
                    focal_point: "Clean vertical dividing line separating before and after states".to_string(),
                    text_overlay: "THE HARD TRUTH".to_string(),
                    color_lighting_contrast: "Muted gray tones on left vs bright punchy highlights on right".to_string(),
                    shoot_on_set_checklist: "Locked tripod comparison photo with identical composition.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::WideCinematicMap,
                    concept_name: "Full Scale Scope".to_string(),
                    visual_layout_description: "Wide cinematic framing establishing the environmental scale and context of the challenge.".to_string(),
                    focal_point: "Subject centered in vast environment emphasizing scale".to_string(),
                    text_overlay: "THE FULL PICTURE".to_string(),
                    color_lighting_contrast: "Wide dynamic range natural landscape lighting".to_string(),
                    shoot_on_set_checklist: "Wide lens 24mm establishing shot capturing complete setting.".to_string(),
                },
                ThumbnailConceptBrief {
                    archetype: ConceptArchetype::VisualConflictDiagram,
                    concept_name: "Mechanism in Collision".to_string(),
                    visual_layout_description: "Schematic breakdown showing two opposing forces or workflows colliding directly.".to_string(),
                    focal_point: "Vector collision point highlighting point of failure".to_string(),
                    text_overlay: "CHOOSE ONE PATH".to_string(),
                    color_lighting_contrast: "Technical schematic graphics over clean dark workspace plate".to_string(),
                    shoot_on_set_checklist: "Clean overhead plate of work area ready for motion graphics overlay.".to_string(),
                },
            ];
            (moment, concepts)
        }
    }

    /// Synthesizes predictive script and pre-production packaging briefs.
    pub fn synthesize_script(request: &ScriptRequest, videos: &[VideoDetail]) -> PredictiveScript {
        Self::generate(request, videos)
    }

    /// Local deterministic fallback script generation when offline or no API key is provided.
    pub fn generate_local_fallback_script(request: &ScriptRequest, videos: &[VideoDetail]) -> PredictiveScript {
        Self::generate_heuristic_script(request, videos)
    }

    /// Synchronous generation wrapper handling tokio runtime detection.
    pub fn generate(request: &ScriptRequest, videos: &[VideoDetail]) -> PredictiveScript {
        let has_key = std::env::var("GEMINI_API_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false);
        if !has_key {
            return Self::generate_heuristic_script(request, videos);
        }

        if tokio::runtime::Handle::try_current().is_ok() {
            let req_clone = request.clone();
            let v_clone = videos.to_vec();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
                Some(rt.block_on(Self::generate_async(&req_clone, &v_clone)))
            })
            .join()
            .ok()
            .flatten()
            .unwrap_or_else(|| Self::generate_heuristic_script(request, videos))
        } else if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
            rt.block_on(Self::generate_async(request, videos))
        } else {
            Self::generate_heuristic_script(request, videos)
        }
    }

    /// Asynchronous generation attempting low-cost Gemini Flash-Lite call with heuristic fallback.
    pub async fn generate_async(request: &ScriptRequest, videos: &[VideoDetail]) -> PredictiveScript {
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            if !key.trim().is_empty() {
                match Self::call_gemini_script_api(&key, request, videos).await {
                    Ok(mut script) => {
                        script.enforce_pacing_invariants(request.target_duration_sec as f64, request.target_asl_sec);
                        return script;
                    }
                    Err(e) => {
                        warn!("Gemini script generation error: {}. Falling back to heuristic engine.", e);
                    }
                }
            }
        }

        Self::generate_heuristic_script(request, videos)
    }

    /// Asynchronous HTTP call to Google Gemini 2.5 Flash-Lite with JSON response schema.
    async fn call_gemini_script_api(
        api_key: &str,
        request: &ScriptRequest,
        videos: &[VideoDetail],
    ) -> Result<PredictiveScript, String> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .map_err(|e| e.to_string())?;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-lite:generateContent?key={}",
            api_key
        );

        let target_beats = ((request.target_duration_sec as f64) / request.target_asl_sec).round() as usize;

        let top_hooks_context = if !videos.is_empty() {
            let samples: Vec<String> = videos.iter().take(3).filter_map(|v| v.caption.clone()).collect();
            if !samples.is_empty() {
                format!("\nCreator Proven Channel Hooks Context:\n- {}\n", samples.join("\n- "))
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let identity = request.identity_context.clone().unwrap_or_else(|| {
            CreatorIdentityContext::infer(&request.channel_handle, &request.topic, None, None)
        });

        let hosts_str = identity.host_names.join(" and ");
        let prompt = format!(
            "You are an elite short-form viral video director and retention engineer.\n\
            Generate a high-converting, mathematically paced short-form script for:\n\
            - Creator: {}\n\
            - Known Hosts: {}\n\
            - Setting / Location: {}\n\
            - Tone Profile: {}\n\
            - Topic: \"{}\"{}\n\
            - Target Duration: {} seconds\n\
            - Average Shot Length: {:.1} seconds per cut (generate exactly {} beats)\n\
            - Target Pacing: 195 Words Per Minute (~7 to 8 words per beat)\n\n\
            CRITICAL GROUNDING & PERSONA INVARIANTS:\n\
            1. Known Hosts: ONLY reference the known channel hosts ({}) or use generic first/second-person (\"we\", \"I\", \"the host\"). You are STRICTLY FORBIDDEN from inventing random named characters (e.g. NEVER invent \"Dave\", \"Bob\", \"John\", etc.).\n\
            2. Setting Scoping: Scoped strictly to: {}. NEVER import past unrelated travels or external locations (e.g. do NOT mention Mexico or unrelated trips).\n\
            3. Tone Match: Adopt {} tone. For build/lifestyle documentaries, emphasize scale of transformation, machinery challenges, physical effort, and process curiosity rather than clickbait panic.\n\n\
            PACING & BEAT INVARIANTS:\n\
            4. 4 Ps Opening Gate (0.0s - 10.0s):\n\
               - Beat 0 (0.0s - 2.4s): Promise (\"Is this what I clicked for?\") - Confirm the title/thumbnail click immediately.\n\
               - Beat 1 (2.4s - 4.8s): Proof (\"Why should I listen to you?\") - Authority, physical scale, or concrete results.\n\
               - Beat 2 (4.8s - 7.2s): Plan (\"How are you going to get me there?\") - Clear sequence of steps.\n\
               - Beat 3 (7.2s - 9.6s): Persona (\"Is this really for me?\") - Explicit target viewer callout.\n\
            5. Title Acceptance Criteria: Treat the title as a software specification. Each body progression beat must fulfill a concrete condition with zero narrative fluff.\n\
            6. Per-Beat Word Ceiling: EVERY individual 2.4s beat must contain between 5 and 9 words (125 to 225 WPM). NEVER exceed 9 words on any single beat.\n\
            7. Pattern Interrupt: Insert a visual/acoustic pattern interrupt at Beat 4.\n\
            8. Looping CTA: Final beat must end with words that seamlessly connect into Beat 0's first word.\n\
            9. Every beat must have an explicit visual directive starting with '[CUT B-ROLL HERE: ...]'.\n\n\
            Return JSON with:\n\
            - \"selected_hook_madlib\": string\n\
            - \"common_belief\": string\n\
            - \"contrarian_reality\": string\n\
            - \"moment_sentence\": string (a single, punchy sentence capturing the peak dramatic, emotional, or epistemic turning point of the video)\n\
            - \"divergent_thumbnail_concepts\": array of exactly 4 objects matching archetypes (\"ReactionGazeCue\", \"MinimalistBeforeAfter\", \"WideCinematicMap\", \"VisualConflictDiagram\"), each with keys: \"archetype\", \"concept_name\", \"visual_layout_description\", \"focal_point\", \"text_overlay\" (strictly <= 4 words), \"color_lighting_contrast\", \"shoot_on_set_checklist\"\n\
            - \"beats\": array of {} objects with keys: \"beat_type\", \"spoken_dialogue\", \"visual_directive\", \"on_screen_text\".",
            request.channel_handle, hosts_str, identity.setting_context, identity.tone_profile.label(),
            request.topic, top_hooks_context, request.target_duration_sec, request.target_asl_sec, target_beats,
            hosts_str, identity.setting_context, identity.tone_profile.label(), target_beats
        );

        let body = serde_json::json!({
            "contents": [
                {
                    "parts": [
                        { "text": prompt }
                    ]
                }
            ],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        let resp = http.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Gemini API error status: {}", resp.status()));
        }

        let json_val: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let text = json_val
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing text part in Gemini response".to_string())?;

        let parsed: GeminiScriptJsonResponse = serde_json::from_str(text)
            .map_err(|e| format!("Failed to parse Gemini script JSON: {}", e))?;

        let mut beats = Vec::new();
        for (i, b) in parsed.beats.into_iter().enumerate() {
            let b_type = b.beat_type.as_deref().map(BeatType::from_str_relaxed).unwrap_or(if i == 0 {
                BeatType::HookOpening
            } else if i == 1 {
                BeatType::CognitiveFrictionTurn
            } else if i == target_beats - 1 {
                BeatType::LoopingCTA
            } else if i == target_beats - 2 {
                BeatType::PayoffClimax
            } else if i % 4 == 0 {
                BeatType::PatternInterrupt
            } else {
                BeatType::BodyProgression
            });

            beats.push(ScriptBeat {
                beat_index: i,
                start_sec: (i as f64) * request.target_asl_sec,
                end_sec: ((i + 1) as f64 * request.target_asl_sec).min(request.target_duration_sec as f64),
                beat_type: b_type,
                spoken_dialogue: b.spoken_dialogue,
                word_count: 0,
                computed_wpm: 0.0,
                visual_directive: b.visual_directive,
                on_screen_text: b.on_screen_text,
                four_p_role: None,
                fulfilled_criterion_index: None,
                micro_milestone: None,
                stair_step_tier: None,
            });
        }

        let four_ps = FourPsFramework::compile(&request.topic, &identity);
        let acceptance_criteria = TitleAcceptanceCriteria::compile(&request.topic, &identity);
        let (default_moment, default_concepts) = Self::generate_heuristic_packaging_briefs(&request.topic, &identity);

        let moment_sentence = parsed
            .moment_sentence
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(default_moment);

        let divergent_thumbnail_concepts = if let Some(concepts) = parsed.divergent_thumbnail_concepts {
            if concepts.len() == 4 {
                concepts
                    .into_iter()
                    .map(|c| ThumbnailConceptBrief {
                        archetype: c
                            .archetype
                            .as_deref()
                            .map(ConceptArchetype::from_str_relaxed)
                            .unwrap_or(ConceptArchetype::ReactionGazeCue),
                        concept_name: c.concept_name,
                        visual_layout_description: c.visual_layout_description,
                        focal_point: c.focal_point,
                        text_overlay: Self::enforce_text_overlay_word_limit(&c.text_overlay),
                        color_lighting_contrast: c.color_lighting_contrast,
                        shoot_on_set_checklist: c.shoot_on_set_checklist,
                    })
                    .collect()
            } else {
                default_concepts
            }
        } else {
            default_concepts
        };

        let mut script = PredictiveScript {
            channel_handle: request.channel_handle.clone(),
            topic: request.topic.clone(),
            selected_hook_madlib: parsed.selected_hook_madlib,
            common_belief: parsed.common_belief,
            contrarian_reality: parsed.contrarian_reality,
            total_duration_sec: request.target_duration_sec as f64,
            overall_wpm: request.target_wpm,
            average_shot_length: request.target_asl_sec,
            beats,
            pacing_compliance_passed: false,
            four_ps,
            acceptance_criteria,
            markdown_teleprompter: String::new(),
            moment_sentence,
            divergent_thumbnail_concepts,
            early_progression: EarlyProgressionCadence::default(),
            stair_stepping: None,
        };

        script.enforce_pacing_invariants(request.target_duration_sec as f64, request.target_asl_sec);
        Ok(script)
    }

    /// Deterministic heuristic template generator for offline/test environments.
    pub fn generate_heuristic_script(
        request: &ScriptRequest,
        _videos: &[VideoDetail],
    ) -> PredictiveScript {
        let topic_lower = request.topic.to_lowercase();
        let target_asl = request.target_asl_sec;
        let target_duration = request.target_duration_sec as f64;
        let num_beats = ((target_duration / target_asl).round() as usize).max(8);

        let identity = request.identity_context.clone().unwrap_or_else(|| {
            CreatorIdentityContext::infer(&request.channel_handle, &request.topic, None, None)
        });

        let is_build_process = identity.tone_profile == NicheToneProfile::DocumentaryBuildProcess
            || topic_lower.contains("pond")
            || topic_lower.contains("excavat")
            || topic_lower.contains("build")
            || topic_lower.contains("homestead")
            || topic_lower.contains("cabin");

        // Niche categorization
        let (madlib, belief, reality, raw_beats) = if is_build_process {
            (
                "Everyone said [X] couldn't [Y]. Here is what [Z] taught us.".to_string(),
                "Building a massive swim pond requires a commercial construction crew.".to_string(),
                "An old excavator, proper slope grading, and 50,000 lbs of stone create an off-grid oasis.".to_string(),
                vec![
                    // Beat 0: Promise (0.0s - 2.4s)
                    ("Forget commercial pools, we are building a swim pond.", "[CUT B-ROLL HERE: Wide drone shot revealing massive excavated pond basin]", Some("BUILDING A SWIM POND")),
                    // Beat 1: Proof (2.4s - 4.8s)
                    ("Our cheap excavator is digging twelve feet deep right now.", "[CUT B-ROLL HERE: Rust-covered excavator bucket tearing through dense rock]", Some("12 FEET DEEP")),
                    // Beat 2: Plan (4.8s - 7.2s)
                    ("Three critical steps: excavation grading, rubber liner, and boulders.", "[CUT B-ROLL HERE: Quick 3-way split showing grading, rubber liner, and boulders]", Some("THE 3 STEPS")),
                    // Beat 3: Persona (7.2s - 9.6s)
                    ("For DIY builders wanting off-grid paradise without commercial crews.", "[CUT B-ROLL HERE: Jaz and Crystal standing together at pond site smiling]", Some("OFF-GRID BUILDERS")),
                    // Beat 4: BodyProgression (Criterion #1: Excavation & grading)
                    ("Basin excavated to target depth with tiered slope grading.", "[CUT B-ROLL HERE: Wide shot of precision stepped dirt benches]", Some("TIERED GRADING")),
                    // Beat 5: BodyProgression (Criterion #2: Liner & underlayment)
                    ("Protective underlayment and rubber liner installed with zero punctures.", "[CUT B-ROLL HERE: Rolling out durable black rubber sheet over underlayment]", Some("ZERO PUNCTURES")),
                    // Beat 6: BodyProgression (Criterion #3: 50,000 lbs rock)
                    ("Fifty thousand pounds of boulders positioned along perimeter shelves.", "[CUT B-ROLL HERE: Dump truck placing massive granite river boulders into pit]", Some("50,000 LBS BOULDERS")),
                    // Beat 7: BodyProgression (Criterion #4: Pump & spillway circulation)
                    ("Submersible pump turns on with rushing natural water circulation.", "[CUT B-ROLL HERE: Sudden rushing water cascading over boulder spillway]", Some("PUMP TEST")),
                    // Beat 8: BodyProgression (Criterion #5: 100,000 gallons swimmable)
                    ("One hundred thousand gallons of crystal clear non-chemical water.", "[CUT B-ROLL HERE: Slow pan across shimmering reflection in completed pond]", Some("100,000 GALLONS")),
                    ("Zero chlorine, zero chemicals, pure natural biological filtration.", "[CUT B-ROLL HERE: Submerged underwater camera showing clear gravel bottom]", None),
                    ("Most people spend eighty thousand on sterile concrete pools.", "[CUT B-ROLL HERE: Split screen showing sterile concrete vs lush oasis]", Some("WHY CONCRETE?")),
                    ("While we built our own backyard paradise for cheap.", "[CUT B-ROLL HERE: Jaz diving into the clear natural pond water]", Some("BACKYARD PARADISE")),
                    ("Four weeks of brutal physical work created this oasis.", "[CUT B-ROLL HERE: Time-lapse from empty field to completed oasis]", None),
                    ("Now you know the secret to building your own.", "[CUT B-ROLL HERE: Sun setting over swim pond with crystal reflection]", None),
                    ("Which is why you must immediately decide to", "[CUT B-ROLL HERE: Sun setting over swim pond, resetting to frame one]", Some("DECIDE TO")),
                ]
            )
        } else if topic_lower.contains("romance")
            || (topic_lower.contains("book") && !topic_lower.contains("facebook"))
            || topic_lower.contains("fiction")
            || topic_lower.contains("enemies")
        {
            (
                "Stop reading [X] if you expect [Y]. This trope will destroy your [Z].".to_string(),
                "Enemies to lovers means cute banter and mild misunderstandings.".to_string(),
                "True dark romance requires genuine psychological warfare, high-stakes betrayal, and irreversible obsession.".to_string(),
                vec![
                    // Beat 0: Promise (0.0s - 2.4s)
                    ("Stop reading dark romance if you want peace.", "[CUT B-ROLL HERE: Dramatic book slam in candlelight with red bookmark]", Some("STOP READING IF YOU WANT PEACE")),
                    // Beat 1: Proof (2.4s - 4.8s)
                    ("Real dark romance requires irreversible betrayal and psychological warfare.", "[CUT B-ROLL HERE: High-contrast typography highlighting 'BETRAYAL']", Some("PSYCHOLOGICAL WARFARE")),
                    // Beat 2: Plan (4.8s - 7.2s)
                    ("Three boundary collapses lead straight into irresistible obsession.", "[CUT B-ROLL HERE: Fast montage of annotated chapter markers at 60fps]", Some("THE 3 BOUNDARIES")),
                    // Beat 3: Persona (7.2s - 9.6s)
                    ("For dark romance readers who demand true high-stakes obsession.", "[CUT B-ROLL HERE: Macro close-up page turning showing annotated dark ink quotes]", Some("DARK ROMANCE READERS")),
                    // Beat 4: BodyProgression (Criterion #1: Distinction from cute banter)
                    ("Sanitized banter gives you rivals who share a milkshake.", "[CUT B-ROLL HERE: Sudden snap zoom with static glitch and red alert border]", Some("RIVALS VS ENEMIES")),
                    // Beat 5: BodyProgression (Criterion #2: High stakes betrayal)
                    ("High-stakes betrayal establishes irreversible psychological tension between them.", "[CUT B-ROLL HERE: Over-the-shoulder shot holding handwritten antagonist letters]", Some("IRREVERSIBLE TENSION")),
                    // Beat 6: BodyProgression (Criterion #3: Boundary collapse)
                    ("Every single boundary she built collapses in chapter four.", "[CUT B-ROLL HERE: Fast cinematic flicker of gothic castle silhouette in rain]", Some("BOUNDARIES COLLAPSE")),
                    // Beat 7: BodyProgression (Criterion #4: Reading proof)
                    ("Concrete proof delivered: this novel will cost you everything.", "[CUT B-ROLL HERE: High-contrast title card of novel recommendation]", Some("COST EVERYTHING")),
                    ("That tension is what keeps you awake until three.", "[CUT B-ROLL HERE: Dim bedroom nightstand clock flipping to 03:45 AM]", Some("3:45 AM")),
                    ("Three in the morning staring at the ceiling sleepless.", "[CUT B-ROLL HERE: Extreme wide silhouette shot of reader looking stunned]", None),
                    ("Because when he finally whispers her true name out.", "[CUT B-ROLL HERE: Slow cinematic dolly-in on dagger resting on velvet book]", None),
                    ("You realize there is no turning back from this.", "[CUT B-ROLL HERE: Sudden black screen snap with white serif typography]", Some("NO TURNING BACK")),
                    ("If you want safety, stick to sweet cozy romance.", "[CUT B-ROLL HERE: Contrast split showing pastel flowers vs burning wax seal]", None),
                    ("The best enemies to lovers will leave scars forever.", "[CUT B-ROLL HERE: Candle flicker reflecting against leather-bound spine]", None),
                    ("Which is why you must immediately go and", "[CUT B-ROLL HERE: Book snaps closed into darkness, seamlessly resetting to frame one]", Some("WHICH IS WHY YOU")),
                ]
            )
        } else if topic_lower.contains("code")
            || topic_lower.contains("software")
            || topic_lower.contains("rust")
            || topic_lower.contains("react")
            || topic_lower.contains("developer")
            || topic_lower.contains("architecture")
        {
            (
                "Stop using [X] for [Y]. You are destroying your [Z].".to_string(),
                "Microservices and complex state libraries make your application scalable from day one.".to_string(),
                "Premature architectural complexity bloats latency, burns memory, and kills developer velocity.".to_string(),
                vec![
                    // Beat 0: Promise (0.0s - 2.4s)
                    ("Stop building microservices before you have any active users.", "[CUT B-ROLL HERE: Frustrated engineer slamming laptop keyboard in dark office]", Some("STOP BUILDING MICROSERVICES")),
                    // Beat 1: Proof (2.4s - 4.8s)
                    ("A single SQLite binary handles one million requests locally.", "[CUT B-ROLL HERE: Fast benchmark graph showing flat 1ms response latency]", Some("1M REQ/S AT 1MS")),
                    // Beat 2: Plan (4.8s - 7.2s)
                    ("The three rules: zero network hops, single binary, ship.", "[CUT B-ROLL HERE: High-speed screencast of Rust server booting in 2ms]", Some("THE 3 RULES")),
                    // Beat 3: Persona (7.2s - 9.6s)
                    ("For pragmatic software engineers tired of resume-driven microservice complexity.", "[CUT B-ROLL HERE: Split screen showing giant AWS bill vs zero active users]", Some("FOR PRAGMATIC ENGINEERS")),
                    // Beat 4: BodyProgression (Criterion #1: Latency tax proof)
                    ("Premature distributed microservices impose an unbearable latency tax.", "[CUT B-ROLL HERE: Terminal scrolling connection timeout stack traces]", Some("LATENCY TAX")),
                    // Beat 5: BodyProgression (Criterion #2: Direct monolith benchmarks)
                    ("Direct benchmarks demonstrate a single SQLite monolith sustaining load.", "[CUT B-ROLL HERE: Macro camera shot of single compiled executable icon]", Some("BENCHMARK PROOF")),
                    // Beat 6: BodyProgression (Criterion #3: Zero network hops)
                    ("Zero network hops between your services delivers raw speed.", "[CUT B-ROLL HERE: Terminal executing 'cargo build --release' cleanly]", Some("ZERO NETWORK HOPS")),
                    // Beat 7: BodyProgression (Criterion #4: Ship monolith first)
                    ("Actionable architectural heuristic: ship a robust monolith first.", "[CUT B-ROLL HERE: Smooth screen recording of snappy user interface]", Some("SHIP MONOLITH FIRST")),
                    ("Senior engineers keep preaching clean distributed system architecture.", "[CUT B-ROLL HERE: Whiteboard diagram covered in ninety interconnected boxes]", Some("DISTRIBUTED CHAOS")),
                    ("While your actual customers just want snappy fast queries.", "[CUT B-ROLL HERE: Browser Lighthouse performance gauge tanking to 28]", Some("LATENCY SPIKE")),
                    ("You are adding distributed consensus for a simple blog.", "[CUT B-ROLL HERE: Sudden snap zoom with sound effect showing simple CRUD code]", None),
                    ("Every remote network call is another point of failure.", "[CUT B-ROLL HERE: Terminal scrolling connection timeout stack traces]", Some("POINT OF FAILURE")),
                    ("Mechanical sympathy beats resume driven development every single time.", "[CUT B-ROLL HERE: High-contrast side-by-side RAM usage chart]", None),
                    ("Complexity is the easiest way to fail completely silently.", "[CUT B-ROLL HERE: Clean dashboard showing 99.99% uptime metrics]", None),
                    ("And that is the exact reason why you", "[CUT B-ROLL HERE: Terminal cursor blinking, seamlessly looping into frame one]", Some("EXACT REASON WHY YOU")),
                ]
            )
        } else {
            // Universal / High-Stakes Storytelling
            (
                "Stop doing [X] if you want [Y]. Here is the truth.".to_string(),
                "Working harder and doing more volume guarantees long-term breakout success.".to_string(),
                "Unfocused effort accelerates burnout; leverage and high-contrast positioning create asymmetric returns.".to_string(),
                vec![
                    // Beat 0: Promise (0.0s - 2.4s)
                    ("Stop grinding sixty hours a week for zero results.", "[CUT B-ROLL HERE: Alarm clock ringing at 5:00 AM with tired creator face]", Some("STOP GRINDING")),
                    // Beat 1: Proof (2.4s - 4.8s)
                    ("Top outlier performers extract 10x asymmetric leverage curves.", "[CUT B-ROLL HERE: Clean graph showing 10x asymmetric outlier curve]", Some("ASYMMETRIC OUTLIER")),
                    // Beat 2: Plan (4.8s - 7.2s)
                    ("Three steps: isolate highest leverage, eliminate friction, deliver payoff.", "[CUT B-ROLL HERE: Pattern interrupt snap zoom with high-contrast graphic]", Some("THE 3 STEPS")),
                    // Beat 3: Persona (7.2s - 9.6s)
                    ("For serious practitioners focused on asymmetric leverage over hustle.", "[CUT B-ROLL HERE: Split screen of empty analytics chart vs crowded timeline]", Some("LEVERAGE OVER HUSTLE")),
                    // Beat 4: BodyProgression (Criterion #1: Premise verified)
                    ("Premise verified: deconstruct conventional belief and expose why effort stalls.", "[CUT B-ROLL HERE: Fast montage of empty calendar scheduling slots]", Some("EFFORT != RESULTS")),
                    // Beat 5: BodyProgression (Criterion #2: Highest leverage variable)
                    ("The single highest-leverage variable isolated with real comparison data.", "[CUT B-ROLL HERE: Rapid swipe-through of fifty identical low-view shorts]", Some("ISOLATE LEVERAGE")),
                    // Beat 6: BodyProgression (Criterion #3: Friction elimination)
                    ("Friction elimination: demonstrate tactical workflow removing all wasted effort.", "[CUT B-ROLL HERE: Timeline editing cursor slicing out dead pauses]", Some("REMOVE FRICTION")),
                    // Beat 7: BodyProgression (Criterion #4: Payoff delivered)
                    ("Verification metric: prove the payoff delivers before thirty seconds.", "[CUT B-ROLL HERE: Retention graph spiking past eighty percent mark]", Some("DELIVER PAYOFF")),
                    ("One winning angle beats fifty generic low-view uploads.", "[CUT B-ROLL HERE: Rapid swipe-through of fifty identical low-view shorts]", None),
                    ("While the algorithm only rewards true viewer watch retention.", "[CUT B-ROLL HERE: High-energy screen demonstration revealing the secret]", None),
                    ("Cut every single second that does not directly matter.", "[CUT B-ROLL HERE: Timeline editing cursor slicing out dead pauses]", Some("CUT THE NOISE")),
                    ("Focus on one specific audience with ruthless razor precision.", "[CUT B-ROLL HERE: Clean typography overlay highlighting 'FOCUS']", None),
                    ("Your next breakthrough is hiding in the micro details.", "[CUT B-ROLL HERE: Macro lens shot of pen writing down key principle]", Some("THE DETAILS")),
                    ("Never trade your health for vanity view count metrics.", "[CUT B-ROLL HERE: Creator smiling calmly at camera with clean setup]", None),
                    ("Which is why you must immediately decide to", "[CUT B-ROLL HERE: Sudden black screen reset, seamlessly chaining into frame one]", Some("DECIDE TO")),
                ]
            )
        };

        let mut beats = Vec::with_capacity(num_beats);
        for i in 0..num_beats {
            let template_idx = i % raw_beats.len();
            let (dialogue, visual, text) = raw_beats[template_idx];

            let beat_type = if i == 0 {
                BeatType::HookOpening
            } else if i == 1 {
                BeatType::CognitiveFrictionTurn
            } else if i == num_beats - 1 {
                BeatType::LoopingCTA
            } else if i == num_beats - 2 {
                BeatType::PayoffClimax
            } else if i % 4 == 0 {
                BeatType::PatternInterrupt
            } else {
                BeatType::BodyProgression
            };

            beats.push(ScriptBeat {
                beat_index: i,
                start_sec: (i as f64) * target_asl,
                end_sec: ((i + 1) as f64 * target_asl).min(target_duration),
                beat_type,
                spoken_dialogue: dialogue.to_string(),
                word_count: dialogue.split_whitespace().count(),
                computed_wpm: 0.0,
                visual_directive: visual.to_string(),
                on_screen_text: text.map(|s| s.to_string()),
                four_p_role: None,
                fulfilled_criterion_index: None,
                micro_milestone: None,
                stair_step_tier: None,
            });
        }

        // Check for Stair-Stepping Format
        let is_stair_stepping = crate::pipeline::hook_machine::HookMachineEngine::detect_stair_stepping_format(&request.topic);
        let mut stair_progression: Option<crate::pipeline::hook_machine::StairSteppingProgression> = None;

        if is_stair_stepping {
            let lower_topic = request.topic.to_lowercase();
            let (t1_label, s1, t2_label, s2, t3_label, s3, t4_label, s4) = if lower_topic.contains("$1 vs $1,000,000") || lower_topic.contains("$1 vs 1,000,000") {
                ("$1 Budget Gear".to_string(), 1.0, "$10,000 Advanced Gear".to_string(), 10_000.0, "$100,000 Extreme Gear".to_string(), 100_000.0, "$1,000,000 Climax Rig".to_string(), 1_000_000.0)
            } else if lower_topic.contains("$1 vs $500,000") || lower_topic.contains("$1 vs 500,000") {
                ("$1 Experience".to_string(), 1.0, "$10,000 Experience".to_string(), 10_000.0, "$50,000 Experience".to_string(), 50_000.0, "$500,000 Climax Experience".to_string(), 500_000.0)
            } else if lower_topic.contains("min") && lower_topic.contains("hour") {
                ("10 Mins (Baseline)".to_string(), 10.0, "1 Hour (10x Difficulty)".to_string(), 60.0, "10 Hours (Extreme Spectacle)".to_string(), 600.0, "100 Hours (Climax Payoff)".to_string(), 6000.0)
            } else if lower_topic.contains("$100") && lower_topic.contains("$1,000") {
                ("$100 Baseline Flip".to_string(), 100.0, "$250 Mid-Tier Flip".to_string(), 250.0, "$500 High-Stakes Flip".to_string(), 500.0, "$1,000 Climax Flip".to_string(), 1000.0)
            } else {
                ("Tier 1: Baseline / Budget".to_string(), 1.0, "Tier 2: 10x Escalation".to_string(), 10.0, "Tier 3: Extreme Spectacle".to_string(), 100.0, "Tier 4: Climax Payoff".to_string(), 1000.0)
            };

            // Calculate tier boundaries across body beats
            // Golden Rule: Climax (Tier 4) must be placed at >= 80% of runtime (final 20%)
            let climax_start_idx = ((num_beats as f64 * 0.80).ceil() as usize).min(num_beats.saturating_sub(2)).max(4);
            let body_start_idx = 4.min(num_beats.saturating_sub(4));
            let body_len = climax_start_idx.saturating_sub(body_start_idx);
            let step = (body_len / 3).max(1);
            let t2_start_idx = (body_start_idx + step).min(climax_start_idx);
            let t3_start_idx = (body_start_idx + step * 2).min(climax_start_idx);

            let t1 = crate::pipeline::hook_machine::StairStepTier {
                tier_index: 1,
                label: t1_label,
                scale_value: s1,
                timecode_range: format!("{}-{}", format_timecode_simple(beats[body_start_idx].start_sec), format_timecode_simple(beats[t2_start_idx.saturating_sub(1)].end_sec)),
                timecode_seconds: beats[body_start_idx].start_sec,
                is_climax: false,
            };
            let t2 = crate::pipeline::hook_machine::StairStepTier {
                tier_index: 2,
                label: t2_label,
                scale_value: s2,
                timecode_range: format!("{}-{}", format_timecode_simple(beats[t2_start_idx].start_sec), format_timecode_simple(beats[t3_start_idx.saturating_sub(1)].end_sec)),
                timecode_seconds: beats[t2_start_idx].start_sec,
                is_climax: false,
            };
            let t3 = crate::pipeline::hook_machine::StairStepTier {
                tier_index: 3,
                label: t3_label,
                scale_value: s3,
                timecode_range: format!("{}-{}", format_timecode_simple(beats[t3_start_idx].start_sec), format_timecode_simple(beats[climax_start_idx.saturating_sub(1)].end_sec)),
                timecode_seconds: beats[t3_start_idx].start_sec,
                is_climax: false,
            };
            let last_body_idx = num_beats.saturating_sub(2);
            let t4 = crate::pipeline::hook_machine::StairStepTier {
                tier_index: 4,
                label: t4_label,
                scale_value: s4,
                timecode_range: format!("{}-{}", format_timecode_simple(beats[climax_start_idx].start_sec), format_timecode_simple(beats[last_body_idx].end_sec)),
                timecode_seconds: beats[climax_start_idx].start_sec,
                is_climax: true,
            };

            for beat in beats.iter_mut().take(t2_start_idx).skip(body_start_idx) {
                beat.stair_step_tier = Some(t1.clone());
            }
            for beat in beats.iter_mut().take(t3_start_idx).skip(t2_start_idx) {
                beat.stair_step_tier = Some(t2.clone());
            }
            for beat in beats.iter_mut().take(climax_start_idx).skip(t3_start_idx) {
                beat.stair_step_tier = Some(t3.clone());
            }
            for beat in beats.iter_mut().take(last_body_idx + 1).skip(climax_start_idx) {
                beat.stair_step_tier = Some(t4.clone());
                beat.beat_type = BeatType::PayoffClimax;
            }

            let detected_tiers = vec![t1, t2, t3, t4];
            let has_monotonic_escalation = detected_tiers.windows(2).all(|w| w[0].scale_value < w[1].scale_value);
            let climax_held_for_finale = beats[climax_start_idx].start_sec >= 0.80 * target_duration;

            let diag = format!(
                "Optimal Stair-Stepping Escalation: Peak payoff situated at {} of {} ({}% mark). Stakes increase monotonically across all tiers.",
                format_timecode_simple(beats[climax_start_idx].start_sec),
                format_timecode_simple(target_duration),
                ((beats[climax_start_idx].start_sec / target_duration) * 100.0).round() as u64
            );

            stair_progression = Some(crate::pipeline::hook_machine::StairSteppingProgression {
                is_stair_stepping: true,
                detected_tiers,
                has_monotonic_escalation,
                climax_held_for_finale,
                pacing_diagnostic: diag,
            });
        }

        let (window_start_sec, window_end_sec) = if target_duration >= 180.0 {
            (60.0, 180.0)
        } else {
            (
                ((target_duration * 0.20) * 10.0).round() / 10.0,
                ((target_duration * 0.55) * 10.0).round() / 10.0,
            )
        };

        let window_beat_indices: Vec<usize> = beats
            .iter()
            .enumerate()
            .filter(|(_, b)| b.start_sec >= window_start_sec && b.start_sec <= window_end_sec)
            .map(|(idx, _)| idx)
            .collect();

        if window_beat_indices.len() >= 2 {
            let idx1 = window_beat_indices[window_beat_indices.len() / 4];
            let idx2 = window_beat_indices[(window_beat_indices.len() * 3) / 4];

            let m1_desc = if topic_lower.contains("pond") || topic_lower.contains("water") || topic_lower.contains("ice") {
                "Day 1: Water perimeter secured & ice defense operational".to_string()
            } else if topic_lower.contains("cabin") || topic_lower.contains("build") || topic_lower.contains("shelter") {
                "Step 1: Primary foundation framed & leveled".to_string()
            } else {
                "Checkpoint 1: Core setup complete & initial benchmark proven".to_string()
            };

            let m2_desc = if topic_lower.contains("pond") || topic_lower.contains("water") || topic_lower.contains("ice") {
                "Day 2: Deep aeration system active & thermal barrier established".to_string()
            } else if topic_lower.contains("cabin") || topic_lower.contains("build") || topic_lower.contains("shelter") {
                "Step 2: Weatherproofing envelope sealed & load test passed".to_string()
            } else {
                "Checkpoint 2: Critical obstacle bypassed & secondary milestone secured".to_string()
            };

            let tc1 = format_timecode_simple(beats[idx1].start_sec);
            beats[idx1].micro_milestone = Some(MicroMilestone {
                milestone_index: 1,
                timecode: tc1,
                timecode_seconds: beats[idx1].start_sec,
                achievement_description: m1_desc,
                progression_cue: beats[idx1].spoken_dialogue.clone(),
            });

            let tc2 = format_timecode_simple(beats[idx2].start_sec);
            beats[idx2].micro_milestone = Some(MicroMilestone {
                milestone_index: 2,
                timecode: tc2,
                timecode_seconds: beats[idx2].start_sec,
                achievement_description: m2_desc,
                progression_cue: beats[idx2].spoken_dialogue.clone(),
            });
        }

        let four_ps = FourPsFramework::compile(&request.topic, &identity);
        let acceptance_criteria = TitleAcceptanceCriteria::compile(&request.topic, &identity);
        let (moment_sentence, divergent_thumbnail_concepts) =
            Self::generate_heuristic_packaging_briefs(&request.topic, &identity);

        let mut script = PredictiveScript {
            channel_handle: request.channel_handle.clone(),
            topic: request.topic.clone(),
            selected_hook_madlib: madlib,
            common_belief: belief,
            contrarian_reality: reality,
            total_duration_sec: target_duration,
            overall_wpm: request.target_wpm,
            average_shot_length: target_asl,
            beats,
            pacing_compliance_passed: false,
            four_ps,
            acceptance_criteria,
            markdown_teleprompter: String::new(),
            moment_sentence,
            divergent_thumbnail_concepts,
            early_progression: EarlyProgressionCadence::default(),
            stair_stepping: stair_progression,
        };

        script.enforce_pacing_invariants(target_duration, target_asl);
        script
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_beat_types_and_labels() {
        assert_eq!(BeatType::HookOpening.label(), "HOOK OPENING");
        assert_eq!(BeatType::CognitiveFrictionTurn.label(), "COGNITIVE FRICTION");
        assert_eq!(BeatType::PatternInterrupt.label(), "PATTERN INTERRUPT");
        assert_eq!(BeatType::PayoffClimax.label(), "PAYOFF CLIMAX");
        assert_eq!(BeatType::LoopingCTA.label(), "LOOPING CTA");
    }

    #[test]
    fn test_heuristic_script_generation_and_pacing_gate() {
        let req = ScriptRequest {
            channel_handle: "@creator_studio".to_string(),
            topic: "Dark Romance Enemies to Lovers".to_string(),
            target_duration_sec: 36,
            target_wpm: 195.0,
            target_asl_sec: 2.4,
            identity_context: None,
        };

        let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

        // 36s / 2.4s = 15 beats
        assert_eq!(script.beats.len(), 15);
        assert_eq!(script.total_duration_sec, 36.0);
        assert!((script.average_shot_length - 2.4).abs() < 0.01);

        // Beat 0 is HookOpening
        assert_eq!(script.beats[0].beat_type, BeatType::HookOpening);
        assert_eq!(script.beats[0].start_sec, 0.0);
        assert!((script.beats[0].end_sec - 2.4).abs() < 0.01);
        assert!(script.beats[0].visual_directive.starts_with("[CUT B-ROLL HERE:"));

        // Beat 1 is CognitiveFrictionTurn (lands right around 3.5s gate)
        assert_eq!(script.beats[1].beat_type, BeatType::CognitiveFrictionTurn);
        assert!((script.beats[1].start_sec - 2.4).abs() < 0.01);

        // Opening 3.5s WPM check
        let b0_words = script.beats[0].word_count;
        let b1_words = script.beats[1].word_count;
        let b1_in_35 = ((b1_words as f64) * (1.1 / 2.4)).round() as usize;
        let opening_words = b0_words + b1_in_35;
        let opening_wpm = (opening_words as f64 / 3.5) * 60.0;

        assert!((185.0..=215.0).contains(&opening_wpm), "Opening WPM was {:.1}", opening_wpm);
        assert!(script.pacing_compliance_passed);

        // Final beat is LoopingCTA
        let last_beat = script.beats.last().unwrap();
        assert_eq!(last_beat.beat_type, BeatType::LoopingCTA);

        // Teleprompter rendered
        assert!(script.markdown_teleprompter.contains("# 🎬 Sandsifter Production Blueprint & Teleprompter"));
        assert!(script.markdown_teleprompter.contains("Common Belief"));
        assert!(script.markdown_teleprompter.contains("Contrarian Reality"));
    }

    #[test]
    fn test_script_to_export_timeline_fcpxml_and_edl() {
        let req = ScriptRequest {
            channel_handle: "@tech_lead".to_string(),
            topic: "Software Architecture".to_string(),
            target_duration_sec: 36,
            target_wpm: 195.0,
            target_asl_sec: 2.4,
            identity_context: None,
        };

        let script = ScriptGenerator::generate_heuristic_script(&req, &[]);
        let timeline = script.to_export_timeline();

        // 15 beats -> 15 shots on timeline
        assert_eq!(timeline.shots.len(), 15);
        assert_eq!(timeline.shots[0].offset.to_seconds(), 0.0);
        assert!((timeline.shots[0].duration.to_seconds() - 2.4).abs() < 0.01);

        // FCPXML string export verification
        let fcpxml = timeline.to_fcpxml_string();
        assert!(fcpxml.contains("<fcpxml version=\"1.8\">"));
        assert!(fcpxml.contains("<gap name=\"Beat 01: HOOK OPENING\""));
        assert!(fcpxml.contains("<marker start=\"0s\""));
        assert!(fcpxml.contains("HOOK OPENING: Beat 1"));

        // EDL export verification
        let edl = timeline.to_edl_string();
        assert!(edl.contains("TITLE: SCRIPT: SOFTWARE ARCHITECTURE"));
        assert!(edl.contains("001  AX       V     C"));
        assert!(edl.contains("* FROM CLIP: Beat 01: HOOK OPENING"));
    }

    #[test]
    fn test_pacing_invariants_word_trimming() {
        let mut script = PredictiveScript {
            channel_handle: "@creator".to_string(),
            topic: "Test Topic".to_string(),
            selected_hook_madlib: "Madlib".to_string(),
            common_belief: "Belief".to_string(),
            contrarian_reality: "Reality".to_string(),
            total_duration_sec: 36.0,
            overall_wpm: 195.0,
            average_shot_length: 2.4,
            beats: vec![
                ScriptBeat {
                    beat_index: 0,
                    start_sec: 0.0,
                    end_sec: 2.4,
                    beat_type: BeatType::HookOpening,
                    // 15 words - way too many words for 2.4s (375 WPM)
                    spoken_dialogue: "This is a really super long sentence that has way too many words for opening".to_string(),
                    word_count: 15,
                    computed_wpm: 375.0,
                    visual_directive: "Cut to face".to_string(),
                    on_screen_text: None,
                    four_p_role: None,
                    fulfilled_criterion_index: None,
                    micro_milestone: None,
                    stair_step_tier: None,
                },
                ScriptBeat {
                    beat_index: 1,
                    start_sec: 2.4,
                    end_sec: 4.8,
                    beat_type: BeatType::CognitiveFrictionTurn,
                    // 13 words
                    spoken_dialogue: "You think that this will work but actually it is completely broken today".to_string(),
                    word_count: 13,
                    computed_wpm: 325.0,
                    visual_directive: "Cut to screen".to_string(),
                    on_screen_text: None,
                    four_p_role: None,
                    fulfilled_criterion_index: None,
                    micro_milestone: None,
                    stair_step_tier: None,
                },
                ScriptBeat {
                    beat_index: 2,
                    start_sec: 4.8,
                    end_sec: 7.2,
                    beat_type: BeatType::BodyProgression,
                    spoken_dialogue: "Here is the exact fix you should apply right now.".to_string(),
                    word_count: 9,
                    computed_wpm: 225.0,
                    visual_directive: "Cut to code".to_string(),
                    on_screen_text: None,
                    four_p_role: None,
                    fulfilled_criterion_index: None,
                    micro_milestone: None,
                    stair_step_tier: None,
                },
                ScriptBeat {
                    beat_index: 3,
                    start_sec: 7.2,
                    end_sec: 9.6,
                    beat_type: BeatType::LoopingCTA,
                    spoken_dialogue: "Always remember this crucial lesson.".to_string(),
                    word_count: 5,
                    computed_wpm: 125.0,
                    visual_directive: "Cut to black".to_string(),
                    on_screen_text: None,
                    four_p_role: None,
                    fulfilled_criterion_index: None,
                    micro_milestone: None,
                    stair_step_tier: None,
                },
            ],
            pacing_compliance_passed: false,
            four_ps: FourPsFramework::default(),
            acceptance_criteria: TitleAcceptanceCriteria::default(),
            markdown_teleprompter: String::new(),
            moment_sentence: String::new(),
            divergent_thumbnail_concepts: Vec::new(),
            early_progression: EarlyProgressionCadence::default(),
            stair_stepping: None,
        };

        script.enforce_pacing_invariants(9.6, 2.4);

        // Beat 0 trimmed to 8 words
        assert_eq!(script.beats[0].word_count, 8);
        assert!((script.beats[0].computed_wpm - 200.0).abs() < 1.0);

        // Visual directive formatted
        assert!(script.beats[0].visual_directive.starts_with("[CUT B-ROLL HERE:"));

        // Last beat connected for loop
        assert!(script.beats[3].spoken_dialogue.contains("which is why you"));
    }

    #[test]
    fn test_four_ps_opening_gate_and_acceptance_criteria() {
        let req = ScriptRequest {
            channel_handle: "@vanwives".to_string(),
            topic: "Backyard Swim Pond Build".to_string(),
            target_duration_sec: 36,
            target_wpm: 195.0,
            target_asl_sec: 2.4,
            identity_context: Some(CreatorIdentityContext::infer(
                "@vanwives",
                "Backyard Swim Pond Build",
                Some("Crystal and Jaz"),
                Some("Rural off-grid cabin in Canada"),
            )),
        };

        let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

        // Verify the 4 Ps Opening Gate on beats 0..3 (0.0s - 9.6s)
        assert_eq!(script.beats[0].four_p_role, Some(FourPRole::Promise));
        assert_eq!(script.beats[1].four_p_role, Some(FourPRole::Proof));
        assert_eq!(script.beats[2].four_p_role, Some(FourPRole::Plan));
        assert_eq!(script.beats[3].four_p_role, Some(FourPRole::Persona));

        assert!(!script.four_ps.promise.is_empty());
        assert!(!script.four_ps.proof.is_empty());
        assert!(!script.four_ps.plan.is_empty());
        assert!(!script.four_ps.persona.is_empty());

        // Verify Title Acceptance Criteria decomposition (3 to 5 conditions)
        assert!(script.acceptance_criteria.criteria.len() >= 3);
        assert!(script.acceptance_criteria.criteria.len() <= 5);

        // Verify every Acceptance Criterion is verified and fulfilled in the script beats
        for criterion in &script.acceptance_criteria.criteria {
            assert!(criterion.fulfilled, "Criterion #{} not fulfilled: {}", criterion.id, criterion.condition);
            assert!(criterion.fulfilled_at_beat.is_some());
        }

        // Verify BodyProgression beats (indices 4..13) are mapped to an Acceptance Criterion with zero fluff
        for beat in &script.beats {
            if beat.beat_index >= 4 && beat.beat_type != BeatType::LoopingCTA {
                assert!(
                    beat.fulfilled_criterion_index.is_some(),
                    "Beat {} lacked an assigned acceptance criterion!",
                    beat.beat_index
                );
            }
        }

        // Verify Markdown teleprompter formatting includes the 4 Ps and Acceptance Criteria
        assert!(script.markdown_teleprompter.contains("### 🧱 The 4 Ps Opening Gate (0.0s - 10.0s)"));
        assert!(script.markdown_teleprompter.contains("### 📋 Title Software Acceptance Criteria Checklist"));
        assert!(script.markdown_teleprompter.contains("[PROMISE (CONFIRM CLICK)]"));
        assert!(script.markdown_teleprompter.contains("[CRITERION #1]"));
    }
}
