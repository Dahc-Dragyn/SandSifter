use sandsifter::math::forensics::{
    ForensicsEngine, ForensicEpisodeRecord, SubjectPolarity, TitleDecomposition,
};

#[test]
fn test_acoustic_delivery_analysis() {
    // 12 words in 3.5s -> (12 / 3.5) * 60 = 205.7 WPM (rapid delivery)
    let rapid = ForensicsEngine::analyze_delivery(250, 12);
    assert_eq!(rapid.pre_speech_silence_ms, 250);
    assert_eq!(rapid.wpm_opening, 205.7);
    assert!(rapid.is_rapid_delivery);
    assert!(!rapid.is_sluggish_delivery);

    // 5 words in 3.5s -> (5 / 3.5) * 60 = 85.7 WPM (sluggish delivery)
    let sluggish = ForensicsEngine::analyze_delivery(800, 5);
    assert_eq!(sluggish.pre_speech_silence_ms, 800);
    assert_eq!(sluggish.wpm_opening, 85.7);
    assert!(!sluggish.is_rapid_delivery);
    assert!(sluggish.is_sluggish_delivery);
}

#[test]
fn test_linguistic_decomposition() {
    let hook_jeopardy = "We had to abandon our truck in the desert in Mexico.";
    let decomp = ForensicsEngine::decompose_linguistics(hook_jeopardy);
    assert!(decomp.has_jeopardy_lexicon);
    assert!(!decomp.has_generic_greeting);
    assert_eq!(decomp.subject_polarity, SubjectPolarity::AssetHazard);

    let hook_greeting = "Welcome back pack. Today we are digging footings.";
    let decomp2 = ForensicsEngine::decompose_linguistics(hook_greeting);
    assert!(!decomp2.has_jeopardy_lexicon);
    assert!(decomp2.has_generic_greeting);
    assert_eq!(decomp2.subject_polarity, SubjectPolarity::Community);

    let hook_time = "It's officially been six weeks since we started building our house in Mexico,";
    let decomp3 = ForensicsEngine::decompose_linguistics(hook_time);
    assert!(decomp3.has_time_bomb);
    assert!(decomp3.is_open_loop);
}

#[test]
fn test_spearman_rho_calculation() {
    // Perfect positive rank correlation
    let pred = vec![10.0, 8.0, 6.0, 4.0, 2.0];
    let actual = vec![1.8, 1.5, 1.2, 0.9, 0.7];
    let rho = ForensicsEngine::calculate_spearman_rho(&pred, &actual);
    assert_eq!(rho, 1.0);

    // Inverse rank correlation
    let actual_rev = vec![0.7, 0.9, 1.2, 1.5, 1.8];
    let rho_rev = ForensicsEngine::calculate_spearman_rho(&pred, &actual_rev);
    assert_eq!(rho_rev, -1.0);
}

#[test]
fn test_lag1_autocorrelation() {
    // Constant series -> 0
    let flat = vec![1.0, 1.0, 1.0, 1.0];
    let r_flat = ForensicsEngine::calculate_lag1_autocorrelation(&flat);
    assert_eq!(r_flat, 0.0);

    // Strongly alternating series -> negative autocorrelation
    let alternating = vec![2.0, 0.5, 2.0, 0.5, 2.0, 0.5];
    let r_alt = ForensicsEngine::calculate_lag1_autocorrelation(&alternating);
    assert!(r_alt < 0.0);
}

