use sandsifter::math::topics::{
    TopicMomentumEngine, TopicVideoInput, TrendClassification,
};

#[test]
fn test_tokenization_and_stopwords() {
    let tokens = TopicMomentumEngine::tokenize("The Mafia's Obsession: A Dark Romance Novel!!");
    assert_eq!(tokens, vec!["the", "mafia", "obsession", "dark", "romance", "novel"]);

    assert!(TopicMomentumEngine::is_stopword("the"));
    assert!(TopicMomentumEngine::is_stopword("video"));
    assert!(!TopicMomentumEngine::is_stopword("mafia"));
    assert!(!TopicMomentumEngine::is_stopword("romance"));
}

#[test]
fn test_ngram_extraction() {
    let tokens = vec![
        "dark".to_string(),
        "mafia".to_string(),
        "romance".to_string(),
        "books".to_string(),
    ];
    let ngrams = TopicMomentumEngine::extract_ngrams(&tokens);

    // Unigrams
    assert!(ngrams.contains(&("dark".to_string(), "Unigram".to_string())));
    assert!(ngrams.contains(&("mafia".to_string(), "Unigram".to_string())));

    // Bigrams
    assert!(ngrams.contains(&("dark mafia".to_string(), "Bigram".to_string())));
    assert!(ngrams.contains(&("mafia romance".to_string(), "Bigram".to_string())));

    // Trigram
    assert!(ngrams.contains(&("dark mafia romance".to_string(), "Trigram".to_string())));
}

#[test]
fn test_velocity_calculation() {
    // 10,000 views uploaded 2 days ago -> 5,000 views/day
    let v = TopicVideoInput::new(
        "Top 5 Dark Romance Books",
        "Recommended dark books",
        "BookTuber1",
        "https://youtube.com/watch?v=123",
        10_000,
        2.0,
    );
    assert!((v.velocity - 5000.0).abs() < 1e-3);
}

#[test]
fn test_breakout_micro_trend_detection() {
    // Create a 10-video cohort with a cohort baseline around 1000 views/day
    let mut videos = Vec::new();

    // 7 baseline videos across channels (velocity ~500 views/day)
    for i in 1..=7 {
        videos.push(TopicVideoInput::new(
            format!("General Reading Vlog #{}", i),
            "Just reading ordinary fiction books this week.",
            format!("Channel{}", i),
            format!("https://youtube.com/watch?v=base{}", i),
            1_000,
            2.0, // velocity: 500 views/day
        ));
    }

    // 3 breakout videos across 3 different channels using the keyword "morally gray billionaire"
    // with very high velocity (~5,000 views/day -> 10x baseline)
    for i in 8..=10 {
        videos.push(TopicVideoInput::new(
            format!("Why The Morally Gray Billionaire Trope Rules Ep {}", i),
            "Deep dive into the morally gray billionaire trope in dark romance.",
            format!("Channel{}", i),
            format!("https://youtube.com/watch?v=break{}", i),
            10_000,
            2.0, // velocity: 5000 views/day
        ));
    }

    let report = TopicMomentumEngine::analyze_cohort_trends("Dark Romance", 7, &videos);

    assert_eq!(report.total_videos_analyzed, 10);
    assert_eq!(report.distinct_channels_count, 10);

    // Should detect "morally gray" or "gray billionaire" as a BreakoutMicroTrend
    let breakout = report.breakout_trends.iter().find(|t| t.entity.contains("gray"));
    assert!(breakout.is_some(), "Expected breakout trend containing 'gray'");

    let b = breakout.unwrap();
    assert_eq!(b.classification, TrendClassification::BreakoutMicroTrend);
    assert!(b.velocity_lift >= 2.0);
    assert_eq!(b.channel_dispersion, 3);
    assert!(b.tactical_advice.contains("EXPLOSIVE TREND"));
}

#[test]
fn test_saturated_fading_detection() {
    let mut videos = Vec::new();

    // 6 videos (60% of cohort) featuring "kindle unlimited" with low velocity (~100 views/day)
    for i in 1..=6 {
        videos.push(TopicVideoInput::new(
            format!("My Kindle Unlimited Dark Romance Picks Part {}", i),
            "Every single book available on kindle unlimited haul.",
            format!("Channel{}", i),
            format!("https://youtube.com/watch?v=sat{}", i),
            200,
            2.0, // velocity: 100 views/day
        ));
    }

    // 4 other videos with high velocity (~2,000 views/day)
    for i in 7..=10 {
        videos.push(TopicVideoInput::new(
            format!("Unfiltered Dark Romance Reactions #{}", i),
            "My unfiltered reaction to shocking scenes.",
            format!("Channel{}", i),
            format!("https://youtube.com/watch?v=high{}", i),
            4_000,
            2.0, // velocity: 2000 views/day
        ));
    }

    let report = TopicMomentumEngine::analyze_cohort_trends("Dark Romance", 7, &videos);

    // "kindle unlimited" appears in 60% of videos but has low velocity lift (<0.8x)
    let saturated = report.saturated_trends.iter().find(|t| t.entity.contains("kindle"));
    assert!(saturated.is_some(), "Expected saturated trend containing 'kindle'");

    let s = saturated.unwrap();
    assert_eq!(s.classification, TrendClassification::SaturatedFading);
    assert!(s.frequency_pct > 30.0);
    assert!(s.velocity_lift < 0.8);
    assert!(s.tactical_advice.contains("AVOID / PIVOT"));
}

