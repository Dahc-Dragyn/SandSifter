use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ingest::resolver::MediaResolver;
use crate::math::baseline::BaselineEngine;
use crate::math::outlier::OutlierEngine;
use crate::models::channel::WatchlistChannel;
use crate::models::video::{VideoChannelDetails, VideoDetail, VideoSnapshot};
use crate::storage::SandsifterDb;

/// Deserializable single historical video record supporting flexible platform field aliases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportVideoItem {
    #[serde(alias = "url", alias = "canonical_url")]
    pub video_url: String,

    #[serde(alias = "view_count")]
    pub views: u64,

    #[serde(default, alias = "like_count")]
    pub likes: Option<u64>,

    #[serde(default, alias = "comment_count")]
    pub comments: Option<u64>,

    #[serde(default, alias = "share_count")]
    pub shares: Option<u64>,

    #[serde(default)]
    pub published_at: Option<String>,

    #[serde(default, alias = "title", alias = "description")]
    pub caption: Option<String>,

    #[serde(default, alias = "channel", alias = "uploader", alias = "uploader_id")]
    pub channel_handle: Option<String>,

    #[serde(default)]
    pub platform: Option<String>,
}

/// Optional channel feed envelope grouping historical videos under a channel header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportChannelFeed {
    #[serde(default, alias = "channel", alias = "handle")]
    pub channel_handle: Option<String>,

    #[serde(default)]
    pub platform: Option<String>,

    pub videos: Vec<ImportVideoItem>,
}

/// Statistical summary of a completed batch ingestion run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportResult {
    pub channels_updated: usize,
    pub videos_imported: usize,
    pub snapshots_recorded: usize,
    pub channel_baselines: Vec<(String, u64)>,
    pub top_outliers: Vec<VideoDetail>,
}

pub struct BatchImporter;

