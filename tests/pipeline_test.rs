use sandsifter::mcp::McpServer;
use sandsifter::models::video::{VideoChannelDetails, VideoDetail};
use sandsifter::pipeline::{HookMachineEngine, NicheToneProfile, TripleVectorScore};

fn create_mock_video(
    uuid: &str,
    handle: &str,
    views: u64,
    likes: u64,
    comments: u64,
    shares: u64,
) -> VideoDetail {
    VideoDetail {
        uuid: uuid.to_string(),
        caption: Some(format!("Sample video {}", uuid)),
        url: format!("https://www.tiktok.com/@{}/video/{}", handle, uuid),
        thumbnail: None,
        channel: VideoChannelDetails {
            uuid: format!("chan_{}", handle),
            platform: "tiktok".to_string(),
            handle: handle.to_string(),
            display_name: Some(handle.to_string()),
            avatar_url: None,
            follower_count: Some(50_000),
            baseline_median_views: Some(15_000),
        },
        view_count: views,
        like_count: Some(likes),
        comment_count: Some(comments),
        share_count: Some(shares),
        outlier_score: 1.0,
        engagement_rate: ((likes + comments + shares) as f64 / views as f64) * 100.0,
        published_at: "2026-03-01T12:00:00Z".to_string(),
        velocity_views_per_day: None,
        analysis: None,
        speech_onset_ms: None,
        wpm_0_to_35: None,
        spoken_hook: None,
    }
}

#[test]
fn test_triple_vector_scoring_weights() {
    // 40% Psych + 35% Lex + 25% Gram
    // 0.40(9.0) + 0.35(8.0) + 0.25(10.0) = 3.6 + 2.8 + 2.5 = 8.9
    let score = TripleVectorScore::compute(
        9.0,
        8.0,
        10.0,
        "High fear of incompetence",
        "Technical entity anchors",
        "Optimal sub-3.5s punch",
    );

    assert_eq!(score.total_score, 8.9);
    assert_eq!(score.psychology_score, 9.0);
    assert_eq!(score.lexicon_score, 8.0);
    assert_eq!(score.grammar_score, 10.0);
}

#[test]
fn test_score_hook_heuristics() {
    let hook = "Stop using standard mutexes for Rust microservices. You are destroying your CPU cache.";
    let score = HookMachineEngine::score_hook(hook);

    assert!(
        score.total_score >= 8.5,
        "Score should be high for top-tier hook, was: {}",
        score.total_score
    );
    assert!(score.psychology_score >= 8.0);
    assert!(score.lexicon_score >= 8.0);
    assert!(score.grammar_score >= 8.0);
}

#[test]
fn test_channel_isolation() {
    // Cohort mixing two different creators
    let videos = vec![
        // 5 videos for @alex_rust: baseline ~15k
        create_mock_video("a1", "@alex_rust", 14_000, 700, 100, 50),
        create_mock_video("a2", "@alex_rust", 15_000, 750, 120, 60),
        create_mock_video("a3", "@alex_rust", 16_000, 800, 110, 70),
        create_mock_video("a4", "@alex_rust", 15_500, 780, 130, 65),
        create_mock_video("a5", "@alex_rust", 300_000, 18_000, 2_000, 1_500), // 20x outlier
        // 5 videos for @sarah_python: baseline ~2k
        create_mock_video("s1", "@sarah_python", 1_800, 90, 20, 10),
        create_mock_video("s2", "@sarah_python", 2_000, 100, 25, 15),
        create_mock_video("s3", "@sarah_python", 2_200, 110, 30, 20),
        create_mock_video("s4", "@sarah_python", 2_100, 105, 22, 18),
        create_mock_video("s5", "@sarah_python", 50_000, 3_000, 500, 400), // 25x outlier
    ];

    // Execute for @alex_rust only
    let report_alex = HookMachineEngine::execute("@alex_rust", "Rust Concurrency", &videos);
    assert_eq!(report_alex.total_videos_analyzed, 5);
    assert!(
        (14_000..=16_000).contains(&report_alex.creator_baseline_views),
        "Alex's baseline should be isolated around 15k, was: {}",
        report_alex.creator_baseline_views
    );

    // Execute for @sarah_python only
    let report_sarah = HookMachineEngine::execute("@sarah_python", "Async Python", &videos);
    assert_eq!(report_sarah.total_videos_analyzed, 5);
    assert!(
        (1_800..=2_500).contains(&report_sarah.creator_baseline_views),
        "Sarah's baseline should be isolated around 2k, was: {}",
        report_sarah.creator_baseline_views
    );
}