#[test]
fn test_channel_forensics_report_generation() {
    let mut records = Vec::new();
    for i in 1..=20 {
        let (title, hook, mult) = if i % 4 == 0 {
            ("Truck Peril", "We had to abandon our truck in the desert", 1.6)
        } else if i % 4 == 1 {
            ("Welcome Pack", "Welcome back pack today we build", 0.8)
        } else if i % 4 == 2 {
            ("Six Weeks In", "It's been six weeks since we started,", 1.3)
        } else {
            ("Status Update", "Hey pack welcome back", 0.9)
        };

        let delivery = ForensicsEngine::analyze_delivery(if mult > 1.2 { 100 } else { 500 }, if mult > 1.2 { 11 } else { 6 });
        let linguistics = ForensicsEngine::decompose_linguistics(hook);
        let title_features = ForensicsEngine::decompose_title(title);

        records.push(ForensicEpisodeRecord {
            episode_index: i,
            title: title.to_string(),
            clean_title: title.to_string(),
            views: (mult * 500_000.0) as u64,
            duration: Some("15:30".to_string()),
            published_at: Some("2025-01-12T00:00:00Z".to_string()),
            days_active: 10.0,
            velocity_views_per_day: (mult * 50_000.0),
            channel_multiplier: mult,
            series_multiplier: mult,
            outlier_multiplier: mult,
            spoken_hook: hook.to_string(),
            delivery,
            linguistics,
            title_features,
            visual_pacing: None,
        });
    }

    let report = ForensicsEngine::generate_channel_report(&records);
    assert_eq!(report.total_episodes, 20);
    assert!(report.mean_wpm_high_performers > report.mean_wpm_low_performers);
    assert!(!report.feature_lifts.is_empty());
    assert!(!report.non_obvious_takeaways.is_empty());
    assert_eq!(report.loocv_ablation.total_episodes, 20);
    assert_eq!(report.loocv_ablation.episode_predictions.len(), 20);
}

#[test]
fn test_title_decomposition_peril_vs_picnic() {
    let perilous = ForensicsEngine::decompose_title("Abandoned truck in Mexico border peril crisis");
    assert!(perilous.has_peril_lexicon);
    assert!(!perilous.has_picnic_lexicon);

    let picnic = ForensicsEngine::decompose_title("Relaxing morning coffee and peaceful picnic routine");
    assert!(!picnic.has_peril_lexicon);
    assert!(picnic.has_picnic_lexicon);
}

#[test]
fn test_ambient_non_speech_gating() {
    let delivery = ForensicsEngine::analyze_delivery_explicit(None, Some(0.0), "[NON-SPEECH / AMBIENT]");
    assert!(delivery.is_non_speech);
    assert_eq!(delivery.wpm_0_to_35, 0.0);
    assert_eq!(delivery.pre_speech_silence_ms, 3500);

    let linguistics = ForensicsEngine::decompose_linguistics("[NON-SPEECH / AMBIENT]");
    assert!(linguistics.is_non_speech);
    assert!(!linguistics.has_jeopardy_lexicon);
    assert!(!linguistics.has_generic_greeting);
}

#[test]
fn test_loocv_ablation_model_a_vs_model_b() {
    let mut records = Vec::new();
    for i in 1..=28 {
        // High performers: Peril title + Rapid spoken Jeopardy hook
        let (title, hook, mult, onset, wpm) = if i <= 10 {
            ("Truck Peril Disaster in Mexico", "We had to abandon our truck in the desert,", 1.8, Some(150), Some(190.0))
        } else if i <= 18 {
            ("Six Weeks In Desert", "It's officially been six weeks since we started building,", 1.2, Some(280), Some(140.0))
        } else if i <= 24 {
            ("Relaxing Morning Coffee Routine", "Welcome back pack today we are enjoying coffee", 0.75, Some(650), Some(95.0))
        } else {
            ("Peaceful Mountain Stream", "[NON-SPEECH / AMBIENT]", 0.60, None, Some(0.0))
        };

        let delivery = ForensicsEngine::analyze_delivery_explicit(onset, wpm, hook);
        let linguistics = ForensicsEngine::decompose_linguistics(hook);
        let title_features = ForensicsEngine::decompose_title(title);

        records.push(ForensicEpisodeRecord {
            episode_index: i,
            title: title.to_string(),
            clean_title: title.to_string(),
            views: (mult * 100_000.0) as u64,
            duration: Some("12:45".to_string()),
            published_at: Some("20250119".to_string()),
            days_active: 5.0,
            velocity_views_per_day: (mult * 20_000.0),
            channel_multiplier: mult,
            series_multiplier: mult,
            outlier_multiplier: mult,
            spoken_hook: hook.to_string(),
            delivery,
            linguistics,
            title_features,
            visual_pacing: None,
        });
    }

    let ablation = ForensicsEngine::run_loocv_ablation(&records);
    assert_eq!(ablation.total_episodes, 28);
    assert_eq!(ablation.episode_predictions.len(), 28);
    assert!(ablation.model_a_title_only_rho > 0.0, "Model A should have positive correlation");
    assert!(ablation.model_b_multimodal_rho >= ablation.model_a_title_only_rho, "Model B should beat or match Model A");
    assert!(ablation.incremental_audio_lift_delta_rho >= 0.0);
}

