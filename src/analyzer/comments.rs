use serde::{Deserialize, Serialize};
use crate::math::retention::RetentionCurveReport;

/// A raw viewer comment extracted from video metadata.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RawVideoComment {
    pub author: String,
    pub text: String,
    pub like_count: u64,
}

/// A parsed timecode mention within a viewer comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimestampMention {
    pub timecode_raw: String,
    pub timestamp_seconds: f64,
    pub comment_text: String,
    pub author: String,
    pub like_count: u64,
}

/// Categorical sentiment or engagement tone of a comment cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterSentiment {
    /// Friction, confusion, pacing drag, criticism, or questions
    ConfusionOrFriction,
    /// Praise, laughter, peak humor, excitement, or favorite moment
    PraiseOrHumor,
    /// Plot shock, narrative suspense, or thematic payoff
    StoryEngagement,
    /// Informational or neutral timestamp
    NeutralTimecode,
}

impl ClusterSentiment {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ConfusionOrFriction => "Friction & Confusion",
            Self::PraiseOrHumor => "Praise & Replay",
            Self::StoryEngagement => "Story Engagement",
            Self::NeutralTimecode => "Neutral Timecode",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::ConfusionOrFriction => "🚨 FRICTION / CONFUSION",
            Self::PraiseOrHumor => "🔥 REPLAY / PRAISE",
            Self::StoryEngagement => "🎬 STORY MOMENT",
            Self::NeutralTimecode => "⏱️ TIMECODE",
        }
    }
}

/// A cluster of viewer comments congregating around a specific video timestamp (within ±10s tolerance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentTimestampCluster {
    /// Center timestamp in seconds (weighted or median)
    pub center_second: f64,
    /// Formatted timecode (MM:SS or HH:MM:SS)
    pub timecode_formatted: String,
    /// Number of distinct comments referencing this sequence window
    pub mention_count: usize,
    /// Aggregated likes across all comments in this cluster
    pub total_likes: u64,
    /// Representative or highest-voted comment quote
    pub representative_quote: String,
    /// Detected consensus keyword or topic
    pub consensus_topic: String,
    /// Dominant sentiment classification
    pub sentiment: ClusterSentiment,
    /// Optional correlated retention curve event (drop cliff or replay spike within ±10s)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlated_retention_event: Option<String>,
}

/// Crowd-sourced timestamp comment mining engine.
pub struct CommentFrictionMiner;