#[test]
fn test_paid_boost_filtering_in_pipeline() {
    let videos = vec![
        // 4 organic videos
        create_mock_video("v1", "@creator", 10_000, 500, 100, 50),
        create_mock_video("v2", "@creator", 12_000, 600, 120, 60),
        create_mock_video("v3", "@creator", 11_000, 550, 110, 55),
        // 1 paid boosted ad: 500,000 views but only 600 interactions (0.12% engagement < 2.0%)
        create_mock_video("v4", "@creator", 500_000, 400, 150, 50),
    ];

    let report = HookMachineEngine::execute("@creator", "Distributed Systems", &videos);
    assert_eq!(report.total_videos_analyzed, 4);
    assert_eq!(report.paid_boosts_filtered, 1, "Paid ad should be filtered out");
    assert_eq!(report.outlier_videos_found, 0, "No organic outliers in this cohort");
}

#[test]
fn test_hook_machine_report_and_markdown() {
    let report = HookMachineEngine::execute("@kallaway", "Data Engineering", &[]);

    assert_eq!(report.adapted_hooks.len(), 5);
    assert_eq!(report.scratch_hooks.len(), 5);
    assert_eq!(report.rubric.rubric_rules.len(), 4);

    let md = report.to_markdown();
    assert!(md.contains("# 🎯 The Hook Machine Report: @kallaway"));
    assert!(md.contains("## 📊 Creator Baseline & Outlier Intelligence"));
    assert!(md.contains("## 📐 Channel Custom Grading Rubric"));
    assert!(md.contains("## 🏆 Top Ranked Hook Overall"));
    assert!(md.contains("### 5 Adapted Hooks"));
    assert!(md.contains("### 5 Scratch Hooks"));
}

#[test]
fn test_mcp_hook_machine_tool_invocation() {
    let mut server = McpServer::new();
    let result = server.execute_tool(
        "/hook-machine",
        &serde_json::json!({
            "channel_handle": "@alex_rust",
            "topic": "Zero-Allocation Networking"
        }),
    );

    assert!(!result.is_error);
    let text = &result.content[0].text;
    assert!(text.contains("The Hook Machine Report: @alex_rust"));
    assert!(text.contains("Zero-Allocation Networking"));
    assert!(text.contains("Top Ranked Hook Overall"));
}

#[test]
fn test_hook_machine_feedback_loop_and_diagnostics() {
    let mut videos = Vec::new();
    // 20 normal videos around 10k
    for i in 1..=20 {
        videos.push(create_mock_video(&format!("v_{}", i), "datasciencedaily", 10_000 + (i as u64 * 100), 500, 30, 20));
    }
    // 2 viral breakout outliers (100k, 500k)
    videos.push(create_mock_video("v_spike1", "datasciencedaily", 100_000, 10_000, 800, 1500));
    videos.push(create_mock_video("v_spike2", "datasciencedaily", 500_000, 45_000, 3500, 9000));

    let validated = vec!["Stop using [X] for [Y]. You are destroying your [Z].".to_string()];
    let report = HookMachineEngine::execute_with_feedback(
        "@datasciencedaily",
        "Polars Benchmarks",
        &videos,
        &validated,
    );

    // Verify diagnostics exposed
    assert!(report.baseline_diagnostics.is_some());
    let diag = report.baseline_diagnostics.as_ref().unwrap();
    assert_eq!(diag.sample_size, 22);
    assert!(diag.upper_clamp_bound < 500_000.0, "Upper clamp should be well below 500k spike");
    assert!(diag.clamped_spikes_count >= 1, "At least 1 mega spike clamped");

    // Verify feedback loop rule incorporated
    assert!(report.rubric.rubric_rules[0].contains("Closed-Loop Empirical Feedback"));
    assert_eq!(report.rubric.winning_formulas[0], "Stop using [X] for [Y]. You are destroying your [Z].");

    let md = report.to_markdown();
    assert!(md.contains("IQR Clamping Floor/Ceiling:"));
    assert!(md.contains("Viral Spikes Clamped:"));
}

