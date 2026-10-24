use sandsifter::math::retention::{
    HeatmapPoint, RetentionEngine, RetentionEventType, VttCue,
};

#[test]
fn test_parse_vtt_timestamps_and_cues() {
    let sample_vtt = r#"WEBVTT
Kind: captions
Language: en

00:00:00.600 --> 00:00:02.500
- [Brian] Welcome back to the boat build.

00:00:02.500 --> 00:00:06.800
We just went from the plane straight to the dock,

00:00:06.800 --> 00:00:10.250
<c.colorE5E5E5>and the mast is finally standing.</c>

00:04:12.000 --> 00:04:15.500
Now basically we have to explain the wiring setup.
"#;

    let cues = RetentionEngine::parse_vtt(sample_vtt);
    assert_eq!(cues.len(), 4);

    assert_eq!(cues[0].start_sec, 0.6);
    assert_eq!(cues[0].end_sec, 2.5);
    assert_eq!(cues[0].text, "Welcome back to the boat build.");

    assert_eq!(cues[1].text, "We just went from the plane straight to the dock,");

    // Tag stripping verification
    assert_eq!(cues[2].text, "and the mast is finally standing.");

    assert_eq!(cues[3].start_sec, 252.0); // 4 * 60 + 12
    assert_eq!(cues[3].text, "Now basically we have to explain the wiring setup.");
}

#[test]
fn test_find_transcript_at_window() {
    let cues = vec![
        VttCue { start_sec: 10.0, end_sec: 14.0, text: "First part of the story.".to_string() },
        VttCue { start_sec: 15.0, end_sec: 20.0, text: "Second critical obstacle.".to_string() },
        VttCue { start_sec: 100.0, end_sec: 105.0, text: "Far away dialogue.".to_string() },
    ];

    let text_near_15 = RetentionEngine::find_transcript_at(&cues, 15.0, 5.0);
    assert!(text_near_15.contains("First part"));
    assert!(text_near_15.contains("Second critical"));
    assert!(!text_near_15.contains("Far away"));

    let text_empty = RetentionEngine::find_transcript_at(&cues, 50.0, 5.0);
    assert!(text_empty.contains("[No active speech"));
}

#[test]
fn test_analyze_retention_curve_dips_and_spikes() {
    // 10 heatmap points across a 100-second video
    // Point 0 (0-10s): 1.0 (start)
    // Point 1 (10-20s): 0.85
    // Point 2 (20-30s): 0.40 -> Sudden drop of -0.45 (Catastrophic drop!)
    // Point 3 (30-40s): 0.38
    // Point 4 (40-50s): 0.35
    // Point 5 (50-60s): 0.55 -> Surge of +0.20 (Replay Spike!)
    // Point 6 (60-70s): 0.50
    // Point 7 (70-80s): 0.48
    // Point 8 (80-90s): 0.45
    // Point 9 (90-100s): 0.40
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 10.0, value: 1.0 },
        HeatmapPoint { start_time: 10.0, end_time: 20.0, value: 0.85 },
        HeatmapPoint { start_time: 20.0, end_time: 30.0, value: 0.40 }, // DIP at 20s
        HeatmapPoint { start_time: 30.0, end_time: 40.0, value: 0.38 },
        HeatmapPoint { start_time: 40.0, end_time: 50.0, value: 0.35 },
        HeatmapPoint { start_time: 50.0, end_time: 60.0, value: 0.55 }, // SPIKE at 50s
        HeatmapPoint { start_time: 60.0, end_time: 70.0, value: 0.50 },
        HeatmapPoint { start_time: 70.0, end_time: 80.0, value: 0.48 },
        HeatmapPoint { start_time: 80.0, end_time: 90.0, value: 0.45 },
        HeatmapPoint { start_time: 90.0, end_time: 100.0, value: 0.40 },
    ];

    let cues = vec![
        VttCue { start_sec: 18.0, end_sec: 25.0, text: "So basically today we are going to explain our logistics.".to_string() },
        VttCue { start_sec: 48.0, end_sec: 55.0, text: "Look at the boat hitting the water for the first time!".to_string() },
    ];

    let report = RetentionEngine::analyze_curve(
        "test_video_123",
        "Our Epic Catamaran Build",
        100.0,
        &heatmap,
        &cues,
    );

    assert_eq!(report.video_id, "test_video_123");
    assert!(!report.major_dips.is_empty(), "Should detect at least 1 major drop");
    let dip = &report.major_dips[0];
    assert_eq!(dip.formatted_timestamp, "00:20");
    assert_eq!(dip.event_type, RetentionEventType::CatastrophicDrop);
    assert!(dip.transcript_context.contains("explain our logistics"));
    assert!(dip.tactical_diagnosis.contains("Exposition Lull"));

    assert!(!report.major_spikes.is_empty(), "Should detect at least 1 rewatch spike");
    let spike = &report.major_spikes[0];
    assert_eq!(spike.formatted_timestamp, "00:50");
    assert_eq!(spike.event_type, RetentionEventType::ReplaySpike);
    assert!(spike.transcript_context.contains("hitting the water"));

    assert!(!report.tactical_rules.is_empty());
    assert!(report.tactical_rules.iter().any(|r| r.contains("00:20")));
}

