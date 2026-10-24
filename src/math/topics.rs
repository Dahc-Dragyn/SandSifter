use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

/// Classification of a topic or keyword entity based on velocity lift and saturation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendClassification {
    /// Velocity lift >= 2.0x, present across >= 3 distinct channels (or >=2 if small cohort)
    BreakoutMicroTrend,
    /// High frequency (>30% of videos), but declining velocity (<0.8x)
    SaturatedFading,
    /// Velocity lift >= 1.3x, present across >= 2 channels
    EstablishedWinner,
    /// Velocity lift between 0.8x and 1.3x
    BaselineVolume,
    /// Velocity lift < 0.8x with low/moderate frequency
    LowPerformance,
}

impl TrendClassification {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::BreakoutMicroTrend => "🔥 BREAKOUT MICRO-TREND",
            Self::SaturatedFading => "⚠️ SATURATED / FADING",
            Self::EstablishedWinner => "🚀 ESTABLISHED WINNER",
            Self::BaselineVolume => "⚖️ BASELINE",
            Self::LowPerformance => "📉 LOW VELOCITY",
        }
    }
}

/// Input video metadata for topic momentum analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicVideoInput {
    pub title: String,
    pub description: String,
    pub channel: String,
    pub video_url: String,
    pub views: u64,
    pub days_since_upload: f64,
    pub velocity: f64, // views per day
}

impl TopicVideoInput {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        channel: impl Into<String>,
        video_url: impl Into<String>,
        views: u64,
        days_since_upload: f64,
    ) -> Self {
        let days = days_since_upload.max(0.1);
        let velocity = (views as f64 / days * 10.0).round() / 10.0;
        Self {
            title: title.into(),
            description: description.into(),
            channel: channel.into(),
            video_url: video_url.into(),
            views,
            days_since_upload: days,
            velocity,
        }
    }
}

/// Evaluated n-gram / keyword entity trend within the cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicEntityTrend {
    /// Extracted phrase or keyword
    pub entity: String,
    /// N-gram type: "Unigram", "Bigram", or "Trigram"
    pub entity_type: String,
    /// Number of videos in the cohort featuring this entity
    pub frequency: usize,
    /// Total videos analyzed in cohort
    pub corpus_size: usize,
    /// Percentage of videos in cohort containing this entity
    pub frequency_pct: f64,
    /// Number of distinct channels using this entity
    pub channel_dispersion: usize,
    /// Mean views/day of videos containing this entity
    pub mean_velocity: f64,
    /// Overall cohort mean views/day
    pub cohort_mean_velocity: f64,
    /// Velocity lift multiplier (mean_velocity / cohort_mean_velocity)
    pub velocity_lift: f64,
    /// Aggregate TF-IDF importance score
    pub tf_idf_score: f64,
    /// Classification (Breakout vs Saturated vs Winner)
    pub classification: TrendClassification,
    /// Top example video titles featuring this entity
    pub sample_titles: Vec<String>,
    /// Actionable tactical advice for creators
    pub tactical_advice: String,
}

/// Jake Trinder's Bounded Rationality & Demand Stacking statistics for a topic cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicDemandStats {
    /// Total cohort sample size of videos with views
    pub sample_size: usize,
    /// Topic Floor (Q1 / Lower Quartile): Baseline expected view count for an average/mediocre video
    pub topic_floor_q1: u64,
    /// Median Baseline (B_c): Rolling median view count across the topic cohort
    pub median_baseline: u64,
    /// Third Quartile (Q3 / Upper Quartile)
    pub q3: u64,
    /// Interquartile Range (IQR = Q3 - Q1)
    pub iqr: f64,
    /// Topic Ceiling (Q3 + 1.5 * IQR): The Winsorized viral breakout ceiling
    pub topic_ceiling: u64,
    /// Upside Multiplier: Ratio of Topic Ceiling to Topic Floor
    pub upside_multiplier: f64,
}

