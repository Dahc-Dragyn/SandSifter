use sandsifter::pipeline::hook_machine::{HookMachineEngine, NicheToneProfile};

#[test]
fn test_stair_stepping_format_detection_and_escalation() {
    // 1. Detection checks across title archetypes
    let t1 = "$1 vs $500,000 Experiences!";
    let t2 = "10 Mins vs 100 Hours of Woodworking";
    let t3 = "Turning $100 into $1,000 Flipping Motors";

    assert!(HookMachineEngine::detect_stair_stepping_format(t1), "Failed to detect: {}", t1);
    assert!(HookMachineEngine::detect_stair_stepping_format(t2), "Failed to detect: {}", t2);
    assert!(HookMachineEngine::detect_stair_stepping_format(t3), "Failed to detect: {}", t3);

    // Negative control
    assert!(!HookMachineEngine::detect_stair_stepping_format("How to Build a Backyard Swim Pond"));

    // 2. Escalation check
    let prog1 = HookMachineEngine::evaluate_stair_stepping_integrity(t1, 1060.0, &[]);
    assert!(prog1.is_stair_stepping);
    assert!(prog1.has_monotonic_escalation, "Expected monotonic escalation for t1");
    assert_eq!(prog1.detected_tiers.len(), 5);

    let prog2 = HookMachineEngine::evaluate_stair_stepping_integrity(t2, 600.0, &[]);
    assert!(prog2.is_stair_stepping);
    assert!(prog2.has_monotonic_escalation, "Expected monotonic escalation for t2");

    let prog3 = HookMachineEngine::evaluate_stair_stepping_integrity(t3, 600.0, &[]);
    assert!(prog3.is_stair_stepping);
    assert!(prog3.has_monotonic_escalation, "Expected monotonic escalation for t3");
}

#[test]
fn test_premature_climax_triggers_retention_warning() {
    // Construct a sequence where the peak $500k tier occurs at minute 3 of a 10-minute (600s) video
    let title = "$1 vs $500,000 Experiences!";
    let duration_sec = 600.0;
    let chapters = vec![
        "00:30 - $1 Experience".to_string(),
        "01:00 - $10k Experience".to_string(),
        "03:00 - $500k Climax Experience".to_string(),
    ];

    let prog = HookMachineEngine::evaluate_stair_stepping_integrity(title, duration_sec, &chapters);

    assert!(prog.is_stair_stepping);
    assert!(!prog.climax_held_for_finale, "Climax at 03:00 of 10:00 must NOT be held for finale");
    assert!(
        prog.pacing_diagnostic.contains("🚨 Premature Climax Breach: Highest-stakes payoff revealed too early at 03:00 (30% into video)."),
        "Diagnostic was: {}",
        prog.pacing_diagnostic
    );
    assert!(
        prog.pacing_diagnostic.contains("destroying back-half retention."),
        "Diagnostic was: {}",
        prog.pacing_diagnostic
    );
}

#[test]
fn test_niche_tone_detects_conversational_podcast() {
    let t1 = "Versus Wolves Episode 14: The Return";
    let t2 = "Deep Dive Interview with Lead Architect";
    let t3 = "AI Roundtable Podcast #42";

    assert_eq!(
        NicheToneProfile::detect(t1, "", ""),
        NicheToneProfile::ConversationalPodcast,
        "Failed to route t1 to ConversationalPodcast"
    );
    assert_eq!(
        NicheToneProfile::detect(t2, "", ""),
        NicheToneProfile::ConversationalPodcast,
        "Failed to route t2 to ConversationalPodcast"
    );
    assert_eq!(
        NicheToneProfile::detect(t3, "", ""),
        NicheToneProfile::ConversationalPodcast,
        "Failed to route t3 to ConversationalPodcast"
    );
}