#[test]
fn test_format_timestamp() {
    assert_eq!(RetentionEngine::format_timestamp(0.0), "00:00");
    assert_eq!(RetentionEngine::format_timestamp(45.2), "00:45");
    assert_eq!(RetentionEngine::format_timestamp(75.0), "01:15");
    assert_eq!(RetentionEngine::format_timestamp(3665.0), "01:01:05");
}

#[test]
fn test_calculate_active_window_wpm_boundary_interpolation() {
    // Simulated speech onset at t = 2.1s (e.g. 2.1s music intro)
    let cues = vec![
        // Cue 1: 2.1s -> 5.1s (3.0s duration), 9 words completely within the 5.0s window [2.1s, 7.1s]
        VttCue {
            start_sec: 2.1,
            end_sec: 5.1,
            text: "welcome to our off grid van life kitchen build".to_string(),
        },
        // Cue 2: 5.1s -> 8.1s (3.0s duration), 10 words straddling the 7.1s boundary
        // Overlap with [2.1s, 7.1s] is [5.1s, 7.1s] = 2.0s -> 2.0 / 3.0 = 66.7% of words = 6.67 words
        VttCue {
            start_sec: 5.1,
            end_sec: 8.1,
            text: "today we are putting up the walls and cedar siding".to_string(),
        },
        // Cue 3: 8.1s -> 11.0s, outside the 5.0s active speech window
        VttCue {
            start_sec: 8.1,
            end_sec: 11.0,
            text: "and it is going to look amazing".to_string(),
        },
    ];

    // Measure over full 5.0s active speech window post-onset (2.1s to 7.1s)
    let wpm = RetentionEngine::calculate_active_window_wpm(&cues, Some(2.1), 5.0);

    // Total words: 9 + 6.67 = 15.67 words over 5.0s -> 15.67 / 5.0 * 60 = 188.0 WPM
    // Prevents the old denominator compression bug (which squeezed 19 words into 3.5s - 2.1s = 1.4s, exploding to 814 WPM)
    assert!((185.0..=190.0).contains(&wpm), "Expected normalized WPM around 188.0, got {:.1}", wpm);
}

#[test]
fn test_vtt_rolling_captions_deduplication() {
    // Realistic YouTube auto-generated subtitle chunk with 10ms flash reset cues and rolling lines
    let sample_yt_vtt = r#"WEBVTT
Kind: captions
Language: en

00:00:00.000 --> 00:00:02.149 align:start position:0%
we're jasmine and crystal this is belle

00:00:02.149 --> 00:00:02.159 align:start position:0%
we're jasmine and crystal this is belle

00:00:02.159 --> 00:00:03.189 align:start position:0%
we're jasmine and crystal this is belle
and izzy

00:00:03.189 --> 00:00:03.199 align:start position:0%
and izzy

00:00:03.199 --> 00:00:05.430 align:start position:0%
and izzy
after two years of full-time van life

00:00:05.430 --> 00:00:05.440 align:start position:0%
after two years of full-time van life

00:00:05.440 --> 00:00:08.470 align:start position:0%
after two years of full-time van life
we're renovating our tiny home on wheels
"#;

    let cues = RetentionEngine::parse_vtt(sample_yt_vtt);
    assert_eq!(cues.len(), 4, "Should deduplicate 10ms flash cues and rolling repetition into 4 distinct cues");

    assert_eq!(cues[0].text, "we're jasmine and crystal this is belle");
    assert_eq!(cues[1].text, "and izzy");
    assert_eq!(cues[2].text, "after two years of full-time van life");
    assert_eq!(cues[3].text, "we're renovating our tiny home on wheels");

    // WPM calculation on cleaned cues over 5.0s active speech window
    let wpm = RetentionEngine::calculate_active_window_wpm(&cues, Some(0.0), 5.0);
    // Words: 7 (cue 0) + 2 (cue 1) + ~5.65 (cue 2 overlap [3.199, 5.0]) = ~14.65 words / 5s * 60 = ~175.8 WPM
    assert!((170.0..=180.0).contains(&wpm), "Expected natural human speech pace (~176 WPM), got {:.1}", wpm);
}