impl CommentFrictionMiner {
    /// Parses all timecode mentions (M:SS, MM:SS, H:MM:SS, HH:MM:SS) from free text.
    /// Implements a zero-allocation, bounded-memory byte scanner with strict word boundaries.
    pub fn extract_timecodes(text: &str) -> Vec<(String, f64)> {
        let mut results = Vec::new();
        let bytes = text.as_bytes();
        let n = bytes.len();
        let mut i = 0;

        while i < n {
            if bytes[i] == b':' {
                // Find digits to the left of the colon
                let mut start = i;
                while start > 0 && bytes[start - 1].is_ascii_digit() {
                    start -= 1;
                }
                let left_digits = i - start;

                // Check boundary before start: must not be alphanumeric or another colon
                let boundary_before = start == 0 || (!bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b':');

                if (1..=2).contains(&left_digits) && boundary_before {
                    // Find digits to the right of the colon
                    let mut right_end = i + 1;
                    while right_end < n && bytes[right_end].is_ascii_digit() {
                        right_end += 1;
                    }
                    let right_digits = right_end - (i + 1);

                    if right_digits == 2 {
                        // Check if there is another colon after: H:MM:SS
                        if right_end < n && bytes[right_end] == b':' {
                            let sec_start = right_end + 1;
                            let mut sec_end = sec_start;
                            while sec_end < n && bytes[sec_end].is_ascii_digit() {
                                sec_end += 1;
                            }
                            let sec_digits = sec_end - sec_start;
                            let boundary_after = sec_end == n || !bytes[sec_end].is_ascii_alphanumeric();

                            if sec_digits == 2 && boundary_after {
                                if let (Ok(h), Ok(m), Ok(s)) = (
                                    text[start..i].parse::<u32>(),
                                    text[i + 1..right_end].parse::<u32>(),
                                    text[sec_start..sec_end].parse::<u32>(),
                                ) {
                                    if m < 60 && s < 60 {
                                        let raw = &text[start..sec_end];
                                        let total_sec = (h as f64 * 3600.0) + (m as f64 * 60.0) + (s as f64);
                                        results.push((raw.to_string(), total_sec));
                                        i = sec_end;
                                        continue;
                                    }
                                }
                            }
                        }

                        // Standard M:SS or MM:SS
                        let boundary_after = right_end == n || !bytes[right_end].is_ascii_alphanumeric();
                        if boundary_after {
                            if let (Ok(m), Ok(s)) = (
                                text[start..i].parse::<u32>(),
                                text[i + 1..right_end].parse::<u32>(),
                            ) {
                                if s < 60 {
                                    let raw = &text[start..right_end];
                                    let total_sec = (m as f64 * 60.0) + (s as f64);
                                    results.push((raw.to_string(), total_sec));
                                    i = right_end;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }

        results
    }

    /// Classifies the sentiment of comment text into friction, praise, story moment, or neutral.
    pub fn classify_sentiment(text: &str) -> ClusterSentiment {
        let lower = text.to_lowercase();

        // 1. Friction / Confusion / Critique
        let friction_tokens = [
            "why", "wait", "confused", "cringe", "skip", "boring", "fake", "mistake",
            "wrong", "annoying", "hate", "terrible", "bad", "slow", "fell off", "lost me",
            "waste", "clickbait", "disappointed", "awkward", "huh", "what happened",
            "ruined", "annoyed", "uncomfortable", "where did",
        ];
        if friction_tokens.iter().any(|&token| lower.contains(token)) {
            return ClusterSentiment::ConfusionOrFriction;
        }

        // 2. Praise / Humor / Replay
        let praise_tokens = [
            "lmao", "lol", "haha", "died", "dead", "best part", "hilarious", "love",
            "goat", "favorite", "epic", "legend", "rewind", "gold", "crying", "favorite part",
            "laughed so hard", "killed me", "replayed", "again and again", "underrated",
            "perfection", "genius", "bro really", "nah this",
        ];
        if praise_tokens.iter().any(|&token| lower.contains(token)) {
            return ClusterSentiment::PraiseOrHumor;
        }

        // 3. Story / Climax
        let story_tokens = [
            "finally", "did he", "did she", "omg", "plot twist", "insane", "ending",
            "moment", "cannot believe", "chills", "goosebumps", "climax", "speechless",
            "shocked", "heartbreaking", "emotional",
        ];
        if story_tokens.iter().any(|&token| lower.contains(token)) {
            return ClusterSentiment::StoryEngagement;
        }

        ClusterSentiment::NeutralTimecode
    }

    /// Extracts timestamp mentions across a batch of comments.
    pub fn parse_timestamp_mentions(comments: &[RawVideoComment]) -> Vec<TimestampMention> {
        let mut mentions = Vec::new();
        for comment in comments {
            let extracted = Self::extract_timecodes(&comment.text);
            for (raw, sec) in extracted {
                mentions.push(TimestampMention {
                    timecode_raw: raw,
                    timestamp_seconds: sec,
                    comment_text: comment.text.clone(),
                    author: comment.author.clone(),
                    like_count: comment.like_count,
                });
            }
        }
        mentions
    }

    /// Clusters timestamp mentions within ±tolerance_seconds (default 10.0s).
    pub fn cluster_mentions(
        mut mentions: Vec<TimestampMention>,
        tolerance_seconds: f64,
    ) -> Vec<CommentTimestampCluster> {
        if mentions.is_empty() {
            return Vec::new();
        }

        // Sort by timestamp seconds
        mentions.sort_by(|a, b| a.timestamp_seconds.partial_cmp(&b.timestamp_seconds).unwrap_or(std::cmp::Ordering::Equal));

        let tol = tolerance_seconds.max(1.0);
        let mut groups: Vec<Vec<TimestampMention>> = Vec::new();

        for mention in mentions {
            let can_place = groups.last().is_some_and(|last_group| {
                let group_avg: f64 = last_group.iter().map(|m| m.timestamp_seconds).sum::<f64>() / last_group.len() as f64;
                (mention.timestamp_seconds - group_avg).abs() <= tol
            });

            if can_place {
                if let Some(last_group) = groups.last_mut() {
                    last_group.push(mention);
                }
            } else {
                groups.push(vec![mention]);
            }
        }

        let mut clusters = Vec::new();

        for group in groups {
            let mention_count = group.len();
            let total_likes: u64 = group.iter().map(|m| m.like_count).sum();

            // Center calculation: weighted by like_count + 1 to give high-voted comments stronger anchor
            let total_weight: f64 = group.iter().map(|m| (m.like_count + 1) as f64).sum();
            let weighted_sum: f64 = group.iter().map(|m| m.timestamp_seconds * ((m.like_count + 1) as f64)).sum();
            let center_second = (weighted_sum / total_weight.max(1.0) * 10.0).round() / 10.0;

            // Formatted timecode
            let total_s = center_second.round() as u64;
            let timecode_formatted = if total_s >= 3600 {
                let h = total_s / 3600;
                let m = (total_s % 3600) / 60;
                let s = total_s % 60;
                format!("{:02}:{:02}:{:02}", h, m, s)
            } else {
                let m = total_s / 60;
                let s = total_s % 60;
                format!("{:02}:{:02}", m, s)
            };

            // Pick representative quote (highest liked comment in group)
            let top_mention = group.iter().max_by_key(|m| m.like_count).unwrap();
            let mut rep_quote = top_mention.comment_text.replace('\n', " ").trim().to_string();
            if rep_quote.chars().count() > 100 {
                rep_quote = format!("{}...", rep_quote.chars().take(97).collect::<String>());
            }

            // Determine dominant sentiment
            let mut friction_count = 0;
            let mut praise_count = 0;
            let mut story_count = 0;

            for m in &group {
                match Self::classify_sentiment(&m.comment_text) {
                    ClusterSentiment::ConfusionOrFriction => friction_count += 1,
                    ClusterSentiment::PraiseOrHumor => praise_count += 1,
                    ClusterSentiment::StoryEngagement => story_count += 1,
                    ClusterSentiment::NeutralTimecode => {}
                }
            }

            let sentiment = if friction_count > 0 && friction_count >= praise_count {
                ClusterSentiment::ConfusionOrFriction
            } else if praise_count >= friction_count && praise_count >= story_count && praise_count > 0 {
                ClusterSentiment::PraiseOrHumor
            } else if story_count > 0 {
                ClusterSentiment::StoryEngagement
            } else {
                ClusterSentiment::NeutralTimecode
            };

            let consensus_topic = match sentiment {
                ClusterSentiment::ConfusionOrFriction => "Viewer confusion / pacing friction".to_string(),
                ClusterSentiment::PraiseOrHumor => "Viewer praise / comedy replay".to_string(),
                ClusterSentiment::StoryEngagement => "Story beat engagement".to_string(),
                ClusterSentiment::NeutralTimecode => "Sequence reference".to_string(),
            };

            clusters.push(CommentTimestampCluster {
                center_second,
                timecode_formatted,
                mention_count,
                total_likes,
                representative_quote: rep_quote,
                consensus_topic,
                sentiment,
                correlated_retention_event: None,
            });
        }

        // Sort clusters by mention_count descending, then center_second ascending
        clusters.sort_by(|a, b| {
            b.mention_count.cmp(&a.mention_count)
                .then_with(|| a.center_second.partial_cmp(&b.center_second).unwrap_or(std::cmp::Ordering::Equal))
        });

        clusters
    }

    /// Correlates comment clusters with retention curve drop cliffs and replay spikes within ±10.0s tolerance.
    pub fn correlate_with_retention(
        clusters: &mut [CommentTimestampCluster],
        retention: &RetentionCurveReport,
    ) {
        for cluster in clusters.iter_mut() {
            let t = cluster.center_second;

            // 1. Check steepest attention breaches / drop cliffs
            let matched_breach = retention.attention_breaches.iter().find(|b| (b.timestamp_sec - t).abs() <= 10.0);
            if let Some(breach) = matched_breach {
                let drop_rate = (breach.r_prime * 100.0).abs();
                cluster.correlated_retention_event = Some(format!(
                    "Retention Cliff (-{:.1}%/s drop at {:.0}s) - {}",
                    drop_rate, breach.timestamp_sec, breach.root_cause.label()
                ));
                continue;
            }

            // 2. Check major dips
            let matched_dip = retention.major_dips.iter().find(|d| (d.timestamp_sec - t).abs() <= 10.0);
            if let Some(dip) = matched_dip {
                let drop_pct = (dip.delta_intensity * 100.0).abs();
                cluster.correlated_retention_event = Some(format!(
                    "Retention Dip (-{:.1}% at {:.0}s)",
                    drop_pct, dip.timestamp_sec
                ));
                continue;
            }

            // 3. Check major replay spikes
            let matched_spike = retention.major_spikes.iter().find(|s| (s.timestamp_sec - t).abs() <= 10.0);
            if let Some(spike) = matched_spike {
                let surge_pct = spike.delta_intensity * 100.0;
                cluster.correlated_retention_event = Some(format!(
                    "Replay Spike (+{:.1}% at {:.0}s)",
                    surge_pct, spike.timestamp_sec
                ));
            }
        }
    }

    /// End-to-end mining pipeline: parses comments, clusters timestamps (±10s),
    /// and correlates with audience retention curve events.
    pub fn mine_comment_clusters(
        comments: &[RawVideoComment],
        retention: Option<&RetentionCurveReport>,
    ) -> Vec<CommentTimestampCluster> {
        let mentions = Self::parse_timestamp_mentions(comments);
        let mut clusters = Self::cluster_mentions(mentions, 10.0);

        if let Some(ret) = retention {
            Self::correlate_with_retention(&mut clusters, ret);
        }

        clusters
    }

    /// Ingests comments via yt-dlp if online, bounded to max_comments (e.g. 50-100).
    pub fn extract_comments_with_ytdlp(url: &str, max_comments: usize) -> Result<Vec<RawVideoComment>, String> {
        let max_c = max_comments.clamp(10, 200);
        let extractor_arg = format!("youtube:max_comments={};max_comment_depth=1", max_c);

        let output = std::process::Command::new("yt-dlp")
            .args([
                "--dump-json",
                "--write-comments",
                "--extractor-args", &extractor_arg,
                "--skip-download",
                "--no-warnings",
                "--no-playlist",
                url,
            ])
            .output()
            .or_else(|_| {
                std::process::Command::new("uvx")
                    .args([
                        "yt-dlp",
                        "--dump-json",
                        "--write-comments",
                        "--extractor-args", &extractor_arg,
                        "--skip-download",
                        "--no-warnings",
                        "--no-playlist",
                        url,
                    ])
                    .output()
            })
            .map_err(|e| format!("Failed to spawn yt-dlp comment extractor: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp comment extraction failed: {}", stderr.trim()));
        }

        let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse yt-dlp JSON: {}", e))?;

        let mut comments = Vec::new();
        if let Some(arr) = json_val["comments"].as_array() {
            for c in arr {
                let author = c["author"].as_str().or_else(|| c["author_id"].as_str()).unwrap_or("viewer").to_string();
                let text = c["text"].as_str().unwrap_or("").to_string();
                let like_count = c["like_count"].as_u64().unwrap_or(0);
                if !text.trim().is_empty() {
                    comments.push(RawVideoComment {
                        author,
                        text,
                        like_count,
                    });
                }
            }
        }

        Ok(comments)
    }
}