#[test]
fn test_calculate_dual_multipliers() {
    // 10,000 views, Channel Baseline = 1,100, Series Baseline = 1,050
    let (chan_m, ser_m) = ForensicsEngine::calculate_dual_multipliers(10_000, 1_100, 1_050);
    assert_eq!(chan_m, 9.09);
    assert_eq!(ser_m, 9.52);

    // 1,100 views, Channel Baseline = 1,100, Series Baseline = 1,050
    let (chan_m2, ser_m2) = ForensicsEngine::calculate_dual_multipliers(1_100, 1_100, 1_050);
    assert_eq!(chan_m2, 1.0);
    assert_eq!(ser_m2, 1.05);

    // Edge cases with 0 baselines
    let (chan_m3, ser_m3) = ForensicsEngine::calculate_dual_multipliers(500, 0, 0);
    assert_eq!(chan_m3, 500.0);
    assert_eq!(ser_m3, 500.0);
}

#[test]
fn test_boilerplate_title_stripper() {
    // 1. Episode marker & series name stripping
    let raw1 = "WINTER ARRIVES: Tractor Barn, Bathroom Addition -EP84- Log Cabin Build on Off-Grid Homestead";
    let stripped1 = ForensicsEngine::strip_boilerplate_title(raw1);
    assert_eq!(stripped1, "WINTER ARRIVES: Tractor Barn, Bathroom Addition");

    // 2. Podcast & separator stripping
    let raw2 = "Why I Stopped Investing in Fixer Uppers | The Graham Stephan Show #42";
    let stripped2 = ForensicsEngine::strip_boilerplate_title(raw2);
    assert_eq!(stripped2, "Why I Stopped Investing in Fixer Uppers");

    // 3. Corpus-wide repetitive suffix stripping
    let corpus = vec![
        "Best Exercises for Chest Hypertrophy | Jeff Nippard Podcast".to_string(),
        "How to Fix Shoulder Pain in Bench Press | Jeff Nippard Podcast".to_string(),
        "Full Day of Eating for Fat Loss | Jeff Nippard Podcast".to_string(),
        "Science Explained: Muscle Protein Synthesis | Jeff Nippard Podcast".to_string(),
    ];
    let cleaned = ForensicsEngine::clean_corpus_titles(&corpus);
    assert_eq!(cleaned[0], "Best Exercises for Chest Hypertrophy");
    assert_eq!(cleaned[1], "How to Fix Shoulder Pain in Bench Press");
    assert_eq!(cleaned[2], "Full Day of Eating for Fat Loss");
    assert_eq!(cleaned[3], "Science Explained: Muscle Protein Synthesis");
}

#[test]
fn test_velocity_views_per_day_calculation() {
    // Reference date: 2025-01-20 (Epoch seconds: 1737331200)
    let ref_now_epoch = 1737331200;

    // 1. Video published 10 days prior (2025-01-10) with 50,000 views
    let days_1 = ForensicsEngine::calculate_days_active_at(Some("20250110"), ref_now_epoch);
    assert_eq!(days_1, 10.0);
    let vel_1 = ForensicsEngine::calculate_velocity_at(50_000, Some("20250110"), ref_now_epoch);
    assert_eq!(vel_1, 5_000.0); // 50,000 / 10 = 5,000 views/day

    // 2. Video published in ISO format 20 days prior (2024-12-31) with 100,000 views
    let days_2 = ForensicsEngine::calculate_days_active_at(Some("2024-12-31T12:00:00Z"), ref_now_epoch);
    assert_eq!(days_2, 20.0);
    let vel_2 = ForensicsEngine::calculate_velocity_at(100_000, Some("2024-12-31T12:00:00Z"), ref_now_epoch);
    assert_eq!(vel_2, 5_000.0); // 100,000 / 20 = 5,000 views/day

    // 3. Brand new video published today (same day): minimum 1.0 day floor prevents division-by-zero
    let days_today = ForensicsEngine::calculate_days_active_at(Some("20250120"), ref_now_epoch);
    assert_eq!(days_today, 1.0);
    let vel_today = ForensicsEngine::calculate_velocity_at(12_000, Some("20250120"), ref_now_epoch);
    assert_eq!(vel_today, 12_000.0); // 12,000 / 1.0 = 12,000 views/day

    // 4. Missing/None published date defaults to 1.0 day floor
    let days_none = ForensicsEngine::calculate_days_active_at(None, ref_now_epoch);
    assert_eq!(days_none, 1.0);
    let vel_none = ForensicsEngine::calculate_velocity_at(8_500, None, ref_now_epoch);
    assert_eq!(vel_none, 8_500.0);
}

