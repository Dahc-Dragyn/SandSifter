use sandsifter::math::visual::{
    CutVelocityClassification, VisualPacingEngine,
};

#[test]
fn test_asl_calculation_basic() {
    // 60 seconds with 12 cuts -> 60.0 / 12 = 5.0s
    let asl = VisualPacingEngine::calculate_asl(60.0, 12);
    assert!((asl - 5.0).abs() < 1e-6);

    // 0 cuts -> max(1, 0) = 1 shot -> 60.0s
    let asl_zero = VisualPacingEngine::calculate_asl(60.0, 0);
    assert!((asl_zero - 60.0).abs() < 1e-6);

    // rapid: 30s with 15 cuts -> 2.0s
    let asl_rapid = VisualPacingEngine::calculate_asl(30.0, 15);
    assert!((asl_rapid - 2.0).abs() < 1e-6);
}

#[test]
fn test_cut_velocity_classification() {
    // ASL <= 2.5 -> RapidFastPaced
    assert_eq!(
        VisualPacingEngine::classify_asl(2.5),
        CutVelocityClassification::RapidFastPaced
    );
    assert_eq!(
        VisualPacingEngine::classify_asl(1.8),
        CutVelocityClassification::RapidFastPaced
    );

    // 2.5 < ASL <= 5.0 -> StandardNarrative
    assert_eq!(
        VisualPacingEngine::classify_asl(2.51),
        CutVelocityClassification::StandardNarrative
    );
    assert_eq!(
        VisualPacingEngine::classify_asl(3.8),
        CutVelocityClassification::StandardNarrative
    );
    assert_eq!(
        VisualPacingEngine::classify_asl(5.0),
        CutVelocityClassification::StandardNarrative
    );

    // ASL > 5.0 -> SlowContemplative
    assert_eq!(
        VisualPacingEngine::classify_asl(5.01),
        CutVelocityClassification::SlowContemplative
    );
    assert_eq!(
        VisualPacingEngine::classify_asl(12.5),
        CutVelocityClassification::SlowContemplative
    );
}

#[test]
fn test_evaluate_visual_pacing() {
    let cuts = vec![2.1, 4.5, 7.0, 9.8, 12.2];
    let metrics = VisualPacingEngine::evaluate_visual_pacing(15.0, &cuts);

    assert_eq!(metrics.total_scene_cuts, 5);
    assert!((metrics.duration_seconds - 15.0).abs() < 1e-6);
    assert!((metrics.average_shot_length_sec - 3.0).abs() < 1e-6);
    assert_eq!(metrics.cut_classification, CutVelocityClassification::StandardNarrative);
    assert!(metrics.tactical_rule.contains("Narrative Pacing") || metrics.tactical_rule.contains("narrative pacing"));
}

#[test]
fn test_synthesize_comparative_rule() {
    // When high performers have tighter edits (lower ASL)
    let rule_faster = VisualPacingEngine::synthesize_comparative_rule(2.2, 4.8);
    assert!(rule_faster.to_uppercase().contains("TIGHTEN") && rule_faster.contains("2.2s"));

    // When high performers have slower, more cinematic pacing (higher ASL)
    let rule_slower = VisualPacingEngine::synthesize_comparative_rule(5.5, 2.8);
    assert!(rule_slower.to_uppercase().contains("BREATHE") && rule_slower.contains("5.5s"));

    // When pacing is statistically similar
    let rule_similar = VisualPacingEngine::synthesize_comparative_rule(3.2, 3.4);
    assert!(rule_similar.to_uppercase().contains("MAINTAIN"));
}

#[test]
fn test_scene_cut_debouncing_rejects_rapid_shake_clusters() {
    // Synthetic sequence of timestamps with rapid shake clusters around 1.0s and 4.0s
    let raw_timestamps = vec![1.0, 1.1, 1.25, 1.38, 4.0, 4.1, 8.0];
    let debounced = VisualPacingEngine::debounce_cuts(&raw_timestamps, Some(0.5));
    assert_eq!(debounced, vec![1.0, 4.0, 8.0]);

    // Verify through end-to-end evaluate_visual_pacing
    let metrics = VisualPacingEngine::evaluate_visual_pacing(12.0, &raw_timestamps);
    assert_eq!(metrics.total_scene_cuts, 3);
    assert_eq!(metrics.cut_timestamps, vec![1.0, 4.0, 8.0]);
    assert_eq!(metrics.average_shot_length_sec, 4.0); // 12.0s / 3 cuts = 4.0s
    assert_eq!(metrics.cut_classification, CutVelocityClassification::StandardNarrative);
}

