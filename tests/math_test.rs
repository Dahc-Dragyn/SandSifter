use sandsifter::math::*;

#[test]
fn test_baseline_standard_distribution() {
    let views = vec![8_000, 9_500, 10_000, 10_200, 11_000, 12_500, 13_000];
    let baseline = BaselineEngine::compute_baseline(&views).expect("Baseline calculation failed");
    // Median of 7 numbers is the 4th element (10,200)
    assert_eq!(baseline, 10_200);
}

#[test]
fn test_baseline_winsorization_resilience_against_mega_spike() {
    // Creator averages ~10,000 views, but had 1 viral breakout of 2,500,000 views
    let views = vec![
        9_000, 9_500, 10_000, 10_200, 10_500, 11_000, 11_500, 12_000, 12_500, 2_500_000,
    ];

    let naive_mean = views.iter().sum::<u64>() / views.len() as u64;
    assert!(
        naive_mean > 250_000,
        "Naive mean is severely skewed to {} views",
        naive_mean
    );

    let robust_baseline = BaselineEngine::compute_baseline(&views).expect("Baseline failed");

    // Robust baseline must remain anchored near 10k-12k, completely rejecting the 2.5M outlier
    assert!(
        (10_000..=13_000).contains(&robust_baseline),
        "Robust baseline was corrupted: {}",
        robust_baseline
    );
}

#[test]
fn test_baseline_edge_cases() {
    // Empty dataset
    assert_eq!(BaselineEngine::compute_baseline(&[]), None);

    // Single item
    assert_eq!(BaselineEngine::compute_baseline(&[5_000]), Some(5_000));

    // Two items
    assert_eq!(BaselineEngine::compute_baseline(&[4_000, 6_000]), Some(5_000));

    // Rolling window
    let stream = vec![1_000, 2_000, 3_000, 10_000, 10_000, 10_000];
    let rolling = BaselineEngine::compute_rolling_baseline(&stream, 3).unwrap();
    assert_eq!(rolling, 10_000);
}

#[test]
fn test_outlier_multiplier_and_tiers() {
    let baseline = 10_000;

    // Underperformer
    let under = OutlierEngine::calculate_multiplier(5_000, baseline);
    assert_eq!(under, 0.5);
    assert_eq!(OutlierEngine::classify_tier(under), OutlierTier::Underperformer);

    // Baseline performer
    let base = OutlierEngine::calculate_multiplier(15_000, baseline);
    assert_eq!(base, 1.5);
    assert_eq!(OutlierEngine::classify_tier(base), OutlierTier::Baseline);

    // Winner outlier
    let winner = OutlierEngine::calculate_multiplier(35_000, baseline);
    assert_eq!(winner, 3.5);
    assert_eq!(OutlierEngine::classify_tier(winner), OutlierTier::WinnerOutlier);
    assert!(OutlierEngine::warrants_deep_analysis(winner, 2.5));

    // Breakout viral
    let breakout = OutlierEngine::calculate_multiplier(120_000, baseline);
    assert_eq!(breakout, 12.0);
    assert_eq!(OutlierEngine::classify_tier(breakout), OutlierTier::BreakoutViral);
}

#[test]
fn test_time_decay_half_life() {
    let raw_views = 100_000;
    let lambda = 0.0866; // ~8 days half-life

    // 0 days elapsed -> 100% views
    let v0 = OutlierEngine::time_decayed_views(raw_views, 0.0, lambda);
    assert_eq!(v0.round() as u64, 100_000);

    // 8 days elapsed -> ~50% views
    let v8 = OutlierEngine::time_decayed_views(raw_views, 8.0, lambda);
    assert!(
        (v8 - 50000.0).abs() < 1000.0,
        "Decay didn't match half-life: {}",
        v8
    );
}

#[test]
fn test_paid_boost_screening_heuristics() {
    // High-view video (200k views) with only 1.2% engagement (likely boosted with paid spend)
    let paid_audit = HeuristicsEngine::audit_engagement(200_000, 2_000, 200, 200, None);
    assert!(!paid_audit.is_organic);
    assert!(paid_audit.is_likely_paid_or_boosted);
    assert!(paid_audit.screening_reason.is_some());

    // Organic video (150k views) with 6.5% engagement (Likes: 8000, Comments: 1200, Shares: 550)
    let organic_audit = HeuristicsEngine::audit_engagement(150_000, 8_000, 1_200, 550, None);
    assert!(organic_audit.is_organic);
    assert!(!organic_audit.is_likely_paid_or_boosted);
    assert!(organic_audit.screening_reason.is_none());
    assert!((organic_audit.engagement_rate_percent - 6.5).abs() < 0.01);
}

#[test]
fn test_winner_loser_delineation_cliff() {
    // Clear performance cliff: top 3 videos are 250k, 220k, 200k, then drops to 45k, 40k, 38k
    let views_sorted = vec![250_000, 220_000, 200_000, 45_000, 40_000, 38_000, 35_000];
    let cutoff = HeuristicsEngine::find_winner_loser_delineation(&views_sorted);

    // Cutoff should identify the cliff at 200k
    assert_eq!(cutoff, Some(200_000));
}