#[test]
fn test_boilerplate_zero_wipeout_fail_safe() {
    // 1. Title that consists strictly of episode/series identifier
    assert_eq!(ForensicsEngine::strip_boilerplate_title("Ep 42"), "Ep 42");
    assert_eq!(ForensicsEngine::strip_boilerplate_title("Episode 100"), "Episode 100");
    assert_eq!(ForensicsEngine::strip_boilerplate_title("#99"), "#99");

    // 2. Title that is just format branding
    assert_eq!(ForensicsEngine::strip_boilerplate_title("Log Cabin Build"), "Log Cabin Build");
    assert_eq!(ForensicsEngine::strip_boilerplate_title("Podcast #1"), "Podcast #1");

    // 3. Leading marker without prefix
    let lead = ForensicsEngine::strip_boilerplate_title("-EP84- Log Cabin Build");
    assert!(!lead.is_empty());
    assert_eq!(lead, "Log Cabin Build");

    // 4. Empty string remains empty
    assert_eq!(ForensicsEngine::strip_boilerplate_title(""), "");
    assert_eq!(ForensicsEngine::strip_boilerplate_title("   "), "");

    // 5. Corpus cleaner zero-wipeout guarantee: non-empty input is NEVER wiped to empty
    let pure_boilerplate_corpus = vec![
        "Homestead Vlog Ep 1".to_string(),
        "Homestead Vlog Ep 2".to_string(),
        "Homestead Vlog Ep 3".to_string(),
    ];
    let cleaned_corpus = ForensicsEngine::clean_corpus_titles(&pure_boilerplate_corpus);
    assert_eq!(cleaned_corpus.len(), 3);
    for c in &cleaned_corpus {
        assert!(!c.trim().is_empty(), "Corpus cleaner must never wipe title to empty");
    }
}

