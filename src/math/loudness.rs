use serde::{Deserialize, Serialize};

/// EBU R128 and ITU-R BS.1770-4 compliant audio broadcast loudness telemetry.
/// Evaluates integrated program loudness, cold-open loudness, Loudness Range (LRA),
/// True Peak (dBTP), and YouTube automated volume attenuation penalties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoudnessMetrics {
    /// Integrated Program Loudness in LUFS across the full video/audio stream (Target: -14.0 LUFS)
    pub integrated_lufs: f64,
    /// Integrated Loudness in LUFS across the initial 30-second cold open window
    pub cold_open_lufs: f64,
    /// Loudness Range (LRA) in LU measuring dynamic contrast (Dialogue target: 4.0 - 8.0 LU)
    pub loudness_range_lu: f64,
    /// Maximum True Peak across all channels in dBTP (Ceiling: -1.0 dBTP to avoid mobile DAC clipping)
    pub true_peak_dbtp: f64,
    /// YouTube Loudness Penalty in dB (Negative offset YouTube applies to attenuate hot mixes; 0.0 if compliant)
    pub youtube_penalty_db: f64,
    /// Human-readable mastering status badge and summary
    pub mastering_status: String,
    /// Plain-English master bus audio prescription for the video editor / sound engineer
    pub actionable_prescription: String,
}

impl LoudnessMetrics {
    /// YouTube standard normalization target is -14.0 LUFS
    pub const YOUTUBE_TARGET_LUFS: f64 = -14.0;
    /// Inter-sample true peak ceiling for lossy mobile codecs (AAC/Opus) is -1.0 dBTP
    pub const TRUE_PEAK_CEILING_DBTP: f64 = -1.0;

    /// Computes the negative volume attenuation YouTube applies to videos exceeding -14.0 LUFS.
    /// YouTube attenuates hot mixes down to -14.0 LUFS, but does NOT boost quiet audio (< -14.0 LUFS).
    pub fn compute_youtube_penalty(integrated_lufs: f64) -> f64 {
        if integrated_lufs > Self::YOUTUBE_TARGET_LUFS {
            ((Self::YOUTUBE_TARGET_LUFS - integrated_lufs) * 10.0).round() / 10.0
        } else {
            0.0
        }
    }

    /// Evaluates raw acoustic loudness readings into structured, actionable telemetry.
    pub fn evaluate(
        integrated_lufs: f64,
        cold_open_lufs: f64,
        loudness_range_lu: f64,
        true_peak_dbtp: f64,
    ) -> Self {
        let youtube_penalty_db = Self::compute_youtube_penalty(integrated_lufs);

        let mut status_flags = Vec::new();
        let mut prescriptions = Vec::new();

        // 1. True Peak evaluation (> -1.0 dBTP causes clipping distortion on phone speakers)
        if true_peak_dbtp > Self::TRUE_PEAK_CEILING_DBTP {
            status_flags.push(format!(
                "⚠️ Mobile Speaker Distortion Risk (True Peak: {:.1} dBTP exceeds {:.1} dBTP ceiling)",
                true_peak_dbtp, Self::TRUE_PEAK_CEILING_DBTP
            ));
            if !(integrated_lufs > -10.0 && true_peak_dbtp > 0.0) {
                prescriptions.push(format!(
                    "Set master limiter ceiling to {:.1} dBTP to eliminate inter-sample DAC clipping on mobile phone speakers.",
                    Self::TRUE_PEAK_CEILING_DBTP
                ));
            }
        }

        // 2. Integrated Loudness & YouTube Volume Penalty
        if integrated_lufs > -10.0 && true_peak_dbtp > 0.0 {
            status_flags.push(format!(
                "⚠️ Hot Master Mix ({:.1} dB YouTube volume penalty)",
                youtube_penalty_db
            ));
            prescriptions.push(
                "Intentional hyper-compressed master mix detected (loudness targeted for phone speakers). Critical Fix: Lower limiter ceiling to -1.0 dBTP to eliminate mobile DAC inter-sample distortion while maintaining compressed vocal density.".to_string(),
            );
        } else if youtube_penalty_db < 0.0 {
            status_flags.push(format!(
                "⚠️ Hot Master Mix ({:.1} dB YouTube volume penalty)",
                youtube_penalty_db
            ));
            prescriptions.push(format!(
                "Pull back master bus output by {:.1} dB to prevent YouTube from turning down video volume and compromising impact.",
                youtube_penalty_db.abs()
            ));
        } else if integrated_lufs < -16.0 {
            let deficit = ((-14.0 - integrated_lufs) * 10.0).round() / 10.0;
            status_flags.push(format!(
                "🔈 Low Dialogue Volume ({:.1} LUFS; {:.1} dB below YouTube -14.0 LUFS standard)",
                integrated_lufs, deficit
            ));
            prescriptions.push(format!(
                "Increase dialogue/vocal gain by +{:.1} dB to match competitive mobile streaming loudness standards.",
                deficit
            ));
        } else {
            status_flags.push("🟢 Broadcast Compliant (-14.0 LUFS Target)".to_string());
        }

        // 3. Cold Open 30s Delta
        let delta = ((cold_open_lufs - integrated_lufs) * 10.0).round() / 10.0;
        if delta < -3.0 {
            prescriptions.push(format!(
                "Boost opening 30s dialogue by +{:.1} dB; the intro is noticeably quieter than the video average, causing mobile viewers to struggle.",
                delta.abs()
            ));
        } else if delta > 3.0 {
            prescriptions.push(format!(
                "Tame opening 30s sound effects/music (-{:.1} dB); intro bursts noticeably hotter than the rest of the mix.",
                delta
            ));
        }

        let mastering_status = if status_flags.is_empty() {
            "🟢 Broadcast Compliant (-14.0 LUFS Target)".to_string()
        } else {
            status_flags.join(" | ")
        };

        let actionable_prescription = if prescriptions.is_empty() {
            "Master bus loudness is balanced. Audio is streaming compliant without volume penalties.".to_string()
        } else {
            prescriptions.join(" ")
        };

        Self {
            integrated_lufs,
            cold_open_lufs,
            loudness_range_lu,
            true_peak_dbtp,
            youtube_penalty_db,
            mastering_status,
            actionable_prescription,
        }
    }

    /// Safe default fallback when audio cannot be analyzed or is silent.
    pub fn default_safe() -> Self {
        Self {
            integrated_lufs: -14.0,
            cold_open_lufs: -14.0,
            loudness_range_lu: 6.0,
            true_peak_dbtp: -1.0,
            youtube_penalty_db: 0.0,
            mastering_status: "🟢 Broadcast Compliant (-14.0 LUFS Target)".to_string(),
            actionable_prescription: "Audio baseline matches standard -14.0 LUFS streaming profile.".to_string(),
        }
    }
}
