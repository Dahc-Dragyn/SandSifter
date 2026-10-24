use sandsifter::export::{
    FlightSheet, HtmlReportGenerator, ShootSheetGenerator, TruncationRadar,
};
use sandsifter::math::marginal_gains::{MarginalGainsEngine, ViewerSatisfactionMetrics};
use sandsifter::math::LoudnessMetrics;
use sandsifter::pipeline::script_generator::{ScriptGenerator, ScriptRequest};
use sandsifter::pipeline::NicheToneProfile;

#[test]
fn test_html_report_contains_embedded_css_and_zero_cdn_links() {
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
    scorecard.outro_exit_note = Some("Early Outro Exit detected at 14:15".to_string());

    let flight_sheet = FlightSheet::from_scorecard(&scorecard)
        .with_duration_seconds(960.0)
        .with_channel_name(channel);

    let html = HtmlReportGenerator::generate(&flight_sheet, &scorecard);

    // Invariant 1: Must be a complete HTML5 document
    assert!(html.contains("<!DOCTYPE html>"), "Must include HTML5 doctype");
    assert!(html.contains("<html lang=\"en\">"));
    assert!(html.contains("</html>"));

    // Invariant 2: ZERO external CDN or web links (pure standalone offline file)
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "HTML report must be 100% standalone and contain zero external http/https CDN links"
    );
    assert!(
        !html.contains("unpkg.com")
            && !html.contains("cdnjs.cloudflare.com")
            && !html.contains("fonts.googleapis.com")
            && !html.contains("cdn."),
        "Report must not reference external CDNs or web font servers"
    );

    // Invariant 3: Embedded modern dark theme CSS
    assert!(html.contains("<style>"), "Must contain embedded <style> block");
    assert!(html.contains("#0f172a"), "Must use dark theme background #0f172a");
    assert!(html.contains("#1e293b"), "Must use card background #1e293b");

    // Invariant 4: Component A (Header & Verdict & Gauge)
    assert!(html.contains("SANDSIFTER"));
    assert!(html.contains("EXECUTIVE FLIGHT SHEET"));
    assert!(html.contains(title));
    assert!(html.contains(channel));
    assert!(html.contains("VIDEO HEALTH GRADE"));
    assert!(html.contains("Multiplicative Reliability Engine"));
    assert!(html.contains("PRIMARY SERIES BOTTLENECK"));
}

