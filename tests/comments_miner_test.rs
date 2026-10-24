use sandsifter::analyzer::comments::{
    ClusterSentiment, CommentFrictionMiner, CommentTimestampCluster, RawVideoComment,
};
use sandsifter::export::flight_sheet::FlightSheet;
use sandsifter::export::timeline::MarkerColor;
use sandsifter::math::marginal_gains::MarginalGainsScorecard;
use sandsifter::math::retention::{
    AttentionBreach, BreachRootCause, ReplayRootCause, RetentionCurveReport, RetentionEvent,
    RetentionEventType,
};

#[test]
fn test_zero_alloc_timecode_extraction() {
    let sample = "Look at 1:45 and also (02:15)! Later at 1:05:30 something wild happens. Ignore 123:456 and 1:99.";
    let extracted = CommentFrictionMiner::extract_timecodes(sample);

    assert_eq!(extracted.len(), 3);
    assert_eq!(extracted[0].0, "1:45");
    assert_eq!(extracted[0].1, 105.0);

    assert_eq!(extracted[1].0, "02:15");
    assert_eq!(extracted[1].1, 135.0);

    assert_eq!(extracted[2].0, "1:05:30");
    assert_eq!(extracted[2].1, 3930.0);
}

#[test]
fn test_comment_clustering_with_tolerance() {
    let comments = vec![
        RawVideoComment {
            author: "Alice".to_string(),
            text: "Wait why did he cut that board so short at 01:40??".to_string(),
            like_count: 50,
        },
        RawVideoComment {
            author: "Bob".to_string(),
            text: "01:44 cringe moment I was so confused".to_string(),
            like_count: 10,
        },
        RawVideoComment {
            author: "Charlie".to_string(),
            text: "01:48 definitely wrong measurement lol".to_string(),
            like_count: 5,
        },
        RawVideoComment {
            author: "Dave".to_string(),
            text: "05:20 lmao that dog running in the background is the best part".to_string(),
            like_count: 120,
        },
    ];

    let clusters = CommentFrictionMiner::mine_comment_clusters(&comments, None);

    assert_eq!(clusters.len(), 2);

    // Cluster 1: The 1:40 - 1:48 friction cluster
    let c1 = &clusters[0];
    assert_eq!(c1.mention_count, 3);
    assert_eq!(c1.total_likes, 65);
    assert_eq!(c1.sentiment, ClusterSentiment::ConfusionOrFriction);
    assert!(c1.center_second >= 100.0 && c1.center_second <= 108.0);
    assert!(c1.representative_quote.contains("why did he cut that board"));

    // Cluster 2: The 5:20 comedy replay cluster
    let c2 = &clusters[1];
    assert_eq!(c2.mention_count, 1);
    assert_eq!(c2.total_likes, 120);
    assert_eq!(c2.sentiment, ClusterSentiment::PraiseOrHumor);
    assert_eq!(c2.center_second, 320.0);
}

#[test]
fn test_sentiment_classification() {
    assert_eq!(
        CommentFrictionMiner::classify_sentiment("why is the pacing so slow, so confused and cringe"),
        ClusterSentiment::ConfusionOrFriction
    );
    assert_eq!(
        CommentFrictionMiner::classify_sentiment("lmao dead best part ever had to rewind 5 times"),
        ClusterSentiment::PraiseOrHumor
    );
    assert_eq!(
        CommentFrictionMiner::classify_sentiment("omg that plot twist ending gave me chills, cannot believe it"),
        ClusterSentiment::StoryEngagement
    );
    assert_eq!(
        CommentFrictionMiner::classify_sentiment("04:30 sequence starts here"),
        ClusterSentiment::NeutralTimecode
    );
}

