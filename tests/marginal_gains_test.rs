use sandsifter::analyzer::PromiseGapReport;
use sandsifter::math::marginal_gains::{
    MarginalGainsEngine, PipelineStage, VectorStatus, ViewerSatisfactionMetrics,
};
use sandsifter::pipeline::{NicheToneProfile, ScriptGenerator};

#[test]
fn test_marginal_gains_cold_open_evaluation() {
    let elite = MarginalGainsEngine::evaluate_cold_open(0.1);
    assert_eq!(elite.status, VectorStatus::Elite);
    assert_eq!(elite.score_0_to_100, 100.0);
    assert_eq!(elite.estimated_retention_lift_pct, 0.0);

    let optimal = MarginalGainsEngine::evaluate_cold_open(0.4);
    assert_eq!(optimal.status, VectorStatus::Optimal);
    assert_eq!(optimal.score_0_to_100, 90.0);

    let drag = MarginalGainsEngine::evaluate_cold_open(1.2);
    assert_eq!(drag.status, VectorStatus::MarginalDrag);
    assert_eq!(drag.score_0_to_100, 65.0);

    let friction = MarginalGainsEngine::evaluate_cold_open(2.6);
    assert_eq!(friction.status, VectorStatus::CriticalFriction);
    assert_eq!(friction.score_0_to_100, 30.0);
    assert!(friction.estimated_retention_lift_pct >= 10.0);
}

#[test]
fn test_marginal_gains_asl_and_cuts_evaluation() {
    // Merged Vector 2: Opening 60s Visual Rhythm (ASL & Cuts)
    let elite_rhythm = MarginalGainsEngine::evaluate_visual_rhythm(1.6, 38);
    assert_eq!(elite_rhythm.status, VectorStatus::Elite);
    assert_eq!(elite_rhythm.score_0_to_100, 100.0);

    let slow_rhythm = MarginalGainsEngine::evaluate_visual_rhythm(4.6, 13);
    assert_eq!(slow_rhythm.status, VectorStatus::CriticalFriction);
    assert_eq!(slow_rhythm.score_0_to_100, 25.0);
    assert!(slow_rhythm.one_percent_fix.contains("Currently 4.6s/cut (13 cuts/min)"));
    assert!(slow_rhythm.one_percent_fix.contains("B-roll cutaways"));

    // Backward-compatible wrappers
    let fast_asl = MarginalGainsEngine::evaluate_asl(1.6);
    assert_eq!(fast_asl.status, VectorStatus::Elite);

    let low_cuts = MarginalGainsEngine::evaluate_cut_density(13);
    assert_eq!(low_cuts.status, VectorStatus::CriticalFriction);
}

#[test]
fn test_marginal_gains_static_hold_and_audio_density() {
    let brief_hold = MarginalGainsEngine::evaluate_static_hold(4.2);
    assert_eq!(brief_hold.status, VectorStatus::Elite);

    let long_freeze = MarginalGainsEngine::evaluate_static_hold(43.9);
    assert_eq!(long_freeze.status, VectorStatus::CriticalFriction);
    assert!(long_freeze.one_percent_fix.contains("punch-ins"));

    // Vector 5: Timeline Audio Density (Breath/Pause Trimming)
    // 150 - 220 WPM is Optimal green zone (no vocal changes needed)
    let natural_wpm = MarginalGainsEngine::evaluate_audio_density(195.0);
    assert_eq!(natural_wpm.status, VectorStatus::Optimal);
    assert_eq!(natural_wpm.score_0_to_100, 100.0);
    assert!(natural_wpm.one_percent_fix.contains("Natural conversational cadence (195 WPM)—no vocal changes needed"));

    // Sub-150 WPM: advice framed as NLE audio timeline pause trimming
    let slow_wpm = MarginalGainsEngine::evaluate_audio_density(137.0);
    assert_eq!(slow_wpm.status, VectorStatus::MarginalDrag);
    assert!(slow_wpm.one_percent_fix.contains("Do NOT try to talk faster on camera"));
    assert!(slow_wpm.one_percent_fix.contains("trim ~"));
    assert!(slow_wpm.one_percent_fix.contains("dead-air breath pauses between sentences in your NLE audio track"));

    // Live-action / ambient audio (< 95 WPM) is classified as Optimal
    let live_action_wpm = MarginalGainsEngine::evaluate_audio_density(54.7);
    assert_eq!(live_action_wpm.status, VectorStatus::Optimal);
    assert_eq!(live_action_wpm.score_0_to_100, 85.0);
    assert!(live_action_wpm.one_percent_fix.contains("Live-action / ambient scene detected"));
    assert!(live_action_wpm.one_percent_fix.contains("55 WPM"));
}

#[test]
fn test_marginal_gains_retention_stability_evaluation() {
    // Vector 4: Mid-Roll Retention Stability (R'(t) Drop Cliff)
    let elite_drop = MarginalGainsEngine::evaluate_retention_stability(Some(-0.8));
    assert_eq!(elite_drop.status, VectorStatus::Elite);
    assert_eq!(elite_drop.score_0_to_100, 100.0);
    assert_eq!(elite_drop.estimated_retention_lift_pct, 0.0);

    let normal_drop = MarginalGainsEngine::evaluate_retention_stability(Some(-1.5));
    assert_eq!(normal_drop.status, VectorStatus::Optimal);
    assert_eq!(normal_drop.score_0_to_100, 88.0);

    let severe_cliff = MarginalGainsEngine::evaluate_retention_stability(Some(-3.8));
    assert_eq!(severe_cliff.status, VectorStatus::CriticalFriction);
    assert_eq!(severe_cliff.score_0_to_100, 25.0);
    assert!(severe_cliff.one_percent_fix.contains("Catastrophic retention bleed"));

    // Missing heatmap defaults to -1.5%/s Optimal
    let default_drop = MarginalGainsEngine::evaluate_retention_stability(None);
    assert_eq!(default_drop.status, VectorStatus::Optimal);
    assert_eq!(default_drop.measured_value, -1.5);
}

