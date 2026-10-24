use sandsifter::export::{FlightSheet, PackagingUpgrade, TruncationRadar, extract_dynamic_subject, sanitize_subject_noun};
use sandsifter::math::marginal_gains::{MarginalGainsEngine, ViewerSatisfactionMetrics};
use sandsifter::pipeline::NicheToneProfile;

#[test]
fn test_flight_sheet_constructs_all_four_punch_list_categories() {
    // Construct a scorecard representing a video with typical retention challenges
    let title = "Building an Off-Grid Cabin With 10,000 lbs of Logs";
    let channel = "Wilderness Craft";

    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "cabin_vid_101",
        title,
        channel,
        Some(NicheToneProfile::DocumentaryBuildProcess),
        2.8,
        None,
        5.2,
        11,
        18.5,
        Some(-6.8),
        Some(124.0),
        160.0,
        Some("Starting the build"),
        Some(ViewerSatisfactionMetrics::new(50000, 2500, 320)),
    );

    // Provide an explicit outro exit note for the fourth category
    scorecard.outro_exit_note = Some("Early Outro Exit detected at 14:15—skip verbal 'wrapping up' cues".to_string());

    // Generate FlightSheet from scorecard
    let flight_sheet = FlightSheet::from_scorecard(&scorecard)
        .with_duration_seconds(960.0) // 16:00
        .with_channel_name(channel);

    // 1. Verify Core Header & Verdict Metadata
    assert_eq!(flight_sheet.video_title, title);
    assert_eq!(flight_sheet.channel_name, channel);
    assert_eq!(flight_sheet.duration_formatted, "16:00");
    assert_eq!(flight_sheet.tone_profile_formatted, "🚜 Documentary Build & Transformation");
    assert!(flight_sheet.overall_verdict.contains("Health Grade"));
    assert!(flight_sheet.compounded_gain_pct > 0.0);
    assert!(flight_sheet.audience_satisfaction_tier.contains("Satisfaction"));

    // 2. Verify All Four Punch List Categories Are Present
    let punch_list = &flight_sheet.timeline_punch_list;
    assert!(punch_list.len() >= 4, "Punch list must have at least 4 items");

    let action_types: Vec<&str> = punch_list.iter().map(|c| c.action_type.as_str()).collect();
    assert!(
        action_types.contains(&"TRIM DEAD AIR"),
        "Missing TRIM DEAD AIR category. Found: {:?}", action_types
    );
    assert!(
        action_types.contains(&"INSERT B-ROLL"),
        "Missing INSERT B-ROLL category. Found: {:?}", action_types
    );
    assert!(
        action_types.contains(&"CUT EXPOSITION"),
        "Missing CUT EXPOSITION category. Found: {:?}", action_types
    );
    assert!(
        action_types.contains(&"OUTRO FIX"),
        "Missing OUTRO FIX category. Found: {:?}", action_types
    );

    // 3. Verify Specific Punch List Details
    let dead_air_cue = punch_list.iter().find(|c| c.action_type == "TRIM DEAD AIR").unwrap();
    assert!(dead_air_cue.timecode_range.starts_with("00:00"));
    assert!(dead_air_cue.impact_summary.contains("autoplay"));

    let broll_cue = punch_list.iter().find(|c| c.action_type == "INSERT B-ROLL").unwrap();
    assert!(broll_cue.remedy_instruction.contains("18.5s") || broll_cue.remedy_instruction.contains("shot"));

    let exposition_cue = punch_list.iter().find(|c| c.action_type == "CUT EXPOSITION").unwrap();
    assert_eq!(exposition_cue.timecode_range, "02:04");
    assert!(exposition_cue.impact_summary.contains("viewer") || exposition_cue.impact_summary.contains("cliff"));

    let outro_cue = punch_list.iter().find(|c| c.action_type == "OUTRO FIX").unwrap();
    assert_eq!(outro_cue.timecode_range, "14:15");
    assert!(outro_cue.remedy_instruction.contains("Early Outro Exit"));

    // 4. Verify Packaging Upgrades
    let pkg = &flight_sheet.packaging_upgrades;
    assert_eq!(pkg.current_title, title);
    assert!(!pkg.title_upgrade_a.is_empty());
    assert!(!pkg.title_upgrade_b.is_empty());
    assert!(!pkg.opening_line_upgraded.is_empty());
    assert!(pkg.opening_line_upgraded.contains("cabin"));
    assert!(!pkg.title_upgrade_a.contains("10,000 lbs of Steel"));

    // 5. Verify Terminal Output Renderer
    let term_output = flight_sheet.render_terminal_string();
    assert!(term_output.contains("EDITOR'S FLIGHT SHEET"));
    assert!(term_output.contains("TIMELINE EDITING PUNCH LIST"));
    assert!(term_output.contains("TRIM DEAD AIR"));
    assert!(term_output.contains("INSERT B-ROLL"));
    assert!(term_output.contains("CUT EXPOSITION"));
    assert!(term_output.contains("OUTRO FIX"));
    assert!(term_output.contains("PACKAGING & HOOK UPGRADES"));
    assert!(term_output.contains("Runtime   : 16:00"));
    assert!(term_output.contains("Niche Tone: 🚜 Documentary Build & Transformation"));

    // 6. Verify Markdown Output Renderer
    let md_output = flight_sheet.to_markdown_string();
    assert!(md_output.contains("# ✈️ Editor's Flight Sheet"));
    assert!(md_output.contains("## 📋 Timeline Punch List (NLE Editing Commands)"));
    assert!(md_output.contains("| `00:00 - 00:03` | **TRIM DEAD AIR**"));
    assert!(md_output.contains("## 📦 Packaging & Hook Upgrades"));
    assert!(md_output.contains("> **Runtime**: `16:00`"));
    assert!(md_output.contains("> **Niche Tone Profile**: `🚜 Documentary Build & Transformation`"));

    // 7. Verify Timeline Export with Markers
    let timeline = flight_sheet.to_export_timeline();
    assert!(!timeline.shots.is_empty());
    assert_eq!(timeline.shots[0].markers.len(), 4);

    let fcpxml = timeline.to_fcpxml_string();
    assert!(fcpxml.contains("start=\"0s\""), "Trim dead air marker should start at 0s: {}", fcpxml);
    assert!(fcpxml.contains("start=\"10s\""), "Insert b-roll marker should be at hold midpoint (10s): {}", fcpxml);
    assert!(fcpxml.contains("start=\"124s\""), "Cut exposition marker should start at 124s (02:04): {}", fcpxml);
    assert!(fcpxml.contains("start=\"855s\""), "Outro fix marker should start at 855s (14:15): {}", fcpxml);
}

