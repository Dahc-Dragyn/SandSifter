use serde::{Deserialize, Serialize};

/// Word with sub-second timestamp boundaries and confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimestampedWord {
    pub word: String,
    pub start_ms: u32,
    pub end_ms: u32,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

fn default_confidence() -> f32 {
    0.95
}

/// Chronological sentence grouping with constituent timestamped words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimestampedSentence {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    #[serde(default)]
    pub words: Vec<TimestampedWord>,
}

/// Complete timestamped transcript for an entire short-form video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FullTranscript {
    pub sentences: Vec<TimestampedSentence>,
    pub total_duration_ms: u32,
}

impl FullTranscript {
    pub fn new() -> Self {
        Self::default()
    }

    /// Combines all sentence texts into a single string.
    pub fn full_text(&self) -> String {
        self.sentences
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Total word count across all sentences.
    pub fn word_count(&self) -> usize {
        self.sentences.iter().map(|s| s.words.len()).sum()
    }

    /// Calculates overall speech cadence in Words Per Minute (WPM).
    pub fn cadence_wpm(&self) -> f64 {
        if self.total_duration_ms == 0 {
            return 0.0;
        }
        let total_words = self.word_count();
        let minutes = (self.total_duration_ms as f64) / 60_000.0;
        if minutes > 0.0 {
            (total_words as f64) / minutes
        } else {
            0.0
        }
    }

    /// Extracts all words that fall within a temporal slice `[start_ms, end_ms]`.
    pub fn slice_words(&self, start_ms: u32, end_ms: u32) -> Vec<TimestampedWord> {
        let mut result = Vec::new();
        for sentence in &self.sentences {
            for word in &sentence.words {
                if word.start_ms >= start_ms && word.start_ms <= end_ms {
                    result.push(word.clone());
                }
            }
        }
        result
    }

    /// Returns the start time of the first spoken word, if any.
    pub fn first_word_onset_ms(&self) -> Option<u32> {
        self.sentences.first().and_then(|s| s.words.first()).map(|w| w.start_ms)
    }

    /// Returns the end time of the last spoken word, if any.
    pub fn last_word_offset_ms(&self) -> Option<u32> {
        self.sentences.last().and_then(|s| s.words.last()).map(|w| w.end_ms)
    }
}