#[test]
fn test_db_tracked_hooks_lifecycle() {
    let db = sandsifter::storage::SandsifterDb::memory().unwrap();
    let hook_id = "test_hook_001";

    let record = sandsifter::storage::TrackedHookRecord {
        id: hook_id.to_string(),
        channel_handle: "@creator_studio".to_string(),
        topic: "Fantasy Writing".to_string(),
        hook_text: "Stop introducing characters in front of a mirror.".to_string(),
        hook_type: "Adapted".to_string(),
        source_formula: Some("Stop [X] in [Y].".to_string()),
        predicted_score: 9.2,
        status: "generated".to_string(),
        published_url: None,
        actual_views: None,
        actual_multiplier: None,
        actual_engagement_rate: None,
        performance_notes: None,
        created_at: "2026-03-01T12:00:00Z".to_string(),
        updated_at: "2026-03-01T12:00:00Z".to_string(),
    };

    db.save_tracked_hook(&record).unwrap();

    let list = db.list_tracked_hooks(Some("@creator_studio"), None, 10).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, "generated");

    // Mark as filmed
    assert!(db.update_tracked_hook_status(hook_id, "filmed", None).unwrap());
    let filmed = db.list_tracked_hooks(None, Some("filmed"), 10).unwrap();
    assert_eq!(filmed.len(), 1);

    // Record performance (e.g. 150,000 views, 3.5x multiplier)
    assert!(db.record_hook_performance(hook_id, 150_000, 3.5, 8.2, Some("High comments debate")).unwrap());

    // Verify feedback loop pulls winning formula
    let winners = db.get_validated_winning_formulas("@creator_studio", 2.0).unwrap();
    assert_eq!(winners.len(), 1);
    assert_eq!(winners[0], "Stop [X] in [Y].");
}

#[test]
fn test_high_stakes_lexicon_scoring_boosts() {
    // High-stakes hook combining Ego Confession + Time Bomb + Threatened Asset + Irreversible Loss
    let high_stakes_hook = "I had 24 hours to burn the letters or watch my sister take the blame.";
    let score = HookMachineEngine::score_hook(high_stakes_hook);

    assert!(score.psychology_score >= 9.5, "Psychology score should be >= 9.5 with high stakes, got: {}", score.psychology_score);
    assert!(score.lexicon_score >= 9.0, "Lexicon score should be >= 9.0 with threatened assets, got: {}", score.lexicon_score);
    assert!(score.total_score >= 9.0, "Total score should be >= 9.0 for top-tier hook, got: {}", score.total_score);
    assert!(score.psychology_notes.contains("High-stakes moral jeopardy"));
    assert!(score.psychology_notes.contains("Empirical Ego Subject"));

    // Neutral hook lacking stakes or peril
    let bland_hook = "Here are five nice books that you might want to read today.";
    let bland_score = HookMachineEngine::score_hook(bland_hook);
    assert!(bland_score.psychology_score <= 7.5, "Bland hook should have baseline psychology score");
    assert!(bland_score.lexicon_score <= 8.5, "Bland hook should have lower lexicon score");
    assert!(score.total_score > bland_score.total_score, "High-stakes hook must significantly outscore bland hook");
}

