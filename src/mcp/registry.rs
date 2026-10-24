use crate::models::McpToolDefinition;
use serde_json::json;

pub struct McpToolRegistry;

impl McpToolRegistry {
    /// Returns the complete catalog of all 9 intelligence tools conforming to MCP 2024-11-05
    pub fn list_tools() -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "/analyze".to_string(),
                description: "Deep-analyzes a specific short-form or long-form video URL. Deconstructs 0-3.5s acoustic delivery, audience retention curve dips/spikes, visual Average Shot Length (ASL), and narrative formulas.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "video_url": {
                            "type": "string",
                            "format": "uri",
                            "description": "Fully qualified URL of the video to analyze (YouTube, Shorts, TikTok)."
                        },
                        "pacing_sample_sec": {
                            "type": "number",
                            "default": 60.0,
                            "description": "Duration in seconds to sample for visual rhythm and Average Shot Length detection."
                        },
                        "include_retention": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to extract and analyze the YouTube audience retention curve & dip bottlenecks."
                        },
                        "include_pacing": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to run low-bandwidth in-memory FFmpeg scene detection for visual rhythm."
                        }
                    },
                    "required": ["video_url"]
                }),
            },
            McpToolDefinition {
                name: "/videos-watchlist".to_string(),
                description: "Retrieves top outlier videos from tracked channels ranked by Outlier Multiplier (views / creator baseline) rather than raw views, isolating true virality.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "niche_filter": {
                            "type": "string",
                            "description": "Optional keyword or thematic niche filter."
                        },
                        "timeframe": {
                            "type": "string",
                            "enum": ["7d", "14d", "30d", "all"],
                            "default": "30d",
                            "description": "Lookback window for video discovery."
                        },
                        "min_multiplier": {
                            "type": "number",
                            "default": 2.0,
                            "description": "Minimum outlier multiplier threshold (default 2.0x)."
                        },
                        "limit": {
                            "type": "integer",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 50,
                            "description": "Maximum number of outlier videos to return."
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "/hooks-watchlist".to_string(),
                description: "Extracts and clusters winning opening hooks (0.0s-3.5s) and fill-in-the-blank madlibs using hybrid Gemini vector and keyword search.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural language concept, genre, trope, or keyword query for hybrid semantic search."
                        },
                        "topic": {
                            "type": "string",
                            "description": "Alternative topic or theme identifier."
                        },
                        "niche_filter": {
                            "type": "string",
                            "description": "Thematic or keyword filter."
                        },
                        "min_multiplier": {
                            "type": "number",
                            "default": 2.0,
                            "description": "Outlier threshold of source videos."
                        },
                        "limit": {
                            "type": "integer",
                            "default": 10,
                            "description": "Number of hook patterns to return."
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "/formats-watchlist".to_string(),
                description: "Surfaces visual and narrative format archetypes, Average Shot Length (ASL) benchmarks, and empirical archetype lifts winning across tracked channels.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "channel": {
                            "type": "string",
                            "description": "Optional creator handle (e.g., @Vanwives) to analyze specific cohort format dynamics."
                        },
                        "platform": {
                            "type": "string",
                            "enum": ["tiktok", "instagram", "youtube"],
                            "description": "Filter by target social media platform."
                        },
                        "limit": {
                            "type": "integer",
                            "default": 5,
                            "description": "Maximum number of format archetypes to return."
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "/topic".to_string(),
                description: "Tracks real-time semantic topic momentum, TF-IDF n-gram clusters, and breakout micro-trends across recent video cohorts before broad market saturation.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "topic": {
                            "type": "string",
                            "description": "Target topic or niche query to evaluate (e.g., 'Dark Romance Books', 'Aluminum Boat Build')."
                        },
                        "topic_query": {
                            "type": "string",
                            "description": "Alias for topic parameter."
                        },
                        "days": {
                            "type": "integer",
                            "default": 7,
                            "description": "Lookback window in days for recent video cohort."
                        },
                        "limit": {
                            "type": "integer",
                            "default": 25,
                            "description": "Maximum number of recent videos to analyze."
                        }
                    }
                }),
            },
            McpToolDefinition {
                name: "/channels-add".to_string(),
                description: "Registers creator URLs or handles to the ongoing monitoring watchlist for baseline computation.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "channel_urls": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of TikTok, Instagram, or YouTube channel URLs."
                        }
                    },
                    "required": ["channel_urls"]
                }),
            },
            McpToolDefinition {
                name: "/channels-recap".to_string(),
                description: "Summarizes recent posting velocity, outlier count, and content strategy shifts for a specific creator.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "handle": {
                            "type": "string",
                            "description": "Creator handle (e.g. @creator)."
                        },
                        "lookback_days": {
                            "type": "integer",
                            "default": 30,
                            "description": "Number of historical days to inspect."
                        }
                    },
                    "required": ["handle"]
                }),
            },
            McpToolDefinition {
                name: "/channels-search".to_string(),
                description: "Executes semantic hybrid search across all channels and creators currently monitored.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search term, niche, or topic description."
                        },
                        "limit": {
                            "type": "integer",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }),
            },
            McpToolDefinition {
                name: "/video-suggest".to_string(),
                description: "Synthesizes winning hook formulas, cognitive friction angles, and 2.4s ASL shot pacing into a complete, mathematically verified short-form predictive script with teleprompter markdown.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "topic": {
                            "type": "string",
                            "description": "Subject matter or title concept of the short-form video."
                        },
                        "channel_handle": {
                            "type": "string",
                            "description": "Creator handle to match historical style and baseline (e.g. '@creator_handle')."
                        },
                        "target_duration_sec": {
                            "type": "integer",
                            "default": 36,
                            "description": "Target video duration in seconds (e.g. 36 or 48)."
                        }
                    },
                    "required": ["topic"]
                }),
            },
            McpToolDefinition {
                name: "current_org_usage".to_string(),
                description: "Returns organization credit balance, analysis quotas, and reset schedule.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpToolDefinition {
                name: "/hook-machine".to_string(),
                description: "Executes The Hook Machine end-to-end workflow for ANY YouTube creator niche: dynamically generates specialized niche trigger lexicons (irreversible loss, time bombs, threatened assets) via Gemini, builds a custom rubric, and generates 5 adapted + 5 universal scratch ranked hooks with Triple-Vector scoring.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "channel_handle": {
                            "type": "string",
                            "description": "Creator channel handle (e.g. '@GrahamStephan', '@JeffNippard', '@wildhomestead', '@kallaway')."
                        },
                        "topic": {
                            "type": "string",
                            "description": "Target topic or niche concept to generate viral hooks for (e.g. 'Real Estate Foreclosures', 'Hypertrophy Deloads', 'Off-Grid Winter Well')."
                        }
                    },
                    "required": ["channel_handle", "topic"]
                }),
            },
            McpToolDefinition {
                name: "/hook-feedback".to_string(),
                description: "Closes the feedback loop: track generated hooks, mark them as 'selected', 'filmed', or 'published', and record actual real-world view counts and outlier multipliers so the system learns what works.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["list", "select", "film", "publish", "record_performance"],
                            "description": "Feedback action to take: list tracked hooks, update status, or record real-world performance."
                        },
                        "hook_id": {
                            "type": "string",
                            "description": "ID or substring of the hook text being updated."
                        },
                        "channel_handle": {
                            "type": "string",
                            "description": "Filter by channel handle when listing hooks."
                        },
                        "status": {
                            "type": "string",
                            "enum": ["generated", "selected", "filmed", "published"],
                            "description": "Target status when updating."
                        },
                        "published_url": {
                            "type": "string",
                            "description": "URL of the published video."
                        },
                        "actual_views": {
                            "type": "integer",
                            "description": "Empirical views achieved by the published video."
                        },
                        "actual_multiplier": {
                            "type": "number",
                            "description": "Empirical outlier multiplier achieved vs channel baseline."
                        },
                        "actual_engagement_rate": {
                            "type": "number",
                            "description": "Empirical engagement rate percentage (likes+comments+shares)/views."
                        },
                        "notes": {
                            "type": "string",
                            "description": "Creator notes on what worked or why it resonated."
                        }
                    },
                    "required": ["action"]
                }),
            },
            McpToolDefinition {
                name: "/marginal-gains".to_string(),
                description: "Synthesizes Sir Dave Brailsford's 6-Point Marginal Gains Telemetry Scorecard (Acoustic Cold-Open, 60s Visual Rhythm, Anti-Stagnation Hold, Mid-Roll Retention Cliff, Timeline Audio Density, and Physical Stakes Lexicon). Computes compounded multiplicative lift and the Top 3 highest-ROI micro-tweaks.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "video_url": {
                            "type": "string",
                            "format": "uri",
                            "description": "Video URL to analyze (YouTube, Shorts, TikTok)."
                        }
                    },
                    "required": ["video_url"]
                }),
            },
        ]
    }
}

