use sandsifter::analyzer::thumbnail::{
    ThumbnailForensicReport, ThumbnailVisionEngine, VisualClickbaitArchetype,
};
use sandsifter::analyzer::{PromiseGapEvaluator, PromiseGapReport};
use sandsifter::export::FlightSheet;
use sandsifter::math::marginal_gains::{MarginalGainsEngine, VectorStatus};
use sandsifter::pipeline::NicheToneProfile;

#[test]
fn test_thumbnail_archetype_serialization() {
    let archetypes = vec![
        VisualClickbaitArchetype::CleanAuthentic,
        VisualClickbaitArchetype::SexuallySuggestiveMetadata,
        VisualClickbaitArchetype::GazeReactionDistortion,
        VisualClickbaitArchetype::DirectingDevices,
        VisualClickbaitArchetype::ManufacturedPeril,
        VisualClickbaitArchetype::PhantomAsset,
        VisualClickbaitArchetype::TabloidStickers,
        VisualClickbaitArchetype::SensoryHyperSaturation,
    ];

    for archetype in &archetypes {
        let serialized = serde_json::to_string(archetype).expect("Serialization failed");
        let deserialized: VisualClickbaitArchetype =
            serde_json::from_str(&serialized).expect("Deserialization failed");
        assert_eq!(*archetype, deserialized);
        assert!(!archetype.badge().is_empty());
    }

    // Verify deceptive classification
    assert!(VisualClickbaitArchetype::PhantomAsset.is_deceptive());
    assert!(VisualClickbaitArchetype::ManufacturedPeril.is_deceptive());
    assert!(VisualClickbaitArchetype::DirectingDevices.is_deceptive());
    assert!(!VisualClickbaitArchetype::CleanAuthentic.is_deceptive());
    assert!(!VisualClickbaitArchetype::SexuallySuggestiveMetadata.is_deceptive());

    // Verify suggestive classification
    assert!(VisualClickbaitArchetype::SexuallySuggestiveMetadata.is_suggestive());
    assert!(!VisualClickbaitArchetype::CleanAuthentic.is_suggestive());
    assert!(!VisualClickbaitArchetype::PhantomAsset.is_suggestive());

    // Verify report roundtrip
    let report = ThumbnailForensicReport {
        suggestive_metadata_score: 0.88,
        primary_archetype: VisualClickbaitArchetype::SexuallySuggestiveMetadata,
        secondary_archetypes: vec![VisualClickbaitArchetype::SensoryHyperSaturation],
        detected_focal_elements: vec!["beachwear".to_string(), "g-string bikini".to_string()],
        packaging_integrity_index: 0.35,
        exogenous_ctr_modifier: 1.55,
        diagnostic_note: "Provocative beachwear frontloaded packaging.".to_string(),
        timestamp_collision_detected: true,
        collision_detail: Some("Text 'SUBSCRIBE NOW' located at x=0.82, y=0.88".to_string()),
        title_text_redundancy_pct: Some(65.0),
        thumbnail_ocr_text: Some("SUBSCRIBE NOW".to_string()),
    };

    let report_json = serde_json::to_string(&report).expect("Report serialization failed");
    let report_back: ThumbnailForensicReport =
        serde_json::from_str(&report_json).expect("Report deserialization failed");
    assert_eq!(report.suggestive_metadata_score, report_back.suggestive_metadata_score);
    assert_eq!(report.primary_archetype, report_back.primary_archetype);
    assert_eq!(report.secondary_archetypes, report_back.secondary_archetypes);
    assert_eq!(report.packaging_integrity_index, report_back.packaging_integrity_index);
    assert_eq!(report.timestamp_collision_detected, report_back.timestamp_collision_detected);
    assert_eq!(report.collision_detail, report_back.collision_detail);
    assert_eq!(report.title_text_redundancy_pct, report_back.title_text_redundancy_pct);
}

