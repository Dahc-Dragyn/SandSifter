use sandsifter::analyzer::vad::VadClassifier;
use sandsifter::analyzer::hook_aligner::PromiseGapEvaluator;
use sandsifter::math::marginal_gains::{MarginalGainsEngine, VectorStatus};
use sandsifter::pipeline::NicheToneProfile;

#[test]
fn test_frame_zero_vad_speech_detection() {
    let vad = VadClassifier::default();

    // 1. Generate 1.0 second of 16kHz 16-bit mono PCM (32,000 bytes)
    // with speech-level acoustic energy (0.20 normalized amplitude, sine wave) in first 200ms.
    let sample_rate = 16000usize;
    let total_samples = sample_rate; // 1 second
    let mut pcm_bytes = Vec::with_capacity(total_samples * 2);

    for i in 0..total_samples {
        let sample: i16 = if i < (sample_rate * 2 / 10) {
            // First 200ms: speech energy (~0.25 RMS)
            let angle = 2.0 * std::f64::consts::PI * 440.0 * (i as f64) / (sample_rate as f64);
            (angle.sin() * 10000.0) as i16
        } else {
            // Remainder: digital silence
            0
        };
        pcm_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    // Speech detected at t < 300ms -> locks t_silence to 0.0s
    assert!(vad.has_frame_zero_speech(&pcm_bytes));
    let calibrated_silence = vad.calibrate_t_silence(Some(&pcm_bytes), Some(2.5));
    assert_eq!(calibrated_silence, 0.0);

    // 2. Generate 1.0 second of audio where dialogue is delayed until 500ms
    let mut delayed_pcm = Vec::with_capacity(total_samples * 2);
    for i in 0..total_samples {
        let sample: i16 = if i >= (sample_rate * 5 / 10) && i < (sample_rate * 7 / 10) {
            // 500ms to 700ms: active speech
            let angle = 2.0 * std::f64::consts::PI * 440.0 * (i as f64) / (sample_rate as f64);
            (angle.sin() * 10000.0) as i16
        } else {
            0
        };
        delayed_pcm.extend_from_slice(&sample.to_le_bytes());
    }

    assert!(!vad.has_frame_zero_speech(&delayed_pcm));
    let calibrated_delayed = vad.calibrate_t_silence(Some(&delayed_pcm), Some(2.5));
    assert!((0.49..=0.55).contains(&calibrated_delayed));

    // 3. Fallback when PCM is absent
    let fallback = vad.calibrate_t_silence(None, Some(2.5));
    assert_eq!(fallback, 2.5);
}

#[test]
fn test_outdoor_tom_family_travel_and_frame_zero_dialogue() {
    let title = "10 Days Fishing, Dog Sledding, & Exploring Alaska";
    let uploader = "Outdoor Tom";
    let opening_hook = "Nate here with Outdoor Tom and today we are in Alaska fishing and dog sledding";

    // 1. Tone routing: Must route to AestheticLifestyleCozy (Family Outdoor Travelogue / Documentary)
    let detected_tone = NicheToneProfile::detect(title, uploader, opening_hook);
    assert_eq!(detected_tone, NicheToneProfile::AestheticLifestyleCozy);
    assert_eq!(detected_tone.label(), "🌿 Family Outdoor Travelogue / Documentary");

    // 2. Frame-zero VAD calibration: Dialogue at t = 0.0s
    let vad = VadClassifier::default();
    // Simulate raw audio burst in first 100ms
    let sample_rate = 16000usize;
    let mut speech_at_zero = Vec::with_capacity(sample_rate * 2);
    for i in 0..sample_rate {
        let sample: i16 = if i < (sample_rate / 10) {
            let angle = 2.0 * std::f64::consts::PI * 300.0 * (i as f64) / (sample_rate as f64);
            (angle.sin() * 8000.0) as i16
        } else {
            0
        };
        speech_at_zero.extend_from_slice(&sample.to_le_bytes());
    }
    let t_silence = vad.calibrate_t_silence(Some(&speech_at_zero), Some(2.5));
    assert_eq!(t_silence, 0.0);

    // 3. Vector 1 Cold-Open Evaluation with calibrated t_silence = 0.0s
    let promise_gap = PromiseGapEvaluator::evaluate(
        title,
        Some("Scenic snow mountains and dog sledding"),
        None,
        Some(opening_hook),
        t_silence,
    );
    assert!(!promise_gap.is_breach, "False breach must not occur when speech starts at 0.0s with title keyword match");
    assert_eq!(promise_gap.promise_gap_score, 100.0);

    let v1 = MarginalGainsEngine::evaluate_cold_open_with_promise_gap(
        t_silence,
        None,
        Some(detected_tone),
        false,
        Some(&promise_gap),
    );
    assert_eq!(v1.status, VectorStatus::Elite);
    assert_eq!(v1.score_0_to_100, 100.0);

    // 4. Scenic Camera Hold: 10-14s scenic hold benchmark
    let hold_vec = MarginalGainsEngine::evaluate_static_hold_with_tone(11.5, detected_tone);
    assert_eq!(hold_vec.status, VectorStatus::Optimal);
    assert_eq!(hold_vec.target_benchmark, 14.0);
    assert_eq!(hold_vec.score_0_to_100, 90.0);
}

#[test]
fn test_bushcraft_camping_trip_tone_and_frame_zero() {
    let title = "My 1st Bushcraft Camping Trip in Alaska";
    let uploader = "Outdoor Tom";
    let opening_hook = "Tom here with the Outdoor Tom YouTube channel and today we are on our first bushcraft camping trip in Alaska";

    // 1. Tone routing: Must route to DocumentaryBuildProcess (Solo Bushcraft & Shelter Build)
    let detected_tone = NicheToneProfile::detect(title, uploader, opening_hook);
    assert_eq!(detected_tone, NicheToneProfile::DocumentaryBuildProcess);
    assert_eq!(detected_tone.label(), "🚜 Documentary Build & Transformation");

    // 2. Handshake: Two-part handshake passed with zero promise breach
    let promise_gap = PromiseGapEvaluator::evaluate(
        title,
        Some("Wilderness forest and skeletal remains"),
        None,
        Some(opening_hook),
        0.0,
    );
    assert!(!promise_gap.is_breach);
    assert_eq!(promise_gap.promise_gap_score, 100.0);
}

#[test]
fn test_fresh_upload_with_empty_vtt_does_not_report_zero_wpm_when_speech_energy_present() {
    use sandsifter::math::RetentionEngine;
    use sandsifter::math::ForensicsEngine;

    // 1. Generate 6.0 seconds of 16kHz 16-bit mono PCM with continuous speech-level energy (60% active vocal bursts)
    let sample_rate = 16000usize;
    let duration_sec = 6usize;
    let total_samples = sample_rate * duration_sec;
    let mut pcm_bytes = Vec::with_capacity(total_samples * 2);

    for i in 0..total_samples {
        // Continuous vocal bursts: active speech for 60% of each 500ms cycle
        let cycle_pos = i % (sample_rate / 2); // 500ms cycle
        let is_burst = cycle_pos < (sample_rate * 3 / 10); // 300ms voiced (60%), 200ms pause (40%)
        let sample: i16 = if is_burst {
            let angle = 2.0 * std::f64::consts::PI * 350.0 * (i as f64) / (sample_rate as f64);
            (angle.sin() * 8000.0) as i16
        } else {
            0
        };
        pcm_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    // Empty VTT subtitles (fresh upload where captions are pending)
    let empty_cues: Vec<sandsifter::math::retention::VttCue> = Vec::new();

    // With PCM speech energy present, RetentionEngine must NOT report 0.0 WPM
    let wpm = RetentionEngine::calculate_active_window_wpm_with_pcm(&empty_cues, Some(0.0), 5.0, Some(&pcm_bytes));
    assert!((150.0..=200.0).contains(&wpm), "Expected conversational WPM between 150 and 200, got {}", wpm);

    // ForensicsEngine delivery must also not report 0.0 WPM or non-speech
    let delivery = ForensicsEngine::analyze_delivery_with_pcm(Some(0), None, "", Some(&pcm_bytes));
    assert!(!delivery.is_non_speech, "Audio with >40% speech energy must not be classified as non-speech");
    assert!((150.0..=200.0).contains(&delivery.wpm_opening), "Expected delivery WPM >= 150, got {}", delivery.wpm_opening);

    // Pure silence (<5% energy) must genuinely report 0.0 WPM
    let silent_pcm = vec![0u8; sample_rate * 2 * 3]; // 3s digital silence
    let silent_wpm = RetentionEngine::calculate_active_window_wpm_with_pcm(&empty_cues, Some(0.0), 5.0, Some(&silent_pcm));
    assert_eq!(silent_wpm, 0.0, "Pure silence must report 0.0 WPM");
}

#[test]
fn test_theme_song_singing_not_counted_as_spoken_dialogue_hook() {
    let vad = VadClassifier::default();

    // 1. Generate 2.0 seconds of 16kHz 16-bit mono PCM simulating an intro theme song:
    // Continuous harmonic musical chords (110Hz + 220Hz + 440Hz harmonic structure) with sustained energy.
    let sample_rate = 16000usize;
    let duration_sec = 2usize;
    let total_samples = sample_rate * duration_sec;
    let mut theme_song_pcm = Vec::with_capacity(total_samples * 2);

    for i in 0..total_samples {
        let t = (i as f64) / (sample_rate as f64);
        let sample_f = 0.4 * (2.0 * std::f64::consts::PI * 110.0 * t).sin()
                     + 0.3 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()
                     + 0.2 * (2.0 * std::f64::consts::PI * 440.0 * t).sin();
        let sample = (sample_f * 20000.0) as i16;
        theme_song_pcm.extend_from_slice(&sample.to_le_bytes());
    }

    // Must detect harmonic music presence
    assert!(
        vad.detect_harmonic_music_presence(&theme_song_pcm),
        "Expected continuous chords to be detected as harmonic music presence"
    );
    assert!(
        !vad.has_spoken_speech_formants(&theme_song_pcm),
        "Harmonic instrumental chords must not be confused with conversational speech formants"
    );

    // When narrative dialogue starts at 78.0s (like in SV Delos Ep. 22),
    // singing / music in cold open must NOT trigger 0.0s speech onset!
    let calibrated_silence = vad.calibrate_t_silence(Some(&theme_song_pcm), Some(78.0));
    assert_eq!(
        calibrated_silence, 78.0,
        "Singing/music over intro theme song must not lock t_silence to 0.0s when narrative dialogue starts later"
    );

    // Also verify that theme song lyrics vs narrative dialogue are distinguished
    let singing_cue = "when we bust the light nobody's from my heart no it's never gonna be another";
    let narrative_cue = "after a big party night on Delos the time was finally here we were headed to check out a lust rally";
    assert!(VadClassifier::is_theme_song_or_singing_cue(singing_cue, Some("Sailing SV Delos Ep. 22")));
    assert!(!VadClassifier::is_theme_song_or_singing_cue(narrative_cue, Some("Sailing SV Delos Ep. 22")));

    // Forensics delivery metrics must flag bumper music opening
    let delivery = sandsifter::math::ForensicsEngine::analyze_delivery_with_pcm(Some(78000), None, narrative_cue, Some(&theme_song_pcm));
    assert!(delivery.is_bumper_music_opening, "Delivery must flag is_bumper_music_opening = true");

    // Vector 1 Cold-Open evaluation must penalize severe bumper lag (not 0.0s ELITE)
    let v1 = MarginalGainsEngine::evaluate_cold_open_with_context(
        calibrated_silence,
        None,
        Some(NicheToneProfile::AestheticLifestyleCozy),
        false,
    );
    assert_ne!(v1.status, VectorStatus::Elite, "78s theme song bumper must not be reported as Elite");
    assert_eq!(v1.status, VectorStatus::CriticalFriction, "78s theme song bumper must be reported as CriticalFriction");
    assert!(v1.one_percent_fix.contains("Severe theme song bumper detected"));
}