#[test]
fn test_fcpxml_marker_distinct_timecodes_and_midpoint_outro() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "midpoint_vid",
        "Building a Log Cabin Alone in the Woods",
        "Wilderness Craft",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.3,
        None,
        4.0,
        15,
        10.0,
        Some(-1.4),
        None,
        180.0,
        Some("Building alone"),
        None,
    );
    // Explicit exposition at 30:11 (1811s)
    if let Some(v4) = scorecard.vectors.get_mut(3) {
        v4.one_percent_fix = "Audience decay is steady (-1.4%/s); place a forward narrative question or open loop at 30:11 before the steepest dip.".to_string();
    }

    let flight_sheet = FlightSheet::from_scorecard(&scorecard)
        .with_duration_seconds(600.0); // 10:00 (600s)

    // Verify punch list contains 30:11 and Final 15%
    let exp_cue = flight_sheet.timeline_punch_list.iter().find(|c| c.action_type == "CUT EXPOSITION").unwrap();
    assert_eq!(exp_cue.timecode_range, "30:11");

    let outro_cue = flight_sheet.timeline_punch_list.iter().find(|c| c.action_type == "OUTRO FIX").unwrap();
    assert_eq!(outro_cue.timecode_range, "Final 15%");

    // Verify FCPXML timeline export
    let timeline = flight_sheet.to_export_timeline();
    let fcpxml = timeline.to_fcpxml_string();

    // 30:11 -> 30*60 + 11 = 1811s
    assert!(fcpxml.contains("start=\"1811s\""), "Must contain 30:11 (1811s) marker: {}", fcpxml);
    // Final 15% of 600s -> 600 * 0.85 = 510s
    assert!(fcpxml.contains("start=\"510s\""), "Must contain Final 15% (510s) marker: {}", fcpxml);
}

#[test]
fn test_flight_sheet_handles_clean_baseline_video() {
    let title = "Building a 10,000 lbs Steel Machine in 10 Days: Full Build Breakdown";
    let channel = "TechDev";

    // Perfect video with zero defects
    let scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "clean_101",
        title,
        channel,
        Some(NicheToneProfile::EducationalBreakdown),
        0.2, // 0.2s elite
        None,
        4.5,
        15,
        5.0, // 5.0s elite hold
        Some(-0.8), // gentle decay
        None,
        170.0,
        Some("Let's build a compiler in 10 minutes"),
        Some(ViewerSatisfactionMetrics::new(100000, 8000, 950)),
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);

    // Punch list must still construct all four categories (with positive/maintenance advice)
    let action_types: Vec<&str> = flight_sheet.timeline_punch_list.iter().map(|c| c.action_type.as_str()).collect();
    assert_eq!(action_types, vec!["TRIM DEAD AIR", "INSERT B-ROLL", "CUT EXPOSITION", "OUTRO FIX"]);

    assert!(flight_sheet.overall_verdict.contains("VIRAL BREAKOUT") || flight_sheet.overall_verdict.contains("HEALTHY"));
    assert_eq!(flight_sheet.primary_bottleneck, "None. Pacing and retention vectors are performing above optimal baseline.");
}

#[test]
fn test_packaging_upgrade_dynamic_tone_no_hardcoded_steel_on_wood_or_sailing() {
    use sandsifter::export::PackagingUpgrade;

    // 1. Woodworking / Tree / Canoe title under DocumentaryBuildProcess
    let wood_title = "Building a Wooden Canoe from a Single Tree Trunk";
    let wood_pkg = PackagingUpgrade::from_title_and_tone(wood_title, Some(NicheToneProfile::DocumentaryBuildProcess));
    assert!(!wood_pkg.title_upgrade_a.contains("Steel"), "Should not contain Steel on wood: {}", wood_pkg.title_upgrade_a);
    assert!(!wood_pkg.title_upgrade_a.contains("10,000 lbs"), "Should not contain 10,000 lbs on wood: {}", wood_pkg.title_upgrade_a);
    assert!(!wood_pkg.title_upgrade_b.contains("Huge Mistake"), "Should not contain hardcoded Huge Mistake: {}", wood_pkg.title_upgrade_b);
    assert!(!wood_pkg.opening_line_upgraded.contains("In the next 60 seconds"), "Should not contain marketing jargon: {}", wood_pkg.opening_line_upgraded);
    assert!(wood_pkg.title_upgrade_a.contains("Canoe") || wood_pkg.title_upgrade_a.contains("Tree"));
    assert!(wood_pkg.opening_line_upgraded.contains("fallen tree") || wood_pkg.opening_line_upgraded.contains("canoe"));

    // 2. Sailing title under AestheticLifestyleCozy
    let sail_title = "Sailing Across the Pacific in a 30ft Boat";
    let sail_pkg = PackagingUpgrade::from_title_and_tone(sail_title, Some(NicheToneProfile::AestheticLifestyleCozy));
    assert!(!sail_pkg.title_upgrade_a.contains("Steel"), "Should not contain Steel on sailing: {}", sail_pkg.title_upgrade_a);
    assert!(!sail_pkg.title_upgrade_a.contains("10,000 lbs"), "Should not contain 10,000 lbs on sailing: {}", sail_pkg.title_upgrade_a);
    assert!(sail_pkg.title_upgrade_a.contains("Sailing") || sail_pkg.title_upgrade_a.contains("Ocean"));
    assert!(sail_pkg.opening_line_upgraded.contains("sailed offshore") || sail_pkg.opening_line_upgraded.contains("ocean"));

    // 3. Log Cabin / Homestead title under DocumentaryBuildProcess
    let cabin_title = "I Left the City to Build a Log Cabin & Off-Grid Homestead |EP1|";
    let cabin_pkg = PackagingUpgrade::from_title_and_tone(cabin_title, Some(NicheToneProfile::DocumentaryBuildProcess));
    assert!(!cabin_pkg.title_upgrade_a.contains("Steel"));
    assert_eq!(cabin_pkg.title_upgrade_a, "Building an Off-Grid Log Cabin from Scratch (30 Days Alone)");
    assert_eq!(cabin_pkg.title_upgrade_b, "Can One Person Build a Log Cabin Alone?");
    assert_eq!(
        cabin_pkg.opening_line_upgraded,
        "\"I gave up everything in the city to see if I could build a log cabin with my own two hands.\""
    );

    // 4. Narrative title under NarrativeEmotionalPitch
    let narrative_title = "Throne of Glass trailer";
    let narrative_pkg = PackagingUpgrade::from_title_and_tone(narrative_title, Some(NicheToneProfile::NarrativeEmotionalPitch));
    assert!(!narrative_pkg.title_upgrade_a.contains("Steel"));
    assert!(narrative_pkg.title_upgrade_a.contains("Blade") || narrative_pkg.title_upgrade_a.contains("Traitor"));
    assert!(narrative_pkg.title_upgrade_b.contains("POV:"));
    assert!(narrative_pkg.opening_line_upgraded.contains("royal guard") || narrative_pkg.opening_line_upgraded.contains("captive"));

    // 5. Educational title under EducationalBreakdown
    let edu_title = "Building a Full-Stack Compiler in Rust";
    let edu_pkg = PackagingUpgrade::from_title_and_tone(edu_title, Some(NicheToneProfile::EducationalBreakdown));
    assert!(!edu_pkg.title_upgrade_a.contains("Steel"));
    assert!(edu_pkg.title_upgrade_a.contains("Production-Ready") || edu_pkg.title_upgrade_a.contains("Architecture"));
    assert!(edu_pkg.opening_line_upgraded.contains("architectural shift"));
}