#[test]
fn test_packaging_integrity_penalty() {
    let title = "Sailing Across The Atlantic Ocean";
    let hook = "We set sail on day one with 20 knots of wind.";
    let tone = Some(NicheToneProfile::HighStakesPanic);

    // Baseline: Clean evaluation without thumbnail
    let base_v6 = MarginalGainsEngine::evaluate_stakes_lexicon_with_tone(title, Some(hook), tone);

    // Case 1: High suggestive score (0.85) AND severe early retention drop (-4.5%/s)
    let suggestive_report = ThumbnailForensicReport {
        suggestive_metadata_score: 0.85,
        primary_archetype: VisualClickbaitArchetype::SexuallySuggestiveMetadata,
        secondary_archetypes: vec![],
        detected_focal_elements: vec!["swimwear bikini".to_string()],
        packaging_integrity_index: 0.38,
        exogenous_ctr_modifier: 1.45,
        diagnostic_note: "⚠️ Viewership heavily CTR-frontloaded via provocative packaging.".to_string(),
        timestamp_collision_detected: false,
        collision_detail: None,
        title_text_redundancy_pct: None,
        thumbnail_ocr_text: None,
    };

    let penalized_v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail(
        title,
        Some(hook),
        tone,
        Some(&suggestive_report),
        Some(-4.5), // Severe cliff at t < 0:30
    );

    // Expectation Mismatch penalty: exactly -15.0 points from base
    let expected_penalized_score = (base_v6.score_0_to_100 - 15.0).clamp(10.0, 100.0);
    assert_eq!(penalized_v6.score_0_to_100, expected_penalized_score);
    assert!(penalized_v6.one_percent_fix.contains("Expectation Mismatch (-15 pts)"));
    assert!(penalized_v6.one_percent_fix.contains("[SUGGESTIVE PACKAGING]"));
    assert_ne!(penalized_v6.status, VectorStatus::Elite);

    // Case 2: Deceptive Phantom Asset AND steep retention drop (-3.5%/s)
    let deceptive_report = ThumbnailForensicReport {
        suggestive_metadata_score: 0.0,
        primary_archetype: VisualClickbaitArchetype::PhantomAsset,
        secondary_archetypes: vec![],
        detected_focal_elements: vec!["luxury helicopter".to_string()],
        packaging_integrity_index: 0.25,
        exogenous_ctr_modifier: 1.30,
        diagnostic_note: "Phantom asset thumbnail not in video.".to_string(),
        timestamp_collision_detected: false,
        collision_detail: None,
        title_text_redundancy_pct: None,
        thumbnail_ocr_text: None,
    };

    let deceptive_v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail(
        title,
        Some(hook),
        tone,
        Some(&deceptive_report),
        Some(-3.5),
    );

    assert_eq!(deceptive_v6.score_0_to_100, expected_penalized_score);
    assert!(deceptive_v6.one_percent_fix.contains("Expectation Mismatch (-15 pts)"));
    assert!(deceptive_v6.one_percent_fix.contains("[DECEPTIVE CLICKBAIT]"));

    // Case 3: High suggestive score but STABLE retention (-1.0%/s) -> Flagged as Exogenous CTR driver without penalty
    let stable_v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail(
        title,
        Some(hook),
        tone,
        Some(&suggestive_report),
        Some(-1.0), // Stable retention
    );

    assert_eq!(stable_v6.score_0_to_100, base_v6.score_0_to_100);
    assert!(stable_v6.one_percent_fix.contains("Exogenous CTR Driver"));
    assert!(stable_v6.one_percent_fix.contains("[SUGGESTIVE PACKAGING]"));
    assert!(!stable_v6.one_percent_fix.contains("Expectation Mismatch (-15 pts)"));
}