impl TopicDemandStats {
    /// Formats an integer with comma thousands separators.
    pub fn format_views(val: u64) -> String {
        let s = val.to_string();
        let mut res = String::new();
        let len = s.len();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                res.push(',');
            }
            res.push(ch);
        }
        res
    }

    /// Renders creator-friendly topic demand bounds without statistical math jargon.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("### 🎯 Topic Demand Bounds (Creator View Benchmarks)\n\n");
        out.push_str("| Metric | View Count | Creator Meaning |\n");
        out.push_str("|:---|:---:|:---|\n");
        out.push_str(&format!("| **Guaranteed Baseline Views (Typical Low)** | **{}** | Minimum expected views for standard execution |\n", Self::format_views(self.topic_floor_q1)));
        out.push_str(&format!("| **Normal Channel Average** | **{}** | Middle-of-the-road typical cohort performance |\n", Self::format_views(self.median_baseline)));
        out.push_str(&format!("| **Normal Top Range** | **{}** | 75th percentile ceiling before breakout multiplier |\n", Self::format_views(self.q3)));
        out.push_str(&format!("| **View Consistency Spread** | **{}** | Standard variance between low and high videos |\n", Self::format_views(self.iqr as u64)));
        out.push_str(&format!("| **Viral Breakout Target** | **{}** | Expected ceiling when title packaging & thumbnail align |\n", Self::format_views(self.topic_ceiling)));
        out.push_str(&format!("| **Breakout Upside Potential** | **{:.1}x** | Upside multiplier from baseline to viral ceiling (Sample: {} videos) |\n", self.upside_multiplier, self.sample_size));
        out
    }
}

/// Actionable demand stacking suggestion to broaden addressable audience.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemandStackingSuggestion {
    /// The scale anchor, authority entity, or high-stakes modifier (e.g. "110k Gallon", "Cheap Excavator")
    pub modifier: String,
    /// Category of demand stacking: "ScaleAnchor", "AuthorityEntity", "BudgetFriction", or "TransformationExtreme"
    pub category: String,
    /// Example combined title packaging
    pub suggested_title: String,
    /// Strategic rationale explaining why this expands the addressable audience
    pub rationale: String,
}

/// Complete topic momentum report across a recent video cohort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicTrendReport {
    pub topic: String,
    pub days_window: u32,
    pub total_videos_analyzed: usize,
    pub distinct_channels_count: usize,
    pub cohort_mean_velocity: f64,
    pub breakout_trends: Vec<TopicEntityTrend>,
    pub saturated_trends: Vec<TopicEntityTrend>,
    pub established_trends: Vec<TopicEntityTrend>,
    pub all_entities: Vec<TopicEntityTrend>,
    pub creator_takeaway: String,
    #[serde(default)]
    pub demand_stats: Option<TopicDemandStats>,
    #[serde(default)]
    pub demand_stacking_suggestions: Vec<DemandStackingSuggestion>,
}

pub struct TopicMomentumEngine;

