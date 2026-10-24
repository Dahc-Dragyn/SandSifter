use sandsifter::ingest::crawler::{NicheOutlierScout, NicheScoutVideoItem};
use sandsifter::math::forensics::ForensicsEngine;
use sandsifter::math::outlier::OutlierEngine;
use sandsifter::math::retention::{
    BreachRootCause, HeatmapPoint, RetentionEngine, VttCue,
};

// =========================================================================
// Task 1: 50x Quota-Engineered Ingestion Tests
// =========================================================================

#[test]
fn test_niche_scout_batch_chunking_50_ids() {
    let ids: Vec<String> = (1..=123).map(|i| format!("video_{:03}", i)).collect();
    let chunks = NicheOutlierScout::chunk_video_ids(&ids);

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].len(), 50);
    assert_eq!(chunks[1].len(), 50);
    assert_eq!(chunks[2].len(), 23);
    assert_eq!(chunks[0][0], "video_001");
    assert_eq!(chunks[2][22], "video_123");
}

#[test]
fn test_niche_scout_48h_velocity_derivative() {
    // 12,000 views in 24 hours -> 500 views/hour
    let v_24h = NicheOutlierScout::calculate_48h_velocity(12_000, 24.0);
    assert!((v_24h - 500.0).abs() < 1e-1);
    assert!(NicheOutlierScout::is_48h_breakout(v_24h, 250.0));

    // 1,000 views in 10 hours -> 100 views/hour
    let v_10h = NicheOutlierScout::calculate_48h_velocity(1_000, 10.0);
    assert!((v_10h - 100.0).abs() < 1e-1);
    assert!(!NicheOutlierScout::is_48h_breakout(v_10h, 250.0));

    // Zero hours elapsed guard (clamped to 0.1h to prevent division-by-zero)
    let v_zero = NicheOutlierScout::calculate_48h_velocity(500, 0.0);
    assert!(v_zero > 0.0);

    // Beyond 48h (clamped to 48h ceiling)
    let v_72h = NicheOutlierScout::calculate_48h_velocity(48_000, 72.0);
    assert!((v_72h - 1000.0).abs() < 1e-1);
}

#[test]
fn test_niche_scout_batch_json_parsing() {
    let sample_json = r#"
{"id": "vid1", "title": "Viral Hook Breakdown", "view_count": 24000, "upload_date": "20261011"}
{"id": "vid2", "title": "Low Pace Vlog", "view_count": 500, "upload_date": "20261011"}
"#;

    let items: Vec<NicheScoutVideoItem> =
        NicheOutlierScout::parse_batch_json(sample_json, "@growthhacker", 250.0);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].video_id, "vid1");
    assert_eq!(items[0].views, 24000);
    assert!(items[0].is_48h_breakout); // 24000 / 24h = 1000 views/hr >= 250

    assert_eq!(items[1].video_id, "vid2");
    assert_eq!(items[1].views, 500);
    assert!(!items[1].is_48h_breakout); // 500 / 24h = 20.8 views/hr < 250
}

// =========================================================================
// Task 2: Velocity-Normalized Outliers (M_norm / CVNOS) Tests
// =========================================================================

#[test]
fn test_cvnos_neutral_momentum() {
    let views = 50_000;
    let baseline = 10_000;
    // Expected steady state: V_7 / V_30 is ~7/30 (approx 0.2333)
    let v_7 = 70_000;
    let v_30 = 300_000;

    let m_norm = OutlierEngine::calculate_m_norm(views, baseline, v_7, v_30, 0.25);
    // Raw multiplier is 5.0x
    let raw = OutlierEngine::calculate_multiplier(views, baseline);
    assert!((raw - 5.0).abs() < 1e-6);

    // m_norm is calculated with momentum dampener
    assert!(m_norm > 0.0);
    assert_eq!(
        m_norm,
        ForensicsEngine::calculate_cvnos(views, baseline, v_7, v_30, 0.25)
    );
}

#[test]
fn test_cvnos_surging_channel_dampening() {
    let views = 100_000;
    let baseline = 20_000;

    // Surging channel: 80% of 30-day views arrived in the last 7 days!
    let v_7_surge = 800_000;
    let v_30_surge = 1_000_000;

    // Normalizing channel: Only 15% of 30-day views arrived in the last 7 days
    let v_7_calm = 150_000;
    let v_30_calm = 1_000_000;

    let m_surge = OutlierEngine::calculate_m_norm(views, baseline, v_7_surge, v_30_surge, 0.25);
    let m_calm = OutlierEngine::calculate_m_norm(views, baseline, v_7_calm, v_30_calm, 0.25);

    // Surging channel gets penalized/dampened because channel momentum is high,
    // whereas calm/dormant channel gets rewarded for a true content breakout!
    assert!(
        m_surge < m_calm,
        "Surging channel m_norm ({}) should be lower than calm channel m_norm ({})",
        m_surge,
        m_calm
    );
}

#[test]
fn test_cvnos_zero_or_missing_velocity_fallback() {
    let views = 30_000;
    let baseline = 10_000;

    // When 30d views are 0, fallback to standard multiplier (3.0x)
    let m_fallback = OutlierEngine::calculate_m_norm(views, baseline, 0, 0, 0.25);
    assert!((m_fallback - 3.0).abs() < 1e-2);

    // When baseline is 0, return 0.0 safely without panic
    let m_zero_base = OutlierEngine::calculate_m_norm(views, 0, 5000, 20000, 0.25);
    assert_eq!(m_zero_base, 0.0);
}