#[test]
fn test_offline_fallback_clean_report() {
    // 1. Default clean URL
    let clean_url = "https://i.ytimg.com/vi/dQw4w9WgXcQ/maxresdefault.jpg";
    let clean_report = ThumbnailVisionEngine::fallback_report(clean_url);

    assert_eq!(clean_report.primary_archetype, VisualClickbaitArchetype::CleanAuthentic);
    assert_eq!(clean_report.suggestive_metadata_score, 0.0);
    assert_eq!(clean_report.packaging_integrity_index, 1.0);
    assert_eq!(clean_report.exogenous_ctr_modifier, 1.0);
    assert!(!clean_report.is_exogenous_ctr_driver());

    // 2. Keyword fallback for test mock URLs
    let suggestive_url = "https://example.com/thumbnails/vanwives_g-string_swimwear.jpg";
    let suggestive_report = ThumbnailVisionEngine::fallback_report(suggestive_url);
    assert_eq!(suggestive_report.primary_archetype, VisualClickbaitArchetype::SexuallySuggestiveMetadata);
    assert!(suggestive_report.suggestive_metadata_score >= 0.70);
    assert!(suggestive_report.is_exogenous_ctr_driver());

    let phantom_url = "https://example.com/thumbnails/bait_switch_phantom_asset.jpg";
    let phantom_report = ThumbnailVisionEngine::fallback_report(phantom_url);
    assert_eq!(phantom_report.primary_archetype, VisualClickbaitArchetype::PhantomAsset);
    assert!(phantom_report.primary_archetype.is_deceptive());

    let peril_url = "https://example.com/thumbnails/sinking_boat_fire_disaster.jpg";
    let peril_report = ThumbnailVisionEngine::fallback_report(peril_url);
    assert_eq!(peril_report.primary_archetype, VisualClickbaitArchetype::ManufacturedPeril);
    assert!(peril_report.primary_archetype.is_deceptive());
}

#[test]
fn test_flight_sheet_renders_visual_packaging_badge_and_note() {
    let title = "Living on a Catamaran in Greece";
    let channel = "Sailing Adventure";

    let thumb_report = ThumbnailForensicReport {
        suggestive_metadata_score: 0.82,
        primary_archetype: VisualClickbaitArchetype::SexuallySuggestiveMetadata,
        secondary_archetypes: vec![VisualClickbaitArchetype::SensoryHyperSaturation],
        detected_focal_elements: vec!["beachwear".to_string()],
        packaging_integrity_index: 0.42,
        exogenous_ctr_modifier: 1.45,
        diagnostic_note: "⚠️ Viewership heavily CTR-frontloaded via provocative packaging; content kinematics lag behind click promise.".to_string(),
        timestamp_collision_detected: false,
        collision_detail: None,
        title_text_redundancy_pct: None,
        thumbnail_ocr_text: None,
    };

    let scorecard = MarginalGainsEngine::generate_scorecard_with_thumbnail(
        "catamaran_101",
        title,
        channel,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        0.5,
        None,
        4.5,
        14,
        8.0,
        Some(-4.2),
        Some(18.0),
        165.0,
        Some("Waking up in Greece"),
        None,
        Some(thumb_report.clone()),
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);

    // 1. Verify terminal output
    let terminal_str = flight_sheet.render_terminal_string();
    assert!(terminal_str.contains("Visual Packaging: [SUGGESTIVE METADATA]"));
    assert!(terminal_str.contains("Packaging Integrity: 42%"));
    assert!(terminal_str.contains("⚠️ Viewership heavily CTR-frontloaded via provocative packaging"));

    // 2. Verify markdown output
    let md_str = flight_sheet.to_markdown_string();
    assert!(md_str.contains("**Visual Packaging**: `[SUGGESTIVE METADATA]`"));
    assert!(md_str.contains("**Packaging Integrity**: `42%"));
    assert!(md_str.contains("**Packaging Note**: ⚠️ Viewership heavily CTR-frontloaded"));
}

