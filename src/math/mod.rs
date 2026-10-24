pub mod baseline;
pub mod forensics;
pub mod heuristics;
pub mod loudness;
pub mod marginal_gains;
pub mod outlier;
pub mod retention;
pub mod shorts;
pub mod topics;
pub mod visual;

pub use loudness::LoudnessMetrics;

pub use baseline::{BaselineDiagnostics, BaselineEngine};
pub use forensics::{
    AcousticDeliveryMetrics, ChannelForensicsReport, FeatureLiftAnalysis,
    ForensicEpisodeRecord, ForensicsEngine, HoldoutEpisodeComparison,
    HoldoutPredictionResult, LinguisticDecomposition, SequenceMomentumAnalysis,
    SubjectPolarity,
};
pub use heuristics::{EngagementAudit, HeuristicsEngine};
pub use marginal_gains::{
    CuttableFatReport, MarginalGainsEngine, MarginalGainsScorecard, PipelineStage,
    PunchInCue, TargetComparisonReport, TelemetryVector, VectorDeltaComparison, VectorStatus,
};
pub use outlier::{OutlierEngine, OutlierTier};
pub use retention::{
    HeatmapPoint, OutroRunwayReport, ReplayRootCause, RetentionCurveReport, RetentionEngine,
    RetentionEvent, RetentionEventType, TranscriptCue, VttCue,
};
pub use shorts::{ShortsDurationStatus, ShortsEngine, ShortsRetentionMetrics};
pub use topics::{
    DemandStackingSuggestion, TopicDemandStats, TopicEntityTrend, TopicMomentumEngine,
    TopicTrendReport, TopicVideoInput, TrendClassification,
};
pub use visual::{
    CutVelocityClassification, VisualPacingEngine, VisualPacingMetrics,
};