#[test]
fn test_packaging_upgrade_scale_downgrade_prevention() {
    use sandsifter::export::PackagingUpgrade;

    // 1. Dual-Milestone Hybrid: 2 Years + 5 Buildings
    let hybrid_title = "I Left the City to Build a Log Cabin Homestead: 2 Years + 5 Buildings";
    let hybrid_pkg = PackagingUpgrade::from_title_and_tone(hybrid_title, Some(NicheToneProfile::DocumentaryBuildProcess));
    assert!(!hybrid_pkg.title_upgrade_a.contains("30 Days"), "Must not downgrade 2 years + 5 buildings to 30 days: {}", hybrid_pkg.title_upgrade_a);
    assert_eq!(hybrid_pkg.title_upgrade_a, "2 Years Off-Grid: Building 5 Log Cabins Completely Alone");
    assert_eq!(hybrid_pkg.title_upgrade_b, "What 2 Years of Building an Off-Grid Homestead Actually Looks Like");
    assert!(hybrid_pkg.opening_line_upgraded.contains("Two years ago"));

    // 2. Solo 1 Year Milestone
    let one_year_title = "ONE YEAR SOLO: Building a Log Cabin & Homestead in the Wild";
    let one_year_pkg = PackagingUpgrade::from_title_and_tone(one_year_title, Some(NicheToneProfile::DocumentaryBuildProcess));
    assert!(!one_year_pkg.title_upgrade_a.contains("30 Days"), "Must not downgrade 1 year solo to 30 days: {}", one_year_pkg.title_upgrade_a);
    assert_eq!(one_year_pkg.title_upgrade_a, "1 Year Off-Grid: Building a Log Cabin & Homestead Completely Alone");
    assert_eq!(one_year_pkg.title_upgrade_b, "What 1 Year of Building an Off-Grid Cabin Alone Actually Looks Like");
    assert!(one_year_pkg.opening_line_upgraded.contains("One year ago"));
}

#[test]
fn test_mechanical_flip_challenge_tone_routing_and_packaging() {
    use sandsifter::export::PackagingUpgrade;

    let flip_title = "How to Turn $100 into $1,000 Flipping Boat Motors! (Best Side Hustle Ever)";
    let channel = "BOAT BUSTERS";

    // 1. Verify tone detection routes to DocumentaryBuildProcess rather than AestheticLifestyleCozy or EducationalBreakdown
    let detected_tone = NicheToneProfile::detect(flip_title, channel, "");
    assert_eq!(
        detected_tone,
        NicheToneProfile::DocumentaryBuildProcess,
        "Mechanical flipping challenge must route to DocumentaryBuildProcess, got: {:?}",
        detected_tone
    );

    // 2. Verify packaging upgrades produce mechanical flip challenge framing
    let flip_pkg = PackagingUpgrade::from_title_and_tone(flip_title, Some(detected_tone));
    assert!(!flip_pkg.title_upgrade_a.contains("Sailing"), "Should not contain Sailing: {}", flip_pkg.title_upgrade_a);
    assert!(!flip_pkg.title_upgrade_a.contains("Open Ocean"), "Should not contain Open Ocean: {}", flip_pkg.title_upgrade_a);
    assert!(flip_pkg.title_upgrade_a.contains("Turning $100 into $1,000") || flip_pkg.title_upgrade_a.contains("Flipping"));
    assert!(flip_pkg.title_upgrade_b.contains("Flipping") || flip_pkg.title_upgrade_b.contains("$1,000"));
    assert!(flip_pkg.opening_line_upgraded.contains("$100 bill"));
}

#[test]
fn test_local_draft_flight_sheet_formatting_and_export_paths() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "render_v1_draft",
        "How I Built a Tiny House in 30 Days",
        "Local Editor Draft",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.3,
        None,
        2.8,
        35,
        5.2,
        Some(-1.1),
        None,
        188.0,
        Some("building a tiny house in 30 days"),
        None,
    );
    scorecard.is_local_draft = true;

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    assert!(flight_sheet.is_local_draft);

    // Verify draft suffix in export paths
    assert!(flight_sheet.export_paths[0].ends_with("_draft_flight_sheet.md"));
    assert!(flight_sheet.export_paths[1].ends_with("_draft_template.fcpxml"));
    assert!(flight_sheet.export_paths[2].ends_with("_draft.edl"));

    // Verify terminal header shows Editor's Pre-Publish Draft Sheet
    let term = flight_sheet.render_terminal_string();
    assert!(term.contains("EDITOR'S PRE-PUBLISH DRAFT SHEET"));

    // Verify saving creates files with _draft suffix
    let temp_dir = tempfile::tempdir().unwrap();
    let md_path = flight_sheet.save_to_markdown(temp_dir.path()).unwrap();
    assert!(md_path.file_name().unwrap().to_str().unwrap().contains("_draft_flight_sheet.md"));
    assert!(md_path.exists());

    let xml_path = flight_sheet.save_fcpxml_template(temp_dir.path()).unwrap();
    assert!(xml_path.file_name().unwrap().to_str().unwrap().contains("_draft_template.fcpxml"));
    assert!(xml_path.exists());

    let xml_content = std::fs::read_to_string(&xml_path).unwrap();
    assert!(xml_content.contains("<!DOCTYPE fcpxml>"));
    assert!(xml_content.contains("<fcpxml version=\"1.8\">"));
    assert!(xml_content.contains("<asset id=\"r2\" name=\"Sandsifter_Guide_Track\""));
    assert!(xml_content.contains("<video ref=\"r2\""));
    assert!(temp_dir.path().join(".sandsifter_slug.png").exists());

    // Verify saving companion DaVinci Resolve EDL template
    let edl_path = flight_sheet.save_edl_template(temp_dir.path()).unwrap();
    assert!(edl_path.file_name().unwrap().to_str().unwrap().contains("_draft.edl"));
    assert!(edl_path.exists());

    let edl_content = std::fs::read_to_string(&edl_path).unwrap();
    assert!(edl_content.contains("TITLE: HOW I BUILT A TINY HOUSE IN 30 DAYS"));
    assert!(edl_content.contains("FCM: NON-DROP FRAME"));
    assert!(edl_content.contains("|C:ResolveColorCyan"));
    assert!(edl_content.contains("|C:ResolveColorPurple"));
}

#[test]
fn test_fcpxml_single_click_slug_asset_and_video_spine() {
    let scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "single_click_resolve_test",
        "Single Click DaVinci Resolve Marker Test",
        "Resolve Master",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.2,
        None,
        2.5,
        40,
        5.0,
        Some(-0.8),
        None,
        195.0,
        Some("single click davinci resolve"),
        None,
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let temp_dir = tempfile::tempdir().unwrap();
    let xml_path = flight_sheet.save_fcpxml_template(temp_dir.path()).unwrap();
    assert!(xml_path.exists());

    let content = std::fs::read_to_string(&xml_path).unwrap();
    assert!(content.contains("<asset id=\"r2\" name=\"Sandsifter_Guide_Track\""));
    assert!(content.contains("<video ref=\"r2\""));
    assert!(content.contains("<marker start="));

    let slug_file = temp_dir.path().join(".sandsifter_slug.png");
    assert!(slug_file.exists());
    assert_eq!(std::fs::read(&slug_file).unwrap().len(), 67);
}

