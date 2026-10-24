use comfy_table::{
    presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table,
};

use crate::math::heuristics::HeuristicsEngine;
use crate::math::outlier::{OutlierEngine, OutlierTier};
use crate::models::video::VideoDetail;
use crate::storage::db::HybridHookResult;

/// Visual terminal table formatter rendering rich UTF-8 tables with color-coded virality tiers.
pub struct TerminalTableRenderer;

impl TerminalTableRenderer {
    /// Formats an integer with standard thousands separators (e.g., 1000000 -> "1,000,000").
    pub fn format_number(n: u64) -> String {
        let s = n.to_string();
        let mut result = String::new();

        for (count, ch) in s.chars().rev().enumerate() {
            if count > 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(ch);
        }

        result.chars().rev().collect()
    }

    /// Evaluates video metrics and classifies into color-coded performance tiers:
    /// - 🟩 Green (>5x): Breakout Viral Outlier
    /// - 🟦 Blue (2x–5x): Statistical Winner Outlier
    /// - 🟨 Yellow (<2x): Baseline Reach
    /// - 🟥 Red: Likely Paid Ad (engagement rate < 2.0% on high views)
    pub fn classify_video_tier(video: &VideoDetail) -> (OutlierTier, bool, Color, &'static str) {
        let has_engagement_metrics =
            video.like_count.is_some() || video.comment_count.is_some() || video.share_count.is_some();

        let is_paid = if has_engagement_metrics {
            let likes = video.like_count.unwrap_or(0);
            let comments = video.comment_count.unwrap_or(0);
            let shares = video.share_count.unwrap_or(0);
            HeuristicsEngine::audit_engagement(video.view_count, likes, comments, shares, None)
                .is_likely_paid_or_boosted
        } else {
            false
        };

        if is_paid {
            return (OutlierTier::Baseline, true, Color::Red, "🟥 Paid Boost (<2%)");
        }

        let tier = OutlierEngine::classify_tier(video.outlier_score);
        match tier {
            OutlierTier::BreakoutViral => (tier, false, Color::Green, "🟩 Breakout (>5x)"),
            OutlierTier::WinnerOutlier => (tier, false, Color::Cyan, "🟦 Viral (2x-5x)"),
            OutlierTier::Baseline => (tier, false, Color::Yellow, "🟨 Baseline (1x-2x)"),
            OutlierTier::Underperformer => (tier, false, Color::Yellow, "🟨 Under (<1.0x)"),
        }
    }

