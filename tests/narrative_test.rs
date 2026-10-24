use sandsifter::analyzer::{
    FullTranscript, HookAligner, KeyframeExtractor, KeyframeOcrResult, NarrativeClassifier,
    OcrEngine, OcrTextOverlay, TimestampedSentence, TimestampedWord,
};
use sandsifter::models::video::VideoAnalysisDetails;

#[test]
fn test_keyframe_extraction_and_timestamps() {
    let extractor = KeyframeExtractor::default();
    let keyframes = extractor.synthesize_test_keyframes();

    assert_eq!(keyframes.len(), 4);
    assert_eq!(keyframes[0].timestamp_ms, 0);
    assert_eq!(keyframes[0].timestamp_seconds(), 0.0);
    assert!(keyframes[0].is_opening_frame);

    assert_eq!(keyframes[1].timestamp_ms, 1000);
    assert_eq!(keyframes[1].timestamp_seconds(), 1.0);
    assert!(!keyframes[1].is_opening_frame);

    assert_eq!(keyframes[2].timestamp_ms, 2000);
    assert_eq!(keyframes[2].timestamp_seconds(), 2.0);

    assert_eq!(keyframes[3].timestamp_ms, 5000);
    assert_eq!(keyframes[3].timestamp_seconds(), 5.0);

    // Verify extraction from arbitrary video bytes
    let dummy_video_data = vec![0x42u8; 100_000];
    let extracted = extractor.extract_from_bytes(&dummy_video_data);
    assert_eq!(extracted.len(), 4);
    assert_eq!(extracted[0].timestamp_ms, 0);
    assert_eq!(extracted[3].timestamp_ms, 5000);
}

#[test]
fn test_keyframe_ocr_matching_spoken_hook() {
    let ocr = KeyframeOcrResult {
        timestamp_ms: 0,
        overlays: vec![OcrTextOverlay {
            text: "STOP USING PANDAS 🚨".to_string(),
            confidence: 0.99,
            is_headline: true,
            bounding_box: Some((0.1, 0.1, 0.8, 0.2)),
        }],
        raw_text: "STOP USING PANDAS 🚨".to_string(),
        has_headline_hook: true,
    };

    let matching_hook = "Stop using pandas for large datasets. You are destroying your RAM.";
    assert!(ocr.matches_spoken_hook(matching_hook));

    let non_matching_hook = "Here are the top three hidden vacation destinations in Japan.";
    assert!(!ocr.matches_spoken_hook(non_matching_hook));
}

#[test]
fn test_ocr_engine_mock_extraction() {
    let ocr_engine = OcrEngine::default();
    let ocr_0ms = OcrEngine::synthesize_mock_ocr(0);
    let ocr_1000ms = OcrEngine::synthesize_mock_ocr(1000);
    let ocr_5000ms = OcrEngine::synthesize_mock_ocr(5000);

    assert!(ocr_0ms.has_headline_hook);
    assert_eq!(ocr_0ms.overlays.len(), 2);
    assert!(ocr_0ms.overlays[0].is_headline);

    assert!(!ocr_1000ms.has_headline_hook);
    assert_eq!(ocr_1000ms.overlays.len(), 1);

    assert!(ocr_5000ms.overlays.is_empty());

    let extractor = KeyframeExtractor::default();
    let keyframes = extractor.synthesize_test_keyframes();

    // Async batch execution test
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let all_ocr = rt.block_on(ocr_engine.extract_all_keyframes(&keyframes));
    assert_eq!(all_ocr.len(), 4);
}

