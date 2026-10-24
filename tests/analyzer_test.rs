use sandsifter::analyzer::{
    AudioTranscriber, FullTranscript, HookAligner, TimestampedSentence, TimestampedWord,
    VadClassifier,
};

#[test]
fn test_vad_silence() {
    let vad = VadClassifier::default();
    // 16000 Hz, 16-bit mono, 1 second = 32,000 bytes of zeros
    let silence = vec![0u8; 32_000];

    let rms = vad.compute_rms_energy(&silence);
    assert_eq!(rms, 0.0);
    assert!(!vad.is_speech_frame(&silence[0..640]));
    assert_eq!(vad.detect_speech_onset_ms(&silence), None);
    assert_eq!(vad.speech_activity_ratio(&silence), 0.0);
    assert!(vad.detect_speech_segments(&silence).is_empty());
}

#[test]
fn test_vad_loud_signal() {
    let vad = VadClassifier::default();
    // Generate 100ms (1600 samples, 3200 bytes) of loud alternating square wave (+16000, -16000)
    let mut loud_pcm = Vec::with_capacity(3200);
    for i in 0..1600 {
        let sample: i16 = if i % 2 == 0 { 16000 } else { -16000 };
        loud_pcm.extend_from_slice(&sample.to_le_bytes());
    }

    let rms = vad.compute_rms_energy(&loud_pcm);
    assert!(rms > 0.40, "RMS should be high, was: {}", rms);
    assert!(vad.is_speech_frame(&loud_pcm));
    assert_eq!(vad.detect_speech_onset_ms(&loud_pcm), Some(0));
    assert_eq!(vad.speech_activity_ratio(&loud_pcm), 1.0);

    let segments = vad.detect_speech_segments(&loud_pcm);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].0, 0);
    assert_eq!(segments[0].1, 100);
}

#[test]
fn test_vad_onset_detection_with_preroll_silence() {
    let vad = VadClassifier::default();
    // 500ms silence (25 frames of 20ms = 16,000 bytes) followed by 500ms of active speech
    let bytes_per_ms = (vad.sample_rate_hz * 2) / 1000;
    let silence_bytes = (500 * bytes_per_ms) as usize;
    let speech_bytes = (500 * bytes_per_ms) as usize;

    let mut audio = vec![0u8; silence_bytes];
    for i in 0..(speech_bytes / 2) {
        let sample: i16 = if i % 2 == 0 { 12000 } else { -12000 };
        audio.extend_from_slice(&sample.to_le_bytes());
    }

    let onset = vad.detect_speech_onset_ms(&audio);
    assert_eq!(onset, Some(500), "Speech onset should be detected at exactly 500ms");

    let segments = vad.detect_speech_segments(&audio);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].0, 500);
    assert_eq!(segments[0].1, 1000);
}

#[test]
fn test_full_transcript_metrics_and_slicing() {
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "The secret to scaling Rust microservices is zero allocations.".to_string(),
                start_ms: 100,
                end_ms: 2600,
                words: vec![
                    TimestampedWord { word: "The".to_string(), start_ms: 100, end_ms: 250, confidence: 0.99 },
                    TimestampedWord { word: "secret".to_string(), start_ms: 260, end_ms: 600, confidence: 0.98 },
                    TimestampedWord { word: "to".to_string(), start_ms: 610, end_ms: 750, confidence: 0.97 },
                    TimestampedWord { word: "scaling".to_string(), start_ms: 760, end_ms: 1150, confidence: 0.99 },
                    TimestampedWord { word: "Rust".to_string(), start_ms: 1160, end_ms: 1400, confidence: 0.99 },
                    TimestampedWord { word: "microservices".to_string(), start_ms: 1410, end_ms: 1950, confidence: 0.98 },
                    TimestampedWord { word: "is".to_string(), start_ms: 1960, end_ms: 2100, confidence: 0.96 },
                    TimestampedWord { word: "zero".to_string(), start_ms: 2110, end_ms: 2350, confidence: 0.98 },
                    TimestampedWord { word: "allocations.".to_string(), start_ms: 2360, end_ms: 2600, confidence: 0.99 },
                ],
            },
        ],
        total_duration_ms: 2600,
    };

    assert_eq!(transcript.word_count(), 9);
    assert_eq!(
        transcript.full_text(),
        "The secret to scaling Rust microservices is zero allocations."
    );
    assert_eq!(transcript.first_word_onset_ms(), Some(100));
    assert_eq!(transcript.last_word_offset_ms(), Some(2600));

    // WPM: 9 words in 2.6s = (9 / (2.6 / 60)) = ~207.69 WPM
    let wpm = transcript.cadence_wpm();
    assert!(wpm > 200.0 && wpm < 215.0, "Cadence was: {}", wpm);

    // Slicing: [600ms, 1500ms] should include "to", "scaling", "Rust", "microservices"
    let sliced = transcript.slice_words(600, 1500);
    let words_str: Vec<_> = sliced.iter().map(|w| w.word.as_str()).collect();
    assert_eq!(words_str, vec!["to", "scaling", "Rust", "microservices"]);

    // Serde round-trip
    let json_str = serde_json::to_string(&transcript).expect("serialize");
    let deserialized: FullTranscript = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(transcript, deserialized);
}