#[test]
fn test_marginal_gains_scorecard_generation_and_top_3() {
    // 2020 Ep 4 telemetry profile:
    // t_silence: 0.0s, ASL: 4.6s, Cuts: 13, Longest Shot: 43.9s, Active WPM: 175.8, Title: "Building a diy Off- Grid Home"
    let scorecard = MarginalGainsEngine::generate_scorecard(
        "4SORyAufEsY",
        "VAN LIFE BUILD | Building a diy Off- Grid Home (as two females)",
        "Outdoor / Tiny House Build",
        0.0,
        4.6,
        13,
        43.9,
        175.8,
        Some("we're jasmine and crystal this is belle and izzy"),
    );

    assert_eq!(scorecard.target_id, "4SORyAufEsY");
    assert_eq!(scorecard.vectors.len(), 6);
    assert_eq!(scorecard.top_3_marginal_gains.len(), 3);

    // Verify all 6 vectors are mathematically independent
    let vector_names: Vec<&str> = scorecard.vectors.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(vector_names, vec![
        "Acoustic Cold-Open (t_silence)",
        "Opening 60s Visual Rhythm (ASL & Cuts)",
        "Longest Static Hold (Anti-Stagnation)",
        "Mid-Roll Retention Stability (R'(t) Cliff)",
        "Timeline Audio Density (Breath/Pause Trimming)",
        "Physical Stakes & Framing Lexicon"
    ]);

    // Top 3 tweaks are now deduplicated across independent dimensions:
    let top_tweak_names: Vec<&str> = scorecard.top_3_marginal_gains.iter().map(|t| t.name.as_str()).collect();
    assert!(top_tweak_names.contains(&"Longest Static Hold (Anti-Stagnation)"));
    assert!(top_tweak_names.contains(&"Opening 60s Visual Rhythm (ASL & Cuts)"));
    assert!(top_tweak_names.contains(&"Physical Stakes & Framing Lexicon"));

    // Multiplier lift must be positive and compounded
    assert!(scorecard.predicted_multiplier_lift >= 20.0, "Expected >=20% compounded lift from top 3 fixes, got {:.1}%", scorecard.predicted_multiplier_lift);

    // Render markdown and verify formatting
    let markdown = MarginalGainsEngine::render_markdown_scorecard(&scorecard);
    assert!(markdown.contains("# 🚴 The Brailsford Marginal Gains Scorecard"));
    assert!(markdown.contains("Top 3 \"1% Tweaks\""));
    assert!(markdown.contains("Longest Uncut Camera Hold (First Minute)"));
    assert!(markdown.contains("First-Minute Cutting Pace (Shots & Cuts/Min)"));
}

#[test]
fn test_script_generator_natural_wpm_word_budget() {
    // 150 WPM creator on a 2.4s shot beat -> (150/60) * 2.4 = 6 words
    assert_eq!(ScriptGenerator::word_budget_for_natural_wpm(150.0, 2.4), 6);

    // 200 WPM creator on a 2.4s shot beat -> (200/60) * 2.4 = 8 words
    assert_eq!(ScriptGenerator::word_budget_for_natural_wpm(200.0, 2.4), 8);

    // Sub-145 WPM is clamped to 145 WPM -> (145/60) * 2.4 = 5.8 -> 6 words
    assert_eq!(ScriptGenerator::word_budget_for_natural_wpm(120.0, 2.4), 6);

    // High WPM > 210 WPM is clamped to 210 WPM -> (210/60) * 2.4 = 8.4 -> 8 words
    assert_eq!(ScriptGenerator::word_budget_for_natural_wpm(245.0, 2.4), 8);

    // 3.5s opening hook window at 150 WPM -> (150/60) * 3.5 = 8.75 -> 9 words
    assert_eq!(ScriptGenerator::word_budget_for_natural_wpm(150.0, 3.5), 9);
}

#[test]
fn test_series_system_reliability_and_spof_ep2_vs_ep5() {
    // Ep 5 (#1 Winner - 677k views): 100, 100, 100, 60, 100, 85
    let ep5 = MarginalGainsEngine::generate_scorecard_with_retention(
        "F9Mz-RJeE68",
        "We Built a 110k Gallon Backyard Swim Pond",
        "Outdoor / Homestead",
        0.0,
        1.8,
        34,
        3.5,
        Some(-1.9),
        195.0,
        Some("we are building a 110k gallon swim pond"),
    );

    // Weakest-Link Weighted Geometric Mean:
    // (1.0 * 1.0 * 1.0 * 0.60 * 1.0 * 1.0)^(1/6) * 100 = 91.8%
    // Health = 0.5 * 91.8% + 0.5 * 60.0% (bottleneck) = 75.9%
    assert_eq!(ep5.series_system_reliability_pct, 75.9);
    // Ep 5 only has 1 flaw: the -1.9%/s mid-roll retention drop cliff.
    // +0.0% lift vectors MUST be filtered out so the list is not padded!
    assert_eq!(ep5.top_3_marginal_gains.len(), 1);
    assert_eq!(
        ep5.top_3_marginal_gains[0].name,
        "Mid-Roll Retention Stability (R'(t) Cliff)"
    );
    assert_eq!(
        ep5.top_3_marginal_gains[0].estimated_retention_lift_pct,
        8.0
    );
    assert_eq!(
        ep5.single_point_of_failure,
        Some("Mid-Video Viewer Drop-Off (Body Retention) @ 60/100 — bottlenecking total series reliability".to_string())
    );

    // Ep 2 (Underperformer - 385k views): 100, 60, 20, 88, 100, 85
    let ep2 = MarginalGainsEngine::generate_scorecard_with_retention(
        "WZx_TTeAOS8",
        "EP 2: Vanwives Natural Swim Pond Build",
        "Outdoor / Homestead",
        0.0,
        3.0,
        20,
        22.2,
        Some(-1.4),
        195.0,
        Some("today we dig the pond"),
    );

    assert_eq!(ep2.series_system_reliability_pct, 50.3);
    // Identifies the 22.2s static camera hold as the Single Point of Failure (SPOF)
    assert_eq!(
        ep2.single_point_of_failure,
        Some("Longest Uncut Camera Hold (First Minute) @ 25/100 — bottlenecking total series reliability".to_string())
    );
    // Ep 2 has 2 genuine friction points (hold 16%, retention 3.5%)
    assert_eq!(ep2.top_3_marginal_gains.len(), 2);
    assert!(ep2
        .top_3_marginal_gains
        .iter()
        .all(|t| t.estimated_retention_lift_pct > 0.0));

    // Contrast: 74.7% vs 43.5% reliability explains the ~300k view gap!
    assert!(
        ep5.series_system_reliability_pct > ep2.series_system_reliability_pct * 1.5,
        "Ep 5 series reliability should be significantly higher than Ep 2 bottlenecked throughput"
    );
}

