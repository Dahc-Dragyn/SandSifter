use sandsifter::export::{ExportTimeline, MarkerColor, RationalTime};
use sandsifter::math::forensics::AcousticDeliveryMetrics;
use sandsifter::math::retention::{
    AttentionBreach, BreachRootCause, RetentionCurveReport, RetentionEvent, RetentionEventType,
};
use sandsifter::math::visual::{CutVelocityClassification, VisualPacingMetrics};

#[test]
fn test_rational_time_framerate_math() {
    // 29.97 fps NTSC (30000 / 1001)
    let rt_2997 = RationalTime::new(1001, 30000);
    assert_eq!(rt_2997.to_fcpxml_str(), "1001/30000s");
    assert!((rt_2997.to_seconds() - 0.0333666).abs() < 0.0001);

    // 24 fps Cinema
    let rt_24 = RationalTime::new(1, 24);
    assert_eq!(rt_24.to_fcpxml_str(), "1/24s");
    assert_eq!(rt_24.to_timecode_str(24.0), "00:00:00:01");

    // 60 fps
    let rt_60 = RationalTime::new(1, 60);
    assert_eq!(rt_60.to_fcpxml_str(), "1/60s");

    // Exact whole second reduction
    let rt_3s = RationalTime::new(90000, 30000);
    assert_eq!(rt_3s.to_fcpxml_str(), "3s");

    // Zero seconds
    let rt_0s = RationalTime::zero();
    assert_eq!(rt_0s.to_fcpxml_str(), "0s");
}

#[test]
fn test_asl_cuts_to_gap_spine_alignment() {
    let pacing = VisualPacingMetrics {
        duration_seconds: 10.0,
        total_scene_cuts: 3,
        average_shot_length_sec: 2.5,
        cut_classification: CutVelocityClassification::RapidFastPaced,
        cut_timestamps: vec![2.0, 4.5, 7.5],
        tactical_rule: "Fast pace".to_string(),
        streaming_dropouts_count: 0,
    };

    let timeline = ExportTimeline::from_forensics(
        "Pacing Test",
        10.0,
        true, // vertical
        Some(&pacing),
        None,
        None,
    );

    assert_eq!(timeline.shots.len(), 4);
    // Shot 1: 0.0 -> 2.0s
    assert_eq!(timeline.shots[0].offset.to_seconds(), 0.0);
    assert!((timeline.shots[0].duration.to_seconds() - 2.0).abs() < 0.01);

    // Shot 2: 2.0 -> 4.5s (2.5s)
    assert!((timeline.shots[1].offset.to_seconds() - 2.0).abs() < 0.01);
    assert!((timeline.shots[1].duration.to_seconds() - 2.5).abs() < 0.01);

    // Shot 3: 4.5 -> 7.5s (3.0s)
    assert!((timeline.shots[2].offset.to_seconds() - 4.5).abs() < 0.01);
    assert!((timeline.shots[2].duration.to_seconds() - 3.0).abs() < 0.01);

    // Shot 4: 7.5 -> 10.0s (2.5s)
    assert!((timeline.shots[3].offset.to_seconds() - 7.5).abs() < 0.01);
    assert!((timeline.shots[3].duration.to_seconds() - 2.5).abs() < 0.01);

    // Check FCPXML serialization for contiguous gaps
    let xml = timeline.to_fcpxml_string();
    assert!(xml.contains("<fcpxml version=\"1.8\">"));
    assert!(xml.contains("<format id=\"r1\" name=\"SandsifterForensicFormat\" frameDuration=\"1001/30000s\" width=\"1080\" height=\"1920\"/>"));
    assert!(xml.contains("<gap name=\"Shot 01 (2.0s)\" offset=\"0s\" duration=\"2s\" start=\"0s\"/>"));
}

