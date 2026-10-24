use sandsifter::analyzer::suitability::{
    SuitabilityAuditor, SuitabilityCategory, SuitabilitySeverity,
};
use sandsifter::math::retention::VttCue;

#[test]
fn test_clean_transcript_is_green_and_immediate_public() {
    let cues = vec![
        VttCue {
            start_sec: 0.2,
            end_sec: 3.5,
            text: "Welcome back to the cabin build in the deep woods.".to_string(),
        },
        VttCue {
            start_sec: 3.6,
            end_sec: 8.0,
            text: "Today we are framing the timber rafters before snow arrives.".to_string(),
        },
        VttCue {
            start_sec: 8.2,
            end_sec: 14.5,
            text: "Every log was harvested by hand using a two-man crosscut saw.".to_string(),
        },
        VttCue {
            start_sec: 15.1,
            end_sec: 25.0,
            text: "Let's notch the first joint and hoist it into position.".to_string(),
        },
    ];

    let report = SuitabilityAuditor::audit_cues(&cues);

    assert_eq!(report.rating, "GREEN");
    assert_eq!(report.recommended_workflow, "IMMEDIATE_PUBLIC");
    assert!(report.hook_window_safe);
    assert_eq!(report.violations_count, 0);
    assert_eq!(report.hook_violations_count, 0);
    assert!(report.violations.is_empty());
    assert!(report.executive_summary.contains("Monetization Safe"));
}

#[test]
fn test_hook_window_profanity_triggers_yellow_and_unlisted_review() {
    let cues = vec![
        VttCue {
            start_sec: 0.5,
            end_sec: 4.0,
            text: "Holy shit, look what just happened to the engine!".to_string(),
        },
        VttCue {
            start_sec: 4.2,
            end_sec: 9.0,
            text: "The piston completely seized up on the highway.".to_string(),
        },
    ];

    let report = SuitabilityAuditor::audit_cues(&cues);

    assert_eq!(report.rating, "YELLOW");
    assert_eq!(report.recommended_workflow, "UNLISTED_MANUAL_REVIEW");
    assert!(!report.hook_window_safe);
    assert_eq!(report.violations_count, 1);
    assert_eq!(report.hook_violations_count, 1);

    let violation = &report.violations[0];
    assert_eq!(violation.category, SuitabilityCategory::InappropriateLanguage);
    assert_eq!(violation.token, "shit");
    assert_eq!(violation.timestamp_sec, 0.5);
    assert!(violation.is_hook_window);
    assert_eq!(violation.severity, SuitabilitySeverity::Severe);
    assert!(report.executive_summary.contains("Advertiser Risk Detected"));
    assert!(report.executive_summary.contains("0:00–0:15 Hook Window"));
}

#[test]
fn test_minor_body_slang_after_hook_remains_green() {
    let cues = vec![
        VttCue {
            start_sec: 1.0,
            end_sec: 5.0,
            text: "In this episode, we rebuild the broken transmission from scratch.".to_string(),
        },
        VttCue {
            start_sec: 15.5,
            end_sec: 20.0,
            text: "Everything aligned cleanly on the first try.".to_string(),
        },
        VttCue {
            start_sec: 75.0,
            end_sec: 80.0,
            text: "This bolt was a stubborn bitch to loosen up, but we got it.".to_string(),
        },
    ];

    let report = SuitabilityAuditor::audit_cues(&cues);

    // Non-hook minor slang (t > 15s) does NOT crash the hook window
    assert_eq!(report.rating, "GREEN");
    assert_eq!(report.recommended_workflow, "IMMEDIATE_PUBLIC");
    assert!(report.hook_window_safe);
    assert_eq!(report.violations_count, 1);
    assert_eq!(report.hook_violations_count, 0);

    let violation = &report.violations[0];
    assert_eq!(violation.token, "bitch");
    assert_eq!(violation.timestamp_sec, 75.0);
    assert!(!violation.is_hook_window);
    assert_eq!(violation.severity, SuitabilitySeverity::Moderate);
    assert!(report.executive_summary.contains("Minor Body Slang"));
}

#[test]
fn test_harmful_content_triggers_yellow_even_after_hook() {
    let cues = vec![
        VttCue {
            start_sec: 2.0,
            end_sec: 6.0,
            text: "Welcome to our discussion on high-stress careers.".to_string(),
        },
        VttCue {
            start_sec: 45.0,
            end_sec: 50.0,
            text: "Many people in extreme burnout talk about wanting to commit suicide or self harm.".to_string(),
        },
    ];

    let report = SuitabilityAuditor::audit_cues(&cues);

    assert_eq!(report.rating, "YELLOW");
    assert_eq!(report.recommended_workflow, "UNLISTED_MANUAL_REVIEW");
    assert!(report.hook_window_safe); // Hook window itself had no flags
    assert_eq!(report.hook_violations_count, 0);
    assert!(report.violations_count >= 1);
    assert!(report.violations.iter().any(|v| v.severity == SuitabilitySeverity::Severe));
}