#[test]
fn test_retention_stability_timestamp_formatting() {
    // Steepest drop cliff at 06:42 (402.0s) with -1.9%/s drop rate
    let cliff_vec = MarginalGainsEngine::evaluate_retention_stability_with_timestamp(
        Some(-1.9),
        Some(402.0),
    );
    assert_eq!(cliff_vec.status, VectorStatus::MarginalDrag);
    assert!(cliff_vec.one_percent_fix.contains("06:42"));
    assert!(cliff_vec.one_percent_fix.contains("-1.9%/s"));
    assert!(cliff_vec
        .one_percent_fix
        .contains("insert a visual pattern interrupt or trim exposition at 06:42"));

    // Catastrophic cliff at 03:15 (195.0s) with -4.2%/s drop rate
    let severe_vec = MarginalGainsEngine::evaluate_retention_stability_with_timestamp(
        Some(-4.2),
        Some(195.0),
    );
    assert_eq!(severe_vec.status, VectorStatus::CriticalFriction);
    assert!(severe_vec.one_percent_fix.contains("03:15"));
    assert!(severe_vec.one_percent_fix.contains("-4.2%/s"));
}

#[test]
fn test_viewer_satisfaction_metrics_tiers() {
    // Ep 8: 653,000 views, 2,500 comments, 35,000 likes -> 3.83 comments/1k views
    let ep8_sat = ViewerSatisfactionMetrics::new(653_000, 35_000, 2_500);
    assert_eq!(ep8_sat.comments_per_1k_views, 3.83);
    assert_eq!(
        ep8_sat.satisfaction_tier,
        "💎 Diamond Satisfaction (High Comment Velocity)"
    );

    // Ep 2: 385,000 views, 715 comments, 15,000 likes -> 1.86 comments/1k views
    let healthy_sat = ViewerSatisfactionMetrics::new(385_000, 15_000, 715);
    assert_eq!(healthy_sat.comments_per_1k_views, 1.86);
    assert_eq!(healthy_sat.satisfaction_tier, "🟢 Healthy Satisfaction");

    // Passive Watch: 500,000 views, 200 comments, 5,000 likes -> 0.4 comments/1k views, 1.0% likes
    let passive_sat = ViewerSatisfactionMetrics::new(500_000, 5_000, 200);
    assert_eq!(passive_sat.comments_per_1k_views, 0.4);
    assert_eq!(
        passive_sat.satisfaction_tier,
        "⚠️ Low Viewer Satisfaction / Passive Watch"
    );

    // Zero views edge case
    let zero_sat = ViewerSatisfactionMetrics::new(0, 0, 0);
    assert_eq!(zero_sat.comments_per_1k_views, 0.0);
    assert_eq!(
        zero_sat.satisfaction_tier,
        "⚠️ Low Viewer Satisfaction / Passive Watch"
    );
}

#[test]
fn test_scorecard_to_markdown_and_satisfaction_integration() {
    let sat = ViewerSatisfactionMetrics::new(653_000, 35_000, 2_500);
    let scorecard = MarginalGainsEngine::generate_scorecard_full(
        "F9Mz-RJeE68",
        "We Built a 110k Gallon Backyard Swim Pond",
        "Outdoor / Homestead",
        0.0,
        1.8,
        34,
        3.5,
        Some(-1.9),
        Some(402.0),
        195.0,
        Some("we are building a 110k gallon swim pond"),
        Some(sat),
    );

    let md = scorecard.to_markdown();
    assert!(md.contains("Overall Video Health (Weakest-Link Grade)"));
    assert!(md.contains("75.9%"));
    assert!(md.contains("#1 Bottleneck Holding This Video Back"));
    assert!(md.contains("💎 Diamond Satisfaction (High Comment Velocity)"));
    assert!(md.contains("Comments/1k Views: **3.83**"));
    assert_eq!(scorecard.top_3_marginal_gains.len(), 1);
    assert!(scorecard.top_3_marginal_gains.iter().all(|t| t.estimated_retention_lift_pct > 0.0));
    let top_tweaks_section = md.split("### 🏆").nth(1).expect("Missing top tweaks section");
    assert!(!top_tweaks_section.contains("+0.0% lift")); // Verifies zero padding of +0.0% lift tweaks in action list
}

#[test]
fn test_tone_conditioned_asl_and_static_hold_benchmarks() {
    // 1. Documentary / Cozy Craft profile (Vanwives pond build):
    // Relaxed pacing: 3.2s ASL is Elite, 4.5s ASL is Optimal.
    let doc_rhythm_elite = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        3.2,
        19,
        NicheToneProfile::DocumentaryBuildProcess,
    );
    assert_eq!(doc_rhythm_elite.status, VectorStatus::Elite);
    assert_eq!(doc_rhythm_elite.score_0_to_100, 100.0);
    assert_eq!(doc_rhythm_elite.target_benchmark, 5.0);

    let doc_rhythm_optimal = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        4.5,
        13,
        NicheToneProfile::AestheticLifestyleCozy,
    );
    assert_eq!(doc_rhythm_optimal.status, VectorStatus::Optimal);
    assert_eq!(doc_rhythm_optimal.score_0_to_100, 90.0);

    // Static hold ceiling relaxed to 12.0s (Elite <= 8.0s)
    let doc_hold_elite = MarginalGainsEngine::evaluate_static_hold_with_tone(
        7.8,
        NicheToneProfile::DocumentaryBuildProcess,
    );
    assert_eq!(doc_hold_elite.status, VectorStatus::Elite);
    assert_eq!(doc_hold_elite.score_0_to_100, 100.0);
    assert_eq!(doc_hold_elite.target_benchmark, 12.0);

    let doc_hold_optimal = MarginalGainsEngine::evaluate_static_hold_with_tone(
        11.5,
        NicheToneProfile::AestheticLifestyleCozy,
    );
    assert_eq!(doc_hold_optimal.status, VectorStatus::Optimal);
    assert_eq!(doc_hold_optimal.score_0_to_100, 90.0);

    // 2. Educational Breakdown profile:
    // ASL <= 4.0s Elite, <= 6.0s Optimal, hold <= 14.0s
    let edu_rhythm = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        3.8,
        16,
        NicheToneProfile::EducationalBreakdown,
    );
    assert_eq!(edu_rhythm.status, VectorStatus::Elite);
    assert_eq!(edu_rhythm.score_0_to_100, 100.0);
    assert_eq!(edu_rhythm.target_benchmark, 6.0);

    let edu_hold = MarginalGainsEngine::evaluate_static_hold_with_tone(
        13.5,
        NicheToneProfile::EducationalBreakdown,
    );
    assert_eq!(edu_hold.status, VectorStatus::Optimal);
    assert_eq!(edu_hold.score_0_to_100, 90.0);
    assert_eq!(edu_hold.target_benchmark, 14.0);
}