#[test]
fn test_retention_markers_color_and_offsets() {
    let pacing = VisualPacingMetrics {
        duration_seconds: 15.0,
        total_scene_cuts: 2,
        average_shot_length_sec: 5.0,
        cut_classification: CutVelocityClassification::StandardNarrative,
        cut_timestamps: vec![5.0, 10.0],
        tactical_rule: "Standard".to_string(),
        streaming_dropouts_count: 0,
    };

    let retention = RetentionCurveReport {
        video_id: "test_vid".to_string(),
        title: "Test Retention".to_string(),
        duration_seconds: 15.0,
        average_intensity: 0.65,
        peak_timestamp_formatted: "00:07".to_string(),
        peak_transcript: "Look at this!".to_string(),
        major_dips: vec![RetentionEvent {
            timestamp_sec: 2.5,
            formatted_timestamp: "00:02".to_string(),
            intensity: 0.40,
            delta_intensity: -0.15,
            event_type: RetentionEventType::CatastrophicDrop,
            transcript_context: "So anyway guys...".to_string(),
            tactical_diagnosis: "Cut rambling intro".to_string(),
            replay_root_cause: None,
        }],
        major_spikes: vec![RetentionEvent {
            timestamp_sec: 7.2,
            formatted_timestamp: "00:07".to_string(),
            intensity: 0.85,
            delta_intensity: 0.20,
            event_type: RetentionEventType::ReplaySpike,
            transcript_context: "Huge wave hits!".to_string(),
            tactical_diagnosis: "Viral peak moment".to_string(),
            replay_root_cause: None,
        }],
        curve_health_verdict: "Strong".to_string(),
        tactical_rules: vec![],
        attention_friction_index: 0.05,
        attention_breaches: vec![],
        editing_prescription: vec![],
        re_engagement_checkpoints: vec![],
    };

    let timeline = ExportTimeline::from_forensics(
        "Retention Test",
        15.0,
        true,
        Some(&pacing),
        Some(&retention),
        None,
    );

    // Shot 1 (0 -> 5s) should contain the Red Dip at 2.5s
    let shot1_markers = &timeline.shots[0].markers;
    assert!(shot1_markers.iter().any(|m| m.color == MarkerColor::Red && m.name.contains("CRITICAL DROP")));

    // Shot 2 (5 -> 10s) should contain the Green Spike at 7.2s
    let shot2_markers = &timeline.shots[1].markers;
    assert!(shot2_markers.iter().any(|m| m.color == MarkerColor::Green && m.name.contains("Retention Spike")));

    let xml = timeline.to_fcpxml_string();
    assert!(xml.contains("CRITICAL DROP (-15.0%)"));
    assert!(xml.contains("Cut rambling intro"));
    assert!(xml.contains("Retention Spike (+20.0%)"));
    assert!(xml.contains("Viral peak moment"));
}

#[test]
fn test_acoustic_hook_markers_mapping() {
    let acoustic = AcousticDeliveryMetrics {
        pre_speech_silence_ms: 450,
        wpm_opening: 195.0,
        word_count_3_5s: 11,
        is_rapid_delivery: true,
        is_sluggish_delivery: false,
        t_silence_ms: Some(450),
        wpm_0_to_35: 195.0,
        is_non_speech: false,
        is_bumper_music_opening: false,
    };

    let timeline = ExportTimeline::from_forensics(
        "Acoustic Test",
        10.0,
        true,
        None,
        None,
        Some(&acoustic),
    );

    assert_eq!(timeline.shots.len(), 1);
    let markers = &timeline.shots[0].markers;
    assert!(markers.iter().any(|m| m.color == MarkerColor::Cyan && m.name.contains("Speech Onset (0.45s)")));
    assert!(markers.iter().any(|m| m.color == MarkerColor::Purple && m.name.contains("195 WPM")));
}

#[test]
fn test_davinci_resolve_schema_compliance() {
    let timeline = ExportTimeline::new_vertical_short("DaVinci Spec Test", 12.0);
    let xml = timeline.to_fcpxml_string();

    // Verify key FCPXML 1.8 tags expected by DaVinci Resolve import parser
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(xml.contains("<!DOCTYPE fcpxml>"));
    assert!(xml.contains("<fcpxml version=\"1.8\">"));
    assert!(xml.contains("<resources>"));
    assert!(xml.contains("<format id=\"r1\""));
    assert!(xml.contains("<library>"));
    assert!(xml.contains("<event name=\"Sandsifter Intelligence\">"));
    assert!(xml.contains("<project name=\"DaVinci Spec Test\">"));
    assert!(xml.contains("<sequence format=\"r1\""));
    assert!(xml.contains("<spine>"));
    assert!(xml.ends_with("</fcpxml>\n"));
}