#[test]
fn test_hook_aligner_single_sentence() {
    let aligner = HookAligner::default();
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "Never use unwrap in production Rust code.".to_string(),
                start_ms: 150,
                end_ms: 2450,
                words: vec![
                    TimestampedWord { word: "Never".to_string(), start_ms: 150, end_ms: 400, confidence: 0.99 },
                    TimestampedWord { word: "use".to_string(), start_ms: 410, end_ms: 600, confidence: 0.98 },
                    TimestampedWord { word: "unwrap".to_string(), start_ms: 610, end_ms: 1100, confidence: 0.99 },
                    TimestampedWord { word: "in".to_string(), start_ms: 1110, end_ms: 1250, confidence: 0.97 },
                    TimestampedWord { word: "production".to_string(), start_ms: 1260, end_ms: 1800, confidence: 0.99 },
                    TimestampedWord { word: "Rust".to_string(), start_ms: 1810, end_ms: 2100, confidence: 0.99 },
                    TimestampedWord { word: "code.".to_string(), start_ms: 2110, end_ms: 2450, confidence: 0.98 },
                ],
            },
        ],
        total_duration_ms: 3000,
    };

    let result = aligner.extract_opening_hook(&transcript).expect("hook extracted");
    assert_eq!(result.spoken_hook, "Never use unwrap in production Rust code.");
    assert_eq!(result.start_second, 0.15);
    assert_eq!(result.end_second, 2.45);
    assert_eq!(result.duration_seconds, 2.3);
    assert_eq!(result.word_count, 7);
    assert!(!result.is_truncated_at_ceiling);
    assert!(result.cadence_wpm > 180.0);
}

#[test]
fn test_hook_aligner_short_first_sentence_combined() {
    let aligner = HookAligner::default();
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "Stop scrolling.".to_string(),
                start_ms: 100,
                end_ms: 700, // 600ms relative duration < 1200ms min
                words: vec![
                    TimestampedWord { word: "Stop".to_string(), start_ms: 100, end_ms: 350, confidence: 0.99 },
                    TimestampedWord { word: "scrolling.".to_string(), start_ms: 360, end_ms: 700, confidence: 0.98 },
                ],
            },
            TimestampedSentence {
                text: "Your backend has a critical memory leak.".to_string(),
                start_ms: 800,
                end_ms: 2800,
                words: vec![
                    TimestampedWord { word: "Your".to_string(), start_ms: 800, end_ms: 1000, confidence: 0.97 },
                    TimestampedWord { word: "backend".to_string(), start_ms: 1010, end_ms: 1400, confidence: 0.98 },
                    TimestampedWord { word: "has".to_string(), start_ms: 1410, end_ms: 1600, confidence: 0.96 },
                    TimestampedWord { word: "a".to_string(), start_ms: 1610, end_ms: 1700, confidence: 0.95 },
                    TimestampedWord { word: "critical".to_string(), start_ms: 1710, end_ms: 2100, confidence: 0.99 },
                    TimestampedWord { word: "memory".to_string(), start_ms: 2110, end_ms: 2450, confidence: 0.98 },
                    TimestampedWord { word: "leak.".to_string(), start_ms: 2460, end_ms: 2800, confidence: 0.99 },
                ],
            },
        ],
        total_duration_ms: 4000,
    };

    let result = aligner.extract_opening_hook(&transcript).expect("hook extracted");
    assert_eq!(
        result.spoken_hook,
        "Stop scrolling. Your backend has a critical memory leak."
    );
    assert_eq!(result.start_second, 0.1);
    assert_eq!(result.end_second, 2.8);
    assert_eq!(result.word_count, 9);
    assert!(!result.is_truncated_at_ceiling);
}