#[test]
fn test_expectation_mismatch_penalties() {
    // 1. Over-cutting in Cozy / Documentary:
    // Hyper-frantic 1.2s cuts on a cozy video violates expectation and is penalized
    let frantic_cozy = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        1.2,
        50,
        NicheToneProfile::AestheticLifestyleCozy,
    );
    assert_eq!(frantic_cozy.status, VectorStatus::MarginalDrag);
    assert_eq!(frantic_cozy.score_0_to_100, 55.0);
    assert!(frantic_cozy.one_percent_fix.contains("Hyper-frantic cutting"));
    assert!(frantic_cozy.one_percent_fix.contains("pacing outruns documentary craft immersion"));

    // 2. Cognitive fatigue from frantic cuts in Educational:
    let frantic_edu = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        1.4,
        43,
        NicheToneProfile::EducationalBreakdown,
    );
    assert_eq!(frantic_edu.status, VectorStatus::MarginalDrag);
    assert_eq!(frantic_edu.score_0_to_100, 55.0);
    assert!(frantic_edu.one_percent_fix.contains("Expectation Mismatch"));
    assert!(frantic_edu.one_percent_fix.contains("cause cognitive fatigue"));

    // 3. Sluggish pacing in High-Stakes Panic:
    let sluggish_high_stakes = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        4.8,
        12,
        NicheToneProfile::HighStakesPanic,
    );
    assert_eq!(sluggish_high_stakes.status, VectorStatus::CriticalFriction);
    assert_eq!(sluggish_high_stakes.score_0_to_100, 25.0);
    assert!(sluggish_high_stakes.one_percent_fix.contains("Expectation Mismatch"));
    assert!(sluggish_high_stakes.one_percent_fix.contains("High-stakes packaging expects rapid cuts"));

    // 4. Severe talking-head freeze in High-Stakes Panic:
    let freeze_high_stakes = MarginalGainsEngine::evaluate_static_hold_with_tone(
        22.0,
        NicheToneProfile::HighStakesPanic,
    );
    assert_eq!(freeze_high_stakes.status, VectorStatus::CriticalFriction);
    assert!(freeze_high_stakes.one_percent_fix.contains("Expectation Mismatch"));
}

#[test]
fn test_bounded_mid_video_drop_window_ignores_outro_cliff() {
    use sandsifter::math::RetentionEngine;

    // 600s video (10 minutes)
    // t_start = 60.0.max(0.12 * 600) = 72.0s
    // t_end = 0.82 * 600 = 492.0s (82% of runtime)
    // Derivatives:
    // - t = 45.0s (intro): -3.0%/s (ignored, < 72.0s)
    // - t = 225.0s (03:45 mid-video): -2.2%/s (TRUE mid-video drop)
    // - t = 350.0s (mid-video): -1.2%/s
    // - t = 550.0s (09:10 outro sign-off, 91.7%): -4.8%/s (outro drop, > 492.0s)
    let derivatives = vec![
        (45.0, -0.030),
        (225.0, -0.022),
        (350.0, -0.012),
        (550.0, -0.048),
    ];

    let min_t = 60.0f64.max(0.12 * 600.0);
    let max_t = 0.82 * 600.0;

    let worst_mid = RetentionEngine::find_steepest_drop_in_window(&derivatives, min_t, max_t);
    assert!(worst_mid.is_some());
    let (t_drop, r_prime) = worst_mid.unwrap();
    // Must select the mid-video drop at 225.0s, NOT the outro sign-off at 550.0s!
    assert_eq!(t_drop, 225.0);
    assert_eq!((r_prime * 1000.0).round(), -22.0);

    // Verify evaluation vector reflects mid-video drop at 03:45
    let vec4 = MarginalGainsEngine::evaluate_retention_stability_with_timestamp(
        Some(r_prime * 100.0),
        Some(t_drop),
    );
    assert_eq!(vec4.status, VectorStatus::MarginalDrag);
    assert!(vec4.one_percent_fix.contains("03:45"));
    assert!(vec4.one_percent_fix.contains("-2.2%/s"));
}

#[test]
fn test_educational_screenshare_pacing_calibration() {
    // Coding tutorial with 60.0s uncut screen recording / dark-mode IDE walkthrough
    let edu_rhythm_freeze = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        60.0,
        0,
        NicheToneProfile::EducationalBreakdown,
    );
    // Adjusted curve: scores 45.0 with +9.0% lift instead of 25.0 with +15.0%
    assert_eq!(edu_rhythm_freeze.status, VectorStatus::CriticalFriction);
    assert_eq!(edu_rhythm_freeze.score_0_to_100, 45.0);
    assert_eq!(edu_rhythm_freeze.estimated_retention_lift_pct, 9.0);
    assert!(edu_rhythm_freeze.one_percent_fix.contains("Continuous screencast walkthrough"));

    // Longest hold of 60.0s on EducationalBreakdown
    let edu_hold_freeze = MarginalGainsEngine::evaluate_static_hold_with_tone(
        60.0,
        NicheToneProfile::EducationalBreakdown,
    );
    // Adjusted curve: scores 45.0 with +8.5% lift instead of 25.0 with +13.0%
    assert_eq!(edu_hold_freeze.status, VectorStatus::CriticalFriction);
    assert_eq!(edu_hold_freeze.score_0_to_100, 45.0);
    assert_eq!(edu_hold_freeze.estimated_retention_lift_pct, 8.5);
    assert!(edu_hold_freeze.one_percent_fix.contains("Continuous screen walkthrough"));

    // Moderate screen-share pacing (asl = 15.0s, 4 cuts)
    let edu_moderate = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        15.0,
        4,
        NicheToneProfile::EducationalBreakdown,
    );
    assert_eq!(edu_moderate.status, VectorStatus::MarginalDrag);
    assert_eq!(edu_moderate.score_0_to_100, 55.0);
    assert!(edu_moderate.one_percent_fix.contains("Extended screen-share pacing"));
}