#[test]
fn test_timestamp_collision_detection() {
    // 1. Clean report builder has no collision
    let clean = ThumbnailForensicReport::clean_authentic();
    assert!(!clean.timestamp_collision_detected);
    assert!(clean.collision_detail.is_none());

    // 2. Direct evaluation of bottom-right quadrant (x >= 0.75, y >= 0.80)
    let (collided, detail) = ThumbnailVisionEngine::evaluate_timestamp_dead_zone(0.82, 0.85, "Burned-in Subtitle");
    assert!(collided);
    assert!(detail.is_some());
    let detail_str = detail.unwrap();
    assert!(detail_str.contains("dead zone"));

    // 3. Coordinate builder on ThumbnailForensicReport
    let coord_report = ThumbnailForensicReport::clean_authentic()
        .with_element_coordinates(0.78, 0.82, "Episode Badge");
    assert!(coord_report.timestamp_collision_detected);
    assert!(coord_report.collision_detail.as_ref().unwrap().contains("dead zone"));
    assert!(coord_report.collision_detail.as_ref().unwrap().contains("Episode Badge"));

    // 4. Safe coordinate evaluation outside bottom-right dead zone
    let (safe_collided, safe_detail) = ThumbnailVisionEngine::evaluate_timestamp_dead_zone(0.20, 0.20, "Top Left Logo");
    assert!(!safe_collided);
    assert!(safe_detail.is_none());

    // 5. Fallback heuristic detection for mock URLs
    let dead_zone_url = "https://example.com/thumbnails/hero_dead_zone_test.jpg";
    let fallback = ThumbnailVisionEngine::fallback_report(dead_zone_url);
    assert!(fallback.timestamp_collision_detected);
    assert!(fallback.collision_detail.unwrap().contains("bottom-right dead zone"));
}

#[test]
fn test_title_thumbnail_verbal_redundancy_filter() {
    // 1. Target requirement: "I Bought The Cheapest Electric Vehicle" vs "Cheapest EV"
    let target_title = "I Bought The Cheapest Electric Vehicle";
    let target_thumb = "Cheapest EV";
    let redundancy_target = ThumbnailVisionEngine::calculate_title_text_redundancy(target_thumb, target_title);
    assert!(
        redundancy_target > 50.0,
        "Expected word overlap > 50%, got {:.1}%",
        redundancy_target
    );

    let target_report = ThumbnailForensicReport::clean_authentic()
        .with_title_verbal_audit(target_title, target_thumb);
    assert_eq!(target_report.title_text_redundancy_pct, Some(redundancy_target));
    let recommendation = target_report.verbal_redundancy_recommendation();
    assert!(recommendation.is_some());
    assert!(
        recommendation.as_ref().unwrap().contains("Shift from Reinforcement to Complementarity"),
        "Recommendation did not contain Complementarity: {:?}",
        recommendation
    );

    // 2. Distinct title/thumbnail pairings: redundancy remains below 50% threshold
    let distinct_title = "I Bought The Cheapest Electric Vehicle";
    let distinct_thumb = "Will It Survive The Desert?";
    let redundancy_distinct = ThumbnailVisionEngine::calculate_title_text_redundancy(distinct_thumb, distinct_title);
    assert!(
        redundancy_distinct < 50.0,
        "Expected redundancy < 50% for distinct pairing, got {:.1}%",
        redundancy_distinct
    );
    let distinct_report = ThumbnailForensicReport::clean_authentic()
        .with_title_verbal_audit(distinct_title, distinct_thumb);
    assert!(distinct_report.title_text_redundancy_pct.is_none());
    assert!(distinct_report.verbal_redundancy_recommendation().is_none());

    // 3. Off-grid log cabin regression
    let cabin_title = "Building an Off-Grid Log Cabin from Scratch Alone";
    let redundant_thumb = "Building Log Cabin Scratch Alone";
    let redundancy_high = ThumbnailVisionEngine::calculate_title_text_redundancy(redundant_thumb, cabin_title);
    assert!(redundancy_high >= 80.0);
}