#[test]
fn test_flight_sheet_markdown_editor_target_matrix() {
    let scorecard = MarginalGainsEngine::generate_scorecard_full(
        "fs_matrix_test",
        "Building a Tiny House in 30 Days",
        "Homestead Craft",
        0.3,
        3.2,
        22,
        8.0,
        Some(-1.2),
        None,
        185.0,
        Some("building tiny house"),
        None,
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    assert_eq!(flight_sheet.telemetry_matrix.len(), 6);

    let md = flight_sheet.to_markdown_string();
    assert!(md.contains("## 📊 6-Vector Telemetry Matrix Audit"));
    assert!(md.contains("| Telemetry Vector | Measured | Editor Target | Score | Status | 1% Actionable Editing Prescription | Projected Lift |"));
    assert!(!md.contains("| Benchmark |"));

    // Verify zero LaTeX markers
    assert!(!md.contains("$\\le"), "FlightSheet markdown contains LaTeX \\le: {}", md);
    assert!(!md.contains("$\\ge"), "FlightSheet markdown contains LaTeX \\ge: {}", md);
    assert!(!md.contains("\\text{"), "FlightSheet markdown contains LaTeX \\text: {}", md);
    assert!(!md.contains("\\%"), "FlightSheet markdown contains LaTeX \\%: {}", md);
}

#[test]
fn test_truncation_radar_cutoffs_45_52_60() {
    // 1. Short title (< 45 chars): all safe
    let short_title = "Building a Tiny House in 30 Days"; // 32 chars
    let radar_short = TruncationRadar::evaluate(short_title);
    assert_eq!(radar_short.title_chars_count, 32);
    assert!(!radar_short.mobile_push_truncated);
    assert!(!radar_short.mobile_feed_ctv_truncated);
    assert!(!radar_short.desktop_search_truncated);
    assert!(radar_short.front_load_recommendation.is_none());

    // 2. Medium title (48-52 chars: > 45 push cutoff, <= 52 mobile/CTV cutoff)
    let med_title = "Building an Off-Grid Cabin Completely From Scratch"; // 50 chars
    let radar_med = TruncationRadar::evaluate(med_title);
    assert!(radar_med.mobile_push_truncated);
    assert!(!radar_med.mobile_feed_ctv_truncated);
    assert!(!radar_med.desktop_search_truncated);
    assert!(radar_med.front_load_recommendation.is_none());

    // 3. CTV-truncated title (56 chars: > 52 CTV cutoff, <= 60 desktop cutoff)
    let ctv_title = "Building an Off-Grid Log Cabin with 10,000 lbs of Timber"; // 56 chars
    let radar_ctv = TruncationRadar::evaluate(ctv_title);
    assert!(radar_ctv.mobile_push_truncated);
    assert!(radar_ctv.mobile_feed_ctv_truncated);
    assert!(!radar_ctv.desktop_search_truncated);
    assert!(radar_ctv.front_load_recommendation.is_some());

    // 4. Long title (74 chars: truncated on all platforms)
    let long_title = "Surviving 100 Days Alone in the Arctic Tundra with Only a Knife and a Tarp"; // 74 chars
    let radar_long = TruncationRadar::evaluate(long_title);
    assert!(radar_long.mobile_push_truncated);
    assert!(radar_long.mobile_feed_ctv_truncated);
    assert!(radar_long.desktop_search_truncated);
    assert!(radar_long.front_load_recommendation.is_some());
}

#[test]
fn test_flight_sheet_renders_truncation_radar_section_2() {
    let title = "Sailing Across The Pacific Ocean in a Tiny 24 Foot Boat Completely Alone"; // 72 chars
    let scorecard = MarginalGainsEngine::generate_scorecard_full(
        "truncation_test",
        title,
        "Solo Sailor",
        0.5,
        4.0,
        18,
        8.0,
        Some(-2.0),
        None,
        150.0,
        Some("Sailing alone"),
        None,
    );

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    assert!(flight_sheet.truncation_radar.is_some());

    let radar = flight_sheet.truncation_radar.as_ref().unwrap();
    assert!(radar.mobile_push_truncated);
    assert!(radar.mobile_feed_ctv_truncated);
    assert!(radar.desktop_search_truncated);

    // Verify Terminal Rendering
    let terminal = flight_sheet.render_terminal_string();
    assert!(terminal.contains("CROSS-DEVICE TITLE TRUNCATION RADAR"));
    assert!(terminal.contains("Mobile Push (45c)"));
    assert!(terminal.contains("Mobile/CTV (52c)"));
    assert!(terminal.contains("Desktop Search (60c)"));
    assert!(terminal.contains("[⚠️ TRUNCATED ON MOBILE/CTV]"));
    assert!(terminal.contains("Truncation Alert: Front-load the core hook!"));

    // Verify Markdown Rendering
    let md = flight_sheet.to_markdown_string();
    assert!(md.contains("### 📱 Cross-Device Title Truncation Radar"));
    assert!(md.contains("`[⚠️ TRUNCATED ON MOBILE/CTV]`"));
    assert!(md.contains("**Recommended Front-Load**"));
}

#[test]
fn test_truncation_radar_multisurface_formatting() {
    // 1. Explicit 68-character title
    let title_68 = "I Left the City to Build a Log Cabin & Off-Grid Homestead Solo |EP1|";
    assert_eq!(title_68.chars().count(), 68);

    let radar = TruncationRadar::evaluate(title_68);
    assert_eq!(radar.title_chars_count, 68);

    // Assert mobile_push truncates at 45 characters with an ellipsis (...)
    assert!(radar.mobile_push_truncated);
    let push_content = radar.mobile_push_preview.trim_matches('"');
    assert_eq!(push_content.chars().count(), 45);
    assert!(push_content.ends_with("..."));

    // Assert mobile_feed_ctv truncates at 52 characters with an ellipsis (...)
    assert!(radar.mobile_feed_ctv_truncated);
    let ctv_content = radar.mobile_feed_ctv_preview.trim_matches('"');
    assert_eq!(ctv_content.chars().count(), 52);
    assert!(ctv_content.ends_with("..."));

    // Assert desktop_search truncates at 60 characters with an ellipsis (...)
    assert!(radar.desktop_search_truncated);
    let desktop_content = radar.desktop_search_preview.trim_matches('"');
    assert_eq!(desktop_content.chars().count(), 60);
    assert!(desktop_content.ends_with("..."));

    // 2. Also verify prompt's example string
    let example_title = "I Left the City to Build a Log Cabin & Off-Grid Homestead |EP1|";
    let radar_ex = TruncationRadar::evaluate(example_title);
    assert!(radar_ex.mobile_push_truncated);
    assert!(radar_ex.mobile_feed_ctv_truncated);
    assert!(radar_ex.desktop_search_truncated);
    assert_eq!(radar_ex.mobile_push_preview.trim_matches('"').chars().count(), 45);
    assert!(radar_ex.mobile_push_preview.ends_with("...\""));
    assert_eq!(radar_ex.mobile_feed_ctv_preview.trim_matches('"').chars().count(), 52);
    assert!(radar_ex.mobile_feed_ctv_preview.ends_with("...\""));
    assert_eq!(radar_ex.desktop_search_preview.trim_matches('"').chars().count(), 60);
    assert!(radar_ex.desktop_search_preview.ends_with("...\""));

    // 3. Assert generated terminal Flight Sheet renders CROSS-DEVICE TITLE TRUNCATION RADAR block with advice
    let scorecard = MarginalGainsEngine::generate_scorecard_full(
        "cabin_truncation_flight_sheet",
        title_68,
        "Homestead Craft",
        0.5,
        4.0,
        18,
        8.0,
        Some(-2.0),
        None,
        150.0,
        Some("Starting cabin build"),
        None,
    );
    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let terminal = flight_sheet.render_terminal_string();

    assert!(terminal.contains("📱 CROSS-DEVICE TITLE TRUNCATION RADAR"));
    assert!(terminal.contains("Mobile Push (45c)"));
    assert!(terminal.contains("Mobile/CTV (52c)"));
    assert!(terminal.contains("Desktop Search (60c)"));
    assert!(terminal.contains("[⚠️ TRUNCATED ON MOBILE/CTV]"));
    assert!(terminal.contains("Truncation Alert: Front-load the core hook!"));
    assert!(terminal.contains("Recommended Front-Load:"));
}

#[test]
fn test_packaging_upgrade_no_raw_title_slot_stuffing() {
    let raw_title = "DON'T BUY AN AIRSTREAM / our journey is coming to an end";
    let pkg = PackagingUpgrade::from_title_and_tone(raw_title, None);

    // 1. Asserts that Option A produces clean consumer advisory packaging
    assert_eq!(
        pkg.title_upgrade_a,
        "The Real Cost of Living in an Airstream (What Dealers Hide)"
    );

    // 2. Asserts that Option B produces clean curiosity & truth packaging
    assert_eq!(
        pkg.title_upgrade_b,
        "Why We're Giving Up on Our Airstream After 2 Years"
    );

    // 3. Asserts 4 Ps opening hook
    assert_eq!(
        pkg.opening_line_upgraded,
        "Before you spend $100,000 on an Airstream, here is the harsh reality nobody tells you."
    );

    // 4. Asserts zero wilderness/log cabin phrasing or raw title slot-stuffing
    assert!(!pkg.title_upgrade_a.contains("Wilderness"));
    assert!(!pkg.title_upgrade_a.contains("Log Cabin"));
    assert!(!pkg.title_upgrade_a.contains("Building a"));
    assert!(!pkg.title_upgrade_b.contains("Homestead"));
    assert!(!pkg.title_upgrade_b.contains("Alone in the"));
    assert!(!pkg.opening_line_upgraded.contains("left the city"));
    assert!(!pkg.title_upgrade_a.contains("our journey is coming to an end"));
}

#[test]
fn test_cuttable_fat_clamps_insane_streaming_shot_holds() {
    // Synthetic video lasting 1000 seconds with 0 detected cuts (unbroken 1,000s shot interval)
    let total_duration_sec = 1000.0;
    let cuts: Vec<f64> = vec![];
    let breath_pauses: Vec<f64> = vec![];

    let fat = MarginalGainsEngine::calculate_cuttable_fat(
        total_duration_sec,
        0.2,
        Some(NicheToneProfile::HighStakesPanic),
        &cuts,
        &breath_pauses,
    );

    // Verify unbroken 1000s shot is flagged as a streaming cut-detection dropout
    assert_eq!(fat.streaming_dropouts_count, 1);

    // Verifies that a synthetic 1,000-second shot is clamped and does NOT report 15 minutes (900s) of false fat
    // (Instead of 993s of false hold fat, it is clamped to <= 20.0s)
    assert!(
        fat.camera_hold_fat_seconds <= 20.0,
        "Camera hold fat must be clamped, got: {:.1}s",
        fat.camera_hold_fat_seconds
    );
    assert!(
        fat.total_cuttable_seconds < 60.0,
        "Total cuttable fat must not report 15 minutes of false fat, got: {:.1}s",
        fat.total_cuttable_seconds
    );

    // Zero punch-in cues should be placed across a streaming dropout
    assert_eq!(fat.punch_in_cues.len(), 0);

    // Verify summary notes the clamped streaming dropout
    assert!(fat.executive_summary.contains("clamped 1 streaming cut dropout"));
}

#[test]
fn test_packaging_upgrade_never_outputs_literal_word_project() {
    let test_titles = [
        ("My 1st Bushcraft Camping Trip in Alaska", NicheToneProfile::DocumentaryBuildProcess),
        ("3 Days Fishing for Alaskan Cod from Shore", NicheToneProfile::DocumentaryBuildProcess),
        ("Solo Winter Survival in Alaska", NicheToneProfile::DocumentaryBuildProcess),
        ("Building a Deep Woods Shelter with Hand Tools", NicheToneProfile::DocumentaryBuildProcess),
        ("Surviving 10 Days in the Wilderness Alone", NicheToneProfile::AestheticLifestyleCozy),
        ("Random Unrecognized Obscure Custom Title", NicheToneProfile::DocumentaryBuildProcess),
    ];

    for (title, tone) in test_titles {
        let upgrade = PackagingUpgrade::from_title_and_tone(title, Some(tone));
        assert!(
            !upgrade.title_upgrade_a.contains("Project"),
            "title_upgrade_a for \"{}\" must never contain literal 'Project', got: {}",
            title,
            upgrade.title_upgrade_a
        );
        assert!(
            !upgrade.title_upgrade_b.contains("Project"),
            "title_upgrade_b for \"{}\" must never contain literal 'Project', got: {}",
            title,
            upgrade.title_upgrade_b
        );
        assert!(
            !upgrade.opening_line_upgraded.contains("Project"),
            "opening_line_upgraded for \"{}\" must never contain literal 'Project', got: {}",
            title,
            upgrade.opening_line_upgraded
        );
    }
}

#[test]
fn test_boat_busters_tone_routing_and_packaging() {
    let title = "I Tried Fixing the Worst Boat on Facebook. What could go Wrong?";
    let channel = "BOAT BUSTERS";
    let detected_tone = NicheToneProfile::detect(title, channel, "");
    assert_eq!(
        detected_tone,
        NicheToneProfile::DocumentaryBuildProcess,
        "Facebook in title must not trigger Romance/Fiction tone"
    );

    let upgrade = PackagingUpgrade::from_title_and_tone(title, Some(detected_tone));
    assert!(
        upgrade.title_upgrade_a.contains("Facebook") || upgrade.title_upgrade_a.contains("Boat"),
        "Must generate boat restoration/marketplace titles, got: {}",
        upgrade.title_upgrade_a
    );
    assert!(
        !upgrade.title_upgrade_a.contains("Project"),
        "Must never contain literal word Project"
    );
    assert!(
        !upgrade.title_upgrade_a.contains("Sailing Across"),
        "Must not generate offshore sailing titles for boat repairs"
    );
}

#[test]
fn test_dynamic_subject_extraction_no_niche_bleed() {
    use sandsifter::export::extract_dynamic_subject;

    // 1. Shower Waterproofing Method: dynamic subject extraction must not bleed into wilderness/camp/fish
    let shower_title = "The Old American Shower Waterproofing Method";
    let shower_subject = extract_dynamic_subject(shower_title);
    assert_eq!(
        shower_subject, "Shower Waterproofing Method",
        "Subject must be 'Shower Waterproofing Method', got: {}",
        shower_subject
    );
    assert!(!shower_subject.contains("Wilderness"));
    assert!(!shower_subject.contains("Camp"));
    assert!(!shower_subject.contains("Fish"));

    let shower_pkg = PackagingUpgrade::from_title_and_tone(shower_title, Some(NicheToneProfile::EducationalBreakdown));
    assert!(!shower_pkg.title_upgrade_a.contains("Wilderness"));
    assert!(!shower_pkg.title_upgrade_a.contains("Camp"));
    assert!(!shower_pkg.title_upgrade_a.contains("Fish"));
    assert!(!shower_pkg.title_upgrade_b.contains("Wilderness"));
    assert!(!shower_pkg.title_upgrade_b.contains("Camp"));
    assert!(!shower_pkg.title_upgrade_b.contains("Fish"));
    assert!(shower_pkg.title_upgrade_a.contains("Shower Waterproofing Method"));

    // 2. Jet Ski title with trailing fishing tag: must preserve Jet Ski subject and avoid Wild Fish hallucination
    let jet_ski_title = "Riding 1300 Miles to Alaska on Jet Ski - Camping & Fishing";
    let jet_ski_subject = extract_dynamic_subject(jet_ski_title);
    assert!(
        jet_ski_subject.contains("Jet Ski"),
        "Subject must relate to 'Jet Ski', got: {}",
        jet_ski_subject
    );
    assert!(!jet_ski_subject.contains("Wild Fish"));

    let jet_ski_pkg = PackagingUpgrade::from_title_and_tone(jet_ski_title, Some(NicheToneProfile::AestheticLifestyleCozy));
    assert!(
        !jet_ski_pkg.title_upgrade_a.contains("Wild Fish"),
        "Title upgrade A must not hallucinate 'Wild Fish', got: {}",
        jet_ski_pkg.title_upgrade_a
    );
    assert!(
        !jet_ski_pkg.title_upgrade_b.contains("Wild Fish"),
        "Title upgrade B must not hallucinate 'Wild Fish', got: {}",
        jet_ski_pkg.title_upgrade_b
    );
    assert!(
        !jet_ski_pkg.opening_line_upgraded.contains("Wild Fish"),
        "Opening line must not hallucinate 'Wild Fish', got: {}",
        jet_ski_pkg.opening_line_upgraded
    );
}

#[test]
fn test_truncation_front_load_preserves_head_clause() {
    let title = "Riding 1300 Miles to Alaska on Jet Ski - Camping & Fishing";
    let recommendation = TruncationRadar::calculate_front_load_recommendation(title, 45);
    assert_eq!(
        recommendation, "Riding 1300 Miles to Alaska on Jet Ski",
        "Must preserve primary head clause up to 45 chars, got: {}",
        recommendation
    );
    assert_eq!(recommendation.chars().count(), 38);
}

#[test]
fn test_question_titles_strip_interrogative_openers() {
    let question_title = "Is Chemistry the Same Throughout the Universe? With Kate the Chemist";
    let subject = extract_dynamic_subject(question_title);
    assert_eq!(
        subject, "Chemistry Throughout the Universe",
        "Interrogative opener and connector must be stripped, got: {}",
        subject
    );

    let pkg = PackagingUpgrade::from_title_and_tone(question_title, Some(NicheToneProfile::EducationalBreakdown));
    assert!(
        !pkg.title_upgrade_a.contains("Is Chemistry"),
        "Title upgrade A must not have ungrammatical 'Is Chemistry', got: {}",
        pkg.title_upgrade_a
    );
    assert!(
        pkg.title_upgrade_a.contains("Chemistry Throughout the Universe"),
        "Title upgrade A must contain clean dynamic subject, got: {}",
        pkg.title_upgrade_a
    );
}

#[test]
fn test_tiered_contrast_packaging_does_not_output_complete_guide() {
    let beast_title = "$1 vs $500,000 Experiences!";
    let beast_pkg = PackagingUpgrade::from_title_and_tone(beast_title, Some(NicheToneProfile::EntertainmentSpectacle));
    
    assert!(
        !beast_pkg.title_upgrade_a.contains("The Complete Guide to"),
        "Tiered contrast must never output 'The Complete Guide to', got: {}",
        beast_pkg.title_upgrade_a
    );
    assert!(
        beast_pkg.title_upgrade_a.contains("Tested to the Absolute Limit"),
        "Tiered contrast must generate extreme testing escalation, got: {}",
        beast_pkg.title_upgrade_a
    );
    assert!(
        beast_pkg.title_upgrade_b.contains("The Shocking Difference Between"),
        "Tiered contrast must generate difference contrast, got: {}",
        beast_pkg.title_upgrade_b
    );
    assert!(
        beast_pkg.opening_line_upgraded.contains("price actually equals quality"),
        "Upgraded opening hook must challenge price vs quality, got: {}",
        beast_pkg.opening_line_upgraded
    );
}

#[test]
fn test_mega_scale_satisfaction_dampener() {
    // 150M views, 2.6M likes (1.74%), 28.5k comments (0.19/1k)
    let mega_metrics = ViewerSatisfactionMetrics::evaluate(150_000_000, 2_600_000, 28_500);
    assert!(
        mega_metrics.satisfaction_tier.contains("Healthy Satisfaction") || mega_metrics.satisfaction_tier.contains("Diamond Satisfaction"),
        "Mega-viral video (150M views, 28.5k comments) must not be flagged as Passive Watch, got: {}",
        mega_metrics.satisfaction_tier
    );
}

#[test]
fn test_re_engagement_checkpoint_warning_and_timeline_cues() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "test_checkpoint_vid",
        "Exploring The Deep Ocean",
        "Adventure",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.5,
        None,
        4.0,
        15,
        10.0,
        Some(-2.5),
        Some(180.0),
        160.0,
        Some("Deep sea hook"),
        None,
    );
    scorecard.duration_seconds = Some(600.0);
    scorecard.re_engagement_checkpoints = vec![
        sandsifter::math::retention::ReEngagementCheckpoint {
            milestone_name: "3-Minute Checkpoint (03:00)".to_string(),
            window_start_sec: 165.0,
            window_end_sec: 210.0,
            local_retention_slope: -2.8,
            has_pacing_reset: false,
            passed: false,
            diagnostic_note: "Audience fatigue cliff detected at 03:00 (-2.8%/s). Insert a narrative twist, sub-challenge, or spectacle reset.".to_string(),
        },
        sandsifter::math::retention::ReEngagementCheckpoint {
            milestone_name: "6-Minute Checkpoint (06:00)".to_string(),
            window_start_sec: 345.0,
            window_end_sec: 390.0,
            local_retention_slope: -0.8,
            has_pacing_reset: true,
            passed: true,
            diagnostic_note: "Pacing reset detected through milestone (-0.8%/s).".to_string(),
        },
    ];

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let term = flight_sheet.render_terminal_string();
    let md = flight_sheet.to_markdown_string();

    assert!(term.contains("⚠️ Retention Milestone Warning: [03:00] failed re-engagement gate. See timeline marker for recommended pacing reset."));
    assert!(md.contains("⚠️ **Retention Milestone Warning**: [03:00] failed re-engagement gate. See timeline marker for recommended pacing reset."));

    let timeline = flight_sheet.to_export_timeline();
    let shot = timeline.shots.first().expect("Timeline must have at least one shot");
    let re_marker = shot.markers.iter().find(|m| m.name.contains("[RE-ENGAGEMENT CUE]"));
    assert!(re_marker.is_some(), "Timeline must contain a [RE-ENGAGEMENT CUE] marker");
    let marker = re_marker.unwrap();
    assert_eq!(marker.color, sandsifter::export::MarkerColor::Red);
    assert!(marker.name.contains("Pacing fatigue at 03:00"));
}