#[test]
fn test_series_funnel_insight_generation() {
    // Ep 1: Top of funnel -> no funnel decay note
    let ep1_sat = ViewerSatisfactionMetrics::new(600_000, 11_000, 300);
    let ep1_scorecard = MarginalGainsEngine::generate_scorecard_full(
        "T_KrYLW4jw8",
        "Rust Tutorial For Beginners #1 - Getting Started",
        "Tech / Programming",
        0.0,
        60.0,
        0,
        60.0,
        Some(-1.4),
        Some(414.0),
        175.0,
        Some("today we install rust"),
        Some(ep1_sat),
    );
    assert_eq!(MarginalGainsEngine::detect_series_episode_number(&ep1_scorecard.title), Some(1));
    assert!(ep1_scorecard.series_funnel_insight.is_none());

    // Ep 2: Sequel with healthy satisfaction (1.00 comments/1k) -> triggers insight
    let ep2_sat = ViewerSatisfactionMetrics::new(264_000, 5_800, 265);
    let ep2_scorecard = MarginalGainsEngine::generate_scorecard_full(
        "gvgBUY8iNO4",
        "Rust Tutorial #2 - Cargo, Crates & Variables",
        "Tech / Programming",
        0.0,
        60.0,
        0,
        60.0,
        Some(-1.6),
        Some(250.0),
        185.0,
        Some("welcome back to the rust tutorial"),
        Some(ep2_sat),
    );
    assert_eq!(MarginalGainsEngine::detect_series_episode_number(&ep2_scorecard.title), Some(2));
    assert!(ep2_scorecard.series_funnel_insight.is_some());
    let insight = ep2_scorecard.series_funnel_insight.as_deref().unwrap();
    assert!(insight.contains("High-Retention Sequel (Episode #2)"));
    assert!(insight.contains("retitle as a standalone problem without '#2'"));

    // Verify markdown renders the insight
    let md = ep2_scorecard.to_markdown();
    assert!(md.contains("> 💡 **Series Funnel Insight**: High-Retention Sequel"));
}

#[test]
fn test_narrative_pitch_ocr_cold_open_not_penalized_as_dead_air() {
    // 1. Without OCR: 2.4s speech onset lag is heavily penalized as dead air (CriticalFriction, score 30)
    let dead_air = MarginalGainsEngine::evaluate_cold_open(2.4);
    assert_eq!(dead_air.status, VectorStatus::CriticalFriction);
    assert_eq!(dead_air.score_0_to_100, 30.0);
    assert!(dead_air.estimated_retention_lift_pct >= 10.0);

    // 2. With OCR Text Hook: 2.4s speech delay is recognized as intentional reading window for silent autoplay scrollers
    let hook_text = "POV: He fell first and will burn the world for you";
    let ocr_gated = MarginalGainsEngine::evaluate_cold_open_with_ocr(2.4, Some(hook_text));
    assert_eq!(ocr_gated.status, VectorStatus::Elite);
    assert!(ocr_gated.score_0_to_100 >= 90.0 && ocr_gated.score_0_to_100 <= 100.0);
    assert_eq!(ocr_gated.estimated_retention_lift_pct, 0.0);
    assert!(ocr_gated.one_percent_fix.contains("Non-verbal text hook detected at frame one"));
    assert!(ocr_gated.one_percent_fix.contains(hook_text));
    assert!(ocr_gated.one_percent_fix.contains("captures silent autoplay scrollers."));

    // 3. Integrated scorecard with OCR Text Hook maintains Elite status on Vector 1
    let scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "BOOK_TRAILER_01",
        "The Dark King's Vow - Official Book Teaser",
        "Fiction & Romance",
        Some(NicheToneProfile::NarrativeEmotionalPitch),
        2.5,
        Some("The villain gets the girl in chapter 32"),
        2.1,
        28,
        6.5,
        Some(-1.1),
        Some(12.0),
        160.0,
        Some("he will burn the kingdom for her"),
        None,
    );
    assert_eq!(scorecard.vectors[0].status, VectorStatus::Elite);
    assert_eq!(scorecard.vectors[0].score_0_to_100, 95.0);
    // Cold open is not flagged as a top marginal gain friction
    assert!(!scorecard.top_3_marginal_gains.iter().any(|v| v.name == "Acoustic Cold-Open (t_silence)"));
}