#[test]
fn test_flight_sheet_renders_timestamp_collision_and_redundancy_alerts() {
    let title = "Restoring an Old Fishing Boat";
    let channel = "Marine Resto";

    let thumb_report = ThumbnailForensicReport::clean_authentic()
        .with_timestamp_collision(true, Some("Hero outboard gearcase obstructed by YouTube timestamp overlay.".to_string()))
        .with_title_verbal_audit(title, "Restoring Old Fishing Boat");

    let scorecard = MarginalGainsEngine::generate_scorecard_with_thumbnail(
        "boat_101",
        title,
        channel,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.4,
        None,
        4.0,
        15,
        7.0,
        Some(-1.5),
        None,
        155.0,
        Some("Restoring the boat"),
        None,
        Some(thumb_report),
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);

    // Verify Terminal Output
    let terminal = flight_sheet.render_terminal_string();
    assert!(terminal.contains("⚠️ Timestamp Collision: Hero outboard gearcase obstructed"));
    assert!(terminal.contains("⚠️ Verbal Redundancy: Thumbnail text duplicates title by"));
    assert!(terminal.contains("wastes persuasive canvas space"));

    // Verify Markdown Output
    let md = flight_sheet.to_markdown_string();
    assert!(md.contains("⚠️ **Timestamp Collision**: Hero outboard gearcase obstructed"));
    assert!(md.contains("⚠️ **Verbal Redundancy**: Thumbnail text duplicates title by"));
    assert!(md.contains("Shift from Reinforcement to Complementarity"));
}

#[test]
fn test_promise_gap_clean_handshake() {
    let title = "Restoring an Abandoned Timber Cabin";
    let thumb_ctx = Some("Abandoned rustic timber cabin in forest");
    let frame_0 = Some("Drone shot descending on abandoned rustic timber cabin in forest");
    let spoken_hook = Some("Today we are restoring this abandoned timber cabin before winter hits.");
    let spoken_onset = 0.4;

    let report = PromiseGapEvaluator::evaluate(title, thumb_ctx, frame_0, spoken_hook, spoken_onset);

    assert!(!report.is_breach);
    assert_eq!(report.promise_gap_score, 100.0);
    assert!(report.visual_handshake_passed);
    assert!(report.verbal_handshake_passed);
    assert!(!report.throat_clearing_detected);
    assert!(report.diagnostic_alert.is_none());
}

#[test]
fn test_promise_gap_throat_clearing_breach() {
    let title = "Surviving 24 Hours in a Freezing Blizzard";
    let thumb_ctx = Some("Snow cave blizzard survival");
    let frame_0 = Some("Snowy mountain blizzard");
    let spoken_hook = Some("Hey guys welcome back to the channel, before we get started hit that subscribe button!");
    let spoken_onset = 0.8;

    let report = PromiseGapEvaluator::evaluate(title, thumb_ctx, frame_0, spoken_hook, spoken_onset);

    assert!(report.is_breach);
    assert!(report.promise_gap_score < 70.0);
    assert!(report.throat_clearing_detected);
    assert!(report.diagnostic_alert.is_some());
    let alert = report.diagnostic_alert.as_ref().unwrap();
    assert_eq!(alert, PromiseGapReport::BREACH_ALERT);
    assert!(alert.contains("🚨 Promise Gap Breach: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk."));
}

#[test]
fn test_promise_gap_logo_animation_breach() {
    let title = "Building a 100,000 Gallon Concrete Swimming Pool";
    let thumb_ctx = Some("Excavator digging giant swimming pool in backyard");
    let frame_0 = Some("Branded intro logo animation with 3D spinning channel sting");
    let spoken_hook = Some("We are digging the foundation of this giant pool.");
    let spoken_onset = 0.5;

    let report = PromiseGapEvaluator::evaluate(title, thumb_ctx, frame_0, spoken_hook, spoken_onset);

    assert!(report.is_breach);
    assert!(!report.visual_handshake_passed);
    assert!(report.throat_clearing_detected);
    assert_eq!(report.diagnostic_alert.as_deref(), Some(PromiseGapReport::BREACH_ALERT));
}