#[test]
fn test_boilerplate_noise_ablation_model_a_lift() {
    // Construct 20 episodes:
    // Episodes 1..=10: Viral Spikes (M = 2.0x..3.35x), True Title = High Stakes Peril ("DISASTER", "CRISIS", etc.)
    // Episodes 11..=20: Low Performers (M = 0.5x..0.77x), True Title = Routine Picnic ("COFFEE", "UPDATE", etc.)
    // Misleading repetitive boilerplate appended to all 20 videos: " - EPxx - Peaceful Mountain Cabin Routine"

    let base_peril = [
        "DISASTER IN MOUNTAINS", "ABANDONED TRUCK CRISIS", "EMERGENCY POLICE ESCAPE",
        "STUCK IN BLIZZARD", "SEIZED VEHICLE PERIL", "WORST TRAIL BREAKDOWN",
        "FATAL DAMAGE WARNING", "STRANDED ALONE IN WILD", "NIGHTMARE OFF-GRID RESCUE", "BORDER DANGER THREAT"
    ];

    let base_picnic = [
        "MORNING COFFEE VLOG", "RELAXING CABIN UPDATE", "EASY DINNER COOKING",
        "PEACEFUL CREEK CHILL", "DAY IN THE LIFE UPDATE", "CABIN TOUR AND HAUL",
        "SHOPPING FOR WINTER", "COZY MORNING ROUTINE", "WEEKEND Q&A UPDATE", "SPRING CABIN TOUR"
    ];

    let mut raw_titles = Vec::new();
    let mut multipliers = Vec::new();

    for (i, p) in base_peril.iter().enumerate() {
        raw_titles.push(format!("{} - EP{:02} - Peaceful Mountain Cabin Routine", p, i + 1));
        multipliers.push(2.0 + (i as f64) * 0.15); // 2.0x to 3.35x
    }
    for (i, p) in base_picnic.iter().enumerate() {
        raw_titles.push(format!("{} - EP{:02} - Peaceful Mountain Cabin Routine", p, i + 11));
        multipliers.push(0.50 + (i as f64) * 0.03); // 0.50x to 0.77x
    }

    // Cohort 1: UNCLEANED raw titles (boilerplate poisons Model A keywords)
    let mut uncleaned_records = Vec::new();
    for (idx, raw_t) in raw_titles.iter().enumerate() {
        let mult = multipliers[idx];
        let delivery = ForensicsEngine::analyze_delivery(200, 10);
        let linguistics = ForensicsEngine::decompose_linguistics("Today we take on the challenge");
        let lower = raw_t.to_lowercase();
        let uncleaned_title_features = TitleDecomposition {
            has_peril_lexicon: lower.contains("disaster") || lower.contains("crisis") || lower.contains("peril") || lower.contains("stuck"),
            has_picnic_lexicon: true, // Poisoned by "Peaceful Mountain Cabin Routine"!
            has_time_bomb: false,
        };

        uncleaned_records.push(ForensicEpisodeRecord {
            episode_index: idx + 1,
            title: raw_t.clone(),
            clean_title: raw_t.clone(),
            views: (mult * 100_000.0) as u64,
            duration: Some("15:00".to_string()),
            published_at: Some("20250101".to_string()),
            days_active: 10.0,
            velocity_views_per_day: mult * 10_000.0,
            channel_multiplier: mult,
            series_multiplier: mult,
            outlier_multiplier: mult,
            spoken_hook: "Today we take on the challenge".to_string(),
            delivery,
            linguistics,
            title_features: uncleaned_title_features,
            visual_pacing: None,
        });
    }

    let uncleaned_loocv = ForensicsEngine::run_loocv_ablation(&uncleaned_records);

    // Cohort 2: CLEANED titles using Statistical Corpus Frequency Filter
    let cleaned_corpus_titles = ForensicsEngine::clean_corpus_titles(&raw_titles);
    let mut cleaned_records = Vec::new();
    for (idx, raw_t) in raw_titles.iter().enumerate() {
        let mult = multipliers[idx];
        let clean_t = &cleaned_corpus_titles[idx];
        let delivery = ForensicsEngine::analyze_delivery(200, 10);
        let linguistics = ForensicsEngine::decompose_linguistics("Today we take on the challenge");
        let cleaned_title_features = ForensicsEngine::decompose_title(clean_t);

        cleaned_records.push(ForensicEpisodeRecord {
            episode_index: idx + 1,
            title: raw_t.clone(),
            clean_title: clean_t.clone(),
            views: (mult * 100_000.0) as u64,
            duration: Some("15:00".to_string()),
            published_at: Some("20250101".to_string()),
            days_active: 10.0,
            velocity_views_per_day: mult * 10_000.0,
            channel_multiplier: mult,
            series_multiplier: mult,
            outlier_multiplier: mult,
            spoken_hook: "Today we take on the challenge".to_string(),
            delivery,
            linguistics,
            title_features: cleaned_title_features,
            visual_pacing: None,
        });
    }

    let cleaned_loocv = ForensicsEngine::run_loocv_ablation(&cleaned_records);

    println!("\n========================================================");
    println!("     BOILERPLATE NOISE ABLATION TEST (EMPIRICAL LIFT)");
    println!("========================================================");
    println!("Uncleaned (Poisoned) Model A Spearman ρ : {:.4}", uncleaned_loocv.model_a_title_only_rho);
    println!("Cleaned (Stripped) Model A Spearman ρ   : {:.4}", cleaned_loocv.model_a_title_only_rho);
    let delta_rho = cleaned_loocv.model_a_title_only_rho - uncleaned_loocv.model_a_title_only_rho;
    println!("Empirical Predictive Lift (Δρ)         : {:+.4}", delta_rho);
    println!("========================================================\n");

    // Verify empirical predictive lift: Cleaned Model A Spearman rho must beat Uncleaned Model A
    assert!(
        cleaned_loocv.model_a_title_only_rho > uncleaned_loocv.model_a_title_only_rho,
        "Cleaned Model A rho ({:.2}) must be strictly higher than Uncleaned rho ({:.2})",
        cleaned_loocv.model_a_title_only_rho,
        uncleaned_loocv.model_a_title_only_rho
    );
    assert!(cleaned_loocv.model_a_title_only_rho >= 0.50, "Cleaned Model A must achieve strong positive rank correlation >= 0.50");
}

