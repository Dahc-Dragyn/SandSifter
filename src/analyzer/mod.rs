pub mod audio_transcriber;
pub mod comments;
pub mod hook_aligner;
pub mod keyframe;
pub mod narrative;
pub mod ocr;
pub mod suitability;
pub mod thumbnail;
pub mod transcript;
pub mod vad;

pub use audio_transcriber::AudioTranscriber;
pub use comments::{
    ClusterSentiment, CommentFrictionMiner, CommentTimestampCluster, RawVideoComment,
    TimestampMention,
};
pub use hook_aligner::{HookAligner, HookExtractionResult, PromiseGapEvaluator, PromiseGapReport};
pub use keyframe::{Keyframe, KeyframeExtractor};
pub use narrative::{synthesize_madlib, NarrativeClassifier};
pub use ocr::{KeyframeOcrResult, OcrEngine, OcrTextOverlay};
pub use suitability::{
    SuitabilityAuditor, SuitabilityCategory, SuitabilitySeverity, SuitabilityViolation,
    VideoSuitabilityReport,
};
pub use thumbnail::{ThumbnailForensicReport, ThumbnailVisionEngine, VisualClickbaitArchetype};
pub use transcript::{FullTranscript, TimestampedSentence, TimestampedWord};
pub use vad::VadClassifier;