#[test]
fn test_flight_sheet_renders_abrupt_ending_radar() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "test_outro",
        "Deep Sea Cod Fishing in Alaska",
        "Commercial Fishing",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.5,
        None,
        4.0,
        15,
        10.0,
        Some(-2.5),
        Some(180.0),
        160.0,
        Some("Deep sea hook"),
        None,
    );
    scorecard.duration_seconds = Some(1061.0);
    scorecard.outro_report = Some(sandsifter::math::retention::OutroRunwayReport {
        total_duration_seconds: 1061.0,
        signoff_cue_timestamp_seconds: Some(1027.0),
        first_signoff_phrase: Some("thanks for watching and I'll see you in the next".to_string()),
        outro_runway_seconds: 34.0,
        is_abrupt_ending: false,
        audience_loss_in_runway_pct: 8.4,
        recommended_blade_timecode: "17:05".to_string(),
        diagnostic_prescription: "Blade timeline at 17:05 (eliminate 34s of verbal wind-down). Saves ~8.4% of audience retention and maximizes end-screen clickthrough.".to_string(),
    });

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let term = flight_sheet.render_terminal_string();
    let md = flight_sheet.to_markdown_string();

    // Verify Dedicated Radar Card appears in terminal and markdown
    assert!(term.contains("🚪 ABRUPT ENDING & OUTRO RUNWAY RADAR"));
    assert!(md.contains("🚪 ABRUPT ENDING & OUTRO RUNWAY RADAR"));
    assert!(term.contains("Verbose Wind-Down (34.0s runway) [⚠️ RETENTION BLEED]"));
    assert!(term.contains("17:07"));
    assert!(term.contains("17:05"));
    assert!(term.contains("Reclaims ~8.4%"));

    // Verify Timeline Punch list has the updated instruction
    let outro_cue = flight_sheet.timeline_punch_list.iter().find(|c| c.action_type == "OUTRO FIX").unwrap();
    assert!(outro_cue.remedy_instruction.contains("Blade timeline at 17:05"));
    assert!(outro_cue.remedy_instruction.contains("eliminate 34s verbal wind-down"));

    // Verify NLE marker export emits Orange blade marker
    let timeline = flight_sheet.to_export_timeline();
    let all_markers: Vec<_> = timeline.shots.iter().flat_map(|s| &s.markers).collect();
    let blade_marker = all_markers.iter().find(|m| m.name.contains("[BLADE TIMELINE HERE: ABRUPT ENDING]"));
    assert!(blade_marker.is_some(), "Must emit [BLADE TIMELINE HERE: ABRUPT ENDING] marker");
    let marker = blade_marker.unwrap();
    assert_eq!(marker.color, sandsifter::export::MarkerColor::Orange);
    assert!(marker.note.contains("Eliminates 34s of verbal wind-down"));

    // Test verified abrupt ending: zero blade warnings emitted
    let mut abrupt_scorecard = scorecard.clone();
    abrupt_scorecard.outro_report = Some(sandsifter::math::retention::OutroRunwayReport {
        total_duration_seconds: 1061.0,
        signoff_cue_timestamp_seconds: None,
        first_signoff_phrase: None,
        outro_runway_seconds: 0.0,
        is_abrupt_ending: true,
        audience_loss_in_runway_pct: 0.0,
        recommended_blade_timecode: "N/A".to_string(),
        diagnostic_prescription: "Abrupt ending verified (0.0s post-payoff runway); zero wind-down drag.".to_string(),
    });

    let abrupt_sheet = FlightSheet::from_scorecard(&abrupt_scorecard);
    let abrupt_timeline = abrupt_sheet.to_export_timeline();
    let abrupt_markers: Vec<_> = abrupt_timeline.shots.iter().flat_map(|s| &s.markers).collect();
    assert!(!abrupt_markers.iter().any(|m| m.name.contains("[BLADE TIMELINE HERE: ABRUPT ENDING]")),
        "Zero blade warnings must be emitted when abrupt ending is verified");
}

