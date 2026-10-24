use sandsifter::math::retention::{
    HeatmapPoint, ReplayRootCause, RetentionEngine, RetentionEventType, VttCue,
};
use sandsifter::models::video::{VideoChannelDetails, VideoDetail};
use sandsifter::pipeline::hook_machine::{GrammaticalTopic, HookMachineEngine, NicheToneProfile};
use sandsifter::pipeline::script_generator::{
    BeatType, CreatorIdentityContext, ScriptBeat, ScriptGenerator, ScriptRequest,
};

#[test]
fn test_fix1_visual_payoff_silence_on_silent_replay_spike() {
    // Ep 2 scenario: 35:29 has a +52.2% replay spike with zero active speech
    let heatmap = vec![
        HeatmapPoint { start_time: 2110.0, end_time: 2120.0, value: 0.25 },
        HeatmapPoint { start_time: 2129.0, end_time: 2139.0, value: 0.772 }, // +0.522 surge at 35:29!
        HeatmapPoint { start_time: 2140.0, end_time: 2150.0, value: 0.70 },
    ];

    // No cues around 2129s (35:29)
    let cues = vec![
        VttCue {
            start_sec: 2000.0,
            end_sec: 2010.0,
            text: "We are packing up our tools for the evening.".to_string(),
        },
    ];

    let report = RetentionEngine::analyze_curve(
        "WZx_TTeAOS8",
        "Major PROBLEM for Our Swim Pond... can cheap excavator save it?",
        2289.0,
        &heatmap,
        &cues,
    );

    // 1. Verify Replay Spike is detected
    assert_eq!(report.major_spikes.len(), 1);
    let spike = &report.major_spikes[0];
    assert_eq!(spike.formatted_timestamp, "35:29");
    assert_eq!(spike.event_type, RetentionEventType::ReplaySpike);
    assert!((spike.delta_intensity - 0.522).abs() < 0.001);

    // 2. Verify it is classified as VisualPayoffSilence, NOT DeadAir
    assert_eq!(spike.replay_root_cause, Some(ReplayRootCause::VisualPayoffSilence));
    assert!(
        spike.tactical_diagnosis.contains("Visual Payoff / Satisfying Ambient Reveal"),
        "Diagnosis did not contain Visual Payoff: {}",
        spike.tactical_diagnosis
    );
    assert!(
        spike.tactical_diagnosis.contains("do NOT cut or talk over this climax"),
        "Diagnosis did not protect climax: {}",
        spike.tactical_diagnosis
    );

    // 3. Verify editing prescription protects the visual climax
    let prescription = RetentionEngine::generate_default_editing_prescription(&report.attention_breaches, Some(spike));
    let spike_bullet = prescription.last().unwrap();
    assert!(
        spike_bullet.contains("Protect the 35:29 Visual Climax"),
        "Prescription did not protect the climax: {}",
        spike_bullet
    );
    assert!(
        spike_bullet.contains("Preserve the natural audio/music and give the visual payoff breathing room"),
        "Prescription did not advise breathing room: {}",
        spike_bullet
    );
    assert!(
        !spike_bullet.to_lowercase().contains("edit this silent gap out entirely"),
        "Prescription instructed deleting the spike!"
    );
}

#[test]
fn test_fix2_niche_tone_profiles_and_grammatical_coherence() {
    let topic = "Natural Backyard Swim Pond Build";
    let channel = "@Vanwives";
    let context = "Building a massive natural swim pond with cheap excavator and old dump truck on off-grid homestead";

    // 1. Tone auto-detection
    let tone = NicheToneProfile::detect(topic, channel, context);
    assert_eq!(tone, NicheToneProfile::DocumentaryBuildProcess);
    assert_eq!(tone.label(), "🚜 Documentary Build & Transformation");

    // 2. Grammatical topic parsing
    let gram = GrammaticalTopic::parse(topic);
    assert_eq!(gram.clean_topic, "Natural Backyard Swim Pond");
    assert_eq!(gram.noun_phrase, "a natural backyard swim pond");
    assert_eq!(gram.activity_phrase, "building a natural backyard swim pond");

    // 3. Generate hooks and verify no broken slot-stuffing
    let mock_video = VideoDetail {
        uuid: "v1".to_string(),
        caption: Some("Digging MASSIVE SWIM POND with Cheap Excavator".to_string()),
        url: "https://youtube.com/watch?v=v1".to_string(),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: "c1".to_string(),
            platform: "youtube".to_string(),
            handle: "@Vanwives".to_string(),
            display_name: Some("Vanwives".to_string()),
            avatar_url: None,
            follower_count: Some(400_000),
            baseline_median_views: Some(480_000),
        },
        view_count: 677_000,
        like_count: Some(29_000),
        comment_count: Some(1_100),
        share_count: Some(500),
        outlier_score: 1.4,
        engagement_rate: 4.5,
        published_at: "2026-03-01T12:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    };

    let report = HookMachineEngine::execute("@Vanwives", topic, &[mock_video]);
    assert_eq!(report.tone_profile, NicheToneProfile::DocumentaryBuildProcess);

    // Ensure all 10 hooks have valid grammar and optimal word count
    let all_hooks: Vec<_> = report.adapted_hooks.iter().chain(report.scratch_hooks.iter()).collect();
    for hook in &all_hooks {
        let text = &hook.hook_text;
        let words: Vec<&str> = text.split_whitespace().collect();

        // Must NOT contain broken slot-stuffing artifacts
        assert!(
            !text.contains("without excavate"),
            "Hook contained broken slot-stuffing: {}",
            text
        );
        assert!(
            !text.contains("fatal Natural Backyard Swim Pond Build"),
            "Hook contained raw ungrammatical topic injection: {}",
            text
        );
        assert!(
            !text.contains("Stop managing Natural"),
            "Hook contained mismatched corporate template: {}",
            text
        );

        // Word count must be in the golden 9-14 words range
        assert!(
            (9..=14).contains(&words.len()),
            "Hook word count was {} (expected 9-14 words): \"{}\"",
            words.len(),
            text
        );

        // Total score must be top-tier
        assert!(
            hook.score.total_score >= 8.0,
            "Hook score was {:.1}, expected >= 8.0: {}",
            hook.score.total_score,
            text
        );
    }
}

