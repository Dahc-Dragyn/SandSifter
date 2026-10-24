use std::path::Path;
use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

use crate::models::{VideoChannelDetails, VideoDetail, VideoSnapshot, WatchlistChannel};
use crate::storage::embeddings::cosine_similarity;
use crate::storage::rrf::{reciprocal_rank_fusion, RrfRankedItem, RRF_SMOOTHING_CONSTANT_K};

/// Unified search result returned from the hybrid retrieval engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HybridHookResult {
    pub hook_uuid: String,
    pub video_uuid: String,
    pub hook_text: String,
    pub spoken_hook_madlib: Option<String>,
    pub rrf_score: f64,
    pub vector_similarity: Option<f32>,
    pub lexical_match: bool,
    #[serde(default)]
    pub video_title: Option<String>,
    #[serde(default)]
    pub channel_handle: Option<String>,
    #[serde(default)]
    pub outlier_score: Option<f64>,
}

impl HybridHookResult {
    /// Formats this search result into a creator-friendly, empowering breakdown.
    pub fn format_for_creator(&self, rank: usize) -> String {
        let madlib = self
            .spoken_hook_madlib
            .as_deref()
            .unwrap_or("Use your own creative twist on this hook!");
        let multiplier_str = self
            .outlier_score
            .map(|s| format!("{:.1}x Reader Resonance", s))
            .unwrap_or_else(|| "Proven Viral Outlier".to_string());
        let channel_str = self
            .channel_handle
            .as_deref()
            .unwrap_or("Creator Watchlist");
        let sim_percent = self
            .vector_similarity
            .map(|sim| format!("{:.1}% Semantic Match", (sim * 100.0).clamp(0.0, 100.0)))
            .unwrap_or_else(|| "Direct Keyword Match".to_string());

        format!(
            "🎣 Hook #{rank}: \"{}\"\n   📝 Fill-in-the-Blank Madlib: \"{}\"\n   📊 Audience Impact: {} (from {})\n   🎯 Relevance: {} (RRF Score: {:.4})\n",
            self.hook_text, madlib, multiplier_str, channel_str, sim_percent, self.rrf_score
        )
    }
}

/// Stored Hook representation in the database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredHook {
    pub uuid: String,
    pub video_uuid: String,
    pub hook_text: String,
    pub spoken_hook_madlib: Option<String>,
    pub duration_seconds: f64,
    pub embedding: Vec<f32>,
}

/// Persistent tracking record for generated hooks to close the feedback loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedHookRecord {
    pub id: String,
    pub channel_handle: String,
    pub topic: String,
    pub hook_text: String,
    pub hook_type: String, // "Adapted" or "Scratch"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_formula: Option<String>,
    pub predicted_score: f64,
    pub status: String, // "generated", "selected", "filmed", "published"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_views: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_multiplier: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_engagement_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Local high-performance relational & vector database for Sandsifter.
#[derive(Clone)]
pub struct SandsifterDb {
    conn: Arc<Mutex<Connection>>,
}

impl SandsifterDb {
    /// Opens an in-memory SQLite database (ideal for zero-disk test harnesses)
    pub fn memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Opens or creates a persistent SQLite database on disk
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Default persistent database file path
    pub fn default_path() -> std::path::PathBuf {
        std::path::PathBuf::from("sandsifter.db")
    }

    /// Opens the default persistent SQLite database on disk
    pub fn open_default() -> Result<Self> {
        Self::open(Self::default_path())
    }