#[test]
fn test_narrative_classifier_heuristic_deconstruction() {
    let transcript = FullTranscript {
        sentences: vec![
            TimestampedSentence {
                text: "Stop using pandas for large datasets. You are destroying your RAM.".to_string(),
                start_ms: 200,
                end_ms: 3200,
                words: vec![
                    TimestampedWord { word: "Stop".to_string(), start_ms: 200, end_ms: 500, confidence: 0.99 },
                    TimestampedWord { word: "using".to_string(), start_ms: 510, end_ms: 800, confidence: 0.98 },
                    TimestampedWord { word: "pandas".to_string(), start_ms: 810, end_ms: 1200, confidence: 0.99 },
                    TimestampedWord { word: "for".to_string(), start_ms: 1210, end_ms: 1400, confidence: 0.97 },
                    TimestampedWord { word: "large".to_string(), start_ms: 1410, end_ms: 1700, confidence: 0.98 },
                    TimestampedWord { word: "datasets.".to_string(), start_ms: 1710, end_ms: 2200, confidence: 0.99 },
                    TimestampedWord { word: "You".to_string(), start_ms: 2300, end_ms: 2500, confidence: 0.98 },
                    TimestampedWord { word: "are".to_string(), start_ms: 2510, end_ms: 2650, confidence: 0.97 },
                    TimestampedWord { word: "destroying".to_string(), start_ms: 2660, end_ms: 2950, confidence: 0.99 },
                    TimestampedWord { word: "your".to_string(), start_ms: 2960, end_ms: 3050, confidence: 0.98 },
                    TimestampedWord { word: "RAM.".to_string(), start_ms: 3060, end_ms: 3200, confidence: 0.99 },
                ],
            },
            TimestampedSentence {
                text: "Here is what happens when you read 15GB into memory: instant Out Of Memory crash.".to_string(),
                start_ms: 3300,
                end_ms: 8500,
                words: vec![],
            },
        ],
        total_duration_ms: 12000,
    };

    let aligner = HookAligner::default();
    let hook_result = aligner.extract_opening_hook(&transcript);
    assert!(hook_result.is_some());

    let ocr_results = vec![OcrEngine::synthesize_mock_ocr(0)];

    let analysis = NarrativeClassifier::heuristic_classify(
        &transcript,
        hook_result.as_ref(),
        &ocr_results,
        Some("Data Engineering"),
    );

    // Verify format classification
    assert_eq!(analysis.format_category, "Educational");
    assert_eq!(analysis.format_type, "Contrarian Mythbuster");
    assert_eq!(analysis.format_flavor, Some("Fast-Paced Screen Split".to_string()));

    // Verify hook madlib abstraction
    assert_eq!(
        analysis.spoken_hook_madlib,
        Some("Stop using [X] for [Y]. You are destroying your [Z].".to_string())
    );
    assert_eq!(analysis.spoken_hook_category, "Direct Warning / Negative Framing");

    // Verify alignment & layout
    assert!(analysis.hook_alignment.is_some());
    assert_eq!(analysis.visual_layout_category, "Split Screen");

    // Verify narrative sections
    let sections = &analysis.narrative_structure.structure_sections;
    assert!(sections.len() >= 2);
    assert_eq!(sections[0].section_name, "Opening Hook");
    assert_eq!(sections[0].start_second, 0.0);
    assert_eq!(sections[0].end_second, 3.2);

    // Verify cognitive friction pair
    assert!(analysis.common_belief.is_some());
    assert!(analysis.contrarian_reality.is_some());
}

#[test]
fn test_madlib_synthesizer_patterns() {
    let transcript = FullTranscript::default();

    // Pattern 1: Nobody is talking about...
    let hook1 = sandsifter::analyzer::HookExtractionResult {
        spoken_hook: "Nobody is talking about this hidden compiler flag.".to_string(),
        start_second: 0.0,
        end_second: 2.5,
        duration_seconds: 2.5,
        word_count: 8,
        cadence_wpm: 192.0,
        is_truncated_at_ceiling: false,
    };
    let a1 = NarrativeClassifier::heuristic_classify(&transcript, Some(&hook1), &[], None);
    assert_eq!(
        a1.spoken_hook_madlib,
        Some("Nobody is talking about this [X] feature that saves [Y] hours.".to_string())
    );
    assert_eq!(a1.spoken_hook_category, "Curiosity Loop / Information Gap");

    // Pattern 2: I replaced my entire...
    let hook2 = sandsifter::analyzer::HookExtractionResult {
        spoken_hook: "I replaced my entire backend with one line of Rust.".to_string(),
        start_second: 0.0,
        end_second: 2.8,
        duration_seconds: 2.8,
        word_count: 9,
        cadence_wpm: 192.0,
        is_truncated_at_ceiling: false,
    };
    let a2 = NarrativeClassifier::heuristic_classify(&transcript, Some(&hook2), &[], None);
    assert_eq!(
        a2.spoken_hook_madlib,
        Some("I replaced my entire [X] stack with one line of [Y].".to_string())
    );
    assert_eq!(a2.spoken_hook_category, "Contrarian Demonstration / High Delta");
}

#[test]
fn test_narrative_serialization_roundtrip() {
    let transcript = FullTranscript::default();
    let hook = sandsifter::analyzer::HookExtractionResult {
        spoken_hook: "Never use unwrap in production Rust code.".to_string(),
        start_second: 0.0,
        end_second: 2.4,
        duration_seconds: 2.4,
        word_count: 7,
        cadence_wpm: 175.0,
        is_truncated_at_ceiling: false,
    };

    let analysis = NarrativeClassifier::heuristic_classify(&transcript, Some(&hook), &[], Some("Rust Backend"));

    // Serialize to JSON
    let json_val = serde_json::to_value(&analysis).expect("serialize to value");
    assert_eq!(json_val["format_category"], "Educational");
    assert_eq!(json_val["spoken_hook"], "Never use unwrap in production Rust code.");
    assert_eq!(
        json_val["spoken_hook_madlib"],
        "Never use [X] in production [Y]. Here is why."
    );

    // Roundtrip back to VideoAnalysisDetails
    let deserialized: VideoAnalysisDetails = serde_json::from_value(json_val).expect("deserialize");
    assert_eq!(analysis, deserialized);
}