#[test]
fn test_chunk_partition_generator_exact_and_remainder() {
    // 1,493.0s (Mav) -> 4 chunks of 300.0s + 1 chunk of 293.0s
    let chunks_mav = sandsifter::ingest::MediaEngine::partition_stream_chunks(1493.0);
    assert_eq!(chunks_mav.len(), 5);
    assert_eq!(chunks_mav[0], sandsifter::ingest::StreamChunk { index: 0, start_sec: 0.0, duration_sec: 300.0 });
    assert_eq!(chunks_mav[1], sandsifter::ingest::StreamChunk { index: 1, start_sec: 300.0, duration_sec: 300.0 });
    assert_eq!(chunks_mav[2], sandsifter::ingest::StreamChunk { index: 2, start_sec: 600.0, duration_sec: 300.0 });
    assert_eq!(chunks_mav[3], sandsifter::ingest::StreamChunk { index: 3, start_sec: 900.0, duration_sec: 300.0 });
    assert_eq!(chunks_mav[4], sandsifter::ingest::StreamChunk { index: 4, start_sec: 1200.0, duration_sec: 293.0 });

    // 600.0s -> exactly 2 chunks of 300.0s
    let chunks_600 = sandsifter::ingest::MediaEngine::partition_stream_chunks(600.0);
    assert_eq!(chunks_600.len(), 2);
    assert_eq!(chunks_600[0], sandsifter::ingest::StreamChunk { index: 0, start_sec: 0.0, duration_sec: 300.0 });
    assert_eq!(chunks_600[1], sandsifter::ingest::StreamChunk { index: 1, start_sec: 300.0, duration_sec: 300.0 });

    // 240.0s -> 1 chunk of 240.0s
    let chunks_240 = sandsifter::ingest::MediaEngine::partition_stream_chunks(240.0);
    assert_eq!(chunks_240.len(), 1);
    assert_eq!(chunks_240[0], sandsifter::ingest::StreamChunk { index: 0, start_sec: 0.0, duration_sec: 240.0 });

    // 604.0s -> remainder 4.0s < 5.0s, cleanly merged into final chunk (304.0s)
    let chunks_604 = sandsifter::ingest::MediaEngine::partition_stream_chunks(604.0);
    assert_eq!(chunks_604.len(), 2);
    assert_eq!(chunks_604[0].duration_sec, 300.0);
    assert_eq!(chunks_604[1].duration_sec, 304.0);
}

#[test]
fn test_seam_debouncing_removes_boundary_duplicates() {
    // Injects cuts at [299.85, 300.10, 600.05, 600.20] straddling 300s and 600s chunk seams
    let boundary_cuts = vec![299.85, 300.10, 600.05, 600.20];
    let debounced = VisualPacingEngine::debounce_cuts(&boundary_cuts, Some(0.5));
    // Collapses into exactly 2 transitions (one per boundary pair)
    assert_eq!(debounced, vec![299.85, 600.05]);
}

#[test]
fn test_local_file_bypass_immunity() {
    use sandsifter::ingest::{is_remote_url, MAX_CONCURRENT_STREAM_WORKERS, STREAM_CHUNK_DURATION_SEC};

    assert!(!is_remote_url("local_draft.mp4"));
    assert!(!is_remote_url("C:\\renders\\cut_v2.mov"));
    assert!(!is_remote_url("./output/draft.mkv"));
    assert!(is_remote_url("https://www.youtube.com/watch?v=741qmO6kvBM"));
    assert!(is_remote_url("http://localhost:8080/stream"));

    assert_eq!(STREAM_CHUNK_DURATION_SEC, 300.0);
    assert_eq!(MAX_CONCURRENT_STREAM_WORKERS, 2);
}