    /// Auto-migrates database tables, constraints, and indexes
    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS channels (
                uuid TEXT PRIMARY KEY,
                platform TEXT NOT NULL,
                handle TEXT NOT NULL UNIQUE,
                display_name TEXT,
                avatar_url TEXT,
                follower_count INTEGER,
                baseline_median_views INTEGER NOT NULL DEFAULT 0,
                video_count INTEGER DEFAULT 0,
                last_crawled_at TEXT,
                added_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS videos (
                uuid TEXT PRIMARY KEY,
                channel_uuid TEXT NOT NULL,
                url TEXT NOT NULL UNIQUE,
                caption TEXT,
                thumbnail TEXT,
                view_count INTEGER NOT NULL DEFAULT 0,
                like_count INTEGER,
                comment_count INTEGER,
                share_count INTEGER,
                outlier_score REAL NOT NULL DEFAULT 1.0,
                engagement_rate REAL NOT NULL DEFAULT 0.0,
                published_at TEXT NOT NULL,
                analyzed INTEGER NOT NULL DEFAULT 0,
                speech_onset_ms INTEGER,
                wpm_0_to_35 REAL,
                spoken_hook TEXT,
                velocity_views_per_day REAL,
                FOREIGN KEY (channel_uuid) REFERENCES channels(uuid) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                uuid TEXT PRIMARY KEY,
                video_uuid TEXT NOT NULL,
                view_count INTEGER NOT NULL,
                like_count INTEGER,
                comment_count INTEGER,
                share_count INTEGER,
                recorded_at TEXT NOT NULL,
                FOREIGN KEY (video_uuid) REFERENCES videos(uuid) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS hooks (
                uuid TEXT PRIMARY KEY,
                video_uuid TEXT NOT NULL,
                hook_text TEXT NOT NULL,
                spoken_hook_madlib TEXT,
                duration_seconds REAL NOT NULL DEFAULT 3.0,
                embedding_json TEXT NOT NULL,
                FOREIGN KEY (video_uuid) REFERENCES videos(uuid) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS tracked_hooks (
                id TEXT PRIMARY KEY,
                channel_handle TEXT NOT NULL,
                topic TEXT NOT NULL,
                hook_text TEXT NOT NULL,
                hook_type TEXT NOT NULL,
                source_formula TEXT,
                predicted_score REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'generated',
                published_url TEXT,
                actual_views INTEGER,
                actual_multiplier REAL,
                actual_engagement_rate REAL,
                performance_notes TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_videos_channel ON videos(channel_uuid);
            CREATE INDEX IF NOT EXISTS idx_videos_outlier ON videos(outlier_score DESC);
            CREATE INDEX IF NOT EXISTS idx_snapshots_video ON snapshots(video_uuid);
            CREATE INDEX IF NOT EXISTS idx_hooks_video ON hooks(video_uuid);
            CREATE INDEX IF NOT EXISTS idx_tracked_hooks_channel ON tracked_hooks(channel_handle);
            CREATE INDEX IF NOT EXISTS idx_tracked_hooks_status ON tracked_hooks(status);
            "#,
        )?;

        // Ensure legacy SQLite databases acquire new kinetics columns seamlessly
        let _ = conn.execute("ALTER TABLE videos ADD COLUMN speech_onset_ms INTEGER;", []);
        let _ = conn.execute("ALTER TABLE videos ADD COLUMN wpm_0_to_35 REAL;", []);
        let _ = conn.execute("ALTER TABLE videos ADD COLUMN spoken_hook TEXT;", []);
        let _ = conn.execute("ALTER TABLE videos ADD COLUMN duration_string TEXT;", []);
        let _ = conn.execute("ALTER TABLE videos ADD COLUMN velocity_views_per_day REAL;", []);

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Channels & Watchlist Management
    // ─────────────────────────────────────────────────────────────────────────

    pub fn upsert_channel(&self, channel: &WatchlistChannel) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO channels (
                uuid, platform, handle, display_name, avatar_url, follower_count,
                baseline_median_views, video_count, last_crawled_at, added_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(handle) DO UPDATE SET
                display_name = excluded.display_name,
                avatar_url = excluded.avatar_url,
                follower_count = excluded.follower_count,
                baseline_median_views = excluded.baseline_median_views,
                video_count = excluded.video_count,
                last_crawled_at = excluded.last_crawled_at;
            "#,
            params![
                channel.uuid,
                channel.platform,
                channel.handle,
                channel.display_name,
                channel.avatar_url,
                channel.follower_count,
                channel.baseline_median_views,
                channel.video_count,
                channel.last_crawled_at,
                channel.added_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_channel_by_handle(&self, handle: &str) -> Result<Option<WatchlistChannel>> {
        let clean_handle = handle.trim_start_matches('@');
        let h_at = format!("@{}", clean_handle);
        let h_raw = clean_handle.to_string();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT uuid, platform, handle, display_name, avatar_url, follower_count,
                   baseline_median_views, video_count, last_crawled_at, added_at
            FROM channels WHERE handle = ?1 OR handle = ?2 OR handle = ?3 OR handle LIKE ?4 OR display_name LIKE ?4
            "#,
        )?;

        let h_like = format!("%{}%", clean_handle);
        let mut rows = stmt.query(params![handle, h_at, h_raw, h_like])?;
        if let Some(row) = rows.next()? {
            Ok(Some(WatchlistChannel {
                uuid: row.get(0)?,
                platform: row.get(1)?,
                handle: row.get(2)?,
                display_name: row.get(3)?,
                avatar_url: row.get(4)?,
                follower_count: row.get(5)?,
                baseline_median_views: row.get(6)?,
                video_count: row.get(7)?,
                last_crawled_at: row.get(8)?,
                added_at: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_channel_baseline(&self, handle: &str, baseline: u64) -> Result<bool> {
        let clean_handle = handle.trim_start_matches('@');
        let h_at = format!("@{}", clean_handle);
        let h_raw = clean_handle.to_string();
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE channels SET baseline_median_views = ?1 WHERE handle = ?2 OR handle = ?3",
            params![baseline, h_at, h_raw],
        )?;
        Ok(affected > 0)
    }

    pub fn list_channels(&self) -> Result<Vec<WatchlistChannel>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT uuid, platform, handle, display_name, avatar_url, follower_count,
                   baseline_median_views, video_count, last_crawled_at, added_at
            FROM channels ORDER BY handle ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(WatchlistChannel {
                uuid: row.get(0)?,
                platform: row.get(1)?,
                handle: row.get(2)?,
                display_name: row.get(3)?,
                avatar_url: row.get(4)?,
                follower_count: row.get(5)?,
                baseline_median_views: row.get(6)?,
                video_count: row.get(7)?,
                last_crawled_at: row.get(8)?,
                added_at: row.get(9)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Videos Management
    // ─────────────────────────────────────────────────────────────────────────

    pub fn upsert_video(&self, video: &VideoDetail) -> Result<()> {
        let velocity = video.velocity_views_per_day.unwrap_or_else(|| {
            crate::math::ForensicsEngine::calculate_velocity(video.view_count, Some(&video.published_at))
        });
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO videos (
                uuid, channel_uuid, url, caption, thumbnail, view_count, like_count,
                comment_count, share_count, outlier_score, engagement_rate, published_at, analyzed,
                speech_onset_ms, wpm_0_to_35, spoken_hook, velocity_views_per_day
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(url) DO UPDATE SET
                view_count = excluded.view_count,
                like_count = excluded.like_count,
                comment_count = excluded.comment_count,
                share_count = excluded.share_count,
                outlier_score = excluded.outlier_score,
                engagement_rate = excluded.engagement_rate,
                analyzed = excluded.analyzed,
                speech_onset_ms = COALESCE(excluded.speech_onset_ms, videos.speech_onset_ms),
                wpm_0_to_35 = COALESCE(excluded.wpm_0_to_35, videos.wpm_0_to_35),
                spoken_hook = COALESCE(excluded.spoken_hook, videos.spoken_hook),
                velocity_views_per_day = COALESCE(excluded.velocity_views_per_day, videos.velocity_views_per_day);
            "#,
            params![
                video.uuid,
                video.channel.uuid,
                video.url,
                video.caption,
                video.thumbnail,
                video.view_count,
                video.like_count,
                video.comment_count,
                video.share_count,
                video.outlier_score,
                video.engagement_rate,
                video.published_at,
                if video.analysis.is_some() { 1 } else { 0 },
                video.speech_onset_ms,
                video.wpm_0_to_35,
                video.spoken_hook,
                velocity,
            ],
        )?;
        Ok(())
    }

    pub fn list_top_outliers(&self, min_multiplier: f64, limit: usize) -> Result<Vec<VideoDetail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT v.uuid, v.url, v.caption, v.thumbnail, v.view_count, v.like_count,
                   v.comment_count, v.share_count, v.outlier_score, v.engagement_rate,
                   v.published_at, v.analyzed,
                   c.uuid, c.platform, c.handle, c.display_name, c.avatar_url,
                   c.follower_count, c.baseline_median_views,
                   v.speech_onset_ms, v.wpm_0_to_35, v.spoken_hook,
                   v.velocity_views_per_day
            FROM videos v
            JOIN channels c ON v.channel_uuid = c.uuid
            WHERE v.outlier_score >= ?1
            ORDER BY v.outlier_score DESC
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![min_multiplier, limit as i64], |row| {
            let channel = VideoChannelDetails {
                uuid: row.get(12)?,
                platform: row.get(13)?,
                handle: row.get(14)?,
                display_name: row.get(15)?,
                avatar_url: row.get(16)?,
                follower_count: row.get(17)?,
                baseline_median_views: row.get(18)?,
            };

            let vel: Option<f64> = row.get(22).ok();
            Ok(VideoDetail {
                uuid: row.get(0)?,
                url: row.get(1)?,
                caption: row.get(2)?,
                thumbnail: row.get(3)?,
                view_count: row.get(4)?,
                like_count: row.get(5)?,
                comment_count: row.get(6)?,
                share_count: row.get(7)?,
                outlier_score: row.get(8)?,
                engagement_rate: row.get(9)?,
                published_at: row.get(10)?,
                velocity_views_per_day: vel,
                channel,
                analysis: None,
                speech_onset_ms: row.get(19)?,
                wpm_0_to_35: row.get(20)?,
                spoken_hook: row.get(21)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn count_videos(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM videos")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn insert_snapshot(&self, snapshot: &VideoSnapshot) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO snapshots (
                uuid, video_uuid, view_count, like_count, comment_count, share_count, recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(uuid) DO UPDATE SET
                view_count = excluded.view_count,
                like_count = excluded.like_count,
                comment_count = excluded.comment_count,
                share_count = excluded.share_count,
                recorded_at = excluded.recorded_at;
            "#,
            params![
                snapshot.uuid,
                snapshot.video_uuid,
                snapshot.view_count,
                snapshot.like_count,
                snapshot.comment_count,
                snapshot.share_count,
                snapshot.recorded_at
            ],
        )?;
        Ok(())
    }

    pub fn count_snapshots(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM snapshots")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn get_videos_by_channel(&self, handle: &str) -> Result<Vec<VideoDetail>> {
        let clean_handle = handle.trim_start_matches('@');
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT v.uuid, v.url, v.caption, v.thumbnail, v.view_count, v.like_count,
                   v.comment_count, v.share_count, v.outlier_score, v.engagement_rate,
                   v.published_at, v.analyzed,
                   c.uuid, c.platform, c.handle, c.display_name, c.avatar_url,
                   c.follower_count, c.baseline_median_views,
                   v.speech_onset_ms, v.wpm_0_to_35, v.spoken_hook,
                   v.velocity_views_per_day
            FROM videos v
            JOIN channels c ON v.channel_uuid = c.uuid
            WHERE c.handle = ?1 OR c.handle = ?2 OR c.handle = ?3 OR c.handle LIKE ?4 OR c.display_name LIKE ?4
            ORDER BY v.published_at DESC
            "#,
        )?;

        let h_at = format!("@{}", clean_handle);
        let h_raw = clean_handle.to_string();
        let h_like = format!("%{}%", clean_handle);

        let rows = stmt.query_map(params![handle, h_at, h_raw, h_like], |row| {
            let channel = VideoChannelDetails {
                uuid: row.get(12)?,
                platform: row.get(13)?,
                handle: row.get(14)?,
                display_name: row.get(15)?,
                avatar_url: row.get(16)?,
                follower_count: row.get(17)?,
                baseline_median_views: row.get(18)?,
            };

            let vel: Option<f64> = row.get(22).ok();
            Ok(VideoDetail {
                uuid: row.get(0)?,
                url: row.get(1)?,
                caption: row.get(2)?,
                thumbnail: row.get(3)?,
                view_count: row.get(4)?,
                like_count: row.get(5)?,
                comment_count: row.get(6)?,
                share_count: row.get(7)?,
                outlier_score: row.get(8)?,
                engagement_rate: row.get(9)?,
                published_at: row.get(10)?,
                velocity_views_per_day: vel,
                channel,
                analysis: None,
                speech_onset_ms: row.get(19)?,
                wpm_0_to_35: row.get(20)?,
                spoken_hook: row.get(21)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Fetches a mapping of video URL to (duration_string, published_at) for all videos in a channel.
    #[allow(clippy::type_complexity)]
    pub fn get_video_metadata_by_channel(&self, handle: &str) -> Result<std::collections::HashMap<String, (Option<String>, Option<String>)>> {
        let clean_handle = handle.trim_start_matches('@');
        let conn = self.conn.lock().unwrap();
        let query = r#"
            SELECT v.url, v.duration_string, v.published_at
            FROM videos v
            JOIN channels c ON v.channel_uuid = c.uuid
            WHERE c.handle = ?1 OR c.handle = ?2 OR c.handle = ?3
        "#;
        let h_at = format!("@{}", clean_handle);
        let h_raw = clean_handle.to_string();
        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map(params![handle, h_at, h_raw], |row| {
            let url: String = row.get(0)?;
            let dur: Option<String> = row.get(1).unwrap_or(None);
            let pub_date: Option<String> = row.get(2).unwrap_or(None);
            Ok((url, (dur, pub_date)))
        })?;
        let mut map = std::collections::HashMap::new();
        for r in rows.flatten() {
            map.insert(r.0, r.1);
        }
        Ok(map)
    }

    pub fn get_video_by_url(&self, url: &str) -> Result<Option<VideoDetail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT v.uuid, v.url, v.caption, v.thumbnail, v.view_count, v.like_count,
                   v.comment_count, v.share_count, v.outlier_score, v.engagement_rate,
                   v.published_at, v.analyzed,
                   c.uuid, c.platform, c.handle, c.display_name, c.avatar_url,
                   c.follower_count, c.baseline_median_views,
                   v.speech_onset_ms, v.wpm_0_to_35, v.spoken_hook,
                   v.velocity_views_per_day
            FROM videos v
            JOIN channels c ON v.channel_uuid = c.uuid
            WHERE v.url = ?1
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query_map(params![url], |row| {
            let channel = VideoChannelDetails {
                uuid: row.get(12)?,
                platform: row.get(13)?,
                handle: row.get(14)?,
                display_name: row.get(15)?,
                avatar_url: row.get(16)?,
                follower_count: row.get(17)?,
                baseline_median_views: row.get(18)?,
            };

            let vel: Option<f64> = row.get(22).ok();
            Ok(VideoDetail {
                uuid: row.get(0)?,
                url: row.get(1)?,
                caption: row.get(2)?,
                thumbnail: row.get(3)?,
                view_count: row.get(4)?,
                like_count: row.get(5)?,
                comment_count: row.get(6)?,
                share_count: row.get(7)?,
                outlier_score: row.get(8)?,
                engagement_rate: row.get(9)?,
                published_at: row.get(10)?,
                velocity_views_per_day: vel,
                channel,
                analysis: None,
                speech_onset_ms: row.get(19)?,
                wpm_0_to_35: row.get(20)?,
                spoken_hook: row.get(21)?,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn get_video_by_uuid(&self, uuid: &str) -> Result<Option<VideoDetail>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT v.uuid, v.url, v.caption, v.thumbnail, v.view_count, v.like_count,
                   v.comment_count, v.share_count, v.outlier_score, v.engagement_rate,
                   v.published_at, v.analyzed,
                   c.uuid, c.platform, c.handle, c.display_name, c.avatar_url,
                   c.follower_count, c.baseline_median_views,
                   v.speech_onset_ms, v.wpm_0_to_35, v.spoken_hook,
                   v.velocity_views_per_day
            FROM videos v
            JOIN channels c ON v.channel_uuid = c.uuid
            WHERE v.uuid = ?1
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query_map(params![uuid], |row| {
            let channel = VideoChannelDetails {
                uuid: row.get(12)?,
                platform: row.get(13)?,
                handle: row.get(14)?,
                display_name: row.get(15)?,
                avatar_url: row.get(16)?,
                follower_count: row.get(17)?,
                baseline_median_views: row.get(18)?,
            };

            let vel: Option<f64> = row.get(22).ok();
            Ok(VideoDetail {
                uuid: row.get(0)?,
                url: row.get(1)?,
                caption: row.get(2)?,
                thumbnail: row.get(3)?,
                view_count: row.get(4)?,
                like_count: row.get(5)?,
                comment_count: row.get(6)?,
                share_count: row.get(7)?,
                outlier_score: row.get(8)?,
                engagement_rate: row.get(9)?,
                published_at: row.get(10)?,
                velocity_views_per_day: vel,
                channel,
                analysis: None,
                speech_onset_ms: row.get(19)?,
                wpm_0_to_35: row.get(20)?,
                spoken_hook: row.get(21)?,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn get_latest_snapshot(&self, video_uuid: &str) -> Result<Option<VideoSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT uuid, video_uuid, view_count, like_count, comment_count, share_count, recorded_at
            FROM snapshots
            WHERE video_uuid = ?1
            ORDER BY recorded_at DESC, rowid DESC
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query_map(params![video_uuid], |row| {
            Ok(VideoSnapshot {
                uuid: row.get(0)?,
                video_uuid: row.get(1)?,
                view_count: row.get(2)?,
                like_count: row.get(3)?,
                comment_count: row.get(4)?,
                share_count: row.get(5)?,
                recorded_at: row.get(6)?,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Hooks & Hybrid Search
    // ─────────────────────────────────────────────────────────────────────────

    pub fn insert_hook(
        &self,
        uuid: &str,
        video_uuid: &str,
        hook_text: &str,
        madlib: Option<&str>,
        duration_seconds: f64,
        embedding: &[f32],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let embedding_json = serde_json::to_string(embedding).unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            r#"
            INSERT INTO hooks (uuid, video_uuid, hook_text, spoken_hook_madlib, duration_seconds, embedding_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(uuid) DO UPDATE SET
                hook_text = excluded.hook_text,
                spoken_hook_madlib = excluded.spoken_hook_madlib,
                duration_seconds = excluded.duration_seconds,
                embedding_json = excluded.embedding_json;
            "#,
            params![uuid, video_uuid, hook_text, madlib, duration_seconds, embedding_json],
        )?;
        Ok(())
    }

    /// Inserts a hook into the database, generating a 768-dim Gemini vector embedding asynchronously.
    pub async fn index_hook_with_embedding(
        &self,
        video_uuid: &str,
        hook_text: &str,
        madlib: Option<&str>,
        duration_seconds: f64,
        client: &crate::storage::gemini_client::GeminiEmbeddingClient,
    ) -> Result<String> {
        let hook_uuid = format!("hook_{:x}", crate::ingest::importer::md5_hash(hook_text));
        let embedding = client.embed_text(hook_text).await;
        self.insert_hook(&hook_uuid, video_uuid, hook_text, madlib, duration_seconds, &embedding)?;
        Ok(hook_uuid)
    }

    /// Synchronous version of hook indexing with embedding generation.
    pub fn index_hook_with_embedding_blocking(
        &self,
        video_uuid: &str,
        hook_text: &str,
        madlib: Option<&str>,
        duration_seconds: f64,
        client: &crate::storage::gemini_client::GeminiEmbeddingClient,
    ) -> Result<String> {
        let hook_uuid = format!("hook_{:x}", crate::ingest::importer::md5_hash(hook_text));
        let embedding = client.embed_text_blocking(hook_text);
        self.insert_hook(&hook_uuid, video_uuid, hook_text, madlib, duration_seconds, &embedding)?;
        Ok(hook_uuid)
    }

    /// Retrieves all hooks from the database for in-process vector and lexical matching
    pub fn get_all_hooks(&self) -> Result<Vec<StoredHook>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT uuid, video_uuid, hook_text, spoken_hook_madlib, duration_seconds, embedding_json
            FROM hooks
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let embedding_raw: String = row.get(5)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_raw).unwrap_or_default();

            Ok(StoredHook {
                uuid: row.get(0)?,
                video_uuid: row.get(1)?,
                hook_text: row.get(2)?,
                spoken_hook_madlib: row.get(3)?,
                duration_seconds: row.get(4)?,
                embedding,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn count_hooks(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM hooks")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Hybrid search combining cosine vector similarity and keyword lexical matching using RRF
    pub fn search_hooks_hybrid(
        &self,
        query_text: &str,
        query_embedding: Option<&[f32]>,
        limit: usize,
    ) -> Result<Vec<HybridHookResult>> {
        let hooks = self.get_all_hooks()?;
        if hooks.is_empty() {
            return Ok(Vec::new());
        }

        let query_lower = query_text.to_lowercase();
        let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

        if query_tokens.is_empty() && query_embedding.is_none() {
            // No filter specified: return stored hooks up to limit with attached video metadata
            let mut results = Vec::new();
            for hook in hooks.into_iter().take(limit) {
                let video_meta = self.get_video_by_uuid(&hook.video_uuid).ok().flatten();
                let (video_title, channel_handle, outlier_score) = match video_meta {
                    Some(v) => (v.caption, Some(v.channel.handle), Some(v.outlier_score)),
                    None => (None, None, None),
                };
                results.push(HybridHookResult {
                    hook_uuid: hook.uuid,
                    video_uuid: hook.video_uuid,
                    hook_text: hook.hook_text,
                    spoken_hook_madlib: hook.spoken_hook_madlib,
                    rrf_score: 1.0,
                    vector_similarity: None,
                    lexical_match: true,
                    video_title,
                    channel_handle,
                    outlier_score,
                });
            }
            return Ok(results);
        }

        // 1. Lexical candidate ranking (scored by query token overlaps)
        let mut lexical_scored: Vec<(&StoredHook, usize)> = hooks
            .iter()
            .filter_map(|h| {
                let text_lower = h.hook_text.to_lowercase();
                let matches = query_tokens
                    .iter()
                    .filter(|&&t| text_lower.contains(t))
                    .count();
                if matches > 0 {
                    Some((h, matches))
                } else {
                    None
                }
            })
            .collect();

        lexical_scored.sort_by(|a, b| b.1.cmp(&a.1));
        let lexical_ranked_ids: Vec<String> = lexical_scored
            .iter()
            .map(|(h, _)| h.uuid.clone())
            .collect();

        // 2. Vector candidate ranking (scored by cosine similarity)
        let mut vector_ranked_ids: Vec<String> = Vec::new();
        let mut sim_map: std::collections::HashMap<String, f32> = std::collections::HashMap::new();

        if let Some(q_vec) = query_embedding {
            let mut vector_scored: Vec<(&StoredHook, f32)> = hooks
                .iter()
                .filter(|h| !h.embedding.is_empty())
                .map(|h| {
                    let sim = cosine_similarity(q_vec, &h.embedding);
                    sim_map.insert(h.uuid.clone(), sim);
                    (h, sim)
                })
                .collect();

            vector_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            vector_ranked_ids = vector_scored.iter().map(|(h, _)| h.uuid.clone()).collect();
        }

        // 3. Reciprocal Rank Fusion
        let rrf_ranked: Vec<RrfRankedItem> = reciprocal_rank_fusion(
            &vector_ranked_ids,
            &lexical_ranked_ids,
            RRF_SMOOTHING_CONSTANT_K,
        );

        let hook_map: std::collections::HashMap<String, &StoredHook> =
            hooks.iter().map(|h| (h.uuid.clone(), h)).collect();

        let mut results = Vec::new();
        for item in rrf_ranked.into_iter().take(limit) {
            if let Some(&hook) = hook_map.get(&item.id) {
                let video_meta = self.get_video_by_uuid(&hook.video_uuid).ok().flatten();
                let (video_title, channel_handle, outlier_score) = match video_meta {
                    Some(v) => (v.caption, Some(v.channel.handle), Some(v.outlier_score)),
                    None => (None, None, None),
                };

                results.push(HybridHookResult {
                    hook_uuid: hook.uuid.clone(),
                    video_uuid: hook.video_uuid.clone(),
                    hook_text: hook.hook_text.clone(),
                    spoken_hook_madlib: hook.spoken_hook_madlib.clone(),
                    rrf_score: item.rrf_score,
                    vector_similarity: sim_map.get(&item.id).copied(),
                    lexical_match: item.lexical_rank.is_some(),
                    video_title,
                    channel_handle,
                    outlier_score,
                });
            }
        }

        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Tracked Hooks (Closing the Feedback Loop)
    // ─────────────────────────────────────────────────────────────────────────

    pub fn save_tracked_hook(&self, record: &TrackedHookRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO tracked_hooks (
                id, channel_handle, topic, hook_text, hook_type, source_formula,
                predicted_score, status, published_url, actual_views, actual_multiplier,
                actual_engagement_rate, performance_notes, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                published_url = excluded.published_url,
                actual_views = excluded.actual_views,
                actual_multiplier = excluded.actual_multiplier,
                actual_engagement_rate = excluded.actual_engagement_rate,
                performance_notes = excluded.performance_notes,
                updated_at = excluded.updated_at
            "#,
            params![
                record.id,
                record.channel_handle,
                record.topic,
                record.hook_text,
                record.hook_type,
                record.source_formula,
                record.predicted_score,
                record.status,
                record.published_url,
                record.actual_views,
                record.actual_multiplier,
                record.actual_engagement_rate,
                record.performance_notes,
                record.created_at,
                record.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn save_tracked_hooks(&self, records: &[TrackedHookRecord]) -> Result<()> {
        for record in records {
            self.save_tracked_hook(record)?;
        }
        Ok(())
    }

    pub fn list_tracked_hooks(
        &self,
        channel_filter: Option<&str>,
        status_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TrackedHookRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut query = "SELECT id, channel_handle, topic, hook_text, hook_type, source_formula, \
                         predicted_score, status, published_url, actual_views, actual_multiplier, \
                         actual_engagement_rate, performance_notes, created_at, updated_at \
                         FROM tracked_hooks WHERE 1=1".to_string();

        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(chan) = channel_filter {
            query.push_str(" AND channel_handle = ?");
            param_values.push(Box::new(chan.to_string()));
        }

        if let Some(stat) = status_filter {
            query.push_str(" AND status = ?");
            param_values.push(Box::new(stat.to_string()));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT ?");
        param_values.push(Box::new(limit as i64));

        let params_slice: Vec<&dyn rusqlite::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(params_slice.as_slice(), |row| {
            Ok(TrackedHookRecord {
                id: row.get(0)?,
                channel_handle: row.get(1)?,
                topic: row.get(2)?,
                hook_text: row.get(3)?,
                hook_type: row.get(4)?,
                source_formula: row.get(5)?,
                predicted_score: row.get(6)?,
                status: row.get(7)?,
                published_url: row.get(8)?,
                actual_views: row.get(9)?,
                actual_multiplier: row.get(10)?,
                actual_engagement_rate: row.get(11)?,
                performance_notes: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?;

        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        Ok(records)
    }

    pub fn update_tracked_hook_status(
        &self,
        id: &str,
        status: &str,
        published_url: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now_iso = "2026-03-01T12:00:00Z";
        let affected = conn.execute(
            r#"
            UPDATE tracked_hooks
            SET status = ?1,
                published_url = COALESCE(?2, published_url),
                updated_at = ?3
            WHERE id = ?4 OR hook_text LIKE ?5
            "#,
            params![status, published_url, now_iso, id, format!("%{}%", id)],
        )?;
        Ok(affected > 0)
    }

    pub fn record_hook_performance(
        &self,
        id: &str,
        views: u64,
        multiplier: f64,
        engagement_rate: f64,
        notes: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now_iso = "2026-03-01T12:00:00Z";
        let affected = conn.execute(
            r#"
            UPDATE tracked_hooks
            SET status = 'published',
                actual_views = ?1,
                actual_multiplier = ?2,
                actual_engagement_rate = ?3,
                performance_notes = ?4,
                updated_at = ?5
            WHERE id = ?6 OR hook_text LIKE ?7
            "#,
            params![
                views as i64,
                multiplier,
                engagement_rate,
                notes,
                now_iso,
                id,
                format!("%{}%", id)
            ],
        )?;
        Ok(affected > 0)
    }

    /// Retrieves proven winning formulas from previously published hooks that achieved >= min_multiplier.
    /// This closes the feedback loop by feeding empirical hits back into custom rubrics.
    pub fn get_validated_winning_formulas(
        &self,
        channel_handle: &str,
        min_multiplier: f64,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT source_formula
            FROM tracked_hooks
            WHERE channel_handle = ?1
              AND actual_multiplier >= ?2
              AND source_formula IS NOT NULL
            "#,
        )?;

        let rows = stmt.query_map(params![channel_handle, min_multiplier], |row| {
            row.get::<_, String>(0)
        })?;

        let formulas = rows.flatten().collect();
        Ok(formulas)
    }
}