    /// Renders the viral outlier watchlist table in UTF-8 with dynamic wrapping and colored tiers.
    pub fn render_watchlist_table(videos: &[VideoDetail]) -> String {
        if videos.is_empty() {
            return "No outlier videos found in watchlist matching current criteria.".to_string();
        }

        let mut table = Table::new();
        table.load_style(UTF8_FULL.with_rounded_corners());
        table.set_content_arrangement(ContentArrangement::Dynamic);

        // Header row with Bold White styling
        table.set_header(vec![
            Cell::new("#").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Status / Tier").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Multiplier").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Creator").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Views").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Baseline").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Eng. Rate").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Title / Caption").add_attribute(Attribute::Bold).fg(Color::White),
        ]);

        for (idx, video) in videos.iter().enumerate() {
            let (_tier, _is_paid, color, badge) = Self::classify_video_tier(video);

            let rank = format!("{}", idx + 1);
            let multiplier_str = format!("{:.1}x", video.outlier_score);
            let views_str = Self::format_number(video.view_count);
            let baseline_str = video
                .channel
                .baseline_median_views
                .map(Self::format_number)
                .unwrap_or_else(|| "N/A".to_string());
            let eng_str = format!("{:.2}%", video.engagement_rate);
            let title_str = video.caption.as_deref().unwrap_or("Untitled Short");

            table.add_row(vec![
                Cell::new(rank).fg(Color::DarkGrey),
                Cell::new(badge).fg(color).add_attribute(Attribute::Bold),
                Cell::new(multiplier_str).fg(color).add_attribute(Attribute::Bold),
                Cell::new(&video.channel.handle).fg(Color::White),
                Cell::new(views_str).fg(Color::White),
                Cell::new(baseline_str).fg(Color::DarkGrey),
                Cell::new(eng_str).fg(if video.engagement_rate >= 2.0 { Color::Green } else { Color::Red }),
                Cell::new(title_str),
            ]);
        }

        table.to_string()
    }

    /// Renders batch ingestion results into clean structured UTF-8 tables.
    pub fn render_import_tables(
        channels_updated: usize,
        videos_imported: usize,
        snapshots_recorded: usize,
        baselines: &[(String, u64)],
        top_outliers: &[VideoDetail],
    ) -> String {
        let mut output = String::new();

        // 1. Ingestion Overview Table
        let mut summary_table = Table::new();
        summary_table.load_style(UTF8_FULL.with_rounded_corners());
        summary_table.set_header(vec![
            Cell::new("Metric").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Value").add_attribute(Attribute::Bold).fg(Color::Cyan),
        ]);
        summary_table.add_row(vec![
            Cell::new("Channels Ingested / Updated"),
            Cell::new(channels_updated.to_string()).fg(Color::Green),
        ]);
        summary_table.add_row(vec![
            Cell::new("Historical Videos Ingested"),
            Cell::new(videos_imported.to_string()).fg(Color::Green),
        ]);
        summary_table.add_row(vec![
            Cell::new("Performance Snapshots Recorded"),
            Cell::new(snapshots_recorded.to_string()).fg(Color::Green),
        ]);

        output.push_str("📥 Batch Ingestion Overview:\n");
        output.push_str(&summary_table.to_string());
        output.push_str("\n\n");

        // 2. Creator Baselines Table
        if !baselines.is_empty() {
            let mut baseline_table = Table::new();
            baseline_table.load_style(UTF8_FULL.with_rounded_corners());
            baseline_table.set_header(vec![
                Cell::new("Creator Channel").add_attribute(Attribute::Bold).fg(Color::White),
                Cell::new("Median Baseline (Bc)").add_attribute(Attribute::Bold).fg(Color::White),
                Cell::new("Algorithm Standard").add_attribute(Attribute::Bold).fg(Color::White),
            ]);

            for (handle, baseline) in baselines {
                baseline_table.add_row(vec![
                    Cell::new(handle).fg(Color::White).add_attribute(Attribute::Bold),
                    Cell::new(format!("{} views", Self::format_number(*baseline))).fg(Color::Cyan),
                    Cell::new("Rolling Median + IQR Winsorization").fg(Color::DarkGrey),
                ]);
            }

            output.push_str("📊 Creator Algorithmic Baselines:\n");
            output.push_str(&baseline_table.to_string());
            output.push_str("\n\n");
        }

        // 3. Top Outliers Table
        if !top_outliers.is_empty() {
            output.push_str("🔥 Detected Top Outlier Videos (>= 2.0x Multiplier):\n");
            output.push_str(&Self::render_watchlist_table(top_outliers));
            output.push('\n');
        }

        output
    }

    /// Renders semantic hook retrieval results into a structured UTF-8 table.
    pub fn render_hooks_table(results: &[HybridHookResult]) -> String {
        if results.is_empty() {
            return "No matching hooks found.".to_string();
        }

        let mut table = Table::new();
        table.load_style(UTF8_FULL.with_rounded_corners());
        table.set_content_arrangement(ContentArrangement::Dynamic);

        table.set_header(vec![
            Cell::new("#").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Resonance").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Relevance").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Spoken Hook (0.0s-3.5s)").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Fill-in-the-Blank Madlib").add_attribute(Attribute::Bold).fg(Color::White),
            Cell::new("Source Creator").add_attribute(Attribute::Bold).fg(Color::White),
        ]);

        for (idx, r) in results.iter().enumerate() {
            let mult = r.outlier_score.unwrap_or(1.0);
            let color = if mult >= 5.0 {
                Color::Green
            } else if mult >= 2.0 {
                Color::Cyan
            } else {
                Color::Yellow
            };

            let rank = format!("{}", idx + 1);
            let mult_str = format!("{:.1}x", mult);
            let rel_str = if let Some(sim) = r.vector_similarity {
                format!("{:.1}% Match", (sim * 100.0).clamp(0.0, 100.0))
            } else {
                "Keyword".to_string()
            };
            let madlib_str = r.spoken_hook_madlib.as_deref().unwrap_or("Custom angle");
            let creator_str = r.channel_handle.as_deref().unwrap_or("@unknown");

            table.add_row(vec![
                Cell::new(rank).fg(Color::DarkGrey),
                Cell::new(mult_str).fg(color).add_attribute(Attribute::Bold),
                Cell::new(rel_str).fg(Color::Cyan),
                Cell::new(&r.hook_text).fg(Color::White),
                Cell::new(madlib_str).fg(Color::Yellow),
                Cell::new(creator_str).fg(Color::DarkGrey),
            ]);
        }

        table.to_string()
    }
}