#[test]
fn test_narrative_tone_auto_detection() {
    // 1. Auto-detect keywords across title, channel name, and context:
    // keywords: book, novel, author, trope, romance, fantasy, lore, story, pov, trailer, teaser, fiction
    let t1 = NicheToneProfile::detect("Dark Romance Book Teaser", "Jane Doe Author", "Read the novel");
    assert_eq!(t1, NicheToneProfile::NarrativeEmotionalPitch);
    assert_eq!(t1.label(), "🎭 Narrative & Emotional Pitch");

    let t2 = NicheToneProfile::detect("POV: When the villain protects you", "Fantasy Lore", "trope breakdown");
    assert_eq!(t2, NicheToneProfile::NarrativeEmotionalPitch);

    let t3 = NicheToneProfile::detect("Official Cinematic Story Trailer", "Indie Fiction Studio", "");
    assert_eq!(t3, NicheToneProfile::NarrativeEmotionalPitch);

    // 2. Vector 2 Pacing Benchmarks: Target ASL <= 3.0s (Elite <= 1.8s)
    let elite_pacing = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        1.6,
        35,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(elite_pacing.status, VectorStatus::Elite);
    assert_eq!(elite_pacing.score_0_to_100, 100.0);
    assert_eq!(elite_pacing.target_benchmark, 3.0);
    assert!(elite_pacing.one_percent_fix.contains("Elite narrative dramatic pacing"));

    let optimal_pacing = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        2.7,
        22,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(optimal_pacing.status, VectorStatus::Optimal);
    assert_eq!(optimal_pacing.score_0_to_100, 90.0);
    assert_eq!(optimal_pacing.target_benchmark, 3.0);

    let drag_pacing = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        4.2,
        14,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(drag_pacing.status, VectorStatus::MarginalDrag);
    assert_eq!(drag_pacing.score_0_to_100, 60.0);
    assert!(drag_pacing.one_percent_fix.contains("Narrative pacing drag"));

    // 3. Vector 3 Camera Hold Benchmarks: Longest Camera Hold <= 8.0s (Elite <= 5.0s)
    let elite_hold = MarginalGainsEngine::evaluate_static_hold_with_tone(
        4.5,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(elite_hold.status, VectorStatus::Elite);
    assert_eq!(elite_hold.score_0_to_100, 100.0);
    assert_eq!(elite_hold.target_benchmark, 8.0);
    assert!(elite_hold.one_percent_fix.contains("Elite dramatic scene rhythm"));

    let optimal_hold = MarginalGainsEngine::evaluate_static_hold_with_tone(
        7.2,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(optimal_hold.status, VectorStatus::Optimal);
    assert_eq!(optimal_hold.score_0_to_100, 90.0);
    assert_eq!(optimal_hold.target_benchmark, 8.0);

    let drag_hold = MarginalGainsEngine::evaluate_static_hold_with_tone(
        11.5,
        NicheToneProfile::NarrativeEmotionalPitch,
    );
    assert_eq!(drag_hold.status, VectorStatus::MarginalDrag);
    assert_eq!(drag_hold.score_0_to_100, 60.0);
    assert!(drag_hold.one_percent_fix.contains("Static shot drag"));

    // 4. Vector 6 Narrative Conflict Lexicon: Trope Juxtapositions, POV Markers, Irreversible Consequences
    let narrative_lexicon = MarginalGainsEngine::evaluate_stakes_lexicon(
        "POV: When the morally gray villain protects you",
        Some("He fell first and will burn the world for you"),
    );
    assert_eq!(narrative_lexicon.status, VectorStatus::Elite);
    assert!(narrative_lexicon.score_0_to_100 >= 80.0);
    assert!(narrative_lexicon.one_percent_fix.contains("Strong narrative stakes"));
    assert!(narrative_lexicon.one_percent_fix.contains("trope framing"));
}

#[test]
fn test_narrative_pitch_title_stakes_diagnostic_not_vlog_or_machinery() {
    let title = "Throne of Glass trailer";
    // Evaluating passive narrative title under NarrativeEmotionalPitch tone
    let v6 = MarginalGainsEngine::evaluate_title_stakes(
        title,
        None,
        Some(NicheToneProfile::NarrativeEmotionalPitch),
    );

    // Score should be < 70.0 (50.0 baseline)
    assert!(v6.score_0_to_100 < 70.0);
    assert!(v6.status == VectorStatus::MarginalDrag || v6.status == VectorStatus::CriticalFriction);

    // Must NOT reference vlog labels ('rough week'), physical milestones, machinery, or financial metrics
    let fix = &v6.one_percent_fix;
    assert!(!fix.to_lowercase().contains("rough week"), "Diagnostic should not reference vlog labels");
    assert!(!fix.to_lowercase().contains("machinery"), "Diagnostic should not reference machinery");
    assert!(!fix.to_lowercase().contains("physical milestones"), "Diagnostic should not reference physical milestones");
    assert!(!fix.to_lowercase().contains("engineering"), "Diagnostic should not reference engineering");
    assert!(!fix.to_lowercase().contains("diary"), "Diagnostic should not reference diary");

    // Must emit the exact specified narrative diagnostic
    assert_eq!(
        fix,
        "Title relies on passive entity naming; add high-stakes trope modifiers, character status inversions, or POV conflict hooks (e.g., 'enemies to lovers', 'when the villain', 'moral gray') to lift CTR."
    );
}

#[test]
fn test_audio_first_asmr_pacing_and_static_hold_not_penalized() {
    let title = "ASMR || Your Supervillain Wife Is Only Soft With You [Enemies to Lovers] [Comfort] [Protective]";
    assert!(MarginalGainsEngine::is_audio_first(title, ""));

    // 1. Vector 5 Audio Density with 116.4 WPM
    let v5 = MarginalGainsEngine::evaluate_audio_density_with_context(116.4, true);
    assert!(v5.score_0_to_100 >= 85.0);
    assert_eq!(v5.status, VectorStatus::Elite);
    assert_eq!(v5.score_0_to_100, 95.0);
    assert!(v5.one_percent_fix.contains("Intentional ambient/ASMR delivery (116 WPM)"));
    assert!(v5.one_percent_fix.contains("natural intimate pacing; do not trim breath pauses or accelerate speech."));
    assert!(!v5.one_percent_fix.contains("Trim dead air"));

    // 2. Vector 3 Static Hold with 60.0s hold
    let v3 = MarginalGainsEngine::evaluate_static_hold_with_context(
        60.0,
        NicheToneProfile::NarrativeEmotionalPitch,
        true,
    );
    assert!(v3.score_0_to_100 >= 85.0);
    assert_eq!(v3.status, VectorStatus::Optimal);
    assert_eq!(v3.score_0_to_100, 85.0);
    assert_eq!(
        v3.one_percent_fix,
        "Static audio-first canvas detected—visuals serve as ambient accompaniment; listener retention is driven by audio fidelity."
    );

    // 3. Full scorecard integration test
    let scorecard = MarginalGainsEngine::generate_scorecard_with_tone(
        "asmr_test",
        title,
        "Audio Drama ASMR",
        Some(NicheToneProfile::NarrativeEmotionalPitch),
        0.0,
        60.0,
        1,
        60.0,
        None,
        None,
        116.4,
        Some("Your supervillain wife returns after a long day"),
        None,
    );

    // Visual Rhythm (Vector 2), Static Hold (Vector 3), and Audio Density (Vector 5) must all score >= 85.0
    assert!(scorecard.vectors[1].score_0_to_100 >= 85.0); // Visual Rhythm
    assert!(scorecard.vectors[2].score_0_to_100 >= 85.0); // Static Hold
    assert!(scorecard.vectors[4].score_0_to_100 >= 85.0); // Audio Density

    // No SPOF on Vectors 2, 3, or 5
    if let Some(ref spof) = scorecard.single_point_of_failure {
        assert!(!spof.contains("Visual Rhythm"), "Visual Rhythm should not be SPOF: {}", spof);
        assert!(!spof.contains("Static Shot"), "Static Shot should not be SPOF: {}", spof);
        assert!(!spof.contains("Voice Pacing"), "Voice Pacing should not be SPOF: {}", spof);
    }
}

#[test]
fn test_cozy_sensory_music_cold_open_not_crashing_health_score() {
    // 1. Direct Vector 1 evaluation under AestheticLifestyleCozy
    let v1 = MarginalGainsEngine::evaluate_cold_open_with_context(
        38.0,
        None,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        false,
    );
    assert_eq!(v1.status, VectorStatus::Optimal);
    assert_eq!(v1.score_0_to_100, 70.0);
    assert_eq!(v1.measured_value, 38.0);
    assert!(v1.one_percent_fix.contains("Atmospheric sensory opening detected"));
    assert!(v1.one_percent_fix.contains("38.0s vocal onset delay"));
    assert_eq!(v1.estimated_retention_lift_pct, 4.0);

    // 2. Full scorecard generation for Minivan freezing temp video
    let title = "Living in a Minivan during Freezing Temperatures";
    let channel = "Foresty Forest";
    let scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "MINIVAN_FREEZING_01",
        title,
        channel,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        38.0, // 38s vocal onset delay
        None,
        3.5,
        18,
        8.0,
        Some(-0.8),
        None,
        155.0,
        Some("Living in a minivan in freezing blizzard"),
        Some(ViewerSatisfactionMetrics::new(8059684, 250000, 12000)),
    );

    // Health Score must remain healthy / robust, not crash to 30%
    assert!(scorecard.series_system_reliability_pct >= 65.0, "Health Grade should be >= 65%: {}", scorecard.series_system_reliability_pct);
    assert_eq!(scorecard.vectors[0].measured_value, 38.0);
    assert_eq!(scorecard.vectors[0].score_0_to_100, 70.0);
}

#[test]
fn test_is_local_file_detection() {
    // 1. Non-existent paths or web URLs must return false
    assert!(!MarginalGainsEngine::is_local_file("https://www.youtube.com/watch?v=eqCNMbcsXWo"));
    assert!(!MarginalGainsEngine::is_local_file("https://youtu.be/3bPxpTKxkgs"));
    assert!(!MarginalGainsEngine::is_local_file("non_existent_video_file_982341.mp4"));

    // 2. Existing local video file created on disk must return true
    let dir = tempfile::tempdir().unwrap();
    let video_path = dir.path().join("render_v1_draft.mp4");
    std::fs::write(&video_path, b"dummy video bytes").unwrap();

    assert!(MarginalGainsEngine::is_local_file(video_path.to_str().unwrap()));

    let mov_path = dir.path().join("timeline_export.mov");
    std::fs::write(&mov_path, b"dummy mov bytes").unwrap();
    assert!(MarginalGainsEngine::is_local_file(mov_path.to_str().unwrap()));
}

#[test]
fn test_target_comparison_report_delta_matrix() {
    use sandsifter::math::TargetComparisonReport;

    // Target A: Winner (4.13M views) - High momentum garage build
    let winner = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "eqCNMbcsXWo",
        "How to Turn $100 into $1,000 Flipping Boat Motors!",
        "BOAT BUSTERS",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.2, // 0.2s vocal delay (Elite)
        None,
        2.9, // 2.9s ASL (Optimal)
        42,
        4.8, // 4.8s camera hold (Elite)
        Some(-0.8), // gentle decay
        None,
        198.0, // 198 WPM (Elite)
        Some("turning $100 into $1,000"),
        Some(ViewerSatisfactionMetrics::new(4130000, 150000, 4800)),
    );

    // Target B: Flop (65k views) - Cooking oil negative control
    let flop = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "TIp2I8PxmGU",
        "Will an Outboard Run on Cooking Oil?",
        "BOAT BUSTERS",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        25.0, // 25s dead-air cold open (Critical Friction)
        None,
        4.8,  // 4.8s ASL
        18,
        20.0, // 20s camera lock (Critical Friction)
        Some(-4.9), // catastrophic cliff
        Some(644.0),
        107.0, // 107 WPM (Critical Friction)
        Some("can you run an outboard on cooking oil"),
        Some(ViewerSatisfactionMetrics::new(65000, 1800, 92)),
    );

    let comparison = TargetComparisonReport::generate(&winner, &flop);

    // Baseline validation: Target A should outperform Target B
    assert_eq!(comparison.target_a_id, "eqCNMbcsXWo");
    assert_eq!(comparison.target_b_id, "TIp2I8PxmGU");
    assert!(comparison.health_grade_a > comparison.health_grade_b);
    assert!(comparison.delta_health_grade >= 30.0, "Delta health grade should be >= 30%: {}", comparison.delta_health_grade);
    assert!(comparison.delta_composite_score > 0.0);

    // Verify side-by-side vector delta matrix items
    assert!(comparison.vector_comparisons.len() >= 6);
    let vocal_delay_comp = comparison.vector_comparisons.iter().find(|v| v.vector_name.contains("Intro Delay") || v.vector_name.contains("Spoken Word")).unwrap();
    assert_eq!(vocal_delay_comp.advantage, "Target A");
    assert!(vocal_delay_comp.delta_score > 0.0);

    let hold_comp = comparison.vector_comparisons.iter().find(|v| v.vector_name.contains("Camera Hold") || v.vector_name.contains("Static")).unwrap();
    assert_eq!(hold_comp.advantage, "Target A");

    // Structural advantages & editing playbook
    assert!(!comparison.structural_advantages_a.is_empty());
    assert!(!comparison.what_to_change_playbook.is_empty());

    // Markdown and terminal rendering
    let md = comparison.to_markdown();
    assert!(md.contains("# ⚔️ Head-to-Head Comparative Audit: Target A vs. Target B"));
    assert!(md.contains("Side-by-Side Telemetry Delta Matrix"));
    assert!(md.contains("What to Change in Target B"));

    let term = comparison.render_terminal();
    assert!(term.contains("HEAD-TO-HEAD COMPARATIVE AUDIT"));
    assert!(term.contains("SIDE-BY-SIDE VECTOR DELTA MATRIX"));
}