#[test]
fn test_stabilized_wpm_calculation() {
    // Case 1: Ultra-short 0.4s burst with 3 words (e.g. "hey welcome back")
    // Without floor: 3 words / 0.4s * 60 = 450 WPM!
    // With 2.0s floor: 3 words / 2.0s * 60 = 90 WPM.
    let wpm_short = ForensicsEngine::calculate_stabilized_wpm(3, 0.4);
    assert_eq!(wpm_short, 90.0);

    // Case 2: 1-second burst with 5 words
    // With 2.0s floor: 5 words / 2.0s * 60 = 150 WPM.
    let wpm_1s = ForensicsEngine::calculate_stabilized_wpm(5, 1.0);
    assert_eq!(wpm_1s, 150.0);

    // Case 3: Standard 3.5s window with 10 words
    // 10 / 3.5 * 60 = 171.4 WPM
    let wpm_full = ForensicsEngine::calculate_stabilized_wpm(10, 3.5);
    assert_eq!(wpm_full, 171.4);

    // Case 4: Extreme outlier clamping to human maximum 260 WPM
    let wpm_clamped = ForensicsEngine::calculate_stabilized_wpm(25, 2.0);
    assert_eq!(wpm_clamped, 260.0);

    // Case 5: analyze_delivery_explicit sanitizes superhuman external WPM
    let delivery_extreme = ForensicsEngine::analyze_delivery_explicit(Some(500), Some(575.1), "Welcome back to the channel today");
    assert!(delivery_extreme.wpm_0_to_35 <= 260.0);
    assert!(delivery_extreme.wpm_opening <= 260.0);

    // Case 6: Standard 5.0s active speech window with 15 words
    // 15 / 5.0 * 60 = 180.0 WPM
    let wpm_5s = ForensicsEngine::calculate_stabilized_wpm(15, 5.0);
    assert_eq!(wpm_5s, 180.0);

    // Case 7: Interpolated fractional word count over 5.0s
    // 14.65 words / 5.0s * 60 = 175.8 WPM
    let wpm_interp = ForensicsEngine::calculate_interpolated_wpm(14.65, 5.0);
    assert_eq!(wpm_interp, 175.8);
}

#[test]
fn test_dynamic_acoustic_rules_synthesis() {
    // Delos profile: High performers take 1.3s visual pause and 133 WPM;
    // Low performers rush speech at 0.5s and 228 WPM.
    let (wpm_rule, silence_rule) = ForensicsEngine::synthesize_local_acoustic_rules(
        133.1,
        228.5,
        1.3,
        0.5,
    );

    // Dynamic silence rule must recognize that visual lag outperformed immediate speech
    assert!(
        silence_rule.contains("dramatic visual hook") || silence_rule.contains("cinematic pause"),
        "Rule should praise visual pause, got: {}", silence_rule
    );
    assert!(!silence_rule.contains("Cut straight to talking"));

    // Dynamic WPM rule must recognize that measured composure beat rushed speech
    assert!(
        wpm_rule.contains("Controlled composure") || wpm_rule.contains("measured speech"),
        "Rule should praise measured speech, got: {}", wpm_rule
    );

    // Contrasting profile: Fast gaming/tech channel where 0.1s dead air and 190 WPM urgency wins
    let (wpm_rule_fast, silence_rule_fast) = ForensicsEngine::synthesize_local_acoustic_rules(
        190.0,
        120.0,
        0.1,
        1.2,
    );

    assert!(silence_rule_fast.contains("Cut straight to talking"));
    assert!(wpm_rule_fast.contains("Urgent pacing"));
}