#[test]
fn test_flight_sheet_and_scorecard_retention_curve_and_breaches_serialization() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "test_retention_video",
        "How to Build a Timber Frame Cabin",
        "Wilderness Craft",
        Some(NicheToneProfile::DocumentaryBuildProcess),
        0.2,
        None,
        4.0,
        15,
        12.0,
        Some(-1.8),
        Some(180.0),
        165.0,
        Some("Building the cabin"),
        None,
    );

    // Attach retention curve and attention breaches
    scorecard.retention_curve = vec![
        sandsifter::math::retention::HeatmapPoint { start_time: 0.0, end_time: 30.0, value: 1.0 },
        sandsifter::math::retention::HeatmapPoint { start_time: 30.0, end_time: 60.0, value: 0.85 },
        sandsifter::math::retention::HeatmapPoint { start_time: 60.0, end_time: 90.0, value: 0.72 },
    ];
    scorecard.attention_breaches = vec![
        sandsifter::math::retention::AttentionBreach {
            timestamp_sec: 60.0,
            formatted_timestamp: "01:00".to_string(),
            intensity: 0.72,
            r_prime: -3.2,
            root_cause: sandsifter::math::retention::BreachRootCause::SlowVisualPacing { shot_length_sec: 14.5 },
            transcript_context: "Talking head explanation without cutaway.".to_string(),
            tactical_remedy: "Insert 115% digital punch-in or B-roll at 01:00.".to_string(),
        },
    ];

    // 1. Verify serialization of scorecard to JSON
    let sc_json = serde_json::to_value(&scorecard).expect("Scorecard must serialize to JSON");
    assert!(sc_json["retention_curve"].is_array());
    assert_eq!(sc_json["retention_curve"].as_array().unwrap().len(), 3);
    assert_eq!(sc_json["attention_breaches"].as_array().unwrap().len(), 1);
    assert_eq!(sc_json["attention_breaches"][0]["formatted_timestamp"], "01:00");

    // 2. Generate FlightSheet and verify retention curve is copied
    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    assert_eq!(flight_sheet.retention_curve.len(), 3);
    assert_eq!(flight_sheet.attention_breaches.len(), 1);
    assert_eq!(flight_sheet.attention_breaches[0].timestamp_sec, 60.0);

    // 3. Verify standalone HTML report contains the interactive scrubber and red breach marker
    let html_report = sandsifter::export::HtmlReportGenerator::generate(&flight_sheet, &scorecard);
    assert!(html_report.contains("Interactive Retention Scrubber & NLE Video Sync"));
    assert!(html_report.contains("report-retention-svg"));
    assert!(html_report.contains("breach-marker"));
    assert!(html_report.contains("data-time=\"60.0\""));
    assert!(html_report.contains("01:00"));
    assert!(html_report.contains("seekPlayer"));
    assert!(html_report.contains("highlightPunchRow"));
}