#[test]
fn test_scorecard_matrix_plain_english_editor_targets() {
    let v1 = MarginalGainsEngine::evaluate_cold_open(2.5);
    assert_eq!(v1.editor_target(), "Under 0.5s (Frames 1-12)");
    assert!(v1.one_percent_fix.contains("under 0.5s (Frames 1-12)"));

    let v2_rapid = MarginalGainsEngine::evaluate_visual_rhythm(1.8, 35);
    assert_eq!(v2_rapid.editor_target(), "Cut every 2s (25+ cuts/min)");

    let v2_cozy = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(4.0, 15, NicheToneProfile::DocumentaryBuildProcess);
    assert_eq!(v2_cozy.editor_target(), "Cut every 3-5s (12-20 cuts/min)");

    let v3_panic = MarginalGainsEngine::evaluate_static_hold(5.0);
    assert_eq!(v3_panic.editor_target(), "Max 7s per shot");

    let v3_cozy = MarginalGainsEngine::evaluate_static_hold_with_tone(11.0, NicheToneProfile::DocumentaryBuildProcess);
    assert_eq!(v3_cozy.editor_target(), "Max 10s per shot");

    let v4 = MarginalGainsEngine::evaluate_retention_stability(Some(-1.5));
    assert_eq!(v4.editor_target(), "Steady decline (No steep cliffs)");

    let v5_talk = MarginalGainsEngine::evaluate_audio_density(195.0);
    assert_eq!(v5_talk.editor_target(), "150-200 WPM (Natural pace)");

    let v5_asmr = MarginalGainsEngine::evaluate_audio_density_with_context(100.0, true);
    assert_eq!(v5_asmr.editor_target(), "85-145 WPM (Ambient / ASMR)");

    let v6 = MarginalGainsEngine::evaluate_stakes_lexicon("110k Gallon Swim Pond", None);
    assert_eq!(v6.editor_target(), "High stakes / Clear scale");

    // Full scorecard rendering verification
    let scorecard = MarginalGainsEngine::generate_scorecard_full(
        "test_editor_target_vid",
        "Building 110k Gallon Pond",
        "Vanwives",
        2.5,
        1.4,
        43,
        3.1,
        Some(-1.5),
        None,
        0.0,
        Some("building swim pond"),
        None,
    );

    let md = MarginalGainsEngine::render_markdown_scorecard(&scorecard);

    // Verify table header has "Editor Target" and not "Benchmark"
    assert!(md.contains("| Telemetry Vector | Measured | Editor Target | Score | Status | 1% Actionable Tweak | Est. Lift |"));
    assert!(!md.contains("| Telemetry Vector | Measured | Benchmark |"));

    // Verify zero LaTeX math markers
    assert!(!md.contains("$\\le"), "Markdown table contains LaTeX \\le: {}", md);
    assert!(!md.contains("$\\ge"), "Markdown table contains LaTeX \\ge: {}", md);
    assert!(!md.contains("\\text{"), "Markdown table contains LaTeX \\text: {}", md);
    assert!(!md.contains("\\%"), "Markdown table contains LaTeX \\%: {}", md);

    // Verify clean editor targets are rendered in table rows
    assert!(md.contains("Under 0.5s (Frames 1-12)"));
    assert!(md.contains("Steady decline (No steep cliffs)"));
    assert!(md.contains("High stakes / Clear scale"));
}