#[test]
fn test_replay_spike_normalization_no_near_zero_explosion() {
    // The exact 2020 Ep 3 anomaly:
    // Heatmap dips to near-zero 0.001 (0.1%), then surges to 0.547 (54.7%)
    let trough = 0.001;
    let peak = 0.547;

    // 1. Absolute percentage-point delta: ΔR = (R_peak - R_trough) * 100
    let delta_surge = RetentionEngine::calculate_replay_surge_delta(trough, peak);
    assert_eq!(delta_surge, 54.6, "ΔR must be cleanly 54.6 percentage points, never +54,644%");

    // 2. Floored relative surge: denominator clamped to >= 0.10 (10.0%)
    let floored_surge = RetentionEngine::calculate_floored_relative_replay_surge(trough, peak);
    assert_eq!(floored_surge, 546.0, "Floored relative surge must be 546.0%, never +54,644%");

    // 3. Curve scanner find_top_replay_surge
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 5.0, value: 1.0 },
        HeatmapPoint { start_time: 5.0, end_time: 100.0, value: 0.20 },
        HeatmapPoint { start_time: 100.0, end_time: 200.0, value: 0.001 }, // Trough
        HeatmapPoint { start_time: 200.0, end_time: 300.0, value: 0.547 }, // Peak surge
        HeatmapPoint { start_time: 300.0, end_time: 400.0, value: 0.30 },
    ];

    let top_surge = RetentionEngine::find_top_replay_surge(&heatmap);
    assert!(top_surge.is_some());
    let (ts, surge_val, fmt_ts) = top_surge.unwrap();
    assert_eq!(ts, 200.0);
    assert_eq!(fmt_ts, "03:20");
    assert_eq!(surge_val, 54.6);
}

#[test]
fn test_re_engagement_gate_triggers_on_steep_decay_at_three_minutes() {
    // 600s video (10 minutes) with a steep -3.0%/s drop around 180s (03:00)
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 160.0, value: 0.85 },
        HeatmapPoint { start_time: 160.0, end_time: 170.0, value: 0.80 },
        HeatmapPoint { start_time: 170.0, end_time: 175.0, value: 0.80 }, // t = 172.5
        HeatmapPoint { start_time: 175.0, end_time: 180.0, value: 0.80 }, // t = 177.5
        HeatmapPoint { start_time: 180.0, end_time: 185.0, value: 0.50 }, // t = 182.5, drop = 0.30 over 10s = -3.0%/s
        HeatmapPoint { start_time: 185.0, end_time: 190.0, value: 0.50 }, // t = 187.5
        HeatmapPoint { start_time: 190.0, end_time: 210.0, value: 0.50 },
        HeatmapPoint { start_time: 210.0, end_time: 600.0, value: 0.45 },
    ];

    // No cuts within the 165s..=210s window
    let cuts = vec![50.0, 100.0, 500.0];

    let checkpoints = RetentionEngine::evaluate_re_engagement_checkpoints(600.0, &heatmap, &cuts);
    assert_eq!(checkpoints.len(), 2, "Long-form 600s video must evaluate 2 checkpoints");

    let cp1 = &checkpoints[0];
    assert_eq!(cp1.milestone_name, "3-Minute Checkpoint (03:00)");
    assert!(!cp1.has_pacing_reset);
    assert!(cp1.local_retention_slope <= -2.0, "Expected steep drop <= -2.0%/s, got {:.1}%/s", cp1.local_retention_slope);
    assert!(!cp1.passed, "Steep -3.0%/s drop without cuts must trigger gate failure");
    assert!(cp1.diagnostic_note.contains("Audience fatigue cliff detected at 03:00"));
}

