use std::path::{Path, PathBuf};
use crate::export::flight_sheet::slugify;
use crate::export::html_report::escape_html;
use crate::pipeline::script_generator::PredictiveScript;

/// 1-Page Pre-Production Shoot Sheet Generator.
/// Emits a clean, high-contrast, printable HTML document styled to fit on one 8.5x11 page.
pub struct ShootSheetGenerator;

impl ShootSheetGenerator {
    /// Generates a standalone, printable 1-page pre-production shoot sheet from a PredictiveScript.
    pub fn generate(script: &PredictiveScript) -> String {
        let topic_esc = escape_html(&script.topic);
        let channel_esc = escape_html(&script.channel_handle);
        let moment_esc = escape_html(&script.moment_sentence);
        let hook_madlib_esc = escape_html(&script.selected_hook_madlib);

        let mut html = String::with_capacity(16384);

        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"UTF-8\">\n");
        html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str(&format!("  <title>Shoot Sheet - {}</title>\n", topic_esc));
        html.push_str("  <style>\n");
        html.push_str(include_str!("shoot_styles.css"));
        html.push_str("  </style>\n</head>\n<body>\n");

        html.push_str("  <div class=\"sheet-container\">\n");

        // Sheet Header
        html.push_str("    <header class=\"sheet-header\">\n");
        html.push_str("      <div class=\"header-left\">\n");
        html.push_str("        <span class=\"badge-doc\">PRE-PRODUCTION SHOOT SHEET</span>\n");
        html.push_str(&format!("        <h1 class=\"sheet-title\">{}</h1>\n", topic_esc));
        html.push_str(&format!("        <div class=\"sheet-meta\">Host / Channel: <strong>{}</strong> • Planned Duration: <strong>{:.0}s</strong> • Target ASL: <strong>{:.1}s</strong></div>\n",
            channel_esc, script.total_duration_sec, script.average_shot_length));
        html.push_str("      </div>\n");
        html.push_str("      <div class=\"header-right no-print\">\n");
        html.push_str("        <button class=\"btn-print\" onclick=\"window.print()\">🖨️ Print 1-Page PDF</button>\n");
        html.push_str("      </div>\n");
        html.push_str("    </header>\n\n");

        // Section 1: Peak Moment Sentence & On-Set Photo Checklist
        html.push_str("    <section class=\"sheet-section\">\n");
        html.push_str("      <div class=\"section-title-bar\">\n");
        html.push_str("        <span class=\"section-number\">1</span>\n");
        html.push_str("        <h2>ON-SET PHOTO CHECKLIST (4 EXTREME OPPOSITES)</h2>\n");
        html.push_str("      </div>\n");

        if !script.moment_sentence.is_empty() {
            html.push_str("      <div class=\"hero-moment-box\">\n");
            html.push_str("        <span class=\"moment-tag\">THE PEAK MOMENT SENTENCE</span>\n");
            html.push_str(&format!("        <p class=\"moment-quote\">\"{}\"</p>\n", moment_esc));
            html.push_str("      </div>\n");
        }

        html.push_str("      <div class=\"concepts-grid\">\n");
        for concept in &script.divergent_thumbnail_concepts {
            let emoji = concept.archetype.emoji_letter();
            let label = concept.archetype.label();
            let name_esc = escape_html(&concept.concept_name);
            let focal_esc = escape_html(&concept.focal_point);
            let overlay_esc = escape_html(&concept.text_overlay);
            let shoot_checklist_esc = escape_html(&concept.shoot_on_set_checklist);

            html.push_str("        <div class=\"concept-card\">\n");
            html.push_str("          <div class=\"concept-header\">\n");
            html.push_str("            <label class=\"checkbox-label\">\n");
            html.push_str("              <input type=\"checkbox\" class=\"on-set-check\">\n");
            html.push_str(&format!("              <span class=\"concept-badge\">{} {}</span>\n", emoji, label));
            html.push_str("            </label>\n");
            html.push_str("          </div>\n");
            html.push_str(&format!("          <div class=\"concept-name\">{}</div>\n", name_esc));
            html.push_str(&format!("          <div class=\"concept-row\"><span class=\"ck\">Focal Point:</span> <span class=\"cv\">{}</span></div>\n", focal_esc));
            html.push_str(&format!("          <div class=\"concept-row\"><span class=\"ck\">Text Overlay:</span> <span class=\"cv highlight-text\">\"{}\"</span></div>\n", overlay_esc));
            html.push_str(&format!("          <div class=\"concept-checklist\"><span class=\"cl-icon\">📷</span> {}</div>\n", shoot_checklist_esc));
            html.push_str("        </div>\n");
        }
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Section 2: The 4 Ps Opening Shot Clock (0.0s - 10.0s)
        html.push_str("    <section class=\"sheet-section\">\n");
        html.push_str("      <div class=\"section-title-bar\">\n");
        html.push_str("        <span class=\"section-number\">2</span>\n");
        html.push_str("        <h2>THE 4 Ps OPENING SHOT CLOCK (0.0s - 10.0s)</h2>\n");
        html.push_str("      </div>\n");

        html.push_str("      <div class=\"shot-clock-grid\">\n");