#[test]
fn test_documentary_over_editing_penalty_preserves_audio_cadence() {
    // 1. Feeding a 1.6s ASL (37 cuts/min) to DocumentaryBuildProcess profile
    let v2_rhythm = MarginalGainsEngine::evaluate_visual_rhythm_with_tone(
        1.6,
        37,
        NicheToneProfile::DocumentaryBuildProcess,
    );
    // Docks the visual rhythm score (55.0 / MarginalDrag) with the exact penalty label
    assert_eq!(v2_rhythm.status, VectorStatus::MarginalDrag);
    assert_eq!(v2_rhythm.score_0_to_100, 55.0);
    assert_eq!(
        v2_rhythm.one_percent_fix,
        "Hyper-frantic cutting (1.6s/cut): pacing outruns documentary craft immersion; slow down cuts to 2.5s-4.0s to allow spatial scale to register."
    );

    // 2. Preserves conversational audio scoring (195 WPM -> 100.0 / Optimal)
    let v5_audio = MarginalGainsEngine::evaluate_audio_density(195.0);
    assert_eq!(v5_audio.status, VectorStatus::Optimal);
    assert_eq!(v5_audio.score_0_to_100, 100.0);
    assert!(v5_audio.one_percent_fix.contains("Natural conversational cadence (195 WPM)—no vocal changes needed"));

    // 3. Decoupling PipelineStage categorization
    assert_eq!(v2_rhythm.pipeline_stage(), PipelineStage::ImmersionMechanics);
    assert_eq!(v2_rhythm.stage_category(), "Immersion Mechanics (Retention Velocity)");

    let v3_hold = MarginalGainsEngine::evaluate_static_hold_with_tone(
        8.0,
        NicheToneProfile::DocumentaryBuildProcess,
    );
    assert_eq!(v3_hold.pipeline_stage(), PipelineStage::ImmersionMechanics);
    assert_eq!(v3_hold.stage_category(), "Immersion Mechanics (Retention Velocity)");

    let v6_stakes = MarginalGainsEngine::evaluate_stakes_lexicon_with_tone(
        "65 Acre Homestead with Dozer",
        None,
        Some(NicheToneProfile::DocumentaryBuildProcess),
    );
    assert_eq!(v6_stakes.pipeline_stage(), PipelineStage::ImpressionGate);
    assert_eq!(v6_stakes.stage_category(), "Impression Gate (Top-of-Funnel Potential)");
}

#[test]
fn test_conversational_podcast_tolerates_extended_holds() {
    let hold_vec = MarginalGainsEngine::evaluate_static_hold_with_tone(
        40.0,
        NicheToneProfile::ConversationalPodcast,
    );
    assert!(
        hold_vec.score_0_to_100 >= 80.0,
        "Score should remain >= 80/100 (Optimal), got {}",
        hold_vec.score_0_to_100
    );
    assert_eq!(hold_vec.status, VectorStatus::Optimal);
}

#[test]
fn test_conversational_podcast_flags_excessive_hold_beyond_60s() {
    let hold_vec = MarginalGainsEngine::evaluate_static_hold_with_tone(
        75.0,
        NicheToneProfile::ConversationalPodcast,
    );
    assert!(
        hold_vec.score_0_to_100 < 80.0,
        "Score should be penalized below 80.0 for hold beyond 60s, got {}",
        hold_vec.score_0_to_100
    );
    assert!(
        hold_vec.one_percent_fix.contains("60.0s broadcast ceiling")
            || hold_vec.one_percent_fix.contains("exceeds 60.0s broadcast limit")
            || hold_vec.one_percent_fix.contains("SlowVisualPacing"),
        "Expected warning for exceeding 60.0s broadcast ceiling, got: {}",
        hold_vec.one_percent_fix
    );
}

#[test]
fn test_frame_zero_onset_never_labeled_drag_under_promise_gap() {
    // Injects t_silence = 0.0s alongside an artificially forced promise gap breach
    let artificial_breach = PromiseGapReport::breach(
        45.0,
        true,
        "Frame 0 visual matches",
        false,
        "First spoken sentence has zero topical overlap with title",
        None, // no throat clearing
        None,
    );
    assert!(artificial_breach.is_breach);

    let v1 = MarginalGainsEngine::evaluate_cold_open_with_promise_gap(
        0.0,
        None,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        false,
        Some(&artificial_breach),
    );

    assert_eq!(
        v1.status,
        VectorStatus::Elite,
        "Frame-zero onset (0.0s) without throat clearing must remain Elite, never Drag: {:?}",
        v1.status
    );
    assert!(
        v1.score_0_to_100 >= 95.0,
        "Score must be >= 95.0 for 0.0s speech onset, got {}",
        v1.score_0_to_100
    );
    assert_ne!(
        v1.status,
        VectorStatus::MarginalDrag,
        "0.0s speech onset must never be labeled as DRAG"
    );
}