#[test]
fn test_promise_gap_speech_latency_breach() {
    let title = "How to Rebuild a Broken Diesel Engine";
    let thumb_ctx = Some("Disassembled diesel engine block");
    let frame_0 = Some("Disassembled engine in workshop");
    let spoken_hook = Some("Today we tear down this diesel engine.");
    let spoken_onset = 5.8; // Delayed past 5.0s threshold

    let report = PromiseGapEvaluator::evaluate(title, thumb_ctx, frame_0, spoken_hook, spoken_onset);

    assert!(report.is_breach);
    assert!(!report.verbal_handshake_passed);
    assert!(report.throat_clearing_detected);
    assert!(report.premise_confirmation_second.is_none());
    assert_eq!(report.diagnostic_alert.as_deref(), Some(PromiseGapReport::BREACH_ALERT));
}

#[test]
fn test_promise_gap_marginal_gains_penalties() {
    let title = "Restoring an Old Fishing Boat";
    let channel = "Marine Resto";

    // 1. Clean scorecard with passing handshake
    let clean_report = PromiseGapReport::clean();
    let clean_sc = MarginalGainsEngine::generate_scorecard_with_handshake(
        "boat_101",
        title,
        channel,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.3,
        None,
        4.0,
        15,
        7.0,
        Some(-1.5),
        None,
        155.0,
        Some("Restoring the boat"),
        None,
        None,
        Some(clean_report),
    );

    let clean_v1 = &clean_sc.vectors[0];
    let clean_v6 = &clean_sc.vectors[5];

    // 2. Scorecard with PromiseGap Breach
    let breach_report = PromiseGapReport::breach(
        35.0,
        true,
        "Visual aligned",
        false,
        "No verbal confirmation within 5.0s",
        Some("Biographical throat-clearing intro"),
        None,
    );

    let breached_sc = MarginalGainsEngine::generate_scorecard_with_handshake(
        "boat_101",
        title,
        channel,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.3,
        None,
        4.0,
        15,
        7.0,
        Some(-1.5),
        None,
        155.0,
        Some("Restoring the boat"),
        None,
        None,
        Some(breach_report),
    );

    let breached_v1 = &breached_sc.vectors[0];
    let breached_v6 = &breached_sc.vectors[5];

    // Verify -15 penalty applied to Vector 1
    assert_eq!(breached_v1.score_0_to_100, (clean_v1.score_0_to_100 - 15.0).clamp(10.0, 100.0));
    assert!(breached_v1.one_percent_fix.contains("PromiseGapMismatch (-15 pts)"));
    assert!(breached_v1.one_percent_fix.contains("🚨 Promise Gap Breach"));

    // Verify -15 penalty applied to Vector 6
    assert_eq!(breached_v6.score_0_to_100, (clean_v6.score_0_to_100 - 15.0).clamp(10.0, 100.0));
    assert!(breached_v6.one_percent_fix.contains("PromiseGapMismatch (-15 pts)"));
    assert!(breached_v6.one_percent_fix.contains("🚨 Promise Gap Breach"));
}