#[test]
fn test_retention_curve_drop_cliff_correlation() {
    let mut clusters = vec![
        CommentTimestampCluster {
            center_second: 102.0,
            timecode_formatted: "01:42".to_string(),
            mention_count: 5,
            total_likes: 200,
            representative_quote: "Why did you skip the foundation build?".to_string(),
            consensus_topic: "Viewer confusion".to_string(),
            sentiment: ClusterSentiment::ConfusionOrFriction,
            correlated_retention_event: None,
        },
        CommentTimestampCluster {
            center_second: 300.0,
            timecode_formatted: "05:00".to_string(),
            mention_count: 8,
            total_likes: 450,
            representative_quote: "The timber framed wall going up was epic!".to_string(),
            consensus_topic: "Viewer praise".to_string(),
            sentiment: ClusterSentiment::PraiseOrHumor,
            correlated_retention_event: None,
        },
    ];

    let retention_report = RetentionCurveReport {
        video_id: "test_vid".to_string(),
        title: "Test Build".to_string(),
        duration_seconds: 600.0,
        average_intensity: 0.55,
        peak_timestamp_formatted: "05:00".to_string(),
        peak_transcript: "Look at that structure!".to_string(),
        major_dips: Vec::new(),
        major_spikes: vec![RetentionEvent {
            timestamp_sec: 302.0,
            formatted_timestamp: "05:02".to_string(),
            intensity: 0.85,
            delta_intensity: 0.18,
            event_type: RetentionEventType::ReplaySpike,
            transcript_context: "epic build".to_string(),
            tactical_diagnosis: "Audience replay surge".to_string(),
            replay_root_cause: Some(ReplayRootCause::VisualPayoffSilence),
        }],
        curve_health_verdict: "Strong".to_string(),
        tactical_rules: Vec::new(),
        attention_friction_index: 12.0,
        attention_breaches: vec![AttentionBreach {
            timestamp_sec: 100.0,
            formatted_timestamp: "01:40".to_string(),
            intensity: 0.40,
            r_prime: -0.045,
            root_cause: BreachRootCause::NarrativePlateau,
            transcript_context: "moving on to the roof".to_string(),
            tactical_remedy: "Insert bridge B-roll".to_string(),
        }],
        editing_prescription: Vec::new(),
        re_engagement_checkpoints: Vec::new(),
    };

    CommentFrictionMiner::correlate_with_retention(&mut clusters, &retention_report);

    // Cluster 1 correlated with drop cliff
    assert!(clusters[0].correlated_retention_event.is_some());
    let rel1 = clusters[0].correlated_retention_event.as_ref().unwrap();
    assert!(rel1.contains("Retention Cliff (-4.5%/s drop at 100s)"));

    // Cluster 2 correlated with replay spike
    assert!(clusters[1].correlated_retention_event.is_some());
    let rel2 = clusters[1].correlated_retention_event.as_ref().unwrap();
    assert!(rel2.contains("Replay Spike (+18.0% at 302s)"));
}

#[test]
fn test_flight_sheet_comment_cluster_timeline_markers() {
    let clusters = vec![
        CommentTimestampCluster {
            center_second: 75.0,
            timecode_formatted: "01:15".to_string(),
            mention_count: 4,
            total_likes: 80,
            representative_quote: "Wait why did you throw away the old battery?".to_string(),
            consensus_topic: "Viewer confusion".to_string(),
            sentiment: ClusterSentiment::ConfusionOrFriction,
            correlated_retention_event: Some("Retention Cliff (-3.2%/s drop at 74s)".to_string()),
        },
        CommentTimestampCluster {
            center_second: 180.0,
            timecode_formatted: "03:00".to_string(),
            mention_count: 12,
            total_likes: 350,
            representative_quote: "Best laugh of the whole video haha".to_string(),
            consensus_topic: "Viewer comedy".to_string(),
            sentiment: ClusterSentiment::PraiseOrHumor,
            correlated_retention_event: Some("Replay Spike (+12.5% at 181s)".to_string()),
        },
    ];

    let scorecard = MarginalGainsScorecard {
        target_id: "test".to_string(),
        title: "DIY Offgrid Solar".to_string(),
        niche_profile: "Off-Grid".to_string(),
        tone_profile: None,
        composite_brailsford_score: 82.0,
        series_system_reliability_pct: 78.0,
        single_point_of_failure: None,
        satisfaction_index: None,
        predicted_multiplier_lift: 15.0,
        vectors: Vec::new(),
        top_3_marginal_gains: Vec::new(),
        series_funnel_insight: None,
        outro_exit_note: None,
        duration_seconds: Some(600.0),
        is_local_draft: false,
        thumbnail_report: None,
        promise_gap_report: None,
        actual_spoken_hook: None,
        loudness_metrics: None,
        cuttable_fat: None,
        comment_clusters: clusters,
        suitability_report: None,
        shorts_metrics: None,
        re_engagement_checkpoints: Vec::new(),
        stair_stepping: None,
        outro_report: None,
        retention_curve: Vec::new(),
        attention_breaches: Vec::new(),
    };

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let timeline = flight_sheet.to_export_timeline();

    let all_markers: Vec<_> = timeline
        .shots
        .iter()
        .flat_map(|s| &s.markers)
        .collect();

    // Check for Red [AUDIENCE FRICTION] marker
    let friction_marker = all_markers
        .iter()
        .find(|m| m.name.contains("[AUDIENCE FRICTION] 01:15"))
        .expect("Should find [AUDIENCE FRICTION] marker");
    assert_eq!(friction_marker.color, MarkerColor::Red);
    assert!(friction_marker.note.contains("why did you throw away the old battery?"));
    assert!(friction_marker.note.contains("Retention Cliff"));

    // Check for Green [AUDIENCE REACTION] marker
    let reaction_marker = all_markers
        .iter()
        .find(|m| m.name.contains("[AUDIENCE REACTION] 03:00"))
        .expect("Should find [AUDIENCE REACTION] marker");
    assert_eq!(reaction_marker.color, MarkerColor::Green);
    assert!(reaction_marker.note.contains("Best laugh of the whole video"));
    assert!(reaction_marker.note.contains("Replay Spike"));
}