impl TopicMomentumEngine {
    /// Common English stopwords to discard from unigrams and n-gram boundaries.
    const STOPWORDS: &'static [&'static str] = &[
        "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
        "aren't", "as", "at", "be", "because", "been", "before", "being", "below", "between",
        "both", "but", "by", "can", "can't", "cannot", "could", "couldn't", "did", "didn't",
        "do", "does", "doesn't", "doing", "don't", "down", "during", "each", "few", "for",
        "from", "further", "had", "hadn't", "has", "hasn't", "have", "haven't", "having", "he",
        "he'd", "he'll", "he's", "her", "here", "here's", "hers", "herself", "him", "himself",
        "his", "how", "how's", "i", "i'd", "i'll", "i'm", "i've", "if", "in", "into", "is",
        "isn't", "it", "it's", "its", "itself", "let's", "me", "more", "most", "mustn't", "my",
        "myself", "no", "nor", "not", "of", "off", "on", "once", "only", "or", "other", "ought",
        "our", "ours", "ourselves", "out", "over", "own", "same", "shan't", "she", "she'd",
        "she'll", "she's", "should", "shouldn't", "so", "some", "such", "than", "that", "that's",
        "the", "their", "theirs", "them", "themselves", "then", "there", "there's", "these",
        "they", "they'd", "they'll", "they're", "they've", "this", "those", "through", "to",
        "too", "under", "until", "up", "very", "was", "wasn't", "we", "we'd", "we'll", "we're",
        "we've", "were", "weren't", "what", "what's", "when", "when's", "where", "where's",
        "which", "while", "who", "who's", "whom", "why", "why's", "with", "won't", "would",
        "wouldn't", "you", "you'd", "you'll", "you're", "you've", "your", "yours", "yourself",
        "yourselves", "video", "youtube", "shorts", "short", "vlog", "ep", "part", "pt", "new",
    ];

    /// Normalizes and cleans raw text into lowercase alphanumeric tokens.
    pub fn tokenize(text: &str) -> Vec<String> {
        let cleaned: String = text
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect();

        cleaned
            .split_whitespace()
            .filter(|t| t.len() >= 2)
            .map(|t| t.to_string())
            .collect()
    }

    /// Checks if a token is an English stopword.
    pub fn is_stopword(token: &str) -> bool {
        Self::STOPWORDS.contains(&token)
    }

    /// Extracts unigrams, bigrams, and trigrams from a token list.
    pub fn extract_ngrams(tokens: &[String]) -> HashSet<(String, String)> {
        let mut ngrams = HashSet::new();

        // 1. Unigrams (length >= 3, non-stopwords)
        for token in tokens {
            if token.len() >= 3 && !Self::is_stopword(token) {
                ngrams.insert((token.clone(), "Unigram".to_string()));
            }
        }

        // 2. Bigrams (at least one non-stopword, boundary cannot be both stopwords)
        for window in tokens.windows(2) {
            let t1 = &window[0];
            let t2 = &window[1];
            if (!Self::is_stopword(t1) || !Self::is_stopword(t2)) && t1.len() >= 2 && t2.len() >= 2 {
                let phrase = format!("{} {}", t1, t2);
                if phrase.len() >= 5 {
                    ngrams.insert((phrase, "Bigram".to_string()));
                }
            }
        }

        // 3. Trigrams (at least two non-stopwords)
        for window in tokens.windows(3) {
            let t1 = &window[0];
            let t2 = &window[1];
            let t3 = &window[2];
            let non_stop_count = (!Self::is_stopword(t1) as usize)
                + (!Self::is_stopword(t2) as usize)
                + (!Self::is_stopword(t3) as usize);
            if non_stop_count >= 2 {
                let phrase = format!("{} {} {}", t1, t2, t3);
                if phrase.len() >= 8 {
                    ngrams.insert((phrase, "Trigram".to_string()));
                }
            }
        }

        ngrams
    }

    /// Computes TF-IDF and momentum metrics for a cohort of videos.
    pub fn analyze_cohort_trends(
        topic: &str,
        days_window: u32,
        videos: &[TopicVideoInput],
    ) -> TopicTrendReport {
        if videos.is_empty() {
            return TopicTrendReport {
                topic: topic.to_string(),
                days_window,
                total_videos_analyzed: 0,
                distinct_channels_count: 0,
                cohort_mean_velocity: 0.0,
                breakout_trends: Vec::new(),
                saturated_trends: Vec::new(),
                established_trends: Vec::new(),
                all_entities: Vec::new(),
                creator_takeaway: "No recent videos found matching topic cohort.".to_string(),
                demand_stats: None,
                demand_stacking_suggestions: Vec::new(),
            };
        }

        let n = videos.len() as f64;
        let total_velocity: f64 = videos.iter().map(|v| v.velocity).sum();
        let cohort_mean_velocity = (total_velocity / n * 10.0).round() / 10.0;

        let distinct_channels_count = videos
            .iter()
            .map(|v| v.channel.clone())
            .collect::<HashSet<_>>()
            .len();

        // 1. Extract n-grams per document
        let doc_ngrams: Vec<HashSet<(String, String)>> = videos
            .iter()
            .map(|v| {
                let full_text = format!("{} {}", v.title, v.description);
                let tokens = Self::tokenize(&full_text);
                Self::extract_ngrams(&tokens)
            })
            .collect();

        // 2. Count document frequency (DF) per n-gram
        let mut df_map: HashMap<(String, String), usize> = HashMap::new();
        for ngrams in &doc_ngrams {
            for ngram in ngrams {
                *df_map.entry(ngram.clone()).or_insert(0) += 1;
            }
        }

        // 3. For each n-gram appearing in >= 2 videos (or >=1 if small cohort), calculate TF-IDF and momentum
        let min_frequency = if videos.len() <= 5 { 1 } else { 2 };
        let mut entity_trends = Vec::new();

        for ((entity, entity_type), freq) in df_map {
            if freq < min_frequency {
                continue;
            }

            // Matching videos
            let mut matching_videos = Vec::new();
            let mut matching_channels = HashSet::new();
            let mut tf_sum = 0.0;

            for (idx, ngrams) in doc_ngrams.iter().enumerate() {
                if ngrams.contains(&(entity.clone(), entity_type.clone())) {
                    let v = &videos[idx];
                    matching_videos.push(v);
                    matching_channels.insert(v.channel.clone());

                    // Term frequency in this video's title/description
                    let tokens = Self::tokenize(&format!("{} {}", v.title, v.description));
                    let term_count = if entity_type == "Unigram" {
                        tokens.iter().filter(|&t| t == &entity).count()
                    } else {
                        1
                    };
                    let tf = term_count as f64 / tokens.len().max(1) as f64;
                    tf_sum += tf;
                }
            }

            let dispersion = matching_channels.len();
            let idf = ((1.0 + n) / (1.0 + freq as f64)).ln() + 1.0;
            let tf_idf_score = ((tf_sum / freq as f64) * idf * 1000.0).round() / 10.0;

            let mean_entity_vel: f64 = matching_videos.iter().map(|v| v.velocity).sum::<f64>()
                / matching_videos.len() as f64;
            let mean_entity_vel_rounded = (mean_entity_vel * 10.0).round() / 10.0;

            let velocity_lift = if cohort_mean_velocity > 0.0 {
                ((mean_entity_vel / cohort_mean_velocity) * 100.0).round() / 100.0
            } else {
                1.0
            };

            let freq_pct = ((freq as f64 / n) * 1000.0).round() / 10.0;

            // Trend Classification
            let classification = if velocity_lift >= 2.0 && dispersion >= 3.min(distinct_channels_count) {
                TrendClassification::BreakoutMicroTrend
            } else if freq_pct > 30.0 && velocity_lift < 0.8 {
                TrendClassification::SaturatedFading
            } else if velocity_lift >= 1.3 && dispersion >= 2.min(distinct_channels_count) {
                TrendClassification::EstablishedWinner
            } else if velocity_lift >= 0.8 {
                TrendClassification::BaselineVolume
            } else {
                TrendClassification::LowPerformance
            };

            // Sample top titles
            let mut sorted_matching = matching_videos.clone();
            sorted_matching.sort_by(|a, b| b.views.cmp(&a.views));
            let sample_titles = sorted_matching
                .into_iter()
                .take(3)
                .map(|v| v.title.clone())
                .collect();

            // Tactical advice
            let tactical_advice = match classification {
                TrendClassification::BreakoutMicroTrend => format!(
                    "🔥 EXPLOSIVE TREND: Surge of {:+.0}% velocity across {} independent creators. Package your next video around this exact angle immediately before it saturates.",
                    (velocity_lift - 1.0) * 100.0,
                    dispersion
                ),
                TrendClassification::SaturatedFading => format!(
                    "⚠️ AVOID / PIVOT: Used in {:.0}% of videos but lagging at {:.2}x baseline velocity. Audience fatigue is setting in; find an untapped sub-trope.",
                    freq_pct,
                    velocity_lift
                ),
                TrendClassification::EstablishedWinner => format!(
                    "🚀 HIGH DEMAND: Solid {:+.0}% velocity lift across {} creators. Proven evergreen pillar concept.",
                    (velocity_lift - 1.0) * 100.0,
                    dispersion
                ),
                TrendClassification::BaselineVolume => format!(
                    "⚖️ STABLE: Performing at cohort baseline ({:.2}x). Useful secondary theme.",
                    velocity_lift
                ),
                TrendClassification::LowPerformance => format!(
                    "📉 UNDERPERFORMING: Generating {:.2}x baseline views. Deprioritize.",
                    velocity_lift
                ),
            };

            entity_trends.push(TopicEntityTrend {
                entity,
                entity_type,
                frequency: freq,
                corpus_size: videos.len(),
                frequency_pct: freq_pct,
                channel_dispersion: dispersion,
                mean_velocity: mean_entity_vel_rounded,
                cohort_mean_velocity,
                velocity_lift,
                tf_idf_score,
                classification,
                sample_titles,
                tactical_advice,
            });
        }

        // Sort by velocity lift descending, then frequency descending
        entity_trends.sort_by(|a, b| {
            b.velocity_lift
                .partial_cmp(&a.velocity_lift)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.frequency.cmp(&a.frequency))
        });

        let mut breakout_trends = Vec::new();
        let mut saturated_trends = Vec::new();
        let mut established_trends = Vec::new();

        for t in &entity_trends {
            match t.classification {
                TrendClassification::BreakoutMicroTrend => breakout_trends.push(t.clone()),
                TrendClassification::SaturatedFading => saturated_trends.push(t.clone()),
                TrendClassification::EstablishedWinner => established_trends.push(t.clone()),
                _ => {}
            }
        }

        // Synthesize executive takeaway for creators
        let creator_takeaway = if let Some(top_breakout) = breakout_trends.first() {
            format!(
                "Winning Opportunity: The '{}' sub-angle is currently breaking out with a {:+.0}% velocity lift across {} channels. Pivot title and hook packaging toward this concept immediately.",
                top_breakout.entity,
                (top_breakout.velocity_lift - 1.0) * 100.0,
                top_breakout.channel_dispersion
            )
        } else if let Some(top_established) = established_trends.first() {
            format!(
                "Solid Demand: '{}' maintains strong performance with {:+.0}% above cohort baseline views/day across {} channels.",
                top_established.entity,
                (top_established.velocity_lift - 1.0) * 100.0,
                top_established.channel_dispersion
            )
        } else {
            "Broad topic distribution. Focus on strong narrative cold opens and visual pacing rather than specific sub-tropes.".to_string()
        };

        // Compute Topic Floor (Q1), Median Baseline (B_c), and Winsorized Viral Ceiling (Q3 + 1.5 * IQR)
        let views: Vec<u64> = videos.iter().map(|v| v.views).collect();
        let demand_stats = crate::math::baseline::BaselineEngine::compute_baseline_diagnostics(&views).map(|diag| {
            let floor = diag.q1.round() as u64;
            let median = diag.raw_median;
            let q3 = diag.q3.round() as u64;
            let ceiling = if diag.upper_clamp_bound.is_finite() {
                diag.upper_clamp_bound.round() as u64
            } else {
                q3
            };
            let upside_multiplier = if floor > 0 {
                ((ceiling as f64 / floor as f64) * 10.0).round() / 10.0
            } else {
                1.0
            };
            TopicDemandStats {
                sample_size: diag.sample_size,
                topic_floor_q1: floor,
                median_baseline: median,
                q3,
                iqr: diag.iqr,
                topic_ceiling: ceiling,
                upside_multiplier,
            }
        });

        let demand_stacking_suggestions = Self::generate_demand_stacking_suggestions(
            topic,
            videos,
            &breakout_trends,
            &established_trends,
        );

        TopicTrendReport {
            topic: topic.to_string(),
            days_window,
            total_videos_analyzed: videos.len(),
            distinct_channels_count,
            cohort_mean_velocity,
            breakout_trends,
            saturated_trends,
            established_trends,
            all_entities: entity_trends,
            creator_takeaway,
            demand_stats,
            demand_stacking_suggestions,
        }
    }

    /// Synthesizes high-leverage demand stacking recommendations by attaching
    /// scale anchors, authority entities, or physical constraints to the core topic.
    pub fn generate_demand_stacking_suggestions(
        topic: &str,
        videos: &[TopicVideoInput],
        breakout_trends: &[TopicEntityTrend],
        established_trends: &[TopicEntityTrend],
    ) -> Vec<DemandStackingSuggestion> {
        let mut suggestions = Vec::new();
        let topic_clean = topic.trim();

        // 1. Check if cohort contains explicit scale tokens in top video titles
        let mut extracted_scale: Option<String> = None;
        let mut sorted_videos = videos.to_vec();
        sorted_videos.sort_by(|a, b| b.views.cmp(&a.views));

        for v in sorted_videos.iter().take(5) {
            let lower = v.title.to_lowercase();
            for word in lower.split_whitespace() {
                if (word.ends_with('k') || word.ends_with("gal") || word.ends_with("gallon") || word.ends_with("lbs") || word.ends_with("ft") || word.ends_with("hr") || word.ends_with("day") || word.ends_with("days"))
                    && word.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                {
                    extracted_scale = Some(word.to_uppercase());
                    break;
                }
            }
            if extracted_scale.is_some() {
                break;
            }
        }

        let topic_lower = topic_clean.to_lowercase();

        // High-leverage category rules based on topic domain
        if topic_lower.contains("pond") || topic_lower.contains("cabin") || topic_lower.contains("build") || topic_lower.contains("homestead") || topic_lower.contains("diy") || topic_lower.contains("tiny") {
            let scale_val = extracted_scale.unwrap_or_else(|| "110k Gallon".to_string());
            suggestions.push(DemandStackingSuggestion {
                modifier: scale_val.clone(),
                category: "ScaleAnchor".to_string(),
                suggested_title: format!("We Built a {} {}", scale_val, topic_clean),
                rationale: "Quantifiable volumetric or physical scale immediately captures mass-market curiosity beyond the DIY niche.".to_string(),
            });

            suggestions.push(DemandStackingSuggestion {
                modifier: "Cheap Mini Excavator".to_string(),
                category: "BudgetFriction".to_string(),
                suggested_title: format!("{}: Can a Cheap Mini Excavator Do It?", topic_clean),
                rationale: "Mechanical friction and equipment stakes transform passive craft footage into high-urgency problem solving.".to_string(),
            });

            suggestions.push(DemandStackingSuggestion {
                modifier: "Mud Pit to Crystal Clear Oasis".to_string(),
                category: "TransformationExtreme".to_string(),
                suggested_title: format!("Transforming Our {} into a Crystal Clear Oasis", topic_clean),
                rationale: "High-contrast before/after state delta confirms viewer investment in the ultimate payoff.".to_string(),
            });
        } else if topic_lower.contains("code") || topic_lower.contains("rust") || topic_lower.contains("software") || topic_lower.contains("system") || topic_lower.contains("database") || topic_lower.contains("api") {
            let scale_val = extracted_scale.unwrap_or_else(|| "1,000,000 Req/s".to_string());
            suggestions.push(DemandStackingSuggestion {
                modifier: scale_val.clone(),
                category: "ScaleAnchor".to_string(),
                suggested_title: format!("Building a {} Handling {}", topic_clean, scale_val),
                rationale: "Quantifiable throughput or latency scale anchors architectural authority and draws high-intent senior engineers.".to_string(),
            });

            suggestions.push(DemandStackingSuggestion {
                modifier: "Zero Dependencies / From Scratch".to_string(),
                category: "BudgetFriction".to_string(),
                suggested_title: format!("Writing a {} Entirely From Scratch", topic_clean),
                rationale: "Purist technical constraint eliminates framework fatigue and amplifies perceived developer mastery.".to_string(),
            });

            suggestions.push(DemandStackingSuggestion {
                modifier: "10x Faster Than C++".to_string(),
                category: "TransformationExtreme".to_string(),
                suggested_title: format!("Optimizing Our {}: 10x Faster Than C++", topic_clean),
                rationale: "Comparative speed benchmarks create immediate intellectual debate and community comments.".to_string(),
            });
        } else {
            // General / Media / Lifestyle
            if let Some(top_breakout) = breakout_trends.first() {
                suggestions.push(DemandStackingSuggestion {
                    modifier: top_breakout.entity.clone(),
                    category: "BreakoutSubTrope".to_string(),
                    suggested_title: format!("{}: The {} Secret", topic_clean, top_breakout.entity),
                    rationale: format!("Capitalizes on an active +{:.0}% velocity lift surging across {} independent cohort creators.", (top_breakout.velocity_lift - 1.0) * 100.0, top_breakout.channel_dispersion),
                });
            }

            if let Some(top_established) = established_trends.first() {
                suggestions.push(DemandStackingSuggestion {
                    modifier: top_established.entity.clone(),
                    category: "AuthorityEntity".to_string(),
                    suggested_title: format!("How {} Transformed Our {}", top_established.entity, topic_clean),
                    rationale: format!("Pairs an established evergreen demand pillar (+{:.0}% lift) to guarantee a reliable topic floor.", (top_established.velocity_lift - 1.0) * 100.0),
                });
            }

            suggestions.push(DemandStackingSuggestion {
                modifier: "Zero Experience / 30-Day Extreme".to_string(),
                category: "TransformationExtreme".to_string(),
                suggested_title: format!("{}: 30 Days With Zero Experience", topic_clean),
                rationale: "Relatable zero-to-hero transformation lowers the barrier of entry for broad mainstream audiences.".to_string(),
            });
        }

        suggestions
    }
}
