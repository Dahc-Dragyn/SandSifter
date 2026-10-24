use sandsifter::math::retention::VttCue;
use sandsifter::math::shorts::{ShortsDurationStatus, ShortsEngine};

#[test]
fn test_optimal_short_with_seamless_loop() {
    let cues = vec![
        VttCue {
            start_sec: 0.4,
            end_sec: 4.0,
            text: "This 200-year-old Japanese hand saw cuts wood like butter.".to_string(),
        },
        VttCue {
            start_sec: 4.1,
            end_sec: 15.0,
            text: "Notice how the teeth pull instead of push, giving you razor-sharp precision.".to_string(),
        },
        VttCue {
            start_sec: 15.2,
            end_sec: 28.5,
            text: "Master woodworkers rely on it for seamless timber joints, which is why...".to_string(),
        },
    ];

    let metrics = ShortsEngine::evaluate(&cues, 30.0, 0.4);

    assert_eq!(metrics.duration_seconds, 30.0);
    assert_eq!(metrics.first_frame_hook_delay, 0.4);
    assert!(metrics.first_frame_hook_safe);
    assert_eq!(metrics.duration_safety_status, ShortsDurationStatus::Optimal);
    assert!(metrics.duration_safety_status.is_optimal());
    assert!(metrics.loop_continuity_score >= 85.0);
    assert!(metrics.loop_seamless);
    assert_eq!(metrics.stay_vs_swipe_risk, "LOW");
    assert!(metrics.estimated_stay_ratio_pct >= 80.0);
    assert!(metrics.loop_bridge_insight.contains("Seamless syntactic connector"));
    assert!(metrics.executive_summary.contains("Elite Short Pacing"));
}

#[test]
fn test_delayed_hook_flags_high_swipe_risk() {
    let cues = vec![
        VttCue {
            start_sec: 2.5,
            end_sec: 7.0,
            text: "Hey everyone, today I'm going to show you a quick trick.".to_string(),
        },
        VttCue {
            start_sec: 7.2,
            end_sec: 20.0,
            text: "How to drill holes in hardened steel without dulling bits.".to_string(),
        },
    ];

    let metrics = ShortsEngine::evaluate(&cues, 35.0, 2.5);

    assert_eq!(metrics.first_frame_hook_delay, 2.5);
    assert!(!metrics.first_frame_hook_safe);
    assert_eq!(metrics.stay_vs_swipe_risk, "HIGH");
    assert!(metrics.estimated_stay_ratio_pct < 75.0);
    assert!(metrics.executive_summary.contains("High Swipe-Away Risk"));
}

#[test]
fn test_extended_and_truncation_risk_durations() {
    // 48 seconds -> Extended
    let metrics_extended = ShortsEngine::evaluate(&[], 48.0, 0.5);
    assert_eq!(metrics_extended.duration_safety_status, ShortsDurationStatus::Extended);
    assert!(!metrics_extended.duration_safety_status.is_optimal());

    // 59.5 seconds -> TruncationRisk
    let metrics_trunc = ShortsEngine::evaluate(&[], 59.5, 0.3);
    assert_eq!(metrics_trunc.duration_safety_status, ShortsDurationStatus::TruncationRisk);
    assert!(metrics_trunc.executive_summary.contains("Truncation Warning"));
}

#[test]
fn test_abrupt_outro_destroys_loop_continuity() {
    let cues = vec![
        VttCue {
            start_sec: 0.2,
            end_sec: 5.0,
            text: "Never make this rookie mistake with your chainsaw.".to_string(),
        },
        VttCue {
            start_sec: 5.2,
            end_sec: 22.0,
            text: "Always check your bar oil flow before touching the wood.".to_string(),
        },
        VttCue {
            start_sec: 22.2,
            end_sec: 27.0,
            text: "Thanks for watching! Make sure to like and subscribe for more.".to_string(),
        },
    ];

    let metrics = ShortsEngine::evaluate(&cues, 28.0, 0.2);

    assert!(metrics.loop_continuity_score <= 30.0);
    assert!(!metrics.loop_seamless);
    assert!(metrics.loop_bridge_insight.contains("Abrupt sign-off detected"));
    assert!(metrics.executive_summary.contains("Abrupt Outro Detected"));
}