#[test]
fn test_flight_sheet_renders_comments_table_zero_latex() {
    let clusters = vec![
        CommentTimestampCluster {
            center_second: 95.0,
            timecode_formatted: "01:35".to_string(),
            mention_count: 7,
            total_likes: 150,
            representative_quote: "Wait is that real copper or aluminum wire?".to_string(),
            consensus_topic: "Audience Confusion".to_string(),
            sentiment: ClusterSentiment::ConfusionOrFriction,
            correlated_retention_event: Some("Retention Cliff (-2.8%/s drop at 94s)".to_string()),
        },
    ];

    let scorecard = MarginalGainsScorecard {
        target_id: "test2".to_string(),
        title: "Wiring My Entire Cabin In 24 Hours".to_string(),
        niche_profile: "DIY Homestead".to_string(),
        tone_profile: None,
        composite_brailsford_score: 75.0,
        series_system_reliability_pct: 71.0,
        single_point_of_failure: None,
        satisfaction_index: None,
        predicted_multiplier_lift: 10.0,
        vectors: Vec::new(),
        top_3_marginal_gains: Vec::new(),
        series_funnel_insight: None,
        outro_exit_note: None,
        duration_seconds: Some(400.0),
        is_local_draft: false,
        thumbnail_report: None,
        promise_gap_report: None,
        actual_spoken_hook: None,
        loudness_metrics: None,
        cuttable_fat: None,
        comment_clusters: clusters,
        suitability_report: None,
        shorts_metrics: None,
        re_engagement_checkpoints: Vec::new(),
        stair_stepping: None,
        outro_report: None,
        retention_curve: Vec::new(),
        attention_breaches: Vec::new(),
    };

    let flight_sheet = FlightSheet::from_scorecard(&scorecard);
    let term = flight_sheet.render_terminal_string();
    let md = flight_sheet.to_markdown_string();

    // Verify sections are present
    assert!(term.contains("CROWD-SOURCED COMMENT FRICTION & ENGAGEMENT CLUSTERS"));
    assert!(term.contains("[01:35]"));
    assert!(term.contains("Wait is that real copper"));

    assert!(md.contains("## 💬 Crowd-Sourced Comment Friction & Engagement Clusters"));
    assert!(md.contains("| `01:35` | 🚨 FRICTION / CONFUSION | 7 | 150 |"));

    // Verify strict ZERO LaTeX rule
    for bad in ["$\\le$", "$\\ge$", "\\text{", "\\%"] {
        assert!(!term.contains(bad), "Terminal output must not contain LaTeX {}", bad);
        assert!(!md.contains(bad), "Markdown output must not contain LaTeX {}", bad);
    }
}