#[test]
fn test_word_count_cadence_ceiling_enforcement() {
    // 12 words: squarely in 9-14 optimal window
    let optimal_hook = "They told me the cabin was abandoned. They lied about who died inside.";
    let opt_score = HookMachineEngine::score_hook(optimal_hook);
    assert!(opt_score.grammar_score >= 9.5, "12 words should score >= 9.5 on grammar, got: {}", opt_score.grammar_score);
    assert!(opt_score.grammar_notes.contains("Optimal 9-14 words"));

    // 22 words: severely exceeds 14-word ceiling
    let verbose_hook = "I had twenty-four hours to burn all the letters or watch my sister take the blame for a terrible crime she never committed in town.";
    let verbose_score = HookMachineEngine::score_hook(verbose_hook);
    assert!(verbose_score.grammar_score <= 6.0, "Over-length hook must be heavily penalized on grammar, got: {}", verbose_score.grammar_score);
    assert!(verbose_score.grammar_notes.contains("Strict penalty: Exceeds 9-14 word ceiling"));

    // 4 words: too brief, lacks narrative tension
    let brief_hook = "I burned the letters.";
    let brief_score = HookMachineEngine::score_hook(brief_hook);
    assert!(brief_score.grammar_score <= 6.5, "Under-length hook must be penalized for brevity, got: {}", brief_score.grammar_score);
    assert!(brief_score.grammar_notes.contains("Too brief"));
}

#[test]
fn test_dynamic_niche_lexicon_synthesis_and_universal_archetypes() {
    // 1. Real Estate Creator Niche
    let report_re = HookMachineEngine::execute(
        "@GrahamStephan",
        "Real Estate Foreclosures",
        &[],
    );

    assert!(report_re.niche_lexicon.niche_category.to_lowercase().contains("real estate"));
    assert!(!report_re.niche_lexicon.irreversible_loss.is_empty());
    assert!(!report_re.niche_lexicon.threatened_assets.is_empty());
    assert_eq!(report_re.scratch_hooks.len(), 5);

    // Verify all scratch hooks have archetype formula tags and proper duration
    for hook in &report_re.scratch_hooks {
        assert!(hook.source_formula.is_some(), "Scratch hook should have a named archetype formula");
        assert!(hook.estimated_duration_sec > 0.0 && hook.estimated_duration_sec <= 5.0);
        assert!(hook.score.total_score >= 8.0, "Real estate scratch hook should achieve elite score, was: {}", hook.score.total_score);
    }

    // Verify presence of universal signature archetypes
    let formulas: Vec<String> = report_re.scratch_hooks.iter().map(|h| h.source_formula.clone().unwrap()).collect();
    assert!(formulas.contains(&"The Irreversible Choice".to_string()));
    assert!(formulas.contains(&"The Asset Under Threat".to_string()));
    assert!(formulas.contains(&"The Ego Confession".to_string()));
    assert!(formulas.contains(&"The Deadline Time-Bomb".to_string()));
    assert!(formulas.contains(&"The Hidden Truth".to_string()));

    // Verify variable injection occurred
    let choice_hook = report_re.scratch_hooks.iter().find(|h| h.source_formula.as_deref() == Some("The Irreversible Choice")).unwrap();
    assert!(choice_hook.hook_text.starts_with("I had"));

    // 2. Fitness / Training Creator Niche
    let report_fit = HookMachineEngine::execute(
        "@JeffNippard",
        "Fitness Hypertrophy Injury",
        &[],
    );
    assert!(report_fit.niche_lexicon.niche_category.to_lowercase().contains("fitness") || report_fit.niche_lexicon.niche_category.to_lowercase().contains("training") || report_fit.niche_lexicon.niche_category.to_lowercase().contains("hypertrophy"));
    assert!(!report_fit.niche_lexicon.irreversible_loss.is_empty());
    assert!(!report_fit.niche_lexicon.threatened_assets.is_empty());

    let md = report_re.to_markdown();
    assert!(md.contains("Niche Category:** Real Estate") || md.contains("Rental Property"));
    assert!(md.contains("High-Stakes Mode:** 🔥 Dynamic Lexicon Active"));
}

