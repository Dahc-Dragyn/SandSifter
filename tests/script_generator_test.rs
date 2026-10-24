use sandsifter::export::RationalTime;
use sandsifter::mcp::*;
use sandsifter::pipeline::script_generator::{
    BeatType, ConceptArchetype, EarlyProgressionCadence, FourPRole, FourPsFramework, MicroMilestone,
    PredictiveScript, ScriptBeat, ScriptGenerator, ScriptRequest, ThumbnailConceptBrief,
    TitleAcceptanceCriteria,
};
use sandsifter::storage::SandsifterDb;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpListener;

#[test]
fn test_script_generator_asl_and_beat_cadence_enforcement() {
    let req = ScriptRequest {
        channel_handle: "@creator_studio".to_string(),
        topic: "Dark Romance Enemies to Lovers".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // Invariant 1: Duration & Beat Count
    // 36.0s / 2.4s = exactly 15 beats
    assert_eq!(script.beats.len(), 15);
    assert_eq!(script.total_duration_sec, 36.0);
    assert!((script.average_shot_length - 2.4).abs() < 0.001);

    // Invariant 2: 0.0s - 3.5s Hook & Cognitive Friction Boundary
    let b0 = &script.beats[0];
    let b1 = &script.beats[1];
    assert_eq!(b0.beat_type, BeatType::HookOpening);
    assert_eq!(b0.start_sec, 0.0);
    assert!((b0.end_sec - 2.4).abs() < 0.001);

    assert_eq!(b1.beat_type, BeatType::CognitiveFrictionTurn);
    assert!((b1.start_sec - 2.4).abs() < 0.001);
    assert!((b1.end_sec - 4.8).abs() < 0.001);

    // Cadence check: opening 3.5s WPM must be strictly within [185.0, 215.0]
    let b0_words = b0.word_count;
    let b1_words = b1.word_count;
    let b1_est_words = ((b1_words as f64) * (1.1 / 2.4)).round() as usize;
    let opening_words = b0_words + b1_est_words;
    let opening_wpm = (opening_words as f64 / 3.5) * 60.0;
    assert!(
        (185.0..=215.0).contains(&opening_wpm),
        "Opening 3.5s WPM was {:.1}, expected within [185.0, 215.0]",
        opening_wpm
    );
    assert!(script.pacing_compliance_passed);

    // Invariant 3: Visual Directive Syntax
    for beat in &script.beats {
        assert!(
            beat.visual_directive.starts_with("[CUT B-ROLL HERE:"),
            "Beat {} visual directive did not have prefix: {}",
            beat.beat_index,
            beat.visual_directive
        );
        assert!(
            beat.visual_directive.ends_with(']'),
            "Beat {} visual directive did not end with bracket: {}",
            beat.beat_index,
            beat.visual_directive
        );
    }

    // Invariant 4: Looping CTA Syntax
    let last_beat = script.beats.last().unwrap();
    assert_eq!(last_beat.beat_type, BeatType::LoopingCTA);
    let first_word = b0
        .spoken_dialogue
        .split_whitespace()
        .next()
        .unwrap()
        .trim_matches(|c: char| !c.is_alphanumeric());
    assert!(
        last_beat.on_screen_text.as_ref().unwrap().contains(&first_word.to_uppercase()),
        "Last beat on-screen text did not contain first word '{}'",
        first_word
    );
}

#[test]
fn test_script_timeline_export_rational_time_zero_drift() {
    let req = ScriptRequest {
        channel_handle: "@tech_lead".to_string(),
        topic: "Software Architecture".to_string(),
        target_duration_sec: 48,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);
    // 48s / 2.4s = 20 beats
    assert_eq!(script.beats.len(), 20);

    let timeline = script.to_export_timeline();
    assert_eq!(timeline.shots.len(), 20);

    // Verify RationalTime precision across every single cut
    let mut cumulative_rational = RationalTime::zero();
    let timescale = 30000;

    for (i, shot) in timeline.shots.iter().enumerate() {
        let expected_offset = RationalTime::from_seconds((i as f64) * 2.4, timescale);
        assert_eq!(shot.offset.to_seconds(), expected_offset.to_seconds());
        assert!((shot.duration.to_seconds() - 2.4).abs() < 0.001);

        cumulative_rational = cumulative_rational + shot.duration;

        // Verify marker presence
        assert!(!shot.markers.is_empty());
        let m0 = &shot.markers[0];
        assert_eq!(m0.start, RationalTime::zero());
        assert!(m0.note.contains("Spoken:"));
    }

    assert!((cumulative_rational.to_seconds() - 48.0).abs() < 0.001);

    // Verify FCPXML and EDL output validity
    let fcpxml = timeline.to_fcpxml_string();
    assert!(fcpxml.contains("<fcpxml version=\"1.8\">"));
    assert!(fcpxml.contains("<gap name=\"Beat 01: HOOK OPENING\""));
    assert!(fcpxml.contains("<marker start=\"0s\""));

    let edl = timeline.to_edl_string();
    assert!(edl.contains("TITLE: SCRIPT: SOFTWARE ARCHITECTURE"));
    assert!(edl.contains("001  AX       V     C"));
}

#[test]
fn test_pacing_invariants_auto_correction_on_outlier_cadence() {
    let mut script = PredictiveScript {
        channel_handle: "@author".to_string(),
        topic: "Extreme Pacing Test".to_string(),
        selected_hook_madlib: "Formula".to_string(),
        common_belief: "Belief".to_string(),
        contrarian_reality: "Reality".to_string(),
        total_duration_sec: 36.0,
        overall_wpm: 195.0,
        average_shot_length: 2.4,
        beats: vec![
            // Overly verbose Beat 0 (18 words -> 450 WPM)
            ScriptBeat {
                beat_index: 0,
                start_sec: 0.0,
                end_sec: 2.4,
                beat_type: BeatType::HookOpening,
                spoken_dialogue: "Hey everyone welcome back today I am going to show you an absolutely insane crazy technique right now".to_string(),
                word_count: 18,
                computed_wpm: 450.0,
                visual_directive: "Cut to talking head".to_string(),
                on_screen_text: None,
                four_p_role: None,
                fulfilled_criterion_index: None,
                micro_milestone: None,
                stair_step_tier: None,
            },
            // Sluggish Beat 1 (3 words -> 75 WPM)
            ScriptBeat {
                beat_index: 1,
                start_sec: 2.4,
                end_sec: 4.8,
                beat_type: BeatType::CognitiveFrictionTurn,
                spoken_dialogue: "This really works.".to_string(),
                word_count: 3,
                computed_wpm: 75.0,
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
                spoken_dialogue: "Here is the exact step to follow right now.".to_string(),
                word_count: 9,
                computed_wpm: 225.0,
                visual_directive: "Cut to demo".to_string(),
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
                spoken_dialogue: "Remember this single rule every single day.".to_string(),
                word_count: 7,
                computed_wpm: 175.0,
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

    // Beat 0 should be trimmed to 8 words
    assert_eq!(script.beats[0].word_count, 8);
    assert!((script.beats[0].computed_wpm - 200.0).abs() < 1.0);

    // Beat 1 should be expanded with urgency token
    assert!(script.beats[1].spoken_dialogue.starts_with("Notice:"));
    assert!(script.beats[1].word_count >= 4);

    // Visual directives should be formatted
    assert!(script.beats[0].visual_directive.starts_with("[CUT B-ROLL HERE:"));
    assert!(script.beats[1].visual_directive.starts_with("[CUT B-ROLL HERE:"));

    // Final beat should bridge for looping
    assert!(script.beats[3].spoken_dialogue.contains("which is why you"));
}

#[test]
fn test_title_acceptance_criteria_and_four_ps_opening_gate() {
    let req = ScriptRequest {
        channel_handle: "@system_engineer".to_string(),
        topic: "Stop Building Microservices Monolith Architecture".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // 1. 4 Ps Opening Gate Verification (0.0s - 9.6s)
    assert_eq!(script.beats[0].four_p_role, Some(FourPRole::Promise));
    assert_eq!(script.beats[1].four_p_role, Some(FourPRole::Proof));
    assert_eq!(script.beats[2].four_p_role, Some(FourPRole::Plan));
    assert_eq!(script.beats[3].four_p_role, Some(FourPRole::Persona));

    assert_eq!(script.beats[0].four_p_role.unwrap().label(), "PROMISE (CONFIRM CLICK)");
    assert_eq!(script.beats[1].four_p_role.unwrap().label(), "PROOF (AUTHORITY / SCALE)");
    assert_eq!(script.beats[2].four_p_role.unwrap().label(), "PLAN (3 STEPS)");
    assert_eq!(script.beats[3].four_p_role.unwrap().label(), "PERSONA (TARGET VIEWER)");

    // 2. Acceptance Criteria Decomposition (3-5 criteria)
    assert!(script.acceptance_criteria.criteria.len() >= 3 && script.acceptance_criteria.criteria.len() <= 5);
    for crit in &script.acceptance_criteria.criteria {
        assert!(crit.fulfilled, "Criterion #{} not fulfilled: {}", crit.id, crit.condition);
        assert!(crit.fulfilled_at_beat.is_some());
    }

    // 3. Body Progression Beats Verification (zero fluff: every body beat checks off a criterion)
    let body_beats: Vec<&ScriptBeat> = script
        .beats
        .iter()
        .filter(|b| b.beat_index >= 4 && b.beat_type != BeatType::LoopingCTA)
        .collect();

    assert!(!body_beats.is_empty());
    for beat in body_beats {
        assert!(
            beat.fulfilled_criterion_index.is_some(),
            "Body beat {} has no assigned acceptance criterion!",
            beat.beat_index
        );
    }

    // 4. Export Timeline Markers include 4P and Criterion tags
    let timeline = script.to_export_timeline();
    assert!(timeline.shots[0].markers[0].name.contains("[PROMISE"));
    assert!(timeline.shots[1].markers[0].name.contains("[PROOF"));
    assert!(timeline.shots[2].markers[0].name.contains("[PLAN"));
    assert!(timeline.shots[3].markers[0].name.contains("[PERSONA"));
}

#[tokio::test]
async fn test_rest_api_script_generation_endpoint() {
    let db = SandsifterDb::memory().expect("In-memory DB creation failed");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral port");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{}", port);

    let server = McpServer::with_db(db.clone());
    tokio::spawn(async move {
        let _ = start_http_server_with_listener(server, Some(db), listener).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = reqwest::Client::new();
    let payload = json!({
        "channel_handle": "@creator_studio",
        "topic": "Dark Romance Enemies to Lovers",
        "target_duration_sec": 36,
        "target_wpm": 195.0,
        "target_asl_sec": 2.4
    });

    let res = client
        .post(format!("{}/api/script", base_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to POST /api/script");

    assert_eq!(res.status(), 200);

    let json_body: serde_json::Value = res.json().await.expect("Failed to parse JSON response");
    assert_eq!(json_body["status"], "success");
    assert_eq!(json_body["script"]["channel_handle"], "@creator_studio");
    assert_eq!(json_body["script"]["topic"], "Dark Romance Enemies to Lovers");
    assert_eq!(json_body["script"]["total_duration_sec"], 36.0);
    assert_eq!(json_body["script"]["beats"].as_array().unwrap().len(), 15);
    assert!(json_body["markdown"].as_str().unwrap().contains("# 🎬 Sandsifter Production Blueprint & Teleprompter"));
}

#[test]
fn test_moment_sentence_and_four_extreme_opposites_synthesis() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond Build: Winterizing and Ice Defense".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // 1. Verifies moment_sentence is non-empty
    assert!(!script.moment_sentence.trim().is_empty());

    // 2. Verifies divergent_thumbnail_concepts.len() == 4
    assert_eq!(script.divergent_thumbnail_concepts.len(), 4);

    // 3. Asserts all 4 archetypes are distinctly represented
    let mut archetypes = std::collections::HashSet::new();
    for concept in &script.divergent_thumbnail_concepts {
        let _brief: &ThumbnailConceptBrief = concept;
        archetypes.insert(concept.archetype);
        // 4. Confirms every text_overlay contains <= 4 words
        let word_count = concept.text_overlay.split_whitespace().count();
        assert!(
            word_count <= 4,
            "Concept '{}' text overlay has {} words (> 4 words): \"{}\"",
            concept.concept_name, word_count, concept.text_overlay
        );
        assert!(!concept.shoot_on_set_checklist.is_empty());
        assert!(!concept.visual_layout_description.is_empty());
        assert!(!concept.focal_point.is_empty());
    }

    assert_eq!(archetypes.len(), 4);
    assert!(archetypes.contains(&ConceptArchetype::ReactionGazeCue));
    assert!(archetypes.contains(&ConceptArchetype::MinimalistBeforeAfter));
    assert!(archetypes.contains(&ConceptArchetype::WideCinematicMap));
    assert!(archetypes.contains(&ConceptArchetype::VisualConflictDiagram));
}

#[test]
fn test_teleprompter_renders_packaging_briefs() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond Build: Winterizing and Ice Defense".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // Verifies the rendered teleprompter markdown contains the PRE-PRODUCTION PACKAGING BRIEFS section
    assert!(script.markdown_teleprompter.contains("🎨 PRE-PRODUCTION PACKAGING BRIEFS (4 Extreme Opposites to Shoot on Set)"));
    assert!(script.markdown_teleprompter.contains("Peak Moment Sentence"));
    assert!(script.markdown_teleprompter.contains(&script.moment_sentence));

    // Verify all 4 archetype badges are rendered in teleprompter
    assert!(script.markdown_teleprompter.contains("🅰️ [REACTION & GAZE CUE]"));
    assert!(script.markdown_teleprompter.contains("🅱️ [MINIMALIST / BEFORE-AFTER]"));
    assert!(script.markdown_teleprompter.contains("🅲 [WIDE CINEMATIC / SCALE]"));
    assert!(script.markdown_teleprompter.contains("🅳 [VISUAL CONFLICT / DIAGRAM]"));
}

#[test]
fn test_moment_sentence_and_packaging_briefs_invariants() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond Build: Winterizing and Ice Defense".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    // 1. Verify synthesize_script
    let script_synth = ScriptGenerator::synthesize_script(&req, &[]);
    assert!(!script_synth.moment_sentence.trim().is_empty());
    assert_eq!(script_synth.divergent_thumbnail_concepts.len(), 4);

    let mut synth_archetypes = std::collections::HashSet::new();
    for concept in &script_synth.divergent_thumbnail_concepts {
        synth_archetypes.insert(concept.archetype);
        let word_count = concept.text_overlay.split_whitespace().count();
        assert!(
            word_count <= 4,
            "Concept '{}' text_overlay has {} words (> 4 words): \"{}\"",
            concept.concept_name, word_count, concept.text_overlay
        );
        assert!(!concept.shoot_on_set_checklist.trim().is_empty());
    }
    assert_eq!(synth_archetypes.len(), 4);
    assert!(synth_archetypes.contains(&ConceptArchetype::ReactionGazeCue));
    assert!(synth_archetypes.contains(&ConceptArchetype::MinimalistBeforeAfter));
    assert!(synth_archetypes.contains(&ConceptArchetype::WideCinematicMap));
    assert!(synth_archetypes.contains(&ConceptArchetype::VisualConflictDiagram));

    // 2. Verify generate_local_fallback_script
    let script_fallback = ScriptGenerator::generate_local_fallback_script(&req, &[]);
    assert!(!script_fallback.moment_sentence.trim().is_empty());
    assert_eq!(script_fallback.divergent_thumbnail_concepts.len(), 4);

    let mut fallback_archetypes = std::collections::HashSet::new();
    for concept in &script_fallback.divergent_thumbnail_concepts {
        fallback_archetypes.insert(concept.archetype);
        let word_count = concept.text_overlay.split_whitespace().count();
        assert!(
            word_count <= 4,
            "Concept '{}' text_overlay has {} words (> 4 words): \"{}\"",
            concept.concept_name, word_count, concept.text_overlay
        );
        assert!(!concept.shoot_on_set_checklist.trim().is_empty());
    }
    assert_eq!(fallback_archetypes.len(), 4);
    assert!(fallback_archetypes.contains(&ConceptArchetype::ReactionGazeCue));
    assert!(fallback_archetypes.contains(&ConceptArchetype::MinimalistBeforeAfter));
    assert!(fallback_archetypes.contains(&ConceptArchetype::WideCinematicMap));
    assert!(fallback_archetypes.contains(&ConceptArchetype::VisualConflictDiagram));
}

#[test]
fn test_early_progression_cadence_enforces_two_milestones() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond: 30 Days of Winter Ice Defense".to_string(),
        target_duration_sec: 48,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // Verifies a generated script contains >= 2 micro-milestones inside the target progression window.
    assert!(script.early_progression.milestone_count >= 2);
    assert_eq!(script.early_progression.completed_milestones.len(), script.early_progression.milestone_count);
    assert!(script.early_progression.meets_velocity_target);

    // Verify milestones are within window
    for ms in &script.early_progression.completed_milestones {
        let _ms_ref: &MicroMilestone = ms;
        assert!(ms.timecode_seconds >= script.early_progression.evaluation_window_start_sec);
        assert!(ms.timecode_seconds <= script.early_progression.evaluation_window_end_sec);
        assert!(!ms.achievement_description.is_empty());
        assert!(!ms.progression_cue.is_empty());
    }
}

#[test]
fn test_chronological_drag_triggers_progression_warning() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Slow Paced Documentary".to_string(),
        target_duration_sec: 48,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let mut script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // Artificially clear all micro-milestones except 1 in the early window
    let mut kept_one = false;
    for beat in &mut script.beats {
        if beat.micro_milestone.is_some() {
            if !kept_one {
                kept_one = true;
            } else {
                beat.micro_milestone = None;
            }
        }
    }

    script.enforce_pacing_invariants(48.0, 2.4);

    assert_eq!(script.early_progression.milestone_count, 1);
    assert!(!script.early_progression.meets_velocity_target);
    assert!(script.early_progression.pacing_diagnostic.contains("Chronological Drag Warning"));
    assert!(script.early_progression.pacing_diagnostic.contains("Only 1 milestone completed"));
}

#[test]
fn test_teleprompter_renders_milestone_flags() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond: 30 Days of Winter Ice Defense".to_string(),
        target_duration_sec: 48,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // Verifies 🚩 [EARLY MILESTONE tags appear in teleprompter markdown output.
    assert!(script.markdown_teleprompter.contains("🚩 [EARLY MILESTONE #1:"));
    assert!(script.markdown_teleprompter.contains("🚩 [EARLY MILESTONE #2:"));
    assert!(script.markdown_teleprompter.contains("⚡ EARLY PROGRESSION CADENCE:"));
}

#[test]
fn test_script_generator_partitions_stair_stepping_tiers() {
    let req = ScriptRequest {
        channel_handle: "@challenge_king".to_string(),
        topic: "$1 vs $1,000,000 Camping Gear".to_string(),
        target_duration_sec: 60,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // 1. Verify stair-stepping format detected and progression attached
    assert!(script.stair_stepping.is_some());
    let ss = script.stair_stepping.as_ref().unwrap();
    assert!(ss.is_stair_stepping);
    assert_eq!(ss.detected_tiers.len(), 4);
    assert!(ss.has_monotonic_escalation);
    assert!(ss.climax_held_for_finale);

    // 2. Verify beats are grouped into 4 escalating tiers
    let tiered_beats: Vec<_> = script.beats.iter().filter(|b| b.stair_step_tier.is_some()).collect();
    assert!(!tiered_beats.is_empty());

    let mut seen_indices = Vec::new();
    for b in &tiered_beats {
        let tier = b.stair_step_tier.as_ref().unwrap();
        if !seen_indices.contains(&tier.tier_index) {
            seen_indices.push(tier.tier_index);
        }
    }
    assert_eq!(seen_indices, vec![1, 2, 3, 4]);

    // 3. Confirm Tier 4 beats are held for the final 20% of beats (start_sec >= 0.80 * total_duration_sec)
    let tier4_beats: Vec<_> = script
        .beats
        .iter()
        .filter(|b| b.stair_step_tier.as_ref().map(|t| t.tier_index == 4).unwrap_or(false))
        .collect();

    assert!(!tier4_beats.is_empty(), "Tier 4 beats must not be empty");
    for beat in &tier4_beats {
        assert!(
            beat.start_sec >= 0.80 * script.total_duration_sec,
            "Tier 4 beat at {:.1}s was not placed in final 20% (>= {:.1}s)",
            beat.start_sec,
            0.80 * script.total_duration_sec
        );
        let tier = beat.stair_step_tier.as_ref().unwrap();
        assert!(tier.is_climax);
    }

    // 4. Verify teleprompter markdown output contains stair-stepping tags
    assert!(script.markdown_teleprompter.contains("🪜 STAIR-STEPPING PROGRESSION:"));
    assert!(script.markdown_teleprompter.contains("🪜 [TIER #1:"));
    assert!(script.markdown_teleprompter.contains("🪜 [TIER #4:"));
}