#[test]
fn test_html_report_renders_loudness_gauge_and_punch_list() {
    let title = "Gemini 4 Argon Deep Dive and Analysis";
    let channel = "Tech Explainer";

    let mut scorecard = MarginalGainsEngine::generate_scorecard_with_tone_and_ocr(
        "tech_vid_202",
        title,
        channel,
        Some(NicheToneProfile::EducationalBreakdown),
        0.3,
        None,
        4.5,
        15,
        8.2,
        Some(-2.1),
        Some(180.0),
        175.0,
        Some("Gemini 4 Argon"),
        Some(ViewerSatisfactionMetrics::new(100000, 5200, 890)),
    );

    // Add explicit EBU R128 Loudness metrics
    scorecard.loudness_metrics = Some(LoudnessMetrics {
        integrated_lufs: -17.4,
        youtube_penalty_db: 0.0,
        cold_open_lufs: -16.9,
        loudness_range_lu: 3.2,
        true_peak_dbtp: -2.8,
        mastering_status: "🔈 Low Dialogue Volume (-17.4 LUFS)".to_string(),
        actionable_prescription: "Increase dialogue/vocal gain by +3.4 dB to match YouTube -14.0 LUFS standard.".to_string(),
    });

    let mut flight_sheet = FlightSheet::from_scorecard(&scorecard)
        .with_duration_seconds(420.0)
        .with_channel_name(channel);

    // Add truncation radar
    flight_sheet.truncation_radar = Some(TruncationRadar::evaluate(title));

    let html = HtmlReportGenerator::generate(&flight_sheet, &scorecard);

    // 1. Loudness Radar & Gauge
    assert!(html.contains("Audio Mastering & YouTube Loudness Radar"));
    assert!(html.contains("-17.4 LUFS"));
    assert!(html.contains("YouTube Volume Cut"));
    assert!(html.contains("True Peak Level"));
    assert!(html.contains("Increase dialogue/vocal gain by +3.4 dB"));

    // 2. Interactive NLE Punch List & Copy Buttons
    assert!(html.contains("Timeline Punch List (NLE Editing Commands)"));
    assert!(!html.contains("00:00 - 00:00"), "Must filter out inactive 00:00 - 00:00 cues");
    assert!(html.contains("INSERT B-ROLL"), "Must render active B-roll pacing cues");
    assert!(html.contains("CUT EXPOSITION"), "Must render active exposition blading cues");
    assert!(html.contains("copyToClipboard"));

    // 3. Simulated Cross-Device Viewports
    assert!(html.contains("Cross-Device Packaging & Truncation Radar"));
    assert!(html.contains("Mobile Push Notification"));
    assert!(html.contains("Mobile & CTV Home Feed"));
    assert!(html.contains("Desktop Search Snippet"));

    // 4. Packaging Upgrades & 4 Ps Hook
    assert!(html.contains("PACKAGING A/B UPGRADES"));
    assert!(html.contains("4 Ps DIRECT ENTRY HOOK"));

    // 5. Verify save_all_assets writes all 4 files
    let temp_dir = std::env::temp_dir().join(format!("sandsifter_test_assets_{}", std::process::id()));
    let saved_paths = flight_sheet.save_all_assets(&temp_dir, &scorecard).expect("Must save all assets");
    assert_eq!(saved_paths.len(), 4, "Must return 4 asset paths (.md, .html, .fcpxml, .edl)");

    for p in &saved_paths {
        assert!(p.exists(), "Asset file must exist on disk: {}", p.display());
    }

    // Verify .html file specifically
    let html_path = temp_dir.join("gemini_4_argon_deep_dive_and_analysis_report.html");
    assert!(html_path.exists(), "Report HTML must exist at expected filename");
    let disk_html = std::fs::read_to_string(&html_path).expect("Must read saved HTML");
    assert!(disk_html.contains("<!DOCTYPE html>"));

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_shoot_sheet_renders_printable_css_and_acceptance_checkboxes() {
    let req = ScriptRequest {
        channel_handle: "@vanwives".to_string(),
        topic: "Natural Swim Pond Build: Winterizing and Ice Defense".to_string(),
        target_duration_sec: 36,
        target_wpm: 195.0,
        target_asl_sec: 2.4,
        identity_context: None,
    };

    let script = ScriptGenerator::generate_heuristic_script(&req, &[]);

    let html = ShootSheetGenerator::generate(&script);

    // Invariant 1: HTML Document & Print Styles
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("@media print"), "Must include print media stylesheet");
    assert!(html.contains("@page"), "Must include @page declaration");
    assert!(html.contains("size: letter portrait"), "Must specify standard letter portrait dimensions");

    // Invariant 2: Section 1 (On-Set Photo Checklist & Peak Moment Sentence)
    assert!(html.contains("ON-SET PHOTO CHECKLIST (4 EXTREME OPPOSITES)"));
    assert!(html.contains("THE PEAK MOMENT SENTENCE"));
    assert!(html.contains("REACTION & GAZE CUE"));
    assert!(html.contains("MINIMALIST / BEFORE-AFTER"));
    assert!(html.contains("WIDE CINEMATIC / SCALE"));
    assert!(html.contains("VISUAL CONFLICT / DIAGRAM"));

    // Invariant 3: Section 2 (The 4 Ps Opening Shot Clock)
    assert!(html.contains("THE 4 Ps OPENING SHOT CLOCK (0.0s - 10.0s)"));
    assert!(html.contains("PROMISE"));
    assert!(html.contains("PROOF"));
    assert!(html.contains("PLAN"));
    assert!(html.contains("PERSONA"));
    assert!(html.contains("0.0s - 2.5s"));
    assert!(html.contains("2.5s - 5.0s"));
    assert!(html.contains("5.0s - 7.5s"));
    assert!(html.contains("7.5s - 10.0s"));

    // Invariant 4: Section 3 (Title Acceptance Criteria)
    assert!(html.contains("TITLE ACCEPTANCE CRITERIA (DO NOT WRAP FILMING UNTIL CHECKED)"));
    assert!(html.contains("CRITERION #01"));
    assert!(html.contains("<input type=\"checkbox\" class=\"on-set-check\">"));

    // Invariant 5: Verify disk save functionality
    let temp_dir = std::env::temp_dir().join(format!("sandsifter_test_shoot_{}", std::process::id()));
    let saved_path = ShootSheetGenerator::save(&script, &temp_dir).expect("Must save shoot sheet");
    assert!(saved_path.exists(), "Shoot sheet HTML must exist on disk");
    assert!(saved_path.to_string_lossy().ends_with("_shoot_sheet.html"));

    let disk_content = std::fs::read_to_string(&saved_path).expect("Must read shoot sheet content");
    assert!(disk_content.contains("PRE-PRODUCTION SHOOT SHEET"));

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);
}
