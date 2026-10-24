use sandsifter::export::FlightSheet;
use sandsifter::ingest::MediaSceneDetector;
use sandsifter::math::{LoudnessMetrics, MarginalGainsEngine};
use std::io::Cursor;

#[test]
fn test_youtube_penalty_calculation() {
    // Exact -14.0 LUFS target: 0.0 penalty
    assert_eq!(LoudnessMetrics::compute_youtube_penalty(-14.0), 0.0);

    // Quiet mix (-16.5 LUFS): YouTube does not boost quiet audio, penalty is 0.0
    assert_eq!(LoudnessMetrics::compute_youtube_penalty(-16.5), 0.0);

    // Hot mix (-10.8 LUFS): YouTube attenuates by -3.2 dB
    assert_eq!(LoudnessMetrics::compute_youtube_penalty(-10.8), -3.2);

    // Extreme club/EDM master (-6.5 LUFS): YouTube attenuates by -7.5 dB
    assert_eq!(LoudnessMetrics::compute_youtube_penalty(-6.5), -7.5);

    // Near target (-13.4 LUFS): YouTube attenuates by -0.6 dB
    assert_eq!(LoudnessMetrics::compute_youtube_penalty(-13.4), -0.6);
}

#[test]
fn test_true_peak_distortion_warning() {
    // True Peak at -1.5 dBTP (under -1.0 dBTP ceiling): Safe
    let safe_mix = LoudnessMetrics::evaluate(-14.2, -14.0, 6.0, -1.5);
    assert!(!safe_mix.mastering_status.contains("Mobile Speaker Distortion Risk"));
    assert_eq!(safe_mix.youtube_penalty_db, 0.0);

    // True Peak at +0.4 dBTP (exceeds -1.0 dBTP ceiling): Mobile clipping alert
    let clipping_mix = LoudnessMetrics::evaluate(-12.5, -12.0, 8.5, 0.4);
    assert!(clipping_mix.mastering_status.contains("Mobile Speaker Distortion Risk"));
    assert!(clipping_mix.actionable_prescription.contains("Set master limiter ceiling to -1.0 dBTP"));
    assert_eq!(clipping_mix.youtube_penalty_db, -1.5);
}

#[test]
fn test_ebur128_summary_parser() {
    let mock_ffmpeg_stderr = r#"
[Parsed_ebur128_0 @ 0000020c5c143040] t: 4.899977   TARGET:-23 LUFS    M: -18.1 S:-117.7     I: -18.1 LUFS       LRA:   0.0 LU  FTPK: -18.1 dBFS  TPK: -18.1 dBFS
[Parsed_ebur128_0 @ 0000020c5c143040] t: 4.999977   TARGET:-23 LUFS    M: -18.1 S:-117.7     I: -18.1 LUFS       LRA:   0.0 LU  FTPK: -18.1 dBFS  TPK: -18.1 dBFS
[Parsed_ebur128_0 @ 0000020c5c143040] Summary:

  Integrated loudness:
    I:         -13.8 LUFS
    Threshold: -24.1 LUFS

  Loudness range:
    LRA:         5.4 LU
    Threshold: -34.2 LUFS
    LRA low:   -17.5 LUFS
    LRA high:  -12.1 LUFS

  True peak:
    Peak:       -1.2 dBFS
[out#0/null @ 0000020c5c144580] video:0KiB audio:431KiB subtitle:0KiB other streams:0KiB global headers:0KiB muxing overhead: unknown
size=N/A time=00:00:05.00 bitrate=N/A speed= 199x elapsed=0:00:00.02
"#;

    let cursor = Cursor::new(mock_ffmpeg_stderr);
    let (i, lra, tp) = MediaSceneDetector::parse_ebur128_summary(cursor);

    assert_eq!(i, Some(-13.8));
    assert_eq!(lra, Some(5.4));
    assert_eq!(tp, Some(-1.2));
}

#[test]
fn test_loudness_metrics_prescription_generation() {
    // Low mix test: -20.0 LUFS
    let quiet_metrics = LoudnessMetrics::evaluate(-20.0, -20.0, 4.0, -3.0);
    assert!(quiet_metrics.mastering_status.contains("Low Dialogue Volume"));
    assert!(quiet_metrics.actionable_prescription.contains("Increase dialogue/vocal gain by +6.0 dB"));

    // Quiet intro test (cold open 4 dB quieter than rest)
    let quiet_intro = LoudnessMetrics::evaluate(-14.0, -18.2, 5.0, -1.5);
    assert!(quiet_intro.actionable_prescription.contains("Boost opening 30s dialogue by +4.2 dB"));

    // Hot intro test (cold open 4 dB hotter than rest)
    let hot_intro = LoudnessMetrics::evaluate(-14.0, -10.0, 7.0, -1.0);
    assert!(hot_intro.actionable_prescription.contains("Tame opening 30s sound effects/music (-4.0 dB)"));
}

#[test]
fn test_flight_sheet_renders_loudness_radar() {
    let mut scorecard = MarginalGainsEngine::generate_scorecard(
        "test_vid",
        "How to Master Audio Mixing in 10 Days",
        "Audio Production",
        0.2,
        3.5,
        18,
        6.5,
        170.0,
        None,
    );

    let loudness = LoudnessMetrics::evaluate(-11.5, -12.0, 6.2, -0.6);
    scorecard.loudness_metrics = Some(loudness.clone());

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let term = flight_sheet.render_terminal_string();
    let md = flight_sheet.to_markdown_string();

    // Verify terminal output
    assert!(term.contains("AUDIO MASTERING & YOUTUBE LOUDNESS RADAR"));
    assert!(term.contains("-11.5 LUFS"));
    assert!(term.contains("-2.5 dB Penalty"));
    assert!(term.contains("Mobile Speaker Distortion Risk"));

    // Verify markdown output
    assert!(md.contains("## 🔊 Audio Mastering & YouTube Loudness Radar"));
    assert!(md.contains("`-11.5 LUFS`"));
    assert!(md.contains("`-2.5 dB Penalty (Hot Mix)`"));
    assert!(md.contains("[⚠️ Mobile Distortion Risk]"));

    // Verify zero LaTeX invariant
    assert!(!md.contains("$\\le"));
    assert!(!md.contains("$\\ge"));
    assert!(!md.contains("\\text{"));
    assert!(!md.contains("\\%"));
}