#[test]
fn test_hook_aligner_truncates_at_3_5s_ceiling() {
    let aligner = HookAligner::new(3500, 1200);
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "Today I am showing you the most insane performance hack in Rust that will double your throughput instantly.".to_string(),
                start_ms: 0,
                end_ms: 6000,
                words: vec![
                    TimestampedWord { word: "Today".to_string(), start_ms: 0, end_ms: 300, confidence: 0.99 },
                    TimestampedWord { word: "I".to_string(), start_ms: 310, end_ms: 450, confidence: 0.99 },
                    TimestampedWord { word: "am".to_string(), start_ms: 460, end_ms: 600, confidence: 0.98 },
                    TimestampedWord { word: "showing".to_string(), start_ms: 610, end_ms: 950, confidence: 0.98 },
                    TimestampedWord { word: "you".to_string(), start_ms: 960, end_ms: 1100, confidence: 0.99 },
                    TimestampedWord { word: "the".to_string(), start_ms: 1110, end_ms: 1250, confidence: 0.97 },
                    TimestampedWord { word: "most".to_string(), start_ms: 1260, end_ms: 1550, confidence: 0.98 },
                    TimestampedWord { word: "insane".to_string(), start_ms: 1560, end_ms: 1950, confidence: 0.99 },
                    TimestampedWord { word: "performance".to_string(), start_ms: 1960, end_ms: 2500, confidence: 0.99 },
                    TimestampedWord { word: "hack".to_string(), start_ms: 2510, end_ms: 2900, confidence: 0.99 },
                    TimestampedWord { word: "in".to_string(), start_ms: 2910, end_ms: 3100, confidence: 0.97 },
                    TimestampedWord { word: "Rust".to_string(), start_ms: 3110, end_ms: 3450, confidence: 0.99 },
                    // Following words exceed 3500ms ceiling
                    TimestampedWord { word: "that".to_string(), start_ms: 3550, end_ms: 3800, confidence: 0.98 },
                    TimestampedWord { word: "will".to_string(), start_ms: 3810, end_ms: 4050, confidence: 0.97 },
                    TimestampedWord { word: "double".to_string(), start_ms: 4060, end_ms: 4500, confidence: 0.99 },
                    TimestampedWord { word: "your".to_string(), start_ms: 4510, end_ms: 4700, confidence: 0.97 },
                    TimestampedWord { word: "throughput".to_string(), start_ms: 4710, end_ms: 5300, confidence: 0.99 },
                    TimestampedWord { word: "instantly.".to_string(), start_ms: 5310, end_ms: 6000, confidence: 0.99 },
                ],
            },
        ],
        total_duration_ms: 6000,
    };

    let result = aligner.extract_opening_hook(&transcript).expect("hook extracted");
    assert_eq!(
        result.spoken_hook,
        "Today I am showing you the most insane performance hack in Rust"
    );
    assert_eq!(result.start_second, 0.0);
    assert_eq!(result.end_second, 3.45);
    assert_eq!(result.word_count, 12);
    assert!(result.is_truncated_at_ceiling);
}

#[test]
fn test_hook_aligner_vad_calibration() {
    let aligner = HookAligner::default();
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "Here is the biggest lie in web development.".to_string(),
                start_ms: 0,
                end_ms: 2500,
                words: vec![],
            },
        ],
        total_duration_ms: 3000,
    };

    // Calibrated with acoustic speech onset at 350ms
    let result = aligner
        .extract_opening_hook_calibrated(&transcript, Some(350))
        .expect("hook extracted");

    assert_eq!(result.start_second, 0.35);
    assert_eq!(result.end_second, 2.50);
    assert_eq!(result.duration_seconds, 2.15);
    assert_eq!(result.word_count, 8);
}

#[test]
fn test_audio_transcriber_mock_pipeline() {
    let mock = AudioTranscriber::synthesize_mock_transcript();
    assert_eq!(mock.sentences.len(), 2);
    assert_eq!(mock.total_duration_ms, 7500);

    let aligner = HookAligner::default();
    let hook = aligner.extract_opening_hook(&mock).expect("opening hook");

    assert_eq!(
        hook.spoken_hook,
        "Stop using pandas for large datasets. You are destroying your RAM."
    );
    assert_eq!(hook.start_second, 0.20);
    assert_eq!(hook.end_second, 3.20);
    assert_eq!(hook.duration_seconds, 3.0);
    assert_eq!(hook.word_count, 11);
    assert!(!hook.is_truncated_at_ceiling);
}