#[test]
fn test_evaluate_cvnos_metrics_breakout() {
    let metrics = OutlierEngine::evaluate_cvnos(100_000, 20_000, 70_000, 300_000, 0.25, 4.0);

    assert_eq!(metrics.v_video, 100_000);
    assert_eq!(metrics.baseline_bc, 20_000);
    assert!((metrics.raw_multiplier - 5.0).abs() < 1e-2);
    assert!(metrics.m_norm > 0.0);
    assert!(metrics.is_content_driven_breakout);
}

// =========================================================================
// Task 3: Retention Heatmap Calculus R'(t) Tests
// =========================================================================

#[test]
fn test_retention_derivative_central_difference() {
    // Construct linear ramp down: 1.0, 0.9, 0.8, 0.7, 0.6 (dt = 1.0s, drop = 0.1/s)
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 1.0, value: 1.0 },
        HeatmapPoint { start_time: 1.0, end_time: 2.0, value: 0.9 },
        HeatmapPoint { start_time: 2.0, end_time: 3.0, value: 0.8 },
        HeatmapPoint { start_time: 3.0, end_time: 4.0, value: 0.7 },
        HeatmapPoint { start_time: 4.0, end_time: 5.0, value: 0.6 },
    ];

    let derivatives = RetentionEngine::compute_retention_derivative(&heatmap);
    assert_eq!(derivatives.len(), 5);

    // Internal points should have R'(t) ~ -0.10
    for &(_t, r_prime) in &derivatives[1..4] {
        assert!((r_prime - (-0.10)).abs() < 1e-2);
    }

    let afi = RetentionEngine::calculate_attention_friction_index(&derivatives);
    assert!(afi > 0.05);
}

#[test]
fn test_attention_breach_multimodal_correlation_dead_air() {
    // Heatmap flat at 0.90 until a steep drop at t = 10.0s (drops from 0.90 to 0.50 in 1 second -> r_prime ~ -0.40/s)
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 2.0, value: 0.90 },
        HeatmapPoint { start_time: 2.0, end_time: 5.0, value: 0.90 },
        HeatmapPoint { start_time: 5.0, end_time: 8.0, value: 0.90 },
        HeatmapPoint { start_time: 8.0, end_time: 9.0, value: 0.90 },
        HeatmapPoint { start_time: 9.0, end_time: 11.0, value: 0.50 }, // Steep breach!
        HeatmapPoint { start_time: 11.0, end_time: 15.0, value: 0.48 },
    ];

    // Cues where speech stops at 7.0s and doesn't resume until 14.0s (Dead air of 7 seconds)
    let cues = vec![
        VttCue { start_sec: 0.0, end_sec: 7.0, text: "Hook setup dialogue here".to_string() },
        VttCue { start_sec: 14.0, end_sec: 18.0, text: "Next line begins here".to_string() },
    ];

    // Cuts are rapid (cut at 8.0s, cut at 9.5s) so visual pacing is NOT slow
    let cuts = vec![0.0, 3.0, 6.0, 8.0, 9.5, 12.0];

    let breaches = RetentionEngine::detect_attention_breaches(&heatmap, &cues, &cuts, 0.015);
    assert!(!breaches.is_empty());

    let dead_air_breach = breaches.iter().find(|b| matches!(b.root_cause, BreachRootCause::DeadAirSilence { .. }));
    assert!(dead_air_breach.is_some(), "Expected a DeadAirSilence breach to be detected");
    let breach = dead_air_breach.unwrap();
    assert!(breach.r_prime < -0.015);
    assert!(breach.tactical_remedy.contains("Dead Air"));
}

#[test]
fn test_attention_breach_multimodal_correlation_slow_visual_pacing() {
    // Heatmap with steep drop at t = 12.0s
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 4.0, value: 0.90 },
        HeatmapPoint { start_time: 4.0, end_time: 8.0, value: 0.85 },
        HeatmapPoint { start_time: 8.0, end_time: 11.0, value: 0.80 },
        HeatmapPoint { start_time: 11.0, end_time: 13.0, value: 0.50 }, // Steep breach!
        HeatmapPoint { start_time: 13.0, end_time: 16.0, value: 0.48 },
    ];

    // Continuous speech throughout (NO dead air)
    let cues = vec![
        VttCue { start_sec: 0.0, end_sec: 6.0, text: "Talking continuously without pause".to_string() },
        VttCue { start_sec: 6.0, end_sec: 15.0, text: "Explaining the entire backstory in one long monologue".to_string() },
    ];

    // But scene cut was back at 2.0s! The shot has been lingering for 10 seconds!
    let cuts = vec![0.0, 2.0];

    let breaches = RetentionEngine::detect_attention_breaches(&heatmap, &cues, &cuts, 0.015);
    assert!(!breaches.is_empty());

    let breach = &breaches[0];
    assert!(matches!(breach.root_cause, BreachRootCause::SlowVisualPacing { .. }));
    assert!(breach.tactical_remedy.contains("Tighten Visual Cuts"));
}

#[test]
fn test_analyze_curve_multimodal_report() {
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 2.0, value: 0.95 },
        HeatmapPoint { start_time: 2.0, end_time: 5.0, value: 0.90 },
        HeatmapPoint { start_time: 5.0, end_time: 8.0, value: 0.85 },
        HeatmapPoint { start_time: 8.0, end_time: 10.0, value: 0.50 },
        HeatmapPoint { start_time: 10.0, end_time: 15.0, value: 0.45 },
    ];
    let cues = vec![
        VttCue { start_sec: 0.0, end_sec: 4.0, text: "Opening dialogue".to_string() },
    ];
    let cuts = vec![0.0, 1.5];

    let report = RetentionEngine::analyze_curve_multimodal(
        "test_vid",
        "Test Viral Video",
        15.0,
        &heatmap,
        &cues,
        &cuts,
    );

    assert_eq!(report.video_id, "test_vid");
    assert!(report.attention_friction_index > 0.0);
    assert!(!report.attention_breaches.is_empty());
}