#[test]
fn test_empty_cohort_graceful() {
    let report = TopicMomentumEngine::analyze_cohort_trends("Empty Niche", 7, &[]);
    assert_eq!(report.total_videos_analyzed, 0);
    assert!(report.breakout_trends.is_empty());
    assert!(report.creator_takeaway.contains("No recent videos"));
    assert!(report.demand_stats.is_none());
    assert!(report.demand_stacking_suggestions.is_empty());
}

#[test]
fn test_topic_floor_ceiling_and_demand_stats() {
    // Cohort of 8 videos with varying views:
    // [10k, 20k, 30k, 40k, 50k, 60k, 80k, 200k]
    let view_counts = [10_000, 20_000, 30_000, 40_000, 50_000, 60_000, 80_000, 200_000];
    let mut videos = Vec::new();
    for (i, &views) in view_counts.iter().enumerate() {
        videos.push(TopicVideoInput::new(
            format!("Backyard Swim Pond Build Part {}", i + 1),
            "Documenting our DIY natural swim pond transformation.",
            format!("Channel{}", i + 1),
            format!("https://youtube.com/watch?v=pond{}", i + 1),
            views,
            2.0,
        ));
    }

    let report = TopicMomentumEngine::analyze_cohort_trends("Backyard Swim Pond Build", 30, &videos);

    assert!(report.demand_stats.is_some());
    let stats = report.demand_stats.unwrap();

    // Verify statistical quartiles and IQR bounds:
    // Sorted: [10k, 20k, 30k, 40k, 50k, 60k, 80k, 200k]
    // Q1 idx = 8 / 4 = 2 -> 30,000
    assert_eq!(stats.topic_floor_q1, 30_000);
    // Median: (40,000 + 50,000) / 2 = 45,000
    assert_eq!(stats.median_baseline, 45_000);
    // Q3 idx = 6 -> 80,000
    assert_eq!(stats.q3, 80_000);
    // IQR: 80,000 - 30,000 = 50,000
    assert_eq!(stats.iqr, 50_000.0);
    // Winsorized Ceiling: Q3 + 1.5 * IQR = 80,000 + 75,000 = 155,000
    assert_eq!(stats.topic_ceiling, 155_000);
    // Upside Multiplier: 155,000 / 30,000 = ~5.2x
    assert_eq!(stats.upside_multiplier, 5.2);
    assert_eq!(stats.sample_size, 8);
}

#[test]
fn test_demand_stacking_suggestions() {
    // 1. Craft / Build Topic with extracted scale:
    let videos = vec![
        TopicVideoInput::new(
            "We Built a 110k Gallon Swim Pond in 60 Days",
            "Our massive natural swim pond.",
            "Vanwives",
            "https://youtube.com/watch?v=winner",
            650_000,
            10.0,
        ),
        TopicVideoInput::new(
            "Digging a Pond With a Cheap Excavator",
            "Excavator testing.",
            "Homesteaders",
            "https://youtube.com/watch?v=excavator",
            200_000,
            10.0,
        ),
    ];

    let report = TopicMomentumEngine::analyze_cohort_trends("Natural Swim Pond Build", 14, &videos);
    let suggestions = report.demand_stacking_suggestions;

    assert!(!suggestions.is_empty());
    // Should include ScaleAnchor, BudgetFriction, and TransformationExtreme
    let categories: Vec<&str> = suggestions.iter().map(|s| s.category.as_str()).collect();
    assert!(categories.contains(&"ScaleAnchor"));
    assert!(categories.contains(&"BudgetFriction"));
    assert!(categories.contains(&"TransformationExtreme"));

    let scale_sug = suggestions.iter().find(|s| s.category == "ScaleAnchor").unwrap();
    assert!(scale_sug.modifier.contains("110K"));
    assert!(scale_sug.suggested_title.contains("Natural Swim Pond Build"));
    assert!(scale_sug.rationale.contains("Quantifiable volumetric or physical scale"));

    // 2. Tech / Coding Topic:
    let tech_videos = vec![
        TopicVideoInput::new(
            "Rust Async Executor From Scratch",
            "Building async executor.",
            "TechChannel",
            "https://youtube.com/watch?v=rust",
            50_000,
            5.0,
        )
    ];
    let tech_report = TopicMomentumEngine::analyze_cohort_trends("Rust Web Framework", 14, &tech_videos);
    let tech_sug = tech_report.demand_stacking_suggestions;

    let tech_categories: Vec<&str> = tech_sug.iter().map(|s| s.category.as_str()).collect();
    assert!(tech_categories.contains(&"ScaleAnchor"));
    assert!(tech_categories.contains(&"BudgetFriction"));
    assert!(tech_categories.contains(&"TransformationExtreme"));

    let friction = tech_sug.iter().find(|s| s.category == "BudgetFriction").unwrap();
    assert!(friction.modifier.contains("Zero Dependencies"));
    assert!(friction.suggested_title.contains("Rust Web Framework"));
}