#[test]
fn test_unmatched_closing_parenthesis_stripped_from_extracted_subject() {
    let title = "(103 Salmon & 7 Grizzlies) Epic Alaska Adventure";
    let subject = extract_dynamic_subject(title);

    assert!(
        !subject.contains(')'),
        "Subject must not contain any dangling ')' characters: {}",
        subject
    );
    assert!(
        !subject.contains('('),
        "Subject must not contain any dangling '(' characters: {}",
        subject
    );

    let pkg = PackagingUpgrade::from_title_and_tone(title, Some(NicheToneProfile::AestheticLifestyleCozy));

    let is_parentheses_balanced = |s: &str| -> bool {
        let mut count = 0;
        for c in s.chars() {
            if c == '(' {
                count += 1;
            } else if c == ')' {
                if count == 0 {
                    return false;
                }
                count -= 1;
            }
        }
        count == 0
    };

    assert!(
        is_parentheses_balanced(&pkg.title_upgrade_a),
        "Title Upgrade A must have balanced parentheses: {}",
        pkg.title_upgrade_a
    );
    assert!(
        is_parentheses_balanced(&pkg.title_upgrade_b),
        "Title Upgrade B must have balanced parentheses: {}",
        pkg.title_upgrade_b
    );
}

#[test]
fn test_bracket_sanitizer_handles_diverse_creator_badges() {
    // Input 1: [SOLO CAMPING] 3 Days in Snow Cave -> Solo Camping: 3 Days in Snow Cave
    let s1 = sanitize_subject_noun("[SOLO CAMPING] 3 Days in Snow Cave");
    assert_eq!(s1, "Solo Camping: 3 Days in Snow Cave");
    assert!(!s1.contains('[') && !s1.contains(']'));

    // Input 2: Catching Trout (Unbelievable!)) -> Catching Trout (Unbelievable!)
    let s2 = sanitize_subject_noun("Catching Trout (Unbelievable!))");
    assert_eq!(s2, "Catching Trout (Unbelievable!)");

    // Input 3: Turning $100 into $1,000 (Challenge) -> correctly preserves balanced (Challenge)
    let s3 = sanitize_subject_noun("Turning $100 into $1,000 (Challenge)");
    assert_eq!(s3, "Turning $100 into $1,000 (Challenge)");
}

