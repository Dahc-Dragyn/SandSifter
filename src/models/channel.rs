use serde::{Deserialize, Serialize};

/// Platform identifier enum for tracked creators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SocialPlatform {
    TikTok,
    Instagram,
    YouTube,
}

impl std::fmt::Display for SocialPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocialPlatform::TikTok => write!(f, "tiktok"),
            SocialPlatform::Instagram => write!(f, "instagram"),
            SocialPlatform::YouTube => write!(f, "youtube"),
        }
    }
}

/// Channel representation for searching across platforms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelSearchItem {
    pub uuid: String,
    pub platform: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follower_count: Option<u64>,
    #[serde(default)]
    pub in_watchlist: bool,
}

/// Watchlist channel entry containing computed baseline stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistChannel {
    pub uuid: String,
    pub platform: String,
    pub handle: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follower_count: Option<u64>,
    #[serde(default)]
    pub baseline_median_views: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_crawled_at: Option<String>,
    pub added_at: String,
}

/// Request to add channel URLs or handles to a workspace watchlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistAddRequest {
    pub channel_urls: Vec<String>,
}

/// Response returned when adding channels to the watchlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistAddResponse {
    pub added: Vec<String>,
    pub submitted_for_crawling: Vec<String>,
    pub skipped: Vec<String>,
}
