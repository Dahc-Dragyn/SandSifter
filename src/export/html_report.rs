use std::path::{Path, PathBuf};
use crate::export::flight_sheet::{slugify, FlightSheet};
use crate::math::marginal_gains::MarginalGainsScorecard;

/// Basic HTML entity escaping for user-supplied strings.
pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Extracts an 11-character YouTube video ID from a URL or raw ID string.
fn extract_yt_id(input: &str) -> Option<String> {
    let clean = input.trim();
    if clean.len() == 11 && clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Some(clean.to_string());
    }
    for marker in &["v=", "/embed/", "youtu.be/", "/v/"] {
        if let Some(pos) = clean.find(marker) {
            let sub = &clean[pos + marker.len()..];
            let end_pos = sub.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-').unwrap_or(sub.len());
            if end_pos == 11 {
                return Some(sub[..11].to_string());
            }
        }
    }
    None
}

/// Standalone Single-File HTML Executive Report Generator.
/// Emits a self-contained, zero-dependency, interactive HTML5 dashboard.
pub struct HtmlReportGenerator;

impl HtmlReportGenerator {
    /// Generates a standalone, fully styled HTML report from a FlightSheet and MarginalGainsScorecard.
    pub fn generate(flight_sheet: &FlightSheet, scorecard: &MarginalGainsScorecard) -> String {
        let title_esc = escape_html(&flight_sheet.video_title);
        let channel_esc = escape_html(&flight_sheet.channel_name);
        let duration_esc = escape_html(&flight_sheet.duration_formatted);
        let tone_esc = escape_html(&flight_sheet.tone_profile_formatted);
        let verdict_esc = escape_html(&flight_sheet.overall_verdict);
        let bottleneck_esc = escape_html(&flight_sheet.primary_bottleneck);
        let audience_tier_esc = escape_html(&flight_sheet.audience_satisfaction_tier);

        let health_grade = scorecard.series_system_reliability_pct.round() as u64;
        let composite_score = scorecard.composite_brailsford_score;
        let roi_lift = flight_sheet.compounded_gain_pct;

        // Determine health grade color
        let grade_color = if health_grade >= 70 {
            "#22c55e" // Emerald
        } else if health_grade >= 40 {
            "#f59e0b" // Amber
        } else {
            "#ef4444" // Rose/Red
        };

        // Gauge SVG calculation: circumference = 2 * PI * r = 2 * 3.14159 * 42 = 263.89
        let circumference = 263.89_f64;
        let offset = circumference * (1.0 - (health_grade as f64 / 100.0).clamp(0.0, 1.0));

        let mut html = String::with_capacity(32768);

        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"UTF-8\">\n");
        html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str(&format!("  <title>Sandsifter Executive Report - {}</title>\n", title_esc));
        html.push_str("  <style>\n");
        html.push_str(include_str!("report_styles.css"));
        html.push_str("  </style>\n</head>\n<body>\n");

        html.push_str("  <div class=\"dashboard-container\">\n");