#[test]
fn test_packaging_upgrade_reflects_actual_spoken_transcript() {
    let title = "(103 Salmon & 7 Grizzlies) Epic Alaska Adventure";
    let actual_spoken = Some("Tom here with the Outdoor Tom YouTube channel and today we're here at the Alagnak River.");
    let pkg = PackagingUpgrade::from_context(title, NicheToneProfile::AestheticLifestyleCozy, actual_spoken);

    assert!(
        pkg.opening_line_current.contains("Tom here with the Outdoor Tom YouTube channel"),
        "Opening line must contain real spoken dialogue: {}",
        pkg.opening_line_current
    );
    assert!(
        !pkg.opening_line_current.contains("Hey everyone, welcome back"),
        "Must eliminate strawman intro placeholder: {}",
        pkg.opening_line_current
    );
}

#[test]
fn test_packaging_upgrade_handles_none_spoken_hook_cleanly() {
    let title = "Snow Cave Solo Camping";
    let pkg = PackagingUpgrade::from_context(title, NicheToneProfile::AestheticLifestyleCozy, None);

    assert!(
        pkg.opening_line_current.contains("[No spoken dialogue in opening 5s"),
        "Must gracefully signal ambient/non-verbal opening when spoken hook is None: {}",
        pkg.opening_line_current
    );
    assert!(
        !pkg.opening_line_current.contains("Hey everyone, welcome back"),
        "Must not output strawman intro placeholder: {}",
        pkg.opening_line_current
    );
}

#[test]
fn test_lifestyle_cozy_tone_generates_adventure_titles_without_b2b_jargon() {
    let title = "(103 Salmon & 7 Grizzlies) Epic Alaska Adventure";
    let pkg = PackagingUpgrade::from_context(title, NicheToneProfile::AestheticLifestyleCozy, None);

    assert!(
        !pkg.title_upgrade_a.contains("The Complete Guide to"),
        "AestheticLifestyleCozy must not output B2B guide titles: {}",
        pkg.title_upgrade_a
    );
    assert!(
        !pkg.title_upgrade_b.contains("The Truth About"),
        "AestheticLifestyleCozy must not output B2B truth titles: {}",
        pkg.title_upgrade_b
    );
    assert!(
        !pkg.opening_line_upgraded.contains("this single principle changes everything"),
        "AestheticLifestyleCozy must not output B2B pitch line: {}",
        pkg.opening_line_upgraded
    );
    assert!(
        pkg.title_upgrade_a.contains("Alaska") || pkg.title_upgrade_a.contains("River") || pkg.title_upgrade_a.contains("Salmon"),
        "Title Upgrade A must incorporate travelogue scale/anchor: {}",
        pkg.title_upgrade_a
    );
    assert!(
        pkg.title_upgrade_b.contains("Wilderness") || pkg.title_upgrade_b.contains("Expedition") || pkg.title_upgrade_b.contains("Surviving"),
        "Title Upgrade B must incorporate expedition narrative challenge: {}",
        pkg.title_upgrade_b
    );
}

#[test]
fn test_first_person_verb_stripped_from_conversion_subject() {
    let raw_title = "We Converted A Cheap Trailer into A Home on Wheels (Start to Finish)";
    let subject = extract_dynamic_subject(raw_title);
    assert!(
        !subject.to_lowercase().contains("we converted"),
        "Subject must strip first-person action verbs, got: {}",
        subject
    );

    let pkg = PackagingUpgrade::from_title_and_tone(raw_title, Some(NicheToneProfile::DocumentaryBuildProcess));
    assert!(
        !pkg.title_upgrade_a.contains("The Complete Guide to We Converted"),
        "Title Upgrade A must not slot-stuff first-person action verbs: {}",
        pkg.title_upgrade_a
    );
    assert!(
        !pkg.title_upgrade_b.contains("The Truth About We Converted"),
        "Title Upgrade B must not slot-stuff first-person action verbs: {}",
        pkg.title_upgrade_b
    );
}

#[test]
fn test_documentary_build_trailer_conversion_generates_craft_titles() {
    let trailer_title = "We Converted A Cheap Trailer into A Home on Wheels (Start to Finish)";
    let pkg = PackagingUpgrade::from_title_and_tone(trailer_title, Some(NicheToneProfile::DocumentaryBuildProcess));

    assert!(
        !pkg.title_upgrade_a.contains("The Complete Guide to"),
        "DocumentaryBuildProcess must not emit B2B guide titles: {}",
        pkg.title_upgrade_a
    );
    assert!(
        !pkg.title_upgrade_b.contains("The Truth About"),
        "DocumentaryBuildProcess must not emit B2B truth titles: {}",
        pkg.title_upgrade_b
    );
    assert!(
        !pkg.opening_line_upgraded.contains("this single principle changes everything"),
        "DocumentaryBuildProcess must not emit corporate principle pitch: {}",
        pkg.opening_line_upgraded
    );
    assert!(
        pkg.title_upgrade_a.contains("Trailer") || pkg.title_upgrade_a.contains("Camper"),
        "Must generate concrete trailer/camper build title: {}",
        pkg.title_upgrade_a
    );
}