#[test]
fn test_fix3_creator_identity_grounding_zero_hallucinated_personas() {
    let channel = "@Vanwives";
    let topic = "Natural Backyard Swim Pond Build";

    // 1. Infer identity
    let identity = CreatorIdentityContext::infer(channel, topic, None, None);
    assert_eq!(identity.host_names, vec!["Jaz".to_string(), "Crystal".to_string()]);
    assert!(identity.setting_context.contains("Canadian homestead"));
    assert_eq!(identity.tone_profile, NicheToneProfile::DocumentaryBuildProcess);

    // 2. Generate script with inferred identity
    let req = ScriptRequest {
        channel_handle: channel.to_string(),
        topic: topic.to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: Some(identity),
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    // 3. Verify zero hallucinated personas and zero geographic leaks
    for beat in &script.beats {
        let visual = &beat.visual_directive;
        let spoken = &beat.spoken_dialogue;

        assert!(
            !visual.contains("Dave"),
            "Visual directive hallucinated 'Dave': {}",
            visual
        );
        assert!(
            !spoken.contains("Dave"),
            "Spoken dialogue hallucinated 'Dave': {}",
            spoken
        );
        assert!(
            !spoken.to_lowercase().contains("mexico"),
            "Spoken dialogue leaked 'Mexico' into Canadian pond build: {}",
            spoken
        );
        assert!(
            !visual.to_lowercase().contains("mexico"),
            "Visual directive leaked 'Mexico' into Canadian pond build: {}",
            visual
        );
    }

    // Verify hosts Jaz and Crystal are referenced
    let mentions_hosts = script.beats.iter().any(|b| {
        b.visual_directive.contains("Jaz") || b.visual_directive.contains("Crystal")
    });
    assert!(mentions_hosts, "Script visual directives did not reference known hosts Jaz/Crystal");
}

#[test]
fn test_fix4_per_beat_wpm_ceiling_enforcement() {
    let mut script = ScriptGenerator::generate_heuristic_script(&ScriptRequest {
        channel_handle: "@Vanwives".to_string(),
        topic: "Natural Backyard Swim Pond Build".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    }, &[]);

    // Deliberately over-stuff Beat 5 with 15 words (375 WPM in 2.4s!)
    script.beats[5] = ScriptBeat {
        beat_index: 5,
        start_sec: 12.0,
        end_sec: 14.4,
        beat_type: BeatType::BodyProgression,
        spoken_dialogue: "We are installing this massive heavy commercial grade pond liner that can never be replaced easily".to_string(),
        word_count: 15,
        computed_wpm: 375.0,
        visual_directive: "[CUT B-ROLL HERE: Rolling out liner]".to_string(),
        on_screen_text: None,
        four_p_role: None,
        fulfilled_criterion_index: None,
        micro_milestone: None,
        stair_step_tier: None,
    };

    // Run enforcement pass
    script.enforce_pacing_invariants(36.0, 2.4);

    // 1. Verify Beat 5 was trimmed to <= 9 words and <= 225.0 WPM
    let b5 = &script.beats[5];
    assert!(
        b5.word_count <= 9,
        "Beat 5 word count was {} (expected <= 9 words): \"{}\"",
        b5.word_count,
        b5.spoken_dialogue
    );
    assert!(
        b5.computed_wpm <= 225.05,
        "Beat 5 WPM was {:.1} (expected <= 225.0 WPM)",
        b5.computed_wpm
    );

    // 2. Verify EVERY individual beat in the script satisfies <= 225.0 WPM
    for beat in &script.beats {
        assert!(
            beat.word_count <= 9,
            "Beat {} had {} words (ceiling is 9): \"{}\"",
            beat.beat_index,
            beat.word_count,
            beat.spoken_dialogue
        );
        assert!(
            beat.computed_wpm <= 225.05,
            "Beat {} had {:.1} WPM (ceiling is 225.0 WPM): \"{}\"",
            beat.beat_index,
            beat.computed_wpm,
            beat.spoken_dialogue
        );
    }

    // 3. Verify compliance passed
    assert!(script.pacing_compliance_passed, "Script failed pacing compliance!");
}