#[test]
fn test_mcp_hook_machine_universal_niche() {
    let mut server = McpServer::new();
    let result = server.execute_tool(
        "/hook-machine",
        &serde_json::json!({
            "channel_handle": "@GrahamStephan",
            "topic": "Real Estate Rental Property"
        }),
    );

    assert!(!result.is_error);
    let text = &result.content[0].text;
    assert!(text.contains("Niche Category:** Real Estate") || text.contains("Rental Property"));
    assert!(text.contains("High-Stakes Mode:** 🔥 Dynamic Lexicon Active"));
    assert!(text.contains("The Irreversible Choice"));
}

#[test]
fn test_culinary_short_does_not_route_to_high_stakes_panic() {
    use sandsifter::pipeline::NicheToneProfile;

    // 1. Dobrovolskyi Chef Short with un-keyworded title
    let chef_short_title = "Today I’m not surprised";
    let chef_channel = "Dobrovolskyi Chef";
    let detected_chef = NicheToneProfile::detect(chef_short_title, chef_channel, "");
    assert_eq!(
        detected_chef,
        NicheToneProfile::AestheticLifestyleCozy,
        "Cooking short by Dobrovolskyi Chef must route to AestheticLifestyleCozy, got: {:?}",
        detected_chef
    );
    assert_ne!(
        detected_chef,
        NicheToneProfile::HighStakesPanic,
        "Cooking short must never route to HighStakesPanic"
    );

    // 2. Explicit culinary title without channel clue
    let egg_title = "Cooking Perfect Scrambled Eggs";
    let detected_egg = NicheToneProfile::detect(egg_title, "", "");
    assert_eq!(
        detected_egg,
        NicheToneProfile::AestheticLifestyleCozy,
        "Title 'Cooking Perfect Scrambled Eggs' must route to AestheticLifestyleCozy, got: {:?}",
        detected_egg
    );
    assert_ne!(
        detected_egg,
        NicheToneProfile::HighStakesPanic,
        "Culinary title must never route to HighStakesPanic"
    );

    // 3. Un-keyworded neutral title with no channel clue should fall through to AestheticLifestyleCozy, NOT HighStakesPanic
    let neutral_title = "Today I’m not surprised";
    let detected_neutral = NicheToneProfile::detect(neutral_title, "", "");
    assert_eq!(
        detected_neutral,
        NicheToneProfile::AestheticLifestyleCozy,
        "Neutral un-keyworded title must fall back to AestheticLifestyleCozy, got: {:?}",
        detected_neutral
    );
    assert_ne!(
        detected_neutral,
        NicheToneProfile::HighStakesPanic,
        "Neutral title must never default to HighStakesPanic"
    );
}

#[test]
fn test_startalk_routes_to_educational_breakdown() {
    let title = "Is Chemistry the Same Throughout the Universe? With Kate the Chemist";
    let channel = "StarTalk";
    let detected = NicheToneProfile::detect(title, channel, "");
    assert_eq!(
        detected,
        NicheToneProfile::EducationalBreakdown,
        "StarTalk chemistry episode must route to EducationalBreakdown, got: {:?}",
        detected
    );
}

#[test]
fn test_mrbeast_does_not_route_to_documentary_build() {
    let title = "$1 vs $500,000 Experiences!";
    let channel = "MrBeast";
    let hook_text = "I rented the most expensive experiences in the world and each one gets crazier and crazier like this $100...";
    let detected = NicheToneProfile::detect(title, channel, hook_text);
    assert_eq!(
        detected,
        NicheToneProfile::EntertainmentSpectacle,
        "MrBeast tiered challenge must route to EntertainmentSpectacle, got: {:?}",
        detected
    );
    assert_ne!(
        detected,
        NicheToneProfile::DocumentaryBuildProcess,
        "MrBeast challenge must never route to DocumentaryBuildProcess"
    );
}