#[test]
fn test_flight_sheet_renders_promise_gap_breach_alert() {
    let title = "Building an Off-Grid Cabin";
    let channel = "Homesteaders";

    let breach_gap = PromiseGapReport::breach(
        30.0,
        false,
        "Frame 0 contains branded logo animation",
        false,
        "Speech delayed past 5.0s",
        Some("Intro bumper animation"),
        None,
    );

    let scorecard = MarginalGainsEngine::generate_scorecard_with_handshake(
        "cabin_001",
        title,
        channel,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.5,
        None,
        4.0,
        15,
        7.0,
        Some(-2.0),
        None,
        155.0,
        Some("Building the cabin"),
        None,
        None,
        Some(breach_gap),
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);

    // Verify Terminal Rendering
    let terminal = flight_sheet.render_terminal_string();
    assert!(terminal.contains("🚨 Promise Gap Breach: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk."));
    assert!(terminal.contains("Intro bumper animation"));

    // Verify Markdown Rendering
    let md = flight_sheet.to_markdown_string();
    assert!(md.contains("🚨 **Promise Gap Breach**: First 5 seconds fail visual/verbal handshake with thumbnail/title; high 30-second Intro retention drop risk."));
    assert!(md.contains("Intro bumper animation"));
}

#[test]
fn test_family_outdoor_travelogue_tone_routing_and_gap_suppression() {
    let travel_titles = [
        "Exploring Alaska with My Family",
        "10 Days Fishing and Dog Sledding in Alaska",
        "Our Ultimate Family Vacation Trip Adventure",
        "Wilderness Canoe Trip in Northern Canada",
        "Epic Mountain Adventure Travelogue",
    ];

    for title in travel_titles {
        let detected = NicheToneProfile::detect(title, "Travel & Outdoors", "");
        assert!(
            detected == NicheToneProfile::AestheticLifestyleCozy || detected == NicheToneProfile::DocumentaryBuildProcess,
            "Title \"{}\" must route to AestheticLifestyleCozy or DocumentaryBuildProcess, got {:?}",
            title,
            detected
        );
        assert_ne!(
            detected,
            NicheToneProfile::HighStakesPanic,
            "Title \"{}\" must never route to HighStakesPanic",
            title
        );
    }

    // Verify label for AestheticLifestyleCozy
    assert_eq!(
        NicheToneProfile::AestheticLifestyleCozy.label(),
        "🌿 Family Outdoor Travelogue / Documentary"
    );

    // Verify Promise Gap suppression when dialogue begins immediately (t <= 0.3s) and verbal handshake passes
    let title = "10 Days Fishing, Dog Sledding, & Exploring Alaska";
    let hook = "Nate here with Outdoor Tom and today we are in Alaska fishing and dog sledding";
    let promise_gap = PromiseGapEvaluator::evaluate(
        title,
        Some("Snowy Alaska mountains"),
        None,
        Some(hook),
        0.0,
    );

    assert!(!promise_gap.is_breach);
    assert_eq!(promise_gap.promise_gap_score, 100.0);

    // Verify evaluate_title_stakes_with_thumbnail_and_gap does not apply -15pts penalty
    let v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail_and_gap(
        title,
        Some(hook),
        Some(NicheToneProfile::AestheticLifestyleCozy),
        None,
        None,
        Some(&promise_gap),
    );
    assert!(!v6.one_percent_fix.contains("PromiseGapMismatch (-15 pts)"));
}

#[test]
fn test_authentic_thumbnail_never_triggers_deceptive_clickbait_penalty() {
    let title = "My 1st Bushcraft Camping Trip in Alaska";
    let hook = "Tom here with the Outdoor Tom YouTube channel";
    let tone = Some(NicheToneProfile::DocumentaryBuildProcess);

    let clean_authentic_report = ThumbnailForensicReport {
        suggestive_metadata_score: 0.0,
        primary_archetype: VisualClickbaitArchetype::CleanAuthentic,
        secondary_archetypes: vec![VisualClickbaitArchetype::SensoryHyperSaturation],
        detected_focal_elements: vec!["wilderness forest".to_string()],
        packaging_integrity_index: 0.90, // 90% authentic
        exogenous_ctr_modifier: 1.0,
        diagnostic_note: "Clean authentic outdoor wilderness thumbnail.".to_string(),
        timestamp_collision_detected: false,
        collision_detail: None,
        title_text_redundancy_pct: None,
        thumbnail_ocr_text: None,
    };

    // Even with a steep retention drop (-3.5%/s), CleanAuthentic thumbnails must NEVER receive -15 pt deceptive clickbait penalty
    let v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail(
        title,
        Some(hook),
        tone,
        Some(&clean_authentic_report),
        Some(-3.5),
    );

    assert!(
        !v6.one_percent_fix.contains("Expectation Mismatch (-15 pts)"),
        "Clean authentic thumbnail must never receive Expectation Mismatch penalty, got: {}",
        v6.one_percent_fix
    );
    assert!(
        !v6.one_percent_fix.contains("[DECEPTIVE CLICKBAIT]"),
        "Clean authentic thumbnail must never report DECEPTIVE CLICKBAIT badge, got: {}",
        v6.one_percent_fix
    );
    assert_ne!(v6.status, VectorStatus::CriticalFriction);
}