        // Top Navigation Bar
        html.push_str("    <header class=\"navbar\">\n");
        html.push_str("      <div class=\"brand\">\n");
        html.push_str("        <span class=\"brand-icon\">🏖️</span>\n");
        html.push_str("        <span class=\"brand-name\">SANDSIFTER</span>\n");
        html.push_str("        <span class=\"brand-tag\">EXECUTIVE FLIGHT SHEET</span>\n");
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"nav-meta\">\n");
        html.push_str(&format!("        <span class=\"badge-tone\">{}</span>\n", tone_esc));
        html.push_str(&format!("        <span class=\"runtime-badge\">⏱️ {}</span>\n", duration_esc));
        html.push_str("        <button class=\"btn-print\" onclick=\"window.print()\">🖨️ Print / PDF</button>\n");
        html.push_str("      </div>\n");
        html.push_str("    </header>\n\n");

        // Component A: Header & Verdict Hero Card
        html.push_str("    <section class=\"hero-verdict-grid\">\n");
        html.push_str("      <div class=\"card hero-summary-card\">\n");
        html.push_str("        <div class=\"video-meta-block\">\n");
        html.push_str(&format!("          <div class=\"channel-name\">{}</div>\n", channel_esc));
        html.push_str(&format!("          <h1 class=\"video-title\">{}</h1>\n", title_esc));
        html.push_str("        </div>\n");

        html.push_str("        <div class=\"verdict-banner\">\n");
        html.push_str("          <div class=\"verdict-title-label\">EDITORIAL VERDICT</div>\n");
        html.push_str(&format!("          <div class=\"verdict-badge-text\">{}</div>\n", verdict_esc));
        html.push_str("        </div>\n");

        html.push_str("        <div class=\"bottleneck-alert-box\">\n");
        html.push_str("          <span class=\"bottleneck-icon\">🚨</span>\n");
        html.push_str("          <div class=\"bottleneck-content\">\n");
        html.push_str("            <span class=\"bottleneck-label\">PRIMARY SERIES BOTTLENECK</span>\n");
        html.push_str(&format!("            <p class=\"bottleneck-desc\">{}</p>\n", bottleneck_esc));
        html.push_str("          </div>\n");
        html.push_str("        </div>\n");

        if let Some(suit) = &flight_sheet.suitability_report {
            let is_green = suit.rating == "GREEN";
            let (status_class, icon, status_text) = if is_green {
                ("ad-banner-green", "🟢", "MONETIZATION SAFE (Full Ad Distribution / Immediate Public Scheduling)")
            } else {
                ("ad-banner-yellow", "🟡", "ADVERTISER RISK (Limited Ads / Unlisted 24h Review Recommended)")
            };
            let summary_esc = escape_html(&suit.executive_summary);

            html.push_str(&format!(
                "        <div class=\"ad-suitability-banner {}\">\n          <span class=\"ad-icon\">{}</span>\n          <div class=\"ad-content\">\n            <div class=\"ad-title\">{}</div>\n            <div class=\"ad-summary\">{}</div>\n          </div>\n        </div>\n",
                status_class, icon, status_text, summary_esc
            ));
        }

        html.push_str("        <div class=\"quick-stats-row\">\n");
        html.push_str(&format!("          <div class=\"stat-pill\"><span class=\"stat-k\">Audience Sentiment:</span> <span class=\"stat-v\">{}</span></div>\n", audience_tier_esc));
        html.push_str(&format!("          <div class=\"stat-pill\"><span class=\"stat-k\">Compounded ROI:</span> <span class=\"stat-v highlight-green\">+{:.1}% Viewership Lift</span></div>\n", roi_lift));
        html.push_str(&format!("          <div class=\"stat-pill\"><span class=\"stat-k\">Brailsford Composite:</span> <span class=\"stat-v\">{:.1} / 100</span></div>\n", composite_score));
        html.push_str("        </div>\n");
        html.push_str("      </div>\n");

        // Health Grade Gauge Card
        html.push_str("      <div class=\"card gauge-card\">\n");
        html.push_str("        <div class=\"card-title\">VIDEO HEALTH GRADE</div>\n");
        html.push_str("        <div class=\"card-subtitle\">Multiplicative Reliability Engine</div>\n");
        html.push_str("        <div class=\"circular-gauge-wrapper\">\n");
        html.push_str("          <svg class=\"gauge-svg\" viewBox=\"0 0 100 100\">\n");
        html.push_str("            <circle class=\"gauge-bg\" cx=\"50\" cy=\"50\" r=\"42\"></circle>\n");
        html.push_str(&format!(
            "            <circle class=\"gauge-meter\" cx=\"50\" cy=\"50\" r=\"42\" style=\"stroke: {}; stroke-dasharray: 263.89; stroke-dashoffset: {:.2};\"></circle>\n",
            grade_color, offset
        ));
        html.push_str("          </svg>\n");
        html.push_str("          <div class=\"gauge-center-text\">\n");
        html.push_str(&format!("            <span class=\"gauge-number\" style=\"color: {};\">{}%</span>\n", grade_color, health_grade));
        html.push_str("            <span class=\"gauge-label\">RELIABILITY</span>\n");
        html.push_str("          </div>\n");
        html.push_str("        </div>\n");
        html.push_str("        <p class=\"gauge-footnote\">Weakest-link failure calculus. 1 bottleneck drags entire video score.</p>\n");
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Component B: Interactive Audio & Loudness Card
        if let Some(loudness) = &flight_sheet.loudness_metrics {
            html.push_str("    <section class=\"dashboard-section\">\n");
            html.push_str("      <div class=\"section-header\">\n");
            html.push_str("        <h2>🔊 Audio Mastering & YouTube Loudness Radar</h2>\n");
            html.push_str("        <span class=\"section-subtitle\">EBU R128 / ITU-R BS.1770 Broadcast Normalization</span>\n");
            html.push_str("      </div>\n");
            html.push_str("      <div class=\"card loudness-card\">\n");
            html.push_str("        <div class=\"loudness-grid\">\n");

            // Meter 1: Integrated LUFS
            let lufs = loudness.integrated_lufs;
            let target_lufs = -14.0_f64;
            let lufs_pct = (((lufs - (-30.0)) / 24.0) * 100.0).clamp(0.0, 100.0);

            let (zone_status_label, zone_status_class, needle_color) = if lufs < -16.0 {
                (format!("🔈 TOO QUIET: {:.1} dB below YouTube -14.0 LUFS standard", target_lufs - lufs), "zone-status-quiet", "#38bdf8")
            } else if lufs <= -13.0 {
                ("🟢 OPTIMAL: Inside YouTube Streaming Sweet Spot (-14.0 LUFS)".to_string(), "zone-status-optimal", "#22c55e")
            } else {
                (format!("🚨 HOT MIX: {:.1} dB above -14.0 LUFS (YouTube will attenuate volume)", lufs - target_lufs), "zone-status-hot", "#ef4444")
            };

            html.push_str("          <div class=\"meter-container\">\n");
            html.push_str("            <div class=\"meter-header\">\n");
            html.push_str("              <div class=\"meter-title-wrap\">\n");
            html.push_str("                <span class=\"meter-title\">Integrated Loudness</span>\n");
            html.push_str(&format!("                <span class=\"meter-status-pill {}\">{}</span>\n", zone_status_class, zone_status_label));
            html.push_str("              </div>\n");
            html.push_str(&format!("              <span class=\"meter-value\">{:.1} LUFS <span class=\"meter-target\">(YT Target: -14.0)</span></span>\n", lufs));
            html.push_str("            </div>\n");

            // Tri-Color Multi-Zone Visual Track
            html.push_str("            <div class=\"lufs-track-container\">\n");
            html.push_str("              <div class=\"lufs-multi-track\">\n");
            html.push_str("                <div class=\"lufs-zone zone-quiet\" style=\"width: 58.33%;\" title=\"Quiet Zone (< -16 LUFS)\"></div>\n");
            html.push_str("                <div class=\"lufs-zone zone-sweet-spot\" style=\"width: 12.5%;\" title=\"YouTube Sweet Spot (-16 to -13 LUFS)\">\n");
            html.push_str("                  <span class=\"zone-badge\">🎯 SWEET SPOT</span>\n");
            html.push_str("                </div>\n");
            html.push_str("                <div class=\"lufs-zone zone-hot\" style=\"width: 29.17%;\" title=\"Hot Mix / Penalty Zone (> -13 LUFS)\"></div>\n");

            // Target Marker Line at -14.0 LUFS (66.67%)
            html.push_str("                <div class=\"lufs-target-line\" style=\"left: 66.67%;\" title=\"YouTube Strict -14.0 LUFS Target\">\n");
            html.push_str("                  <span class=\"target-flag\">-14.0 YT</span>\n");
            html.push_str("                </div>\n");

            // Current Reading Needle Indicator
            html.push_str(&format!(
                "                <div class=\"lufs-needle\" style=\"left: {:.1}%; border-color: {};\">\n                  <div class=\"needle-pointer\" style=\"background: {};\"></div>\n                  <div class=\"needle-badge\" style=\"background: {};\">{:.1} LUFS</div>\n                </div>\n",
                lufs_pct, needle_color, needle_color, needle_color, lufs
            ));

            html.push_str("              </div>\n");
            html.push_str("              <div class=\"meter-scale-labels\">\n");
            html.push_str("                <span>-30 LUFS (Quiet)</span>\n");
            html.push_str("                <span>-24</span>\n");
            html.push_str("                <span>-18</span>\n");
            html.push_str("                <span class=\"scale-yt-sweet\">-16 to -13 (YT Target)</span>\n");
            html.push_str("                <span>-10 (Hot)</span>\n");
            html.push_str("                <span>-6 LUFS (Penalty)</span>\n");
            html.push_str("              </div>\n");
            html.push_str("            </div>\n");
            html.push_str("          </div>\n");

            // Audio Key Performance Indicators with Plain-English Human Context
            html.push_str("          <div class=\"loudness-stats-grid\">\n");

            // 1. YouTube Penalty Box
            let penalty_class = if loudness.youtube_penalty_db < 0.0 { "kpi-danger" } else { "kpi-safe" };
            let penalty_text = if loudness.youtube_penalty_db < 0.0 {
                format!("{:.1} dB Penalty (Hot Mix)", loudness.youtube_penalty_db)
            } else {
                "0.0 dB (Safe / Compliant)".to_string()
            };
            let penalty_sub = if loudness.youtube_penalty_db < 0.0 {
                "YouTube will automatically attenuate your audio, dulling impact."
            } else {
                "Zero volume cut; YouTube will NOT lower your video's volume."
            };
            html.push_str(&format!(
                "            <div class=\"kpi-box {}\">\n              <span class=\"kpi-label\">YouTube Volume Cut</span>\n              <span class=\"kpi-value\">{}</span>\n              <span class=\"kpi-subtext\">{}</span>\n            </div>\n",
                penalty_class, penalty_text, penalty_sub
            ));

            // 2. True Peak Level
            let is_clipping = loudness.true_peak_dbtp > crate::math::LoudnessMetrics::TRUE_PEAK_CEILING_DBTP;
            let peak_class = if is_clipping { "kpi-danger" } else { "kpi-safe" };
            let peak_text = if is_clipping {
                format!("{:.1} dBTP (Clipping Risk)", loudness.true_peak_dbtp)
            } else {
                format!("{:.1} dBTP [SAFE]", loudness.true_peak_dbtp)
            };
            let peak_sub = if is_clipping {
                "Exceeds -1.0 dBTP ceiling; will cause crackle/distortion on phone speakers."
            } else {
                "Safe ceiling; prevents audio clipping & speaker buzz on mobile DACs."
            };
            html.push_str(&format!(
                "            <div class=\"kpi-box {}\">\n              <span class=\"kpi-label\">True Peak Level</span>\n              <span class=\"kpi-value\">{}</span>\n              <span class=\"kpi-subtext\">{}</span>\n            </div>\n",
                peak_class, peak_text, peak_sub
            ));

            // 3. Cold Open Loudness (Hook 0-30s)
            let cold_diff = -14.0 - loudness.cold_open_lufs;
            let (cold_class, cold_sub) = if loudness.cold_open_lufs < -16.0 {
                ("kpi-danger", format!("Hook is quiet ({:.1} dB below YT target); mobile scrollers may struggle to hear you.", cold_diff))
            } else if loudness.cold_open_lufs <= -13.0 {
                ("kpi-safe", "Dialed hook volume; loud and crystal clear during mobile autoplay feed scrolling.".to_string())
            } else {
                ("kpi-danger", format!("Hot intro ({:.1} dB above target); risk of jarring or deafening viewers.", loudness.cold_open_lufs - (-14.0)))
            };
            html.push_str(&format!(
                "            <div class=\"kpi-box {}\">\n              <span class=\"kpi-label\">Cold-Open (0:00-0:30)</span>\n              <span class=\"kpi-value\">{:.1} LUFS</span>\n              <span class=\"kpi-subtext\">{}</span>\n            </div>\n",
                cold_class, loudness.cold_open_lufs, cold_sub
            ));

            // 4. Dynamic Range (LRA)
            let (lra_class, lra_sub) = if loudness.loudness_range_lu < 2.0 {
                ("kpi-safe", "Very tight dynamic range; uniform speech with zero quiet whispers.")
            } else if loudness.loudness_range_lu <= 7.0 {
                ("kpi-safe", "Optimal dynamic range; natural speech cadence with comfortable listening balance.")
            } else {
                ("kpi-danger", "High dynamic range; whisper-to-shout contrast may force viewers to ride the volume knob.")
            };
            html.push_str(&format!(
                "            <div class=\"kpi-box {}\">\n              <span class=\"kpi-label\">Loudness Range (LRA)</span>\n              <span class=\"kpi-value\">{:.1} LU</span>\n              <span class=\"kpi-subtext\">{}</span>\n            </div>\n",
                lra_class, loudness.loudness_range_lu, lra_sub
            ));

            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            // Human Educational Explainer Card
            let delta_human = target_lufs - lufs;
            let human_verdict = if lufs < -16.0 {
                format!("Your video's dialogue is <strong>{:.1} dB quieter</strong> than typical YouTube videos. Because negative numbers work backward in audio (-14.0 is louder than {:.1}), viewers on phones or laptops in noisy rooms will struggle to hear you clearly unless you boost dialogue.", delta_human, lufs)
            } else if lufs <= -13.0 {
                "Your audio sits precisely in YouTube's sweet spot. Your video will sound loud, punchy, and competitive on mobile phones without triggering YouTube's volume penalty.".to_string()
            } else {
                format!("Your mix is <strong>{:.1} dB hotter</strong> than YouTube's -14.0 LUFS ceiling. YouTube will automatically attenuate (turn down) your video by that amount upon upload.", lufs - target_lufs)
            };

            html.push_str("        <div class=\"lufs-human-explainer\">\n");
            html.push_str("          <div class=\"explainer-header\">💡 Human Translation: What does LUFS mean for this video?</div>\n");
            html.push_str(&format!("          <p class=\"explainer-body\">{} The <strong>Cold-Open ({:.1} LUFS)</strong> measures the first 30 seconds—the critical hook where 40%+ of autoplay viewers decide to stay or swipe.</p>\n", human_verdict, loudness.cold_open_lufs));
            html.push_str("        </div>\n");

            // Master Bus Prescription Banner
            html.push_str("        <div class=\"prescription-banner\">\n");
            html.push_str("          <span class=\"prescription-icon\">🎛️</span>\n");
            html.push_str("          <div class=\"prescription-content\">\n");
            html.push_str("            <span class=\"prescription-title\">MASTER BUS EDITORIAL PRESCRIPTION</span>\n");
            html.push_str(&format!("            <p class=\"prescription-text\">{}</p>\n", escape_html(&loudness.actionable_prescription)));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            html.push_str("      </div>\n");
            html.push_str("    </section>\n\n");
        }

        // Component B.2: Dedicated YouTube Shorts Mobile Retention Radar
        if let Some(shorts) = &flight_sheet.shorts_metrics {
            html.push_str("    <section class=\"dashboard-section\">\n");
            html.push_str("      <div class=\"section-header\">\n");
            html.push_str("        <h2>📱 YouTube Shorts Mobile Retention &amp; Loop Radar</h2>\n");
            html.push_str("        <span class=\"section-subtitle\">Mobile Feed Mechanics • View-vs-Swiped-Away Analysis • Infinite Loop Continuity</span>\n");
            html.push_str("      </div>\n");
            html.push_str("      <div class=\"card shorts-radar-card\">\n");
            html.push_str("        <div class=\"shorts-grid\" style=\"display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1rem; margin-bottom: 1.5rem;\">\n");

            let hook_badge = if shorts.first_frame_hook_safe { "badge-green" } else { "badge-red" };
            let hook_text = if shorts.first_frame_hook_safe { "SAFE (&le; 1.5s)" } else { "HIGH SWIPE RISK (&gt; 1.5s)" };

            html.push_str(&format!(
                "          <div class=\"stat-box\"><div class=\"stat-label\">1.5s First-Frame Hook</div><div class=\"stat-value\">{:.2}s</div><span class=\"badge {}\">{}</span></div>\n",
                shorts.first_frame_hook_delay, hook_badge, hook_text
            ));

            let dur_badge = match shorts.duration_safety_status {
                crate::math::shorts::ShortsDurationStatus::Optimal => "badge-green",
                crate::math::shorts::ShortsDurationStatus::Compact => "badge-cyan",
                crate::math::shorts::ShortsDurationStatus::Extended => "badge-orange",
                crate::math::shorts::ShortsDurationStatus::TruncationRisk => "badge-red",
            };

            html.push_str(&format!(
                "          <div class=\"stat-box\"><div class=\"stat-label\">Duration Status</div><div class=\"stat-value\">{:.1}s</div><span class=\"badge {}\">{}</span></div>\n",
                shorts.duration_seconds, dur_badge, shorts.duration_safety_status.label()
            ));

            let loop_badge = if shorts.loop_seamless { "badge-green" } else { "badge-orange" };
            let loop_text = if shorts.loop_seamless { "SEAMLESS LOOP" } else { "ABRUPT OUTRO" };

            html.push_str(&format!(
                "          <div class=\"stat-box\"><div class=\"stat-label\">Loop Continuity Score</div><div class=\"stat-value\">{:.0} / 100</div><span class=\"badge {}\">{}</span></div>\n",
                shorts.loop_continuity_score, loop_badge, loop_text
            ));

            let swipe_badge = if shorts.stay_vs_swipe_risk == "LOW" { "badge-green" } else if shorts.stay_vs_swipe_risk == "MODERATE" { "badge-orange" } else { "badge-red" };

            html.push_str(&format!(
                "          <div class=\"stat-box\"><div class=\"stat-label\">Stay-vs-Swipe Ratio</div><div class=\"stat-value\">{:.1}%</div><span class=\"badge {}\">{} RISK</span></div>\n",
                shorts.estimated_stay_ratio_pct, swipe_badge, shorts.stay_vs_swipe_risk
            ));

            html.push_str("        </div>\n");

            html.push_str("        <div class=\"prescription-banner\" style=\"margin-top: 1rem;\">\n");
            html.push_str("          <span class=\"prescription-icon\">🔄</span>\n");
            html.push_str("          <div class=\"prescription-content\">\n");
            html.push_str("            <span class=\"prescription-title\">LOOP BRIDGE ANALYSIS</span>\n");
            html.push_str(&format!("            <p class=\"prescription-text\">{}</p>\n", escape_html(&shorts.loop_bridge_insight)));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            html.push_str("        <div class=\"prescription-banner\" style=\"margin-top: 0.75rem;\">\n");
            html.push_str("          <span class=\"prescription-icon\">⚡</span>\n");
            html.push_str("          <div class=\"prescription-content\">\n");
            html.push_str("            <span class=\"prescription-title\">SHORTS ACTION DIRECTIVE</span>\n");
            html.push_str(&format!("            <p class=\"prescription-text\">{}</p>\n", escape_html(&shorts.executive_summary)));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            html.push_str("      </div>\n");
            html.push_str("    </section>\n\n");
        }

        // Component B.3: Interactive Retention Scrubber & Video Suite (Issue 3)
        let duration_sec = flight_sheet.duration_seconds_f64();
        let yt_id = extract_yt_id(&scorecard.target_id);

        let retention_points = if !flight_sheet.retention_curve.is_empty() {
            flight_sheet.retention_curve.clone()
        } else {
            let steps = 100;
            let dt = duration_sec / steps as f64;
            let mut pts = Vec::with_capacity(steps + 1);
            let mut r = 1.0_f64;
            for i in 0..=steps {
                let t = i as f64 * dt;
                r = (r - 0.005).max(0.20);
                pts.push(crate::math::retention::HeatmapPoint {
                    start_time: t,
                    end_time: t + dt,
                    value: r,
                });
            }
            pts
        };

        let svg_width = 1000.0_f64;
        let graph_bottom = 195.0_f64;
        let graph_top = 20.0_f64;
        let graph_height = graph_bottom - graph_top;

        let mut path_d = String::new();
        let mut area_d = String::new();
        let mut coords = Vec::with_capacity(retention_points.len());

        for (idx, pt) in retention_points.iter().enumerate() {
            let t = (pt.start_time + pt.end_time) / 2.0;
            let x = ((t / duration_sec) * svg_width).clamp(0.0, svg_width);
            let val = pt.value.clamp(0.0, 1.0);
            let y = graph_bottom - (val * graph_height);
            coords.push((x, y, t, val));
            if idx == 0 {
                path_d.push_str(&format!("M {:.1},{:.1}", x, y));
                area_d.push_str(&format!("M {:.1},{:.1} L {:.1},{:.1}", x, graph_bottom, x, y));
            } else {
                path_d.push_str(&format!(" L {:.1},{:.1}", x, y));
                area_d.push_str(&format!(" L {:.1},{:.1}", x, y));
            }
        }
        if let Some(&(last_x, _, _, _)) = coords.last() {
            area_d.push_str(&format!(" L {:.1},{:.1} Z", last_x, graph_bottom));
        }

        let mut grid_svg = String::new();
        for &val in &[0.25_f64, 0.50, 0.75, 1.0] {
            let gy = graph_bottom - (val * graph_height);
            grid_svg.push_str(&format!(
                "          <line x1=\"0\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"rgba(255, 255, 255, 0.08)\" stroke-width=\"1\" stroke-dasharray=\"2 2\" />\n          <text x=\"6\" y=\"{:.1}\" fill=\"rgba(255, 255, 255, 0.3)\" font-size=\"9\" font-family=\"monospace\">{:.0}%</text>\n",
                gy, svg_width, gy, gy - 4.0, val * 100.0
            ));
        }

        let mut time_ticks_svg = String::new();
        let num_ticks = 6;
        for i in 0..=num_ticks {
            let t = (i as f64 / num_ticks as f64) * duration_sec;
            let tx = (i as f64 / num_ticks as f64) * svg_width;
            let anchor = if i == 0 { "start" } else if i == num_ticks { "end" } else { "middle" };
            let mm = (t / 60.0).floor() as u64;
            let ss = (t % 60.0).round() as u64;
            time_ticks_svg.push_str(&format!(
                "          <text x=\"{:.1}\" y=\"214\" fill=\"rgba(255, 255, 255, 0.4)\" font-size=\"10\" font-family=\"monospace\" text-anchor=\"{}\">{:02}:{:02}</text>\n",
                tx, anchor, mm, ss
            ));
        }

        let mut checkpoints_svg = String::new();
        if duration_sec >= 180.0 {
            let x3 = (180.0 / duration_sec) * svg_width;
            checkpoints_svg.push_str(&format!(
                "          <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#38bdf8\" stroke-dasharray=\"4 4\" stroke-width=\"1.5\" />\n          <text x=\"{:.1}\" y=\"{:.1}\" fill=\"#38bdf8\" font-size=\"10\" font-weight=\"600\">03:00 Gate</text>\n",
                x3, graph_top, x3, graph_bottom, x3 + 4.0, graph_top + 14.0
            ));
        }
        if duration_sec >= 360.0 {
            let x6 = (360.0 / duration_sec) * svg_width;
            checkpoints_svg.push_str(&format!(
                "          <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#38bdf8\" stroke-dasharray=\"4 4\" stroke-width=\"1.5\" />\n          <text x=\"{:.1}\" y=\"{:.1}\" fill=\"#38bdf8\" font-size=\"10\" font-weight=\"600\">06:00 Gate</text>\n",
                x6, graph_top, x6, graph_bottom, x6 + 4.0, graph_top + 14.0
            ));
        }

        let mut markers_svg = String::new();
        for (idx, breach) in flight_sheet.attention_breaches.iter().enumerate() {
            let t = breach.timestamp_sec;
            let bx = ((t / duration_sec) * svg_width).clamp(0.0, svg_width);
            let mut by = graph_bottom - (0.60 * graph_height);
            for i in 0..coords.len().saturating_sub(1) {
                if coords[i].0 <= bx && coords[i + 1].0 >= bx {
                    let dx = coords[i + 1].0 - coords[i].0;
                    let ratio = if dx.abs() < 0.001 { 0.0 } else { (bx - coords[i].0) / dx };
                    by = coords[i].1 + ratio * (coords[i + 1].1 - coords[i].1);
                    break;
                }
            }
            let cause_label = match &breach.root_cause {
                crate::math::retention::BreachRootCause::SlowVisualPacing { .. } => "Lingering Camera Take",
                crate::math::retention::BreachRootCause::DeadAirSilence { .. } => "Cold-Open Dead Air",
                crate::math::retention::BreachRootCause::CompoundFriction { .. } => "Compound Pacing Friction",
                crate::math::retention::BreachRootCause::NarrativePlateau => "Narrative Plateau",
            };
            let tc_esc = escape_html(&breach.formatted_timestamp);
            let cause_esc = escape_html(cause_label);
            let remedy_esc = escape_html(&breach.tactical_remedy);

            markers_svg.push_str(&format!(
                "          <g class=\"breach-marker\" data-index=\"{}\" data-time=\"{:.1}\" data-tc=\"{}\" data-rprime=\"{:.1}\" data-cause=\"{}\" data-remedy=\"{}\">\n            <circle class=\"marker-pulse\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"6\" fill=\"rgba(239, 68, 68, 0.45)\" />\n            <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5.5\" fill=\"#ef4444\" stroke=\"#ffffff\" stroke-width=\"1.5\" />\n            <text x=\"{:.1}\" y=\"{:.1}\" fill=\"#ef4444\" font-size=\"10\" font-weight=\"700\" text-anchor=\"middle\">▼</text>\n          </g>\n",
                idx, t, tc_esc, breach.r_prime, cause_esc, remedy_esc, bx, by, bx, by, bx, by - 9.0
            ));
        }

        let player_html = if let Some(ref ytid) = yt_id {
            format!(
                "            <iframe id=\"report-youtube-iframe\" src=\"https://www.youtube-nocookie.com/embed/{}?enablejsapi=1&autoplay=0&rel=0\" allow=\"accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture\" allowfullscreen></iframe>\n",
                ytid
            )
        } else {
            "            <div class=\"video-placeholder-state\">\n              <span style=\"font-size: 32px;\">🎬</span>\n              <div><strong>Local Video Draft Preview</strong></div>\n              <span>Run Sandsifter daemon to enable live media scrubbing, or upload master render.</span>\n            </div>\n".to_string()
        };

        html.push_str("    <section class=\"dashboard-section\">\n");
        html.push_str("      <div class=\"section-header\">\n");
        html.push_str("        <h2>🎬 Interactive Retention Scrubber & NLE Video Sync</h2>\n");
        html.push_str("        <span class=\"section-subtitle\">Click any red retention cliff or timeline timecode to scrub playhead to that exact second</span>\n");
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"card scrubber-card\">\n");
        html.push_str("        <div class=\"scrubber-header-row\">\n");
        html.push_str("          <div class=\"scrubber-title-wrap\">\n");
        html.push_str("            <span style=\"font-weight: 700; color: #fff;\">Audience Retention Curve R(t) & Video Synchronization</span>\n");
        html.push_str("          </div>\n");
        html.push_str(&format!("          <div class=\"timecode-badge\" id=\"report-timecode-badge\">00:00 / {}</div>\n", duration_esc));
        html.push_str("        </div>\n");
        html.push_str("        <div class=\"scrubber-layout-grid\">\n");
        html.push_str("          <div class=\"video-preview-wrapper\">\n");
        html.push_str(&player_html);
        html.push_str("          </div>\n");
        html.push_str("          <div class=\"curve-container-wrapper\">\n");
        html.push_str("            <div class=\"svg-canvas-box\">\n");
        html.push_str("              <svg class=\"retention-svg\" id=\"report-retention-svg\" viewBox=\"0 0 1000 220\" preserveAspectRatio=\"none\">\n");
        html.push_str("                <defs>\n");
        html.push_str("                  <linearGradient id=\"report-retention-gradient\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">\n");
        html.push_str("                    <stop offset=\"0%\" stop-color=\"#22c55e\" stop-opacity=\"0.45\" />\n");
        html.push_str("                    <stop offset=\"50%\" stop-color=\"#38bdf8\" stop-opacity=\"0.2\" />\n");
        html.push_str("                    <stop offset=\"100%\" stop-color=\"#ef4444\" stop-opacity=\"0.05\" />\n");
        html.push_str("                  </linearGradient>\n");
        html.push_str("                </defs>\n");
        html.push_str(&grid_svg);
        html.push_str(&checkpoints_svg);
        html.push_str(&format!("                <path d=\"{}\" fill=\"url(#report-retention-gradient)\" />\n", area_d));
        html.push_str(&format!("                <path d=\"{}\" fill=\"none\" stroke=\"#22c55e\" stroke-width=\"2.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\" />\n", path_d));
        html.push_str(&markers_svg);
        html.push_str(&format!("                <line id=\"report-playhead-line\" x1=\"0\" y1=\"12\" x2=\"0\" y2=\"{:.1}\" class=\"scrubber-playhead-line\" />\n", graph_bottom));
        html.push_str("                <polygon id=\"report-playhead-thumb\" points=\"0,12 -6,2 6,2\" class=\"scrubber-playhead-thumb\" />\n");
        html.push_str(&time_ticks_svg);
        html.push_str("                <rect class=\"scrubber-hit-rect\" id=\"report-scrubber-hitbox\" x=\"0\" y=\"0\" width=\"1000\" height=\"220\" fill=\"transparent\" />\n");
        html.push_str("              </svg>\n");
        html.push_str("              <div class=\"scrubber-tooltip\" id=\"report-scrubber-tooltip\"></div>\n");
        html.push_str("            </div>\n");
        html.push_str("            <div class=\"scrubber-legend\">\n");
        html.push_str("              <div class=\"legend-items\">\n");
        html.push_str("                <div class=\"legend-item\"><span class=\"legend-dot green\"></span> Retention R(t)</div>\n");
        html.push_str("                <div class=\"legend-item\"><span class=\"legend-dot red\"></span> Attention Breach R'(t)</div>\n");
        html.push_str("                <div class=\"legend-item\"><span class=\"legend-dot cyan\"></span> 03:00 / 06:00 Gate</div>\n");
        html.push_str("              </div>\n");
        html.push_str("              <span>💡 Click or drag playhead to scrub</span>\n");
        html.push_str("            </div>\n");
        html.push_str("          </div>\n");
        html.push_str("        </div>\n");
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Component C: Interactive NLE Punch List
        html.push_str("    <section class=\"dashboard-section\">\n");
        html.push_str("      <div class=\"section-header\">\n");
        html.push_str("        <h2>📋 Timeline Punch List (NLE Editing Commands)</h2>\n");
        html.push_str("        <span class=\"section-subtitle\">High-leverage micro-edits for Premiere Pro, DaVinci Resolve & FCP</span>\n");
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"card table-card\">\n");
        html.push_str("        <div class=\"table-responsive\">\n");
        html.push_str("          <table class=\"punch-list-table\">\n");
        html.push_str("            <thead>\n");
        html.push_str("              <tr>\n");
        html.push_str("                <th>TIMECODE</th>\n");
        html.push_str("                <th>ACTION REQUIRED</th>\n");
        html.push_str("                <th>EDITING COMMAND & PRESCRIPTION</th>\n");
        html.push_str("                <th>PROJECTED IMPACT</th>\n");
        html.push_str("                <th class=\"text-center\">SEEK & COPY</th>\n");
        html.push_str("              </tr>\n");
        html.push_str("            </thead>\n");
        html.push_str("            <tbody>\n");

        let mut rendered_cues = 0;
        for (idx, cue) in flight_sheet.timeline_punch_list.iter().enumerate() {
            let tc_clean = cue.timecode_range.trim();
            let is_inactive = tc_clean == "00:00 - 00:00"
                || (cue.impact_summary.contains("+0.0%") && cue.remedy_instruction.to_lowercase().contains("maintain"));

            if is_inactive {
                continue;
            }
            rendered_cues += 1;

            let tc_esc = escape_html(&cue.timecode_range);
            let action_esc = escape_html(&cue.action_type);
            let remedy_esc = escape_html(&cue.remedy_instruction);
            let impact_esc = escape_html(&cue.impact_summary);

            let badge_class = match cue.action_type.as_str() {
                "TRIM DEAD AIR" => "badge-cyan",
                "INSERT B-ROLL" => "badge-purple",
                "CUT EXPOSITION" => "badge-red",
                "OUTRO FIX" => "badge-orange",
                _ => "badge-blue",
            };

            let copy_text = format!("{}: {}", cue.timecode_range, cue.remedy_instruction);
            let copy_esc = escape_html(&copy_text);

            html.push_str(&format!("              <tr id=\"punch-row-{}\" data-timecode=\"{}\" class=\"punch-table-row\">\n", idx, tc_esc));
            html.push_str(&format!("                <td class=\"tc-cell\"><a href=\"javascript:void(0)\" class=\"tc-link\" onclick=\"seekToTimecode('{}')\" style=\"color: #38bdf8; text-decoration: underline;\"><code>{}</code></a></td>\n", tc_esc, tc_esc));
            html.push_str(&format!("                <td><span class=\"badge {}\">{}</span></td>\n", badge_class, action_esc));
            html.push_str(&format!("                <td class=\"remedy-cell\">{}</td>\n", remedy_esc));
            html.push_str(&format!("                <td class=\"impact-cell\">{}</td>\n", impact_esc));
            html.push_str(&format!(
                "                <td class=\"text-center\"><button class=\"btn-copy\" style=\"margin-right: 6px; padding: 4px 8px;\" onclick=\"seekToTimecode('{}')\">▶ Seek</button><button class=\"btn-copy\" id=\"copy-btn-{}\" onclick=\"copyToClipboard(this, '{}')\">📋 Copy</button></td>\n",
                tc_esc, idx, copy_esc
            ));
            html.push_str("              </tr>\n");
        }

        if rendered_cues == 0 {
            html.push_str("              <tr>\n");
            html.push_str("                <td colspan=\"5\" class=\"text-center all-clear-cell\">\n");
            html.push_str("                  <span class=\"badge badge-green\">✅ ALL CLEAR</span>\n");
            html.push_str("                  <span class=\"all-clear-text\">No critical timeline trimming or pacing friction detected. Video passes all active editorial gates.</span>\n");
            html.push_str("                </td>\n");
            html.push_str("              </tr>\n");
        }

        html.push_str("            </tbody>\n");
        html.push_str("          </table>\n");
        html.push_str("        </div>\n");
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Component D: Cross-Device Truncation Radar & Packaging Suite
        html.push_str("    <section class=\"dashboard-section\">\n");
        html.push_str("      <div class=\"section-header\">\n");
        html.push_str("        <h2>📱 Cross-Device Packaging & Truncation Radar</h2>\n");
        html.push_str("        <span class=\"section-subtitle\">Platform viewport simulations & high-converting title upgrades</span>\n");
        html.push_str("      </div>\n");

        if let Some(radar) = &flight_sheet.truncation_radar {
            html.push_str("      <div class=\"device-preview-grid\">\n");

            // Device 1: Mobile Push Notification (45 chars)
            let push_truncated = radar.mobile_push_truncated;
            let push_tag = if push_truncated { "<span class=\"status-tag tag-warning\">⚠️ TRUNCATED</span>" } else { "<span class=\"status-tag tag-safe\">SAFE</span>" };
            html.push_str("        <div class=\"device-card\">\n");
            html.push_str("          <div class=\"device-header\">\n");
            html.push_str("            <span class=\"device-name\">📲 Mobile Push Notification</span>\n");
            html.push_str(&format!("            <span class=\"device-limit\">45 chars limit {}</span>\n", push_tag));
            html.push_str("          </div>\n");
            html.push_str("          <div class=\"device-mockup push-mockup\">\n");
            html.push_str("            <div class=\"push-app-bar\"><span class=\"app-dot\"></span> YouTube • now</div>\n");
            html.push_str(&format!("            <div class=\"push-title\">{}</div>\n", escape_html(&radar.mobile_push_preview)));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            // Device 2: Mobile / CTV Home Feed (52 chars)
            let feed_truncated = radar.mobile_feed_ctv_truncated;
            let feed_tag = if feed_truncated { "<span class=\"status-tag tag-warning\">⚠️ TRUNCATED ON CTV</span>" } else { "<span class=\"status-tag tag-safe\">SAFE</span>" };
            html.push_str("        <div class=\"device-card\">\n");
            html.push_str("          <div class=\"device-header\">\n");
            html.push_str("            <span class=\"device-name\">📺 Mobile & CTV Home Feed</span>\n");
            html.push_str(&format!("            <span class=\"device-limit\">52 chars limit {}</span>\n", feed_tag));
            html.push_str("          </div>\n");
            html.push_str("          <div class=\"device-mockup feed-mockup\">\n");
            html.push_str("            <div class=\"mock-thumb-box\"><span class=\"mock-play\">▶</span></div>\n");
            html.push_str(&format!("            <div class=\"feed-title\">{}</div>\n", escape_html(&radar.mobile_feed_ctv_preview)));
            html.push_str(&format!("            <div class=\"feed-sub\">{} • 124K views</div>\n", channel_esc));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            // Device 3: Desktop Search Result (60 chars)
            let desktop_truncated = radar.desktop_search_truncated;
            let desktop_tag = if desktop_truncated { "<span class=\"status-tag tag-warning\">⚠️ TRUNCATED</span>" } else { "<span class=\"status-tag tag-safe\">SAFE</span>" };
            html.push_str("        <div class=\"device-card\">\n");
            html.push_str("          <div class=\"device-header\">\n");
            html.push_str("            <span class=\"device-name\">💻 Desktop Search Snippet</span>\n");
            html.push_str(&format!("            <span class=\"device-limit\">60 chars limit {}</span>\n", desktop_tag));
            html.push_str("          </div>\n");
            html.push_str("          <div class=\"device-mockup search-mockup\">\n");
            html.push_str(&format!("            <div class=\"search-title\">{}</div>\n", escape_html(&radar.desktop_search_preview)));
            html.push_str(&format!("            <div class=\"search-desc\">{} explores {} with in-depth analysis...</div>\n", channel_esc, title_esc));
            html.push_str("          </div>\n");
            html.push_str("        </div>\n");

            html.push_str("      </div>\n\n");
        }

        // Packaging A/B Title & Hook Upgrade Cards
        let pkg = &flight_sheet.packaging_upgrades;
        html.push_str("      <div class=\"card packaging-upgrade-card\">\n");
        html.push_str("        <div class=\"card-title\">📦 PACKAGING A/B UPGRADES (ZERO RAW SLOT-STUFFING)</div>\n");
        html.push_str("        <div class=\"upgrades-grid\">\n");

        // Current Title
        html.push_str("          <div class=\"upgrade-item\">\n");
        html.push_str("            <span class=\"upgrade-tag tag-current\">CURRENT TITLE</span>\n");
        html.push_str(&format!("            <div class=\"upgrade-text\">{}</div>\n", escape_html(&pkg.current_title)));
        html.push_str("          </div>\n");

        // Upgrade A
        let copy_a_esc = escape_html(&pkg.title_upgrade_a);
        html.push_str("          <div class=\"upgrade-item item-highlight\">\n");
        html.push_str("            <div class=\"upgrade-header-row\">\n");
        html.push_str("              <span class=\"upgrade-tag tag-a\">UPGRADE A (CONCRETE SCALE / ANCHOR)</span>\n");
        html.push_str(&format!("              <button class=\"btn-copy\" onclick=\"copyToClipboard(this, '{}')\">📋 Copy</button>\n", copy_a_esc));
        html.push_str("            </div>\n");
        html.push_str(&format!("            <div class=\"upgrade-text\">{}</div>\n", escape_html(&pkg.title_upgrade_a)));
        html.push_str("          </div>\n");

        // Upgrade B
        let copy_b_esc = escape_html(&pkg.title_upgrade_b);
        html.push_str("          <div class=\"upgrade-item item-highlight\">\n");
        html.push_str("            <div class=\"upgrade-header-row\">\n");
        html.push_str("              <span class=\"upgrade-tag tag-b\">UPGRADE B (HIGH-STAKES CURIOSITY GAP)</span>\n");
        html.push_str(&format!("              <button class=\"btn-copy\" onclick=\"copyToClipboard(this, '{}')\">📋 Copy</button>\n", copy_b_esc));
        html.push_str("            </div>\n");
        html.push_str(&format!("            <div class=\"upgrade-text\">{}</div>\n", escape_html(&pkg.title_upgrade_b)));
        html.push_str("          </div>\n");

        // Hook Opening Upgrade
        let copy_hook_esc = escape_html(&pkg.opening_line_upgraded);
        html.push_str("          <div class=\"upgrade-item item-highlight full-width\">\n");
        html.push_str("            <div class=\"upgrade-header-row\">\n");
        html.push_str("              <span class=\"upgrade-tag tag-hook\">4 Ps DIRECT ENTRY HOOK (FIRST 5 SECONDS)</span>\n");
        html.push_str(&format!("              <button class=\"btn-copy\" onclick=\"copyToClipboard(this, '{}')\">📋 Copy Hook</button>\n", copy_hook_esc));
        html.push_str("            </div>\n");
        html.push_str(&format!("            <div class=\"upgrade-text hook-quote\">{}</div>\n", escape_html(&pkg.opening_line_upgraded)));
        html.push_str("          </div>\n");

        html.push_str("        </div>\n");
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Component E: Cumulative Cuttable Fat & Digital Punch-Ins
        if let Some(fat) = &flight_sheet.cuttable_fat {
            html.push_str("    <section class=\"dashboard-section\">\n");
            html.push_str("      <div class=\"section-header\">\n");
            html.push_str("        <h2>✂️ Cumulative Cuttable Fat & 115% Punch-In Budget</h2>\n");
            html.push_str("        <span class=\"section-subtitle\">Micro-pause shaving & talking-head freeze elimination</span>\n");
            html.push_str("      </div>\n");
            html.push_str("      <div class=\"card fat-summary-card\">\n");
            html.push_str(&format!("        <div class=\"fat-executive-lead\">{}</div>\n", escape_html(&fat.executive_summary)));
            html.push_str("        <div class=\"fat-metrics-grid\">\n");
            html.push_str(&format!("          <div class=\"fat-box\"><span class=\"fat-num\">{:.1}s</span><span class=\"fat-lbl\">Total Shavable Runtime</span></div>\n", fat.total_cuttable_seconds));
            html.push_str(&format!("          <div class=\"fat-box\"><span class=\"fat-num\">{}</span><span class=\"fat-lbl\">Micro-Edits Required</span></div>\n", fat.micro_edits_count));
            html.push_str(&format!("          <div class=\"fat-box\"><span class=\"fat-num\">{:.1}s</span><span class=\"fat-lbl\">Excess Hold Stagnation</span></div>\n", fat.camera_hold_fat_seconds));
            html.push_str(&format!("          <div class=\"fat-box\"><span class=\"fat-num\">{:.1}s</span><span class=\"fat-lbl\">Dead Air / Breath Pauses</span></div>\n", fat.dead_air_breath_seconds));
            html.push_str("        </div>\n");

            if !fat.punch_in_cues.is_empty() {
                html.push_str("        <div class=\"punch-in-cues-section\">\n");
                html.push_str("          <div class=\"sub-title\">🎯 Midpoint 115% Digital Punch-In Markers</div>\n");
                html.push_str("          <div class=\"cues-list\">\n");
                for cue in &fat.punch_in_cues {
                    let copy_cue_esc = escape_html(&format!("{}: 115% Punch-In - {}", cue.timecode, cue.edit_instruction));
                    html.push_str("            <div class=\"cue-item\">\n");
                    html.push_str(&format!("              <span class=\"cue-tc\"><code>{}</code></span>\n", escape_html(&cue.timecode)));
                    html.push_str(&format!("              <span class=\"cue-scale\">{}% Scale</span>\n", cue.crop_scale_pct));
                    html.push_str(&format!("              <span class=\"cue-dur\">Take: {:.1}s</span>\n", cue.hold_duration_seconds));
                    html.push_str(&format!("              <span class=\"cue-desc\">{}</span>\n", escape_html(&cue.edit_instruction)));
                    html.push_str(&format!("              <button class=\"btn-copy\" onclick=\"copyToClipboard(this, '{}')\">📋</button>\n", copy_cue_esc));
                    html.push_str("            </div>\n");
                }
                html.push_str("          </div>\n");
                html.push_str("        </div>\n");
            }
            html.push_str("      </div>\n");
            html.push_str("    </section>\n\n");
        }

        // Component F: 6-Vector Telemetry Matrix Audit Table
        html.push_str("    <section class=\"dashboard-section\">\n");
        html.push_str("      <div class=\"section-header\">\n");
        html.push_str("        <h2>📊 6-Vector Telemetry Matrix Audit</h2>\n");
        html.push_str("        <span class=\"section-subtitle\">The Brailsford marginal gains compounding engine</span>\n");
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"card table-card\">\n");
        html.push_str("        <div class=\"table-responsive\">\n");
        html.push_str("          <table class=\"punch-list-table telemetry-matrix-table\">\n");
        html.push_str("            <thead>\n");
        html.push_str("              <tr>\n");
        html.push_str("                <th>VECTOR</th>\n");
        html.push_str("                <th>MEASURED</th>\n");
        html.push_str("                <th>TARGET BENCHMARK</th>\n");
        html.push_str("                <th>SCORE</th>\n");
        html.push_str("                <th>STATUS</th>\n");
        html.push_str("                <th>ACTIONABLE 1% PRESCRIPTION</th>\n");
        html.push_str("                <th>EST. LIFT</th>\n");
        html.push_str("              </tr>\n");
        html.push_str("            </thead>\n");
        html.push_str("            <tbody>\n");

        for v in &flight_sheet.telemetry_matrix {
            let label_esc = escape_html(v.human_label());
            let target_esc = escape_html(&v.editor_target());
            let fix_esc = escape_html(&v.one_percent_fix);
            let badge_class = match v.status {
                crate::math::marginal_gains::VectorStatus::Elite => "badge-green",
                crate::math::marginal_gains::VectorStatus::Optimal => "badge-cyan",
                crate::math::marginal_gains::VectorStatus::MarginalDrag => "badge-orange",
                crate::math::marginal_gains::VectorStatus::CriticalFriction => "badge-red",
            };

            html.push_str("              <tr>\n");
            html.push_str(&format!("                <td class=\"col-vector\"><strong>{}</strong></td>\n", label_esc));
            html.push_str(&format!("                <td class=\"col-measured\"><strong>{:.1}{}</strong></td>\n", v.measured_value, escape_html(&v.unit)));
            html.push_str(&format!("                <td class=\"col-target\">{}</td>\n", target_esc));
            html.push_str(&format!("                <td class=\"col-score\"><strong>{:.0}</strong></td>\n", v.score_0_to_100));
            html.push_str(&format!("                <td><span class=\"badge {}\">{}</span></td>\n", badge_class, escape_html(v.status.badge())));
            html.push_str(&format!("                <td class=\"col-fix\">{}</td>\n", fix_esc));
            html.push_str(&format!("                <td class=\"col-lift\"><strong>+{:.1}%</strong></td>\n", v.estimated_retention_lift_pct));
            html.push_str("              </tr>\n");
        }

        html.push_str("            </tbody>\n");
        html.push_str("          </table>\n");
        html.push_str("        </div>\n");
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Footer
        html.push_str("    <footer class=\"dashboard-footer\">\n");
        html.push_str("      <div>Generated by <strong>Sandsifter v0.5.0</strong> // High-Performance Social Media Outlier &amp; Video Telemetry Engine</div>\n");
        html.push_str("      <div class=\"footer-sub\">Advertiser-Friendly Pre-Flight Gate • YouTube Shorts Mobile Retention Engine • Zero-disk EBU R128 Mastering</div>\n");
        html.push_str("    </footer>\n");

        html.push_str("  </div>\n\n"); // End .dashboard-container

        // Vanilla JavaScript for clipboard interactions
        html.push_str("  <script>\n");
        html.push_str(&format!("    var totalDurationSec = {:.1};\n", duration_sec));
        html.push_str("    var ytIframe = document.getElementById('report-youtube-iframe');\n");
        html.push_str("    var localVideo = document.getElementById('report-local-video');\n");
        html.push_str("    var svg = document.getElementById('report-retention-svg');\n");
        html.push_str("    var hitbox = document.getElementById('report-scrubber-hitbox');\n");
        html.push_str("    var playheadLine = document.getElementById('report-playhead-line');\n");
        html.push_str("    var playheadThumb = document.getElementById('report-playhead-thumb');\n");
        html.push_str("    var timecodeBadge = document.getElementById('report-timecode-badge');\n");
        html.push_str("    var tooltip = document.getElementById('report-scrubber-tooltip');\n");
        html.push_str("    var isDragging = false;\n\n");
        html.push_str("    function formatTime(sec) {\n");
        html.push_str("      var total = Math.max(0, Math.round(sec));\n");
        html.push_str("      var m = Math.floor(total / 60);\n");
        html.push_str("      var s = total % 60;\n");
        html.push_str("      return (m < 10 ? '0' : '') + m + ':' + (s < 10 ? '0' : '') + s;\n");
        html.push_str("    }\n\n");
        html.push_str("    function parseTimecode(tc) {\n");
        html.push_str("      if (!tc) return 0;\n");
        html.push_str("      var clean = tc.split('-')[0].trim();\n");
        html.push_str("      var parts = clean.split(':').map(Number);\n");
        html.push_str("      if (parts.length === 2 && !isNaN(parts[0]) && !isNaN(parts[1])) return parts[0] * 60 + parts[1];\n");
        html.push_str("      if (parts.length === 3 && !isNaN(parts[0]) && !isNaN(parts[1]) && !isNaN(parts[2])) return parts[0] * 3600 + parts[1] * 60 + parts[2];\n");
        html.push_str("      return 0;\n");
        html.push_str("    }\n\n");
        html.push_str("    function seekPlayer(sec) {\n");
        html.push_str("      var clamped = Math.max(0, Math.min(totalDurationSec, sec));\n");
        html.push_str("      if (localVideo) { localVideo.currentTime = clamped; }\n");
        html.push_str("      if (ytIframe && ytIframe.contentWindow) {\n");
        html.push_str("        try {\n");
        html.push_str("          ytIframe.contentWindow.postMessage(JSON.stringify({\n");
        html.push_str("            event: 'command',\n");
        html.push_str("            func: 'seekTo',\n");
        html.push_str("            args: [clamped, true]\n");
        html.push_str("          }), '*');\n");
        html.push_str("        } catch(e) {}\n");
        html.push_str("      }\n");
        html.push_str("      updatePlayhead(clamped / totalDurationSec, clamped);\n");
        html.push_str("    }\n\n");
        html.push_str("    function updatePlayhead(pct, sec) {\n");
        html.push_str("      var clampedPct = Math.max(0, Math.min(1, pct));\n");
        html.push_str("      var x = clampedPct * 1000;\n");
        html.push_str("      if (playheadLine) {\n");
        html.push_str("        playheadLine.setAttribute('x1', x.toFixed(1));\n");
        html.push_str("        playheadLine.setAttribute('x2', x.toFixed(1));\n");
        html.push_str("      }\n");
        html.push_str("      if (playheadThumb) {\n");
        html.push_str("        playheadThumb.setAttribute('transform', 'translate(' + x.toFixed(1) + ', 0)');\n");
        html.push_str("      }\n");
        html.push_str("      if (timecodeBadge) {\n");
        html.push_str("        timecodeBadge.textContent = formatTime(sec) + ' / ' + formatTime(totalDurationSec);\n");
        html.push_str("      }\n");
        html.push_str("    }\n\n");
        html.push_str("    function seekToTimecode(tc) {\n");
        html.push_str("      var sec = parseTimecode(tc);\n");
        html.push_str("      seekPlayer(sec);\n");
        html.push_str("      highlightPunchRow(tc, sec);\n");
        html.push_str("    }\n\n");
        html.push_str("    function highlightPunchRow(tc, sec) {\n");
        html.push_str("      var rows = document.querySelectorAll('.punch-table-row');\n");
        html.push_str("      rows.forEach(function(r) { r.classList.remove('punch-row-highlight'); });\n");
        html.push_str("      var targetRow = null;\n");
        html.push_str("      rows.forEach(function(r) {\n");
        html.push_str("        var rowTc = r.getAttribute('data-timecode') || '';\n");
        html.push_str("        var rowSec = parseTimecode(rowTc);\n");
        html.push_str("        if (rowTc === tc || Math.abs(rowSec - sec) <= 3.0) {\n");
        html.push_str("          if (!targetRow) targetRow = r;\n");
        html.push_str("        }\n");
        html.push_str("      });\n");
        html.push_str("      if (targetRow) {\n");
        html.push_str("        targetRow.classList.add('punch-row-highlight');\n");
        html.push_str("        targetRow.scrollIntoView({ behavior: 'smooth', block: 'nearest' });\n");
        html.push_str("      }\n");
        html.push_str("    }\n\n");
        html.push_str("    if (hitbox && svg) {\n");
        html.push_str("      function handleScrub(e) {\n");
        html.push_str("        var rect = svg.getBoundingClientRect();\n");
        html.push_str("        var pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));\n");
        html.push_str("        seekPlayer(pct * totalDurationSec);\n");
        html.push_str("      }\n");
        html.push_str("      hitbox.addEventListener('mousedown', function(e) {\n");
        html.push_str("        isDragging = true;\n");
        html.push_str("        handleScrub(e);\n");
        html.push_str("      });\n");
        html.push_str("      window.addEventListener('mousemove', function(e) {\n");
        html.push_str("        if (isDragging) handleScrub(e);\n");
        html.push_str("      });\n");
        html.push_str("      window.addEventListener('mouseup', function() { isDragging = false; });\n");
        html.push_str("      hitbox.addEventListener('mousemove', function(e) {\n");
        html.push_str("        if (!tooltip) return;\n");
        html.push_str("        var rect = svg.getBoundingClientRect();\n");
        html.push_str("        var pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));\n");
        html.push_str("        tooltip.style.left = (e.clientX - rect.left) + 'px';\n");
        html.push_str("        tooltip.style.top = (e.clientY - rect.top) + 'px';\n");
        html.push_str("        tooltip.innerHTML = '<strong>⏱️ ' + formatTime(pct * totalDurationSec) + '</strong> (Click to scrub)';\n");
        html.push_str("        tooltip.classList.add('visible');\n");
        html.push_str("      });\n");
        html.push_str("      hitbox.addEventListener('mouseleave', function() {\n");
        html.push_str("        if (tooltip) tooltip.classList.remove('visible');\n");
        html.push_str("      });\n");
        html.push_str("      document.querySelectorAll('.breach-marker').forEach(function(marker) {\n");
        html.push_str("        marker.addEventListener('click', function(e) {\n");
        html.push_str("          e.stopPropagation();\n");
        html.push_str("          var sec = parseFloat(marker.getAttribute('data-time'));\n");
        html.push_str("          seekPlayer(sec);\n");
        html.push_str("          var tc = marker.getAttribute('data-tc');\n");
        html.push_str("          highlightPunchRow(tc, sec);\n");
        html.push_str("        });\n");
        html.push_str("        marker.addEventListener('mouseenter', function(e) {\n");
        html.push_str("          if (!tooltip) return;\n");
        html.push_str("          var rect = svg.getBoundingClientRect();\n");
        html.push_str("          var mRect = marker.getBoundingClientRect();\n");
        html.push_str("          tooltip.style.left = (mRect.left - rect.left + mRect.width / 2) + 'px';\n");
        html.push_str("          tooltip.style.top = (mRect.top - rect.top) + 'px';\n");
        html.push_str("          tooltip.innerHTML = '<div style=\"font-weight: 700; color: #ef4444;\">🚨 ' + (marker.getAttribute('data-cause') || 'Retention Cliff') + ' (' + marker.getAttribute('data-tc') + ')</div><div>Drop Rate: <strong>' + marker.getAttribute('data-rprime') + '%/s</strong></div><div style=\"color: #94a3b8; font-size: 10px;\">💡 ' + marker.getAttribute('data-remedy') + '</div>';\n");
        html.push_str("          tooltip.classList.add('visible');\n");
        html.push_str("        });\n");
        html.push_str("        marker.addEventListener('mouseleave', function() {\n");
        html.push_str("          if (tooltip) tooltip.classList.remove('visible');\n");
        html.push_str("        });\n");
        html.push_str("      });\n");
        html.push_str("    }\n\n");
        html.push_str("    if (localVideo) {\n");
        html.push_str("      localVideo.addEventListener('timeupdate', function() {\n");
        html.push_str("        if (!isDragging && totalDurationSec > 0) {\n");
        html.push_str("          var cur = localVideo.currentTime;\n");
        html.push_str("          updatePlayhead(cur / totalDurationSec, cur);\n");
        html.push_str("        }\n");
        html.push_str("      });\n");
        html.push_str("    }\n\n");
        html.push_str("    function copyToClipboard(button, text) {\n");
        html.push_str("      if (navigator.clipboard && window.isSecureContext) {\n");
        html.push_str("        navigator.clipboard.writeText(text).then(function() {\n");
        html.push_str("          showCopySuccess(button);\n");
        html.push_str("        }).catch(function() {\n");
        html.push_str("          fallbackCopy(button, text);\n");
        html.push_str("        });\n");
        html.push_str("      } else {\n");
        html.push_str("        fallbackCopy(button, text);\n");
        html.push_str("      }\n");
        html.push_str("    }\n");
        html.push_str("    function fallbackCopy(button, text) {\n");
        html.push_str("      var textArea = document.createElement('textarea');\n");
        html.push_str("      textArea.value = text;\n");
        html.push_str("      textArea.style.position = 'fixed';\n");
        html.push_str("      textArea.style.left = '-999999px';\n");
        html.push_str("      document.body.appendChild(textArea);\n");
        html.push_str("      textArea.focus();\n");
        html.push_str("      textArea.select();\n");
        html.push_str("      try { document.execCommand('copy'); showCopySuccess(button); } catch(err) {}\n");
        html.push_str("      document.body.removeChild(textArea);\n");
        html.push_str("    }\n");
        html.push_str("    function showCopySuccess(button) {\n");
        html.push_str("      var orig = button.innerHTML;\n");
        html.push_str("      button.innerHTML = '✅ Copied!';\n");
        html.push_str("      button.classList.add('copied');\n");
        html.push_str("      setTimeout(function() {\n");
        html.push_str("        button.innerHTML = orig;\n");
        html.push_str("        button.classList.remove('copied');\n");
        html.push_str("      }, 2000);\n");
        html.push_str("    }\n");
        html.push_str("  </script>\n");

        html.push_str("</body>\n</html>\n");

        html
    }

    /// Saves the generated HTML report to `output/<slug>_report.html` (or `_draft_report.html`).
    pub fn save(
        flight_sheet: &FlightSheet,
        scorecard: &MarginalGainsScorecard,
        output_dir: &Path,
    ) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let slug = slugify(&flight_sheet.video_title);
        let suffix = if flight_sheet.is_local_draft { "_draft" } else { "" };
        let filename = format!("{}{}_report.html", slug, suffix);
        let target_path = output_dir.join(filename);
        let content = Self::generate(flight_sheet, scorecard);
        std::fs::write(&target_path, content)?;
        Ok(target_path)
    }
}
