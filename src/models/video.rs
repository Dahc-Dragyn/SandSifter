use serde::{Deserialize, Serialize};

/// Detailed metadata of the channel associated with a video.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VideoChannelDetails {
    pub uuid: String,
    pub platform: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follower_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_median_views: Option<u64>,
}

/// Chronological storytelling section of a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NarrativeStructureSection {
    pub section_name: String,
    pub section_description: String,
    #[serde(default)]
    pub transcript_sentences: Vec<String>,
    pub start_second: f64,
    pub end_second: f64,
}

/// High-level narrative structure summarizing the video's arc and timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NarrativeStructure {
    pub structure_summary: String,
    #[serde(default)]
    pub structure_sections: Vec<NarrativeStructureSection>,
}

/// Deep deconstructed video analysis containing hook mechanics, visual layouts, and narrative categorization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoAnalysisDetails {
    pub format_category: String,
    pub format_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_flavor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_reasoning: Option<String>,
    pub spoken_hook: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoken_hook_madlib: Option<String>,
    pub spoken_hook_category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_alignment: Option<String>,
    pub visual_layout_category: String,
    pub visual_layout_type: String,
    pub narrative_structure: NarrativeStructure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    pub topic: String,
    pub angle: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supporting_evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_belief: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contrarian_reality: Option<String>,
}

/// Complete video detail response representing an analyzed or unanalyzed short-form video.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VideoDetail {
    pub uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    pub channel: VideoChannelDetails,
    pub view_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_count: Option<u64>,
    #[serde(default)]
    pub outlier_score: f64,
    #[serde(default)]
    pub engagement_rate: f64,
    pub published_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub velocity_views_per_day: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<VideoAnalysisDetails>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_onset_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wpm_0_to_35: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoken_hook: Option<String>,
}

/// Lightweight item representation returned in video search queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSearchItem {
    pub uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    pub channel: VideoChannelDetails,
    pub view_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<u64>,
    #[serde(default)]
    pub outlier_score: f64,
    #[serde(default)]
    pub engagement_rate: f64,
    pub published_at: String,
    #[serde(default)]
    pub analyzed: bool,
}

/// Multi-attribute filter criteria for querying videos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VideoSearchFilters {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_uuids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlier_score_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outlier_score_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views_min: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views_max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engagement_rate_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engagement_rate_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookback_value: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookback_interval: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_analyzed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_unanalyzed: Option<bool>,
}

/// Request body for querying videos with pagination and search filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VideoSearchBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<VideoSearchFilters>,
    #[serde(default = "default_page_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_page_limit() -> u32 {
    20
}

/// Request payload to trigger multimodal analysis on a video URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoAnalyzeRequest {
    pub video_url: String,
    #[serde(default)]
    pub force_refresh: bool,
}

/// Historical snapshot record tracking video performance metrics over time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSnapshot {
    pub uuid: String,
    pub video_uuid: String,
    pub view_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub like_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_count: Option<u64>,
    pub recorded_at: String,
}