#[test]
fn test_promise_gap_pending_captions_suppresses_penalty() {
    let title = "Riding 1300 Miles to Alaska on Jet Ski - Camping & Fishing";
    let pending_text = "[Spoken dialogue detected - captions pending]";
    let spoken_onset = 0.2;

    let report = PromiseGapEvaluator::evaluate(
        title,
        Some("Jet Ski in cold Alaskan waters"),
        Some("Action shot of jet ski launching"),
        Some(pending_text),
        spoken_onset,
    );

    // 1. Assert verbal handshake passed with neutral score and no breach
    assert!(report.verbal_handshake_passed, "Verbal handshake must pass on pending captions");
    assert!(report.promise_gap_score >= 85.0, "Score must be neutral/optimal (>= 85.0), got: {}", report.promise_gap_score);
    assert!(!report.is_breach, "Must not flag as breach when speech energy is verified and captions pending");
    assert!(report.diagnostic_alert.is_none(), "Diagnostic alert must be None");
    assert!(report.verbal_handshake_detail.contains("caption processing") || report.verbal_handshake_detail.contains("captions pending"));

    // 2. Assert MarginalGainsEngine does not apply -15 pt penalty to Vector 1 or Vector 6
    let v1 = MarginalGainsEngine::evaluate_cold_open_with_promise_gap(
        spoken_onset,
        None,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        true,
        Some(&report),
    );
    assert!(!v1.one_percent_fix.contains("PromiseGapMismatch (-15 pts)"), "Vector 1 must not apply -15 pt penalty: {}", v1.one_percent_fix);

    let v6 = MarginalGainsEngine::evaluate_title_stakes_with_thumbnail_and_gap(
        title,
        Some(pending_text),
        Some(NicheToneProfile::AestheticLifestyleCozy),
        None,
        Some(-1.5),
        Some(&report),
    );
    assert!(!v6.one_percent_fix.contains("PromiseGapMismatch (-15 pts)"), "Vector 6 must not apply -15 pt penalty: {}", v6.one_percent_fix);
}

#[test]
fn test_semantic_synonyms_satisfy_verbal_handshake() {
    let title = "(103 Salmon & 7 Grizzlies) Epic Alaska Adventure";
    let speech = "Tom here and today we are fishing on the river surrounded by wildlife.";

    let report = PromiseGapEvaluator::evaluate(
        title,
        Some("Action shot of Tom fishing in Alaska with grizzly bear backdrop"),
        Some("Tom standing in river with fly rod"),
        Some(speech),
        0.0,
    );

    assert!(
        report.verbal_handshake_passed,
        "Semantic synonyms (fishing/river/wildlife for salmon/grizzlies) must pass verbal handshake: {}",
        report.verbal_handshake_detail
    );
    assert!(
        !report.is_breach,
        "Must have zero promise gap breach penalty: {:?}",
        report.diagnostic_alert
    );
    assert_eq!(report.promise_gap_score, 100.0);
}