impl BatchImporter {
    /// Ingests a raw JSON string (array of videos or channel feed) directly into the SQLite database.
    pub fn import_from_json_str(
        db: &SandsifterDb,
        json_str: &str,
        fallback_handle: Option<&str>,
    ) -> Result<ImportResult, String> {
        let items: Vec<ImportVideoItem> = if let Ok(list) = serde_json::from_str::<Vec<ImportVideoItem>>(json_str) {
            list
        } else if let Ok(feed) = serde_json::from_str::<ImportChannelFeed>(json_str) {
            let mut list = feed.videos;
            if let Some(h) = feed.channel_handle {
                for item in &mut list {
                    if item.channel_handle.is_none() {
                        item.channel_handle = Some(h.clone());
                    }
                }
            }
            list
        } else {
            return Err("Invalid JSON format. Expected JSON array of video objects or { videos: [...] } feed.".to_string());
        };

        if items.is_empty() {
            return Ok(ImportResult {
                channels_updated: 0,
                videos_imported: 0,
                snapshots_recorded: 0,
                channel_baselines: Vec::new(),
                top_outliers: Vec::new(),
            });
        }

        // Group videos by channel handle
        let mut by_channel: HashMap<String, Vec<ImportVideoItem>> = HashMap::new();

        for item in items {
            let handle = item
                .channel_handle
                .clone()
                .or_else(|| fallback_handle.map(|s| s.to_string()))
                .unwrap_or_else(|| {
                    // Try to extract handle from video URL
                    if let Some(at_idx) = item.video_url.find("/@") {
                        let remainder = &item.video_url[at_idx + 1..];
                        remainder.split('/').next().unwrap_or("unknown_creator").to_string()
                    } else {
                        "imported_creator".to_string()
                    }
                });

            let clean_handle = if handle.starts_with('@') { handle } else { format!("@{}", handle) };
            by_channel.entry(clean_handle).or_default().push(item);
        }

        let mut total_videos = 0usize;
        let mut total_snapshots = 0usize;
        let mut baselines = Vec::new();
        let now_iso = "2026-03-01T12:00:00Z";

        for (channel_handle, videos) in &by_channel {
            // 1. Determine platform
            let platform_str = videos
                .first()
                .and_then(|v| v.platform.clone())
                .unwrap_or_else(|| {
                    let first_url = &videos[0].video_url;
                    let (p, _) = MediaResolver::parse_video_url(first_url);
                    p.to_string()
                });

            // 2. Compute Creator Baseline using real IQR Winsorization & rolling median
            let view_counts: Vec<u64> = videos.iter().map(|v| v.views).collect();
            let baseline = BaselineEngine::compute_baseline(&view_counts).unwrap_or(10_000);
            baselines.push((channel_handle.clone(), baseline));

            let channel_uuid = format!("chan_{:x}", md5_hash(channel_handle));

            let channel_obj = WatchlistChannel {
                uuid: channel_uuid.clone(),
                platform: platform_str,
                handle: channel_handle.clone(),
                display_name: Some(channel_handle.trim_start_matches('@').to_string()),
                avatar_url: None,
                follower_count: None,
                baseline_median_views: baseline,
                video_count: Some(videos.len() as u64),
                last_crawled_at: Some(now_iso.to_string()),
                added_at: now_iso.to_string(),
            };

            db.upsert_channel(&channel_obj)
                .map_err(|e| format!("Failed to upsert channel {}: {}", channel_handle, e))?;

            // 3. Upsert individual videos and record snapshots
            for item in videos {
                let video_uuid = format!("vid_{:x}", md5_hash(&item.video_url));
                let multiplier = OutlierEngine::calculate_multiplier(item.views, baseline);

                let likes = item.likes.unwrap_or(0);
                let comments = item.comments.unwrap_or(0);
                let shares = item.shares.unwrap_or(0);
                let engagement_rate = if item.views > 0 {
                    ((likes + comments + shares) as f64 / item.views as f64) * 100.0
                } else {
                    0.0
                };

                let pub_at = item.published_at.as_deref().unwrap_or(now_iso);

                let video_obj = VideoDetail {
                    uuid: video_uuid.clone(),
                    caption: item.caption.clone(),
                    url: item.video_url.clone(),
                    thumbnail: None,
                    channel: VideoChannelDetails {
                        uuid: channel_uuid.clone(),
                        platform: channel_obj.platform.clone(),
                        handle: channel_handle.clone(),
                        display_name: channel_obj.display_name.clone(),
                        avatar_url: None,
                        follower_count: None,
                        baseline_median_views: Some(baseline),
                    },
                    view_count: item.views,
                    like_count: item.likes,
                    comment_count: item.comments,
                    share_count: item.shares,
                    outlier_score: multiplier,
                    engagement_rate,
                    published_at: pub_at.to_string(),
                    velocity_views_per_day: Some(crate::math::ForensicsEngine::calculate_velocity(item.views, Some(pub_at))),
                    analysis: None,
                    speech_onset_ms: None,
                    wpm_0_to_35: None,
                    spoken_hook: None,
                };

                db.upsert_video(&video_obj)
                    .map_err(|e| format!("Failed to upsert video {}: {}", item.video_url, e))?;
                total_videos += 1;

                // Index viral outlier hook with Gemini vector embedding
                if multiplier >= 2.0 {
                    if let Some(caption) = &item.caption {
                        if !caption.trim().is_empty() {
                            let madlib = crate::analyzer::synthesize_madlib(caption);
                            let client = crate::storage::gemini_client::GeminiEmbeddingClient::default();
                            let _ = db.index_hook_with_embedding_blocking(&video_uuid, caption, Some(&madlib), 3.0, &client);
                        }
                    }
                }

                // Record historical snapshot
                let snap_uuid = format!("snap_{:x}", md5_hash(&format!("{}_{}", video_uuid, pub_at)));
                let snap = VideoSnapshot {
                    uuid: snap_uuid,
                    video_uuid: video_uuid.clone(),
                    view_count: item.views,
                    like_count: item.likes,
                    comment_count: item.comments,
                    share_count: item.shares,
                    recorded_at: pub_at.to_string(),
                };
                db.insert_snapshot(&snap)
                    .map_err(|e| format!("Failed to insert snapshot: {}", e))?;
                total_snapshots += 1;
            }
        }

        // Fetch top outliers discovered across imported cohort
        let top_outliers = db.list_top_outliers(2.0, 10).unwrap_or_default();

        Ok(ImportResult {
            channels_updated: by_channel.len(),
            videos_imported: total_videos,
            snapshots_recorded: total_snapshots,
            channel_baselines: baselines,
            top_outliers,
        })
    }
}

pub fn md5_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
