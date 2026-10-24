use sandsifter::export::{FlightSheet, RationalTime};
use sandsifter::math::{MarginalGainsEngine, PunchInCue};
use sandsifter::pipeline::NicheToneProfile;

#[test]
fn test_cuttable_fat_aggregation_math() {
    let total_duration_sec = 65.0; // 01:05
    let t_silence_sec = 1.2; // Excess cold open: 1.2 - 0.3 = 0.9s

    // Cuts array with two shots exceeding the 7.0s panic/fast benchmark (18.0s and 22.0s)
    let cuts = vec![5.0, 23.0, 30.0, 52.0, 58.0];
    // Breath pauses: two pauses > 0.4s (0.8s and 1.1s)
    let breath_pauses = vec![0.2, 0.8, 0.35, 1.1, 0.2];

    let fat = MarginalGainsEngine::calculate_cuttable_fat(
        total_duration_sec,
        t_silence_sec,
        Some(NicheToneProfile::HighStakesPanic),
        &cuts,
        &breath_pauses,
    );

    // Breath excess: (0.8 - 0.25) + (1.1 - 0.25) = 0.55 + 0.85 = 1.4s
    assert_eq!(fat.dead_air_breath_seconds, 1.4);

    // Cold open excess: 1.2 - 0.3 = 0.9s
    assert_eq!(fat.cold_open_lag_seconds, 0.9);

    // Excess holds:
    // Shot 2: 5.0 to 23.0 (18.0s) -> 18.0 - 7.0 = 11.0s excess
    // Shot 4: 30.0 to 52.0 (22.0s) -> 22.0 - 7.0 = 15.0s excess
    // Total camera hold excess: 11.0 + 15.0 = 26.0s
    assert_eq!(fat.camera_hold_fat_seconds, 26.0);

    // Punch in cues should have exactly 2 cues for the two extended holds
    assert_eq!(fat.punch_in_cues.len(), 2);
    assert_eq!(fat.punch_in_cues[0].crop_scale_pct, 115);
    assert_eq!(fat.punch_in_cues[0].hold_duration_seconds, 18.0);
    // Midpoint of Shot 2: 5.0 + 9.0 = 14.0s (00:14)
    assert_eq!(fat.punch_in_cues[0].timecode_seconds, 14.0);
    assert_eq!(fat.punch_in_cues[0].timecode, "00:14");

    // Total cuttable: 0.9 + 1.4 + (26.0 * 0.4) = 2.3 + 10.4 = 12.7s
    assert_eq!(fat.total_cuttable_seconds, 12.7);

    // Micro edits count: 2 breaths + 1 cold open + 2 punch ins = 5 micro edits
    assert_eq!(fat.micro_edits_count, 5);

    // Executive summary checks
    assert!(fat.executive_summary.contains("You can shave 13s across 5 micro-edits (01:05 -> 00:52, -19.5%) without deleting story beats."));
}

#[test]
fn test_midpoint_punch_in_cue_generation() {
    let cues = vec![
        PunchInCue {
            timecode: "02:15".to_string(),
            timecode_seconds: 135.0,
            hold_duration_seconds: 16.5,
            crop_scale_pct: 115,
            edit_instruction: "Shot 03 static hold (16.5s exceeds 7.0s limit). Apply 115% digital punch-in or cut to alternate angle at midpoint to reset visual decay.".to_string(),
        },
    ];

    let mut timeline = sandsifter::export::ExportTimeline::new_landscape("Test Video", 300.0);
    timeline.shots.push(sandsifter::export::ShotClipPlaceholder {
        shot_index: 1,
        offset: RationalTime::zero(),
        duration: timeline.duration,
        label: "Master Shot".to_string(),
        markers: Vec::new(),
    });

    timeline.inject_punch_in_cues(&cues);

    assert_eq!(timeline.shots[0].markers.len(), 1);
    let marker = &timeline.shots[0].markers[0];
    assert_eq!(marker.name, "[PUNCH-IN 115%]");
    assert_eq!(marker.color, sandsifter::export::MarkerColor::Cyan);
    assert_eq!(marker.start.to_seconds(), 135.0);
    assert!(marker.note.contains("16.5s exceeds 7.0s limit"));

    // Check FCPXML and EDL output
    let fcpxml = timeline.to_fcpxml_string();
    assert!(fcpxml.contains("value=\"[PUNCH-IN 115%]\""));
    assert!(fcpxml.contains("135s"));

    let edl = timeline.to_edl_string();
    assert!(edl.contains("* MARKER: [PUNCH-IN 115%] @ 00:02:15:00"));
    assert!(edl.contains("|C:ResolveColorCyan"));
}

#[test]
fn test_flight_sheet_renders_cuttable_fat_budget() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard(
        "test_vid",
        "Van Life Solo Alaska Journey",
        "Outdoor Travel",
        0.8,
        4.2,
        15,
        14.0,
        160.0,
        None,
    );

    let cuts = vec![10.0, 35.0, 70.0, 120.0];
    let breath_pauses = vec![0.5, 0.9, 0.6, 1.2];
    let fat = MarginalGainsEngine::calculate_cuttable_fat(
        600.0,
        0.8,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        &cuts,
        &breath_pauses,
    );
    scorecard.cuttable_fat = Some(fat);

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let term = flight_sheet.render_terminal_string();
    let md = flight_sheet.to_markdown_string();

    // Verify terminal output
    assert!(term.contains("CUMULATIVE CUTTABLE FAT & RE-TIMING BUDGET"));
    assert!(term.contains("Reclaimable Runtime"));
    assert!(term.contains("115% Punch-In Cues"));

    // Verify markdown output
    assert!(md.contains("## ✂️ Cumulative Cuttable Fat & Re-Timing Budget"));
    assert!(md.contains("Midpoint 115% Digital Punch-In Markers"));
    assert!(md.contains("115%"));

    // Test timeline generated from FlightSheet contains punch-in markers
    let timeline = flight_sheet.to_export_timeline();
    let edl = timeline.to_edl_string();
    assert!(edl.contains("[PUNCH-IN 115%]"));
    assert!(edl.contains("|C:ResolveColorCyan"));

    // Verify zero LaTeX invariant
    assert!(!md.contains("$\\le"));
    assert!(!md.contains("$\\ge"));
    assert!(!md.contains("\\text{"));
    assert!(!md.contains("\\%"));
}