        let p_items = [
            ("PROMISE", "0.0s - 2.5s", "Confirm the click within 12 frames. Zero throat-clearing.", &script.four_ps.promise, "#38bdf8"),
            ("PROOF", "2.5s - 5.0s", "Show authority, physical scale, or irreversible stakes.", &script.four_ps.proof, "#22c55e"),
            ("PLAN", "5.0s - 7.5s", "Set the 3-step cognitive roadmap so viewers stay engaged.", &script.four_ps.plan, "#f59e0b"),
            ("PERSONA", "7.5s - 10.0s", "Call out the exact viewer avatar before the 10s gate.", &script.four_ps.persona, "#a855f7"),
        ];

        for (p_name, p_time, p_goal, p_desc, p_color) in p_items {
            html.push_str("        <div class=\"shot-clock-step\">\n");
            html.push_str("          <div class=\"step-header\">\n");
            html.push_str(&format!("            <span class=\"step-name\" style=\"color: {};\">{}</span>\n", p_color, p_name));
            html.push_str(&format!("            <span class=\"step-tc\"><code>{}</code></span>\n", p_time));
            html.push_str("          </div>\n");
            html.push_str(&format!("          <div class=\"step-goal\">{}</div>\n", p_goal));
            html.push_str(&format!("          <div class=\"step-dialogue\">\"{}\"</div>\n", escape_html(p_desc)));
            html.push_str("        </div>\n");
        }
        html.push_str("      </div>\n");

        // First 4 opening beats table
        if !script.beats.is_empty() {
            html.push_str("      <div class=\"opening-beats-table-wrap\">\n");
            html.push_str("        <table class=\"beats-table\">\n");
            html.push_str("          <thead>\n");
            html.push_str("            <tr>\n");
            html.push_str("              <th>TIMECODE</th>\n");
            html.push_str("              <th>BEAT ROLE</th>\n");
            html.push_str("              <th>SPOKEN DIALOGUE (TARGET: &le; 9 WORDS)</th>\n");
            html.push_str("              <th>WPM</th>\n");
            html.push_str("              <th>CAMERA DIRECTIVE</th>\n");
            html.push_str("            </tr>\n");
            html.push_str("          </thead>\n");
            html.push_str("          <tbody>\n");

            for beat in script.beats.iter().take(4) {
                let role_label = beat.four_p_role.map(|p| p.label()).unwrap_or(beat.beat_type.label());
                let dialogue_esc = escape_html(&beat.spoken_dialogue);
                let visual_esc = escape_html(&beat.visual_directive);

                html.push_str("            <tr>\n");
                html.push_str(&format!("              <td><code>{:.1}s - {:.1}s</code></td>\n", beat.start_sec, beat.end_sec));
                html.push_str(&format!("              <td><span class=\"role-tag\">{}</span></td>\n", role_label));
                html.push_str(&format!("              <td class=\"dialogue-cell\">\"{}\"</td>\n", dialogue_esc));
                html.push_str(&format!("              <td class=\"wpm-cell\">{:.0}</td>\n", beat.computed_wpm));
                html.push_str(&format!("              <td class=\"visual-cell\">{}</td>\n", visual_esc));
                html.push_str("            </tr>\n");
            }

            html.push_str("          </tbody>\n");
            html.push_str("        </table>\n");
            html.push_str("      </div>\n");
        }
        html.push_str("    </section>\n\n");

        // Section 3: Title Acceptance Criteria Checklist
        html.push_str("    <section class=\"sheet-section\">\n");
        html.push_str("      <div class=\"section-title-bar\">\n");
        html.push_str("        <span class=\"section-number\">3</span>\n");
        html.push_str("        <h2>TITLE ACCEPTANCE CRITERIA (DO NOT WRAP FILMING UNTIL CHECKED)</h2>\n");
        html.push_str("      </div>\n");

        html.push_str("      <div class=\"criteria-list\">\n");
        for crit in &script.acceptance_criteria.criteria {
            let cond_esc = escape_html(&crit.condition);
            let beat_str = crit.fulfilled_at_beat.map(|b| format!("(Verified @ Beat {})", b + 1)).unwrap_or_default();

            html.push_str("        <div class=\"criteria-item\">\n");
            html.push_str("          <label class=\"checkbox-label\">\n");
            html.push_str("            <input type=\"checkbox\" class=\"on-set-check\">\n");
            html.push_str(&format!("            <span class=\"crit-number\">CRITERION #{:02}</span>\n", crit.id));
            html.push_str(&format!("            <span class=\"crit-text\">{}</span>\n", cond_esc));
            if !beat_str.is_empty() {
                html.push_str(&format!("            <span class=\"crit-beat\">{}</span>\n", beat_str));
            }
            html.push_str("          </label>\n");
            html.push_str("        </div>\n");
        }
        html.push_str("      </div>\n");
        html.push_str("    </section>\n\n");

        // Sheet Footer
        html.push_str("    <footer class=\"sheet-footer\">\n");
        html.push_str(&format!("      <span>Selected Hook Formula: <em>{}</em></span>\n", hook_madlib_esc));
        html.push_str("      <span>Sandsifter Pre-Production Suite • 100% Pacing Compliance</span>\n");
        html.push_str("    </footer>\n");

        html.push_str("  </div>\n"); // End .sheet-container
        html.push_str("</body>\n</html>\n");

        html
    }

    /// Saves the generated shoot sheet to `output/<slug>_shoot_sheet.html`.
    pub fn save(script: &PredictiveScript, output_dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(output_dir)?;
        let slug = slugify(&script.topic);
        let filename = format!("{}_shoot_sheet.html", slug);
        let target_path = output_dir.join(filename);
        let content = Self::generate(script);
        std::fs::write(&target_path, content)?;
        Ok(target_path)
    }
}