#[test]
fn test_multimodal_attention_breach_root_cause_and_replay_spike_markers() {
    let pacing = VisualPacingMetrics {
        duration_seconds: 20.0,
        total_scene_cuts: 2,
        average_shot_length_sec: 6.6,
        cut_classification: CutVelocityClassification::StandardNarrative,
        cut_timestamps: vec![6.0, 14.0],
        tactical_rule: "Standard Narrative".to_string(),
        streaming_dropouts_count: 0,
    };

    let acoustic = AcousticDeliveryMetrics {
        pre_speech_silence_ms: 320,
        wpm_opening: 175.0,
        word_count_3_5s: 10,
        is_rapid_delivery: true,
        is_sluggish_delivery: false,
        t_silence_ms: Some(320),
        wpm_0_to_35: 175.0,
        is_non_speech: false,
        is_bumper_music_opening: false,
    };

    let retention = RetentionCurveReport {
        video_id: "breach_test".to_string(),
        title: "Breach Test Video".to_string(),
        duration_seconds: 20.0,
        average_intensity: 0.55,
        peak_timestamp_formatted: "00:15".to_string(),
        peak_transcript: "Awesome payoff!".to_string(),
        major_dips: vec![],
        major_spikes: vec![RetentionEvent {
            timestamp_sec: 15.0,
            formatted_timestamp: "00:15".to_string(),
            intensity: 0.90,
            delta_intensity: 0.35,
            event_type: RetentionEventType::ReplaySpike,
            transcript_context: "The lion roared!".to_string(),
            tactical_diagnosis: "Viral animal climax".to_string(),
            replay_root_cause: None,
        }],
        curve_health_verdict: "Strong".to_string(),
        tactical_rules: vec![],
        attention_friction_index: 0.08,
        attention_breaches: vec![AttentionBreach {
            timestamp_sec: 8.5,
            formatted_timestamp: "00:08".to_string(),
            intensity: 0.42,
            r_prime: -0.045, // -4.5%/sec drop
            root_cause: BreachRootCause::SlowVisualPacing { shot_length_sec: 8.5 },
            transcript_context: "Nothing is happening here...".to_string(),
            tactical_remedy: "Insert fast B-roll or dynamic angle change".to_string(),
        }],
        editing_prescription: vec![],
        re_engagement_checkpoints: vec![],
    };

    let timeline = ExportTimeline::from_forensics(
        "Breach Timeline Test",
        20.0,
        true,
        Some(&pacing),
        Some(&retention),
        Some(&acoustic),
    );

    // 1. Check cut spines
    assert_eq!(timeline.shots.len(), 3); // 0..6, 6..14, 14..20

    // 2. Shot 1 should have Cyan acoustic marker
    let shot1_markers = &timeline.shots[0].markers;
    assert!(shot1_markers.iter().any(|m| m.color == MarkerColor::Cyan && m.name.contains("Speech Onset (0.32s)")));

    // 3. Shot 2 (6..14s) should contain Red Attention Breach marker at 8.5s
    let shot2_markers = &timeline.shots[1].markers;
    let breach_marker = shot2_markers.iter().find(|m| m.color == MarkerColor::Red).expect("Expected red breach marker");
    assert!(breach_marker.name.contains("Attention Breach"));
    assert!(breach_marker.note.contains("Root Cause: 🎥 Visual Fatigue / Lingering Shot"));
    assert!(breach_marker.note.contains("Insert fast B-roll"));

    // 4. Shot 3 (14..20s) should contain Green Top Replay Spike marker at 15.0s
    let shot3_markers = &timeline.shots[2].markers;
    let spike_marker = shot3_markers.iter().find(|m| m.color == MarkerColor::Green).expect("Expected green spike marker");
    assert!(spike_marker.name.contains("Top Replay Spike"));
    assert!(spike_marker.note.contains("Viral animal climax"));

    // 5. XML Serialization Check
    let xml = timeline.to_fcpxml_string();
    assert!(xml.contains("Root Cause: &#127909; Visual Fatigue / Lingering Shot") || xml.contains("Visual Fatigue / Lingering Shot"));
    assert!(xml.contains("Top Replay Spike"));
    assert!(xml.contains("Speech Onset (0.32s)"));
}