#[test]
fn test_re_engagement_gate_passes_on_flat_slope_or_cut_reset() {
    // 1. Passes on flat slope (slope >= -1.5%/s) without cuts
    let flat_heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 160.0, value: 0.80 },
        HeatmapPoint { start_time: 160.0, end_time: 180.0, value: 0.78 },
        HeatmapPoint { start_time: 180.0, end_time: 200.0, value: 0.76 }, // -0.02 over 20s = -0.1%/s
        HeatmapPoint { start_time: 200.0, end_time: 600.0, value: 0.70 },
    ];
    let checkpoints_flat = RetentionEngine::evaluate_re_engagement_checkpoints(600.0, &flat_heatmap, &[]);
    assert_eq!(checkpoints_flat.len(), 2);
    assert!(checkpoints_flat[0].passed, "Flat slope >= -1.5%/s must pass cleanly");

    // 2. Passes when a steep drop occurs BUT a clean visual cut reset is detected within [165, 210]
    let steep_heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 160.0, value: 0.85 },
        HeatmapPoint { start_time: 160.0, end_time: 170.0, value: 0.80 },
        HeatmapPoint { start_time: 170.0, end_time: 180.0, value: 0.80 },
        HeatmapPoint { start_time: 180.0, end_time: 190.0, value: 0.50 }, // -3.0%/s drop
        HeatmapPoint { start_time: 190.0, end_time: 600.0, value: 0.50 },
    ];
    let cuts_with_reset = vec![180.0]; // Visual cut right at 180s in the window
    let checkpoints_reset = RetentionEngine::evaluate_re_engagement_checkpoints(600.0, &steep_heatmap, &cuts_with_reset);
    assert_eq!(checkpoints_reset.len(), 2);
    assert!(checkpoints_reset[0].has_pacing_reset, "Scene cut within window must flag pacing reset");
    assert!(checkpoints_reset[0].passed, "Pacing reset cut must allow checkpoint to pass");
}

#[test]
fn test_re_engagement_gate_skipped_on_short_videos() {
    let heatmap = vec![
        HeatmapPoint { start_time: 0.0, end_time: 100.0, value: 0.90 },
        HeatmapPoint { start_time: 100.0, end_time: 200.0, value: 0.30 },
        HeatmapPoint { start_time: 200.0, end_time: 300.0, value: 0.20 },
    ];
    // 300s video (5 minutes) < 480s threshold
    let checkpoints = RetentionEngine::evaluate_re_engagement_checkpoints(300.0, &heatmap, &[]);
    assert!(checkpoints.is_empty(), "Videos under 480s must skip re-engagement checkpoints entirely");
}

#[test]
fn test_outro_runway_detects_verbose_wind_down() {
    let cues = vec![
        VttCue {
            start_sec: 10.0,
            end_sec: 20.0,
            text: "Welcome to the video".to_string(),
        },
        VttCue {
            start_sec: 560.0,
            end_sec: 565.0,
            text: "thanks for watching".to_string(),
        },
    ];

    let report = RetentionEngine::evaluate_outro_runway(600.0, &cues, None);
    assert!(!report.is_abrupt_ending);
    assert_eq!(report.outro_runway_seconds, 40.0);
    assert_eq!(report.recommended_blade_timecode, "09:20");
}

#[test]
fn test_outro_runway_recognizes_abrupt_ending() {
    let cues = vec![
        VttCue {
            start_sec: 10.0,
            end_sec: 20.0,
            text: "Welcome to the video".to_string(),
        },
        VttCue {
            start_sec: 598.0,
            end_sec: 600.0,
            text: "And that is the final climax payoff!".to_string(),
        },
    ];

    let report = RetentionEngine::evaluate_outro_runway(600.0, &cues, None);
    assert!(report.is_abrupt_ending);
    assert_eq!(report.outro_runway_seconds, 0.0);
    assert!(report.signoff_cue_timestamp_seconds.is_none());
}

