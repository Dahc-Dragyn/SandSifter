# 🏖️ Sandsifter

> **Autonomous Video Intelligence & Pre-Publish NLE Telemetry Engine in 100% Native Rust.**  
> *Sandsifter decompiles the physics of viral video. By fusing sub-second acoustic speech kinematics, in-memory visual rhythm detection, audience retention derivatives ($R'(t)$), and Sir Dave Brailsford's Marginal Gains theory, Sandsifter audits video drafts before publishing and exports non-stacking NLE timeline markers directly into DaVinci Resolve, Final Cut Pro, and Premiere Pro.*

---

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-2024--11--05-blue.svg)](https://modelcontextprotocol.io)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Zero Disk Overhead](https://img.shields.io/badge/IO-Zero--Disk%20Streaming-brightgreen.svg)]()

### What is Sandsifter?
Traditional YouTube analytics tell you *what* happened days after you publish, when it's already too late. **Sandsifter tells you *why* before you hit publish.** 

Operating either as a standalone CLI, an automated watch-folder daemon, or a native **Model Context Protocol (MCP)** server for AI agent workflows (Antigravity, Claude Desktop), Sandsifter evaluates videos across 6 physical telemetry dimensions:
1. **Cold-Open Latency & Audio Decoupling**: Measures vocal delivery delay ($t_{\text{silence}}$) down to the millisecond.
2. **Visual Rhythm & Average Shot Length (ASL)**: Streams video directly through in-memory FFmpeg scene filters with zero temporary disk writes to measure shot pacing.
3. **Static Stagnation & Freeze Ceilings**: Flags extended camera holds that induce viewer fatigue and prescribes 115% digital punch-ins.
4. **Retention Derivative Modeling**: Calculates the first derivative of audience attention ($R'(t)$) to isolate mid-video drop-off cliffs and rewatch surges.
5. **Acoustic Cadence & Breath-Tightening**: Calculates active speaking WPM and tallies cuttable dead-air fat across phrases.
6. **Packaging-to-Story Handshake**: Evaluates thumbnail/title promise gaps and generates tone-grounded title and 4 Ps opening hook upgrades.


## ⚡ Architecture Overview

```
                                ┌────────────────────────────────────────┐
                                │          Video Input Streams           │
                                │   (YouTube URLs // Local NLE Drafts)   │
                                └───────────────────┬────────────────────┘
                                                    │
                      ┌─────────────────────────────┴─────────────────────────────┐
                      ▼                                                           ▼
        ┌───────────────────────────┐                               ┌───────────────────────────┐
        │    YouTube Stream Pipe    │                               │     Local Media Bypass    │
        │  (yt-dlp // Heatmap R'(t) │                               │  (.mp4/.mov Direct Ingest │
        │  In-Memory <25MB Ceiling) │                               │  Zero Network, Zero Disk) │
        └─────────────┬─────────────┘                               └─────────────┬─────────────┘
                      │                                                           │
                      └─────────────────────────────┬─────────────────────────────┘
                                                    │
                                                    ▼
                                      ┌───────────────────────────┐
                                      │   Acoustic & Scene VAD    │
                                      │ (Sub-second RMS VAD, WPM, │
                                      │  In-Memory Scene Cuts)    │
                                      └─────────────┬─────────────┘
                                                    │
                      ┌─────────────────────────────┴─────────────────────────────┐
                      ▼                                                           ▼
        ┌───────────────────────────┐                               ┌───────────────────────────┐
        │  Series System Reliability│                               │ Predictive Risk Calculus  │
        │ (Weakest-Link R_sys Math, │                               │ (Visual Fatigue Clifffs,  │
        │  Viewer Satisfaction S>R) │                               │  Autoplay Bounce Scoring) │
        └─────────────┬─────────────┘                               └─────────────┬─────────────┘
                      │                                                           │
                      └─────────────────────────────┬─────────────────────────────┘
                                                    │
                                                    ▼
                                      ┌───────────────────────────┐
                                      │  Editor's Flight Sheet &  │
                                      │  Apple FCPXML v1.8 Engine │
                                      │  (Non-Stacking Markers)   │
                                      └─────────────┬─────────────┘
                                                    │
                      ┌─────────────────────────────┴─────────────────────────────┐
                      ▼                                                           ▼
        ┌───────────────────────────┐                               ┌───────────────────────────┐
        │  Head-to-Head Comparison  │                               │    Native Rust MCP Core   │
        │ (--compare <win> <flop>   │                               │ (11 Tools // JSON-RPC 2.0 │
        │  Vector Delta Matrix)     │                               │  Stdio & HTTP SSE Server) │
        └───────────────────────────┘                               └───────────────────────────┘
```

---

## 🚀 Key Features

1. **Empirical Batch Ingestion & SQLite Persistence**:
   - Ingest historical creator cohorts via `--import <file.json>` with flexible JSON schema aliases (`views`/`view_count`, `likes`/`like_count`, `comments`/`comment_count`, etc.).
   - Persists records across `channels`, `videos`, and `snapshots` tables in local SQLite (`sandsifter.db`).
   - Tracks velocity and engagement changes across historical snapshots over time.

2. **Real Network Video Extraction via `yt-dlp`**:
   - Zero-dependency mock elimination: extracts canonical metadata (`title`, `uploader`, `view_count`, `like_count`, `comment_count`) via `yt-dlp --dump-json`.
   - Resolves direct CDN audio streams or pipes `yt-dlp` stdout directly into bounded memory buffers (<25MB ceiling) with **zero temporary files left on disk**.

3. **Virality Math & IQR Outlier Engine**:
   - Calculates creator-specific rolling median view baselines ($B_c$) to prevent mega-influencer skew.
   - **IQR Winsorization** ($Q_3 + 1.5 \times \text{IQR}$) clamping hyper-viral spikes so creator baselines remain statistically sound.
   - **2.0% Engagement Floor heuristic** ($(\text{Likes} + \text{Comments} + \text{Shares}) / \text{Views} < 2.0\%$) screening out paid TikTok Spark Ads and Meta boosts.

4. **Sub-3.5s Precision Audio Hook Aligner & VAD**:
   - Voice Activity Detection (VAD) with normalized Root Mean Square (RMS) energy thresholding to isolate true acoustic speech onsets.
   - Sub-second word-level timestamp alignment with Words Per Minute (WPM) cadence calculations.
   - Hard 3.5s retention boundary with word-edge truncation ($t \le 3.5\text{s}$) and grace window logic.

5. **Visual Keyframe OCR & Narrative Classifier**:
   - Sub-second keyframe extraction at retention drop timestamps ($0\text{s}, 1\text{s}, 2\text{s}, 5\text{s}$).
   - On-screen sticker and burned-in headline text detection with auditory-visual sync verification (`hook_alignment`).
   - Powered by **Gemini 2.5 Flash-Lite** (low-cost tier) to extract fill-in-the-blank `spoken_hook_madlib` formulas, `format_category`, and the `common_belief` vs. `contrarian_reality` cognitive friction pair.

6. **The Hook Machine (Kallaway Engine)**:
   - **Channel Isolation**: Evaluates videos independently per creator channel before cross-channel synthesis.
   - **Triple-Vector Scoring**:
     1. *Psychology* (Curiosity loops, status threats, competence gaps, FOMO)
     2. *Trigger Lexicon* (Specific benchmarks, named frameworks, urgency verbs)
     3. *Grammar & Cadence* (Sub-3.5s length, active voice, declarative punch)
   - **Custom Rubric Generator**: Synthesizes a tailored grading rubric based on empirical historical outliers.
   - **Hook Generator & Grader**: Produces 5 Adapted Hooks (from channel madlibs) + 5 Scratch Hooks (novel angles) ranked by composite score.

7. **Native Model Context Protocol (MCP 2024-11-05)**:
   - 11 tools available over stdio or HTTP SSE.
   - Bound directly to persistent SQLite database for empirical outlier ranking and channel intelligence.
   - Clean separation: Stdout reserved exclusively for JSON-RPC 2.0; telemetry routed to Stderr.

8. **Deep Media Forensics, LOOCV Ablation & Creator Matrix**:
   - Computes sub-3.5s acoustic delivery metrics: Pre-Speech Latency (`t_silence`) and active Speaking Pace (`wpm_0_to_35`).
   - Hard VAD gating (5% RMS energy ceiling, 60ms burst minimum) tagging silent openings as `[NON-SPEECH / AMBIENT]`, preventing LLM hallucination on b-roll.
   - Full-cohort Leave-One-Out Cross-Validation (LOOCV) backtesting proving multimodal audio lift over title-only heuristics ($\Delta\rho = +0.62$).
   - Rich Creator Transparency Matrix: exposes Actual Rank, Views, Duration (`M:SS`), and Published Date alongside Model A vs B ranks.
   - Jargon-free Tactical Playbook translating statistical wedges into practical creator rules.

9. **Retention Calculus & Inflection Engine (`--retention`)**:
   - Numerical differentiation of YouTube granular retention curves ($R'(t)$) detecting steepest audience drop-off cliffs and rewatch surges.
   - Built-in 3.0s Autoplay Grace Window eliminating misleading bounce anomalies at 00:00.
   - Multimodal correlation matching drop-offs against spoken transcript cues and visual scene cut intervals into structured root causes:
     - 🔇 *Dead Air Silence* (silence $\ge 1.5$s)
     - 🎥 *Visual Fatigue* (single shot $\ge 5.0$s)
     - ⚠️ *Compound Friction* (dead air + lingering visual shot)
     - 📉 *Narrative Plateau* (active speech and cuts, but storytelling stalled)
   - Dynamic Gemini 2.5 editing prescriptions detailing second-by-second B-roll insertions and pattern interrupts.

10. **In-Memory Streaming Visual Rhythm & ASL Scene Detector (`--pacing`)**:
    - Zero-disk streaming architecture resolving raw $<360$p stream URLs via `yt-dlp` and piping into FFmpeg's native `gt(scene,0.35)` scene detector filter.
    - Captures `pts_time` directly from stderr via in-memory buffer without writing video frames to disk.
    - Calculates Average Shot Length ($\text{ASL} = \text{Duration} / N_{\text{cuts}}$) and classifies pacing into ⚡ Rapid ($\le 2.5$s), 🎬 Standard Narrative ($2.5 - 5.0$s), and 🌿 Slow/Contemplative ($> 5.0$s).

11. **Semantic Topic Momentum & TF-IDF Trend Forecast (`--trending`)**:
    - Scrapes 7-day niche cohorts via date-sorted search URLs.
    - Computes smoothed TF-IDF, Channel Dispersion (count of unique creators using the angle), and Velocity Lift ($v_{\text{entity}} / v_{\text{cohort}}$).
    - Classifies topics into 🔥 Breakout Micro-Trends ($\ge 2.0\text{x}$ lift across $\ge 3$ channels), 🚀 Established Winners, and ⚠️ Saturated / Fading Angles.

12. **NLE Timeline Exporter: Apple FCPXML v1.8/v1.9 & Native DaVinci Resolve EDL (`--timeline`)**:
    - Generates ready-to-import NLE templates for **DaVinci Resolve**, **Final Cut Pro**, and **Adobe Premiere Pro**.
    - **Zero Timecode Drift**: Employs `RationalTime` (1001/30000s integer fractional math for 29.97fps NTSC) preventing drop-frame misalignment.
    - **No "Media Offline" Errors**: Maps detected cuts into sequential transparent `<gap>` spine elements (`Shot 01`, `Shot 02`, etc.) so creators can drag-and-drop B-roll into pre-timed pacing slots without red missing-media screens.
    - **DaVinci Resolve Native Marker EDL Workflow**:
      - *Step 1*: Import `.fcpxml` (`File` > `Import` > `Timeline...`) to set up sequence duration, framerate, and 9:16 vertical / 16:9 aspect ratio.
      - *Step 2*: In the Media Pool, right-click the timeline > `Timelines` > `Import` > `Timeline Markers from EDL...` and select the companion `.edl` file.
      - *Step 3*: Resolve's ruler and **Edit Index -> Markers** panel instantly populate with native color-coded markers:
        - 🔴 **Red Markers (`ResolveColorRed`)**: Critical Attention Breaches & retention drop cliffs with Root Cause and tactical remedy notes.
        - 🟢 **Green Markers (`ResolveColorGreen`)**: Top Replay Spikes where viewers looped or scrubbed backwards.
        - 🔵 **Cyan Markers (`ResolveColorCyan`)**: Acoustic speech onset ($t_{\text{silence}}$) for dead-air trimming.
        - 🟣 **Purple Markers (`ResolveColorPurple`)**: Hook pacing & spoken delivery velocity.
        - 🟠 **Orange Markers (`ResolveColorOrange`)**: Outro / exit pacing preservation notes.

13. **Predictive Short-Form Script Generator ("The Pull Engine") (`--script`)**:
    - Synthesizes ready-to-film, mathematically paced short-form scripts (default 36.0s / 15 beats at 2.4s ASL).
    - **The 4 Ps Opening Gate ($0.0\text{s} - 10.0\text{s}$)**: Structures initial beats to answer all 4 doubts before the drop cliff:
      - Beat 0: **Promise** (confirms title/click immediately).
      - Beat 1: **Proof** (authority, physical scale, or concrete results).
      - Beat 2: **Plan** (3-step sequence delivering the premise).
      - Beat 3: **Persona** (explicit target audience callout).
    - **Title Software Acceptance Criteria Compiler**: Decomposes title into 3–5 explicit software specifications (*"What must be true by the end of this video for this title to be true?"*), verifying that every BodyProgression beat fulfills a condition with zero narrative fluff.
    - **Mathematical Pacing Enforcement**: Enforces the 185–215 WPM opening cadence (~11-12 words in opening 3.5s) to guarantee viewers clear the initial retention drop gate.
    - **Per-Beat Word Ceilings**: Hard-caps every individual 2.4s beat to $\le 9$ words ($\le 225.0\text{ WPM}$) so creators never suffer breath-starvation during delivery.
    - **Creator Identity Anchoring (`CreatorIdentityContext`)**: Auto-detects and anchors known hosts (e.g., Jaz & Crystal for `@Vanwives`) and locked geographic settings, preventing persona ("Dave") and location hallucinations.
    - **Seamless Looping CTA**: Final beat syntactically connects into Beat 0's opening word to trigger rewatch loops and >100% retention replays.
    - **Pre-Production NLE Export**: Generates `<slug>_script_template.fcpxml` and `<slug>_script.edl` with color-coded teleprompter markers for DaVinci Resolve & Premiere Pro.
    - CLI: `cargo run -- --script "<@creator>" "<topic>" [--duration 36] [--export-timeline]`.

14. **Derivative-Gated Retention Calibration & Niche Tone Engine**:
    - **`VisualPayoffSilence` Protection**: Categorizes replay surges with low/no speech ($R'(t) > 0$) as aesthetic payoffs, safeguarding non-verbal beauty shots from algorithmic over-snipping.
    - **`NicheToneProfile` Classification**: Auto-detects genre expectations (`DocumentaryBuildProcess`, `AestheticLifestyleCozy`, `EducationalBreakdown`, `HighStakesPanic`).
    - **Grammatical Topic Sanitization**: Converts raw multi-word playlist titles into natural English noun and activity phrases.

15. **Longitudinal "Marginal Gains" Telemetry Suite**:
    - Measures Sir Dave Brailsford's 1% compounding micro-variable evolution across multi-year creator playlists.
    - Evaluates dead-air shaving ($-89.3\%$ pre-speech latency), visual cutting rhythm acceleration ($+22\%$ cuts/min), camera freeze reduction ($-68\%$ static holds), and vocal pacing maturation ($398 \rightarrow 211\text{ WPM}$).
    - Documented in [`MARGINAL_GAINS_5YR_EVOLUTION.md`](file:///c:/Antigravity%20projects/Rust/Sandsifter/MARGINAL_GAINS_5YR_EVOLUTION.md).

16. **Series System Reliability Engineering & Viewer Satisfaction Index (`--marginal-gains`)**:
    - Encodes Jake Trinder's unifying series system theorem: YouTube video production is a **Series System** ($R_{\text{sys}} = \prod_{i=1}^n p_i$), where any weak stage forms a Single Point of Failure (SPOF) that collapses overall throughput.
    - **Weakest-Link Reliability Metric ($R_{\text{sys}})**: Computes $R_{\text{sys}} = \prod p_i \times 100.0$ ($p_i = \text{clamp}(\text{score}_i / 100.0, 0.15, 1.0)$), exposing fatal bottlenecks that additive averages conceal ($2.7\%\text{ vs } 51.0\%$).
    - **Single Point of Failure (SPOF) Auto-Detector**: Automatically isolates any vector scoring $< 70/100$ (e.g. Ep 2's 22.2s camera freeze).
    - **Viewer Satisfaction Index (`ViewerSatisfactionMetrics`)**: Enforces $S > R_{\text{raw}}$ by measuring Comments per 1k views and Likes percentage to classify engagement into Diamond ($\ge 2.0\text{ comments/1k}$), Healthy, and Passive tiers.
    - **Zero-Padding Actionable Tweak Filter**: Filters out `+0.0% lift` vectors so creators only receive fixes for broken elements.
    - **Drop Cliff Timecode Tagging**: Extracts exact drop timestamps (`MM:SS`) from the retention curve derivative to point editors to the exact second of viewer abandonment.

17. **Packaging-to-Editing Expectation Alignment & Tone-Conditioned ASL Benchmarks (`--marginal-gains`)**:
    - Conditions visual cutting velocity ($\text{ASL}$) and static camera hold ceilings on the video's detected `NicheToneProfile`:
      - **High-Stakes Panic**: Elite ASL $\le 1.8\text{s}$, Target ASL $\le 2.2\text{s}$, Static Hold $\le 7.0\text{s}$.
      - **Documentary Build & Aesthetic Cozy Craft**: Elite ASL $\le 3.5\text{s}$, Target ASL $\le 5.0\text{s}$, Static Hold ceiling relaxed to $\le 12.0\text{s}$ for ambient craft and scenery immersion.
      - **Educational Breakdown**: Elite ASL $\le 4.0\text{s}$, Target ASL $\le 6.0\text{s}$, Static Hold relaxed to $\le 14.0\text{s}$ for diagram and code clarity.
    - **Expectation Mismatch Flags**: Penalizes hyper-frantic cuts ($< 1.8\text{s}$) on cozy/documentary videos for violating raw viewer expectations, while penalizing sluggish talking-head freezes on high-stakes videos.

18. **Topic Demand Bounds ($Q_1$ Floor, Median Baseline, $Q_3 + 1.5 \times \text{IQR}$ Ceiling) & Demand Stacking Playbook (`--trending`)**:
    - Incorporates Jake Trinder's **Bounded Rationality & Demand Stacking** principles:
      - **Topic Demand Floor ($Q_1$)**: The baseline lower-quartile safety net for standard/mediocre videos on this topic.
      - **Median Baseline ($B_c$)**: Rolling median cohort view count.
      - **Topic Ceiling ($Q_3 + 1.5 \times \text{IQR}$)**: The Winsorized viral outlier breakout ceiling.
      - **Upside Multiplier**: Quantifies total market elasticity from floor to ceiling.
    - **Demand Stacking Auto-Suggester**: Automatically recommends high-authority entities, scale anchors (e.g., "110k Gallon", "1,000,000 Req/s"), mechanical friction ("Cheap Mini Excavator", "Zero Dependencies"), and extreme transformation angles to expand the Total Addressable Market (TAM).

19. **Pre-Publish Local Video Draft Auditing (`--audit <path/to/video.mp4>`)**:
    - **Local Media Bypass**: Bypasses `yt-dlp` network extraction when the target is a local file (`.mp4`, `.mov`, `.mkv`, `.webm`, `.avi`).
    - **Zero Disk / In-Memory Audio Streaming**: Uses `ffprobe` to determine duration and pipes 16kHz 16-bit mono PCM into RAM via `ffmpeg` for microsecond VAD speech onset and pace extraction without generating temporary audio files on disk.
    - **Predictive Retention Risk Calculus (No Heatmap Required)**:
      - Simulates audience abandonment risk on unreleased cuts directly from DaVinci Resolve or Premiere Pro.
      - **Visual Fatigue Cliff**: Flags any static camera hold exceeding the niche benchmark ($\le 7.0\text{s}$ panic, $\le 12.0\text{s}$ cozy/craft, $\le 15.0\text{s}$ educational) as a predicted retention breach at that exact second ($t = \text{hold\_start} + \text{benchmark}$).
      - **Autoplay Bounce**: Flags vocal onset delays $> 0.4\text{s}$ as autoplay bounce friction.
      - **Weak-Link Reliability ($R_{\text{sys}})**: Computes estimated pre-publish Health Grade based on physical editing kinematics.
    - **Pre-Publish Assets**: Outputs `output/<slug>_draft_flight_sheet.md` and `output/<slug>_draft_template.fcpxml` with non-stacking markers ready for direct NLE import.

20. **Automated Head-to-Head Comparative Engine (`--compare <target_a> <target_b>`)**:
    - Ingests any two video targets (local file drafts or YouTube URLs).
    - **Side-by-Side Vector Delta Matrix**: Directly contrasts Health Grade, Composite Score, and all 6 physical telemetry vectors with clear advantage tagging (`[Target A]`, `[Target B]`, `[Tied]`).
    - **Key Structural Advantages of Target A**: Mathematically identifies why Target A outperformed Target B with exact metric deltas.
    - **Actionable NLE Playbook ("What to Change in Target B")**: Produces specific timestamped editing prescriptions (e.g., trim pre-speech gap, split static holds at second $X$, tighten sentence breath gaps).
    - Automatically exports to ANSI terminal output and saves full report to `output/comparison_<slug_a>_vs_<slug_b>.md`.

21. **Dynamic Tone-Aware Packaging Upgrades (`NicheToneProfile`) & Scale Protection**:
    - Synthesizes dynamic, context-aware title upgrades and direct-entry 4 Ps opening lines conditioned on `NicheToneProfile` rather than static placeholder templates.
    - **Scale Downgrade Prevention**: Preserves multi-year or multi-milestone scope (e.g., *"2 Years + 5 Buildings"* is never downgraded to *"30 Days"*).
    - **Mechanical Challenge Routing**: Correctly identifies flipping, motor, engine, and restoration challenges as `DocumentaryBuildProcess` rather than lifestyle or educational topics.

22. **Multi-Channel Empirical Calibration Matrix & Outlier Case Studies**:
    - Calibrated across 4 distinct creator channels and 20+ videos to establish invariant benchmarks:
      - **BOAT BUSTERS**: 4.13M Winner (*"$100 into $1,000"*) vs. 65k Negative Control Flop (*"Cooking Oil"*): 75% vs. 36% Health Grade split, isolating a 125x longer dead-air delay, a 45.8% drop in verbal cadence (197 vs 107 WPM), and a -4.9%/s retention cliff.
      - **WILD HOMESTEAD**: 5.8M Mega-Outlier (*"ONE YEAR SOLO"*) vs. Debut Upload: Proved that introducing an active 180.4 WPM vocal storytelling layer halved the retention decay rate (-0.6%/s vs -1.4%/s) across double the runtime (99:55).
      - **Foresty Forest**: 5.46M Bushcraft & 8.05M Minivan Blizzard Outliers: Calibrated `AestheticLifestyleCozy` tone profile to protect atmospheric sensory openings (up to 38s vocal delay) and physical craft pacing (up to 12.0s hold).
      - **Sailing La Vagabonde**: 9-Video Rapido 40 Cohort Audit (591K to 4.26M Views): Topic Demand Bounds (Guaranteed Baseline 1.07M, Median 1.32M, Ceiling 4.26M, 7.20x upside potential). Comparative audit proved that uncut camera holds $>20\text{s}$ (21.7s hold) and hesitant speech ($112.7\text{ WPM}$) trigger visual stagnation and early bounce regardless of scenery.

23. **Single-Click DaVinci Resolve FCPXML Guide Track & Native EDL Marker Engine**:
    - **Single-Click Resolve FCPXML Import**: Overcomes DaVinci Resolve's `<gap>` tag trap (where markers nested in empty space are discarded). Automatically writes a lightweight 67-byte 1x1 black PNG (`output/.sandsifter_slug.png`), declares it as an `<asset>` in `<resources>`, and binds timeline shots to `<video ref="r2">` on the `<spine>`. Creators import the FCPXML in a single click (`File` > `Import` > `Timeline...`) with all markers immediately visible on Track V1 along the ruler—zero media offline banners!
    - **Dual Compatibility**: Seamlessly retains native `<gap>` elements when no slug asset is specified for pure Final Cut Pro backward compatibility.
    - **Native Resolve CMX 3600 Marker EDL**: Every flight sheet, audit, and predictive script generates a companion `.edl` with pipe-delimited Resolve tags (`|C:ResolveColor<Color> |M:<Name> - <Note> |D:<Duration>`) as a secondary fallback via `Timelines` > `Import` > `Timeline Markers from EDL...`.

24. **The Packaging Safety Suite & Multi-Surface Truncation Radar (`TruncationRadar`)**:
    - Models platform-specific title truncation cutoffs across modern viewing surfaces:
      - **Mobile Push Notifications**: Hard cutoff at 45 characters (`...`).
      - **Mobile Feed & CTV Home Screen**: Cutoff at 52 characters (`...`), representing $>70\%$ of aggregate platform watch time.
      - **Desktop Search & Suggested**: Cutoff at 60 characters (`...`).
    - **Dynamic Front-Loading Advice**: Automatically generates actionable front-load recommendations if critical hooks are occluded on mobile/CTV.
    - **Bottom-Right Timestamp Dead-Zone Radar**: Audits visual bounding coordinates and flags presenter facial features, hero assets, or burned-in subtitles landing in the $x \ge 0.75 \land y \ge 0.80$ quadrant (physically occluded by YouTube's duration badge).
    - **Verbal Redundancy Filter**: Audits thumbnail OCR text against video title keywords; flags duplication $\ge 50\%$ and advises shifting from *Reinforcement* to *Complementarity* to reclaim persuasive canvas space.

25. **The 0:00–0:05 Two-Part Handshake & Promise Gap Index (`PromiseGapEvaluator`)**:
    - Enforces the 2026 platform architecture where *Valued Watch Time* and session continuation supersede raw CTR.
    - **Visual Handshake (0:00.0)**: Verifies that Frame 0 imagery visually corresponds to the hero subject or setting promised in the thumbnail.
    - **Verbal Handshake (0:00–0:05)**: Confirms that the first spoken sentence delivers the title's core topic or challenge within the first 5.0 seconds.
    - **Throat-Clearing Penalties**: Assigns a `-15 point` `PromiseGapMismatch` penalty to Vector 1 (Intro Delay) and Vector 6 (Title Hook & Stakes) if delayed by branded logo animations, sponsor reads, or biographical throat-clearing, warning editors of severe 30-second retention drop risk.

26. **Pre-Production "Moment Sentence" & 4 Extreme Opposites Ideator (`ScriptGenerator`)**:
    - Operationalizes Jake Trinder's pre-production packaging rule: *define the thumbnail shoot list before rolling camera*.
    - **Peak Moment Sentence**: Synthesizes the single peak emotional or visual turning point of the video (e.g., *"The exact moment the ice cracked under the excavator track"*).
    - **4 Extreme Opposite Thumbnail Concept Briefs**: Generates 4 divergent, tone-aware packaging briefs for on-set shooting:
      - 🅰️ **Reaction & Gaze Cue (`ReactionGazeCue`)**: Tight macro close-up of host emotion with eyeline locked at 45° toward the subject.
      - 🅱️ **Minimalist / Before-After (`MinimalistBeforeAfter`)**: Clean high-contrast split-screen hero object comparison.
      - 🅲 **Wide Cinematic / Scale (`WideCinematicMap`)**: 24mm wide environmental plate establishing physical scope and setting context.
      - 🅳 **Visual Conflict / Diagram (`VisualConflictDiagram`)**: Schematic breakdown or opposing workflow vectors in direct collision.
    - **Strict $\le 4$ Word Overlay Ceiling**: Hard-caps thumbnail text overlays to $\le 4$ words for rapid subconscious cognitive parsing.
    - **On-Set Shoot Checklist**: Concrete camera, lighting, framing, and shutter speed commands ready for the production floor.
    - Rendered in Section 4 of the Markdown Teleprompter and exported directly to NLE templates.

27. **Audio Mastering & YouTube Loudness Radar (`src/math/loudness.rs`, FFmpeg `ebur128`)**:
    - Zero-disk streaming audio pipeline: evaluates Integrated Loudness (LUFS), True Peak (dBTP), and Loudness Range (LRA) via FFmpeg's `ebur128` filter.
    - Measures exact YouTube Volume Attenuation Penalty offset ($\Delta\text{dB}$ relative to YouTube's $-14.0\text{ LUFS}$ target), preventing creators from publishing hot mixes that YouTube will automatically clamp down.
    - Flags high-energy True Peak clipping risks ($> -1.0\text{ dBTP}$) that cause digital distortion on mobile speakers.
    - Emits actionable Master Bus compressor, limiter, and ceiling prescriptions in plain English.

28. **Cumulative "Cuttable Fat" Shave Meter & 115% Digital Punch-Ins (`src/math/marginal_gains.rs`, `src/export/timeline.rs`)**:
    - Aggregates micro-fat across three physical editing dimensions:
      - Breath pauses $> 0.4\text{s}$ tightened to crisp conversational cadence ($0.25\text{s}$).
      - Excess static camera holds lingering beyond tone-conditioned thresholds.
      - Pre-speech cold-open lag (bumper silence before the first spoken syllable).
    - Translates abstract metrics into an exact editorial shaving budget (e.g., *"Shave 2m 14s across 38 micro-edits without losing narrative beats"*).
    - Automatically injects Cyan `[PUNCH-IN 115%]` markers at the exact midpoint of extended static takes where no secondary B-roll exists, curing talking-head stagnation.

29. **Crowd-Sourced Timestamp Comment Friction Mining (`src/analyzer/comments.rs`)**:
    - Zero-allocation byte scanner extracting free-text timecode mentions (`M:SS`, `MM:SS`, `H:MM:SS`) with strict word-boundary validation (rejects ratios like `123:456` and invalid seconds).
    - Categorical sentiment classifier isolating `ConfusionOrFriction` (e.g., "why", "cringe", "skip"), `PraiseOrHumor` (e.g., "lmao", "rewind", "best part"), and `StoryEngagement`.
    - Clusters comment mentions within a $\pm 10.0\text{s}$ tolerance window, weighting cluster center timestamps by comment likes.
    - Cross-references audience comments against $R'(t)$ first-derivative retention drop cliffs and replay spikes.
    - Maps into NLE timeline templates as Red `[AUDIENCE FRICTION]` markers and Green `[AUDIENCE REACTION]` markers with consensus quotes.

30. **NLE Ambient Watch-Folder Daemon (`src/ingest/watcher.rs`, `cargo run -- --watch <dir>`)**:
    - Background filesystem daemon monitoring editor render folders for newly completed `.mp4`, `.mov`, `.mkv`, and `.webm` video exports.
    - File lock debouncing: monitors file write locks and size stabilization over a configurable debounce window, preventing partial render corruption.
    - Automatically executes `evaluate_local_draft` and generates the complete production suite alongside the render:
      - Plain-English Editor's Flight Sheet (`.md`)
      - Apple FCPXML v1.8 template with embedded 1x1 black slug asset for single-click import in DaVinci Resolve
      - DaVinci Resolve Marker EDL (`.edl`)
    - Idempotent signature tracking prevents redundant re-analysis while automatically detecting updated re-exports.

31. **Professional Deliverables Suite: Standalone HTML Executive Report & 1-Page Shoot Sheet**:
    - **Standalone HTML Report (`output/<slug>_report.html`)**: Emits a self-contained, zero-dependency HTML5 dashboard with dark theme CSS, circular SVG Health Grade gauge, EBU R128 loudness meter, YouTube attenuation KPI, and interactive NLE punch list with 1-click clipboard copy.
    - **1-Page Printable Shoot Sheet (`output/<slug>_shoot_sheet.html`)**: Clean, printable HTML document styled for 8.5x11 PDF/letter printing with Peak Moment Sentence, 4 Extreme Opposite thumbnail briefs, 4 Ps Opening Shot Clock, and Title Acceptance Criteria checkboxes.

32. **Embedded Single-Binary Studio Dashboard (`web/dist/` via `rust-embed`)**:
    - Production web client embedded directly into the Rust executable at compile time via `rust-embed` (`v8.5`) with zero external runtime file dependencies.
    - Served directly from RAM via Axum HTTP server at `http://localhost:8080/` with dynamic MIME type resolution via `mime_guess`.
    - Full client-side interactive telemetry dashboard: circular SVG Health Grade gauge, SPOF bottleneck alert, 6-Point Telemetry Matrix with visual progress bars, Top 3 ROI Tweaks, and instant Flight Sheet markdown preview.

33. **Native OS File Picker for Unreleased Local Draft Ingestion (`POST /api/pick-file`)**:
    - Invokes native Windows `OpenFileDialog` in the foreground using PowerShell UTF-16LE Base64 `-EncodedCommand` with `$form.TopMost = $true`.
    - Completely bypasses Chromium transient user-activation security restrictions that block synthetic `<input type="file">` clicks after async network calls.
    - Feeds verified absolute paths directly into the zero-disk pre-publish risk audit engine.

34. **Fresh-Upload 0 WPM VTT Fallback via Raw PCM Audio Energy VAD Estimation**:
    - Fixes false 0.0 WPM reporting on brand-new YouTube uploads where WebVTT auto-captions have not yet been processed by platform servers.
    - Analyzes 6.0s raw 16kHz PCM audio stream via `VadClassifier`: if vocal burst energy is $>40\%$, estimates natural conversational pacing (~165–195 WPM) instead of falling back to ambient non-speech.

35. **Cold-Open Harmonic Music & Theme Song Discrimination & Web Studio Live Reload**:
    - Distinguishes background music vocals / intro theme songs from spoken creator dialogue in cold opens. Computes normalized autocorrelation across 80–800 Hz pitch lags ($\tau \in [20, 200]$) and checks for speech formant Zero Crossing Rate variance.
    - Prevents theme song singing from falsely locking `t_silence` to `0.0s ELITE`, correctly detecting intro bumper delays.
    - Added instant local disk hot-reloading for Web Studio frontend assets (`web/dist/`) and first-class `cargo run dev` server launcher.

36. **Mid-Video Re-Engagement Checkpoints (`03:00` and `06:00` Retention Gates)**:
    - Evaluates the slope of the retention curve derivative ($R'(t)$) across critical 45-second retention drop windows (`[02:45–03:30]` and `[05:45–06:30]`) on long-form content ($\ge 8\text{ minutes}$).
    - Flags viewer fatigue cliffs (decay $\le -1.5\%/\text{s}$ or $-2.0\%/\text{s}$) where pacing stalls.
    - Recognizes visual pacing cut resets (scene transitions or recovery spikes) within the window to prevent false failure alarms.
    - Directly generates Timeline Punch List Row 3 actions and native Resolve markers.

37. **The Visual Packaging & Two-Part Handshake Radar**:
    - Evaluates thumbnail archetype alignment and authenticity against spoken narrative content without triggering deceptive clickbait penalties on genuine lifestyle or documentary content.
    - Flags 3-way verbal redundancy where title, thumbnail text, and spoken hook are identically duplicated.
    - Measures Promise Gap speech latency and throat-clearing delays in the critical 0:00–0:05 opening window.

38. **Cross-Device Title Truncation Radar & Tiered Contrast A/B Packaging**:
    - Multisurface character boundary simulation: Mobile Push (45c), Mobile/CTV Feed (52c), and Desktop Search (60c).
    - Verifies that core subject anchors appear before character truncation boundaries.
    - Tiered Contrast Packaging: Generates Title Upgrade A (Concrete Scale / Physical Anchor) and Title Upgrade B (High-Stakes Curiosity Gap) alongside upgraded 4 Ps opening lines.

39. **Strict "Abrupt Ending" Audit & Post-Climax Runway Enforcer (MrBeast Ending Protocol)**:
    - Audits the trailing 20% outro window (`duration * 0.80`) for verbal wind-down exit triggers ("thank you for watching", "hope you enjoyed", "let me know in the comments", "future video ideas").
    - Multi-Cue Rolling VTT Alignment: Combines sliding 2-cue and 3-cue phrase windows to capture sign-off triggers split across auto-caption line breaks.
    - Conversational Wind-Down Walkback: Traces backward through contiguous closing monologue cues to locate the true narrative conclusion and prescribe the exact NLE blade cut (e.g. at 24:34 in Mav's video).
    - FCPXML & EDL Integration: Automatically inserts orange `[BLADE TIMELINE HERE: ABRUPT ENDING]` markers directly into exported timelines.

40. **Dynamic Multi-Language Subtitle Ingestion & YouTube 429 Resilience**:
    - Automatically requests `--sub-lang "en-orig,en,en-US"` from `yt-dlp` to bypass YouTube's HTTP 429 rate-limiting on translated tracks.
    - Dynamically scans the system temporary directory for matching caption files and enforces strict Mechanical Sympathy teardown upon extraction.

---

## 🛠️ CLI Usage

```powershell
# 0. Launch Web Studio & MCP Daemon for Local Development (Live Hot-Reload on port 8080)
cargo run dev
# or specify a custom port
cargo run dev 8080

# 1. Pre-Publish Local Video Draft Auditing (bypasses yt-dlp, direct in-memory FFmpeg)
cargo run -- --audit "path/to/local_video_render.mp4"

# 2. Automated Head-to-Head Comparative Audit (URLs or local files)
cargo run -- --compare "https://www.youtube.com/watch?v=WINNER_ID" "https://www.youtube.com/watch?v=FLOP_ID"

# 3. Evaluate 6-Point "Brailsford Marginal Gains" Scorecard & Flight Sheet on YouTube URL
cargo run -- --audit "https://www.youtube.com/watch?v=VIDEO_ID"
# or alias
cargo run -- --marginal-gains "https://www.youtube.com/watch?v=VIDEO_ID"

# 4. Generate Apple FCPXML & CMX 3600 EDL timeline templates for DaVinci / Premiere
cargo run -- --timeline "https://www.youtube.com/watch?v=VIDEO_ID"

# 5. Analyze audience retention curves, R'(t) derivative drops & rewatch spikes
cargo run -- --retention "https://www.youtube.com/watch?v=VIDEO_ID"

# 6. Extract Average Shot Length (ASL) and visual scene cut rhythm (zero-disk streaming)
cargo run -- --pacing "https://www.youtube.com/watch?v=VIDEO_ID" 60

# 7. Synthesize mathematically paced short-form script & teleprompter timeline
cargo run -- --script "@Vanwives" "Natural Swim Pond Build" --duration 36 --export-timeline

# 8. Forecast semantic topic momentum & breakout trends across 7-day cohorts
cargo run -- --trending "Dark Romance Book Tropes" --days 7 --limit 50

# 9. Run as Model Context Protocol (MCP) server over stdio
cargo run -- --mcp

# 10. Launch HTTP & Server-Sent Events (SSE) MCP daemon
cargo run -- --sse 8080

# 11. Live harvest channel shorts & track view acceleration (Phase 2A)
cargo run -- --crawl @author_handle 30

# 12. Hybrid semantic hook search using Gemini text-embedding-004 vectors (Phase 2B)
cargo run -- --hooks "book tropes"

# 13. Run The Hook Machine with empirical SQLite baseline loading
cargo run -- --hook-machine "@creator_handle" "Target Topic"

# 14. Batch import historical creator data into SQLite database
cargo run -- --import "path/to/creator_history.json"

# 15. Deep acoustic media forensics, LOOCV ablation & creator matrix
cargo run -- --forensics "@author_handle"

# 16. View top empirical viral outlier video watchlist from database
cargo run -- --watchlist

# 17. Check system status & database storage metrics
cargo run -- --status

# 18. Launch NLE Ambient Watch-Folder Daemon
cargo run -- --watch "C:\Exports" --output-dir "output" --debounce 2
```

---

## 🔌 Native Rust MCP Tool Matrix

Sandsifter implements the complete **Model Context Protocol (MCP 2024-11-05)** specification over standard I/O and HTTP SSE:

| Tool Name | Key Parameters | Operational Description |
| :--- | :--- | :--- |
| `/marginal-gains` | `video_url` | Synthesizes the 6-Point Marginal Gains Scorecard & Editor's Flight Sheet with Series System Reliability ($R_{\text{sys}} = \prod p_i$), Single Point of Failure (SPOF) detection, and Viewer Satisfaction Index ($S > R_{\text{raw}}$). Supports both YouTube URLs and unreleased local video drafts (`.mp4`, `.mov`). |
| `/analyze` | `video_url`, `pacing_sample_sec`, `include_retention` | Deep-analyzes video URL: sub-3.5s acoustic VAD, audience retention curve dips, visual ASL pacing, and narrative formulas. |
| `/videos-watchlist` | `niche_filter`, `min_multiplier`, `limit` | Surfaces true viral outliers (views / channel baseline $\ge 2.0\text{x}$) screened through the 2.0% engagement gate. |
| `/hooks-watchlist` | `query`, `topic`, `limit` | Hybrid semantic vector search (768-dim Gemini embeddings + RRF) retrieving top winning opening hooks and madlibs. |
| `/formats-watchlist` | `channel`, `platform`, `limit` | Surfaces visual & narrative format archetypes, Average Shot Length (ASL) benchmarks, and empirical multiplier lifts. |
| `/topic` | `topic`, `days`, `limit` | Tracks real-time semantic topic momentum, TF-IDF n-gram clusters, and breakout micro-trends across recent video cohorts. |
| `/hook-machine` | `channel_handle`, `topic` | Runs the Universal Hook Machine: extracts channel formulas, scores against 3 vectors, and tracks new hooks in SQLite. |
| `/hook-feedback` | `action`, `channel_handle`, `status` | Closes the creator feedback loop: tracks, updates, and iterates on generated hook performance in SQLite. |
| `/channels-add` | `channel_urls` | Registers creator URLs or handles to ongoing background tracking. |
| `/channels-recap` | `handle`, `lookback_days` | Summarizes creator posting velocity, baseline shifts, and breakout counts. |
| `/channels-search` | `query`, `platform` | Searches tracked creators and historical channels. |
| `/video-suggest` | `topic`, `channel_handle`, `target_duration_sec` | Synthesizes a mathematically verified short-form predictive script with 2.4s ASL shot pacing and teleprompter markdown. |
| `current_org_usage` | *(none)* | Returns local analysis quota and open-source engine status. |

---

## 🌐 Production Daemon & Client Integration (Phase 2D)

Sandsifter operates in two production modes: **Local Stdio MCP** for paired IDE development, and **HTTP & Server-Sent Events (SSE) Daemon** for remote agent orchestration, Google Cloud Run, and embedded web frontends for creators.

### 1. Launching the HTTP & SSE Daemon Locally
```powershell
# Launch daemon on default port 8080 (or custom port)
cargo run -- --sse 8080
# or alias
cargo run -- --server 8080
```

### 2. Available Endpoints & REST API
All endpoints include permissive CORS (`Access-Control-Allow-Origin: *`) allowing direct calls from local frontends (Vite, Next.js, or browser extensions):
- `GET /`: Serves the embedded Web Studio Dashboard from RAM (`rust-embed`).
- `GET /health`: Liveness probe for Google Cloud Run container health.
- `GET /sse`: MCP 2024-11-05 Server-Sent Events connection.
- `POST /message` / `POST /rpc`: JSON-RPC 2.0 message handler.
- `GET /api/status`: System status and database volume statistics.
- `GET /api/watchlist?limit=25&min_multiplier=1.5`: Real-time empirical outlier list.
- `GET /api/hooks?query=trope&limit=10`: Semantic vector + keyword hook search.
- `POST /api/pick-file`: Invokes native Windows file picker dialog and returns absolute path for local video drafts.
- `POST /api/crawl`: `{ "channel": "@author", "limit": 30 }` live harvest.
- `POST /api/hook-machine`: `{ "channel": "@author", "topic": "Enemies to Lovers" }` instant report.
- `POST /api/script`: `{ "channel_handle": "@author", "topic": "Enemies to Lovers", "target_duration_sec": 36 }` complete predictive script & teleprompter.
- `POST /api/marginal-gains`: `{ "video_url": "https://youtube.com/watch?v=..." }` complete Brailsford Marginal Gains scorecard JSON + markdown.


### 3. Client Configuration

#### Antigravity IDE / Claude Desktop (`mcp_config.json`):
```json
{
  "mcpServers": {
    "sandsifter": {
      "command": "path/to/sandsifter/target/release/sandsifter.exe",
      "args": ["--mcp"],
      "env": {
        "GEMINI_API_KEY": "YOUR_GEMINI_API_KEY"
      }
    }
  }
}
```

#### Remote MCP SSE (for Antigravity or Web UI pointing to Cloud Run):
```json
{
  "mcpServers": {
    "sandsifter-cloud": {
      "url": "https://sandsifter-service-xxxx-uc.a.run.app/sse"
    }
  }
}
```

### 4. Deploying to Google Cloud Run
Sandsifter includes a production multi-stage [`Dockerfile`](file:///c:/Antigravity%20projects/Rust/Sandsifter/Dockerfile) with automatic `$PORT` binding:
```powershell
# Deploy directly with gcloud CLI
gcloud run deploy sandsifter \
  --source . \
  --platform managed \
  --region us-central1 \
  --port 8080 \
  --set-env-vars GEMINI_API_KEY="YOUR_KEY" \
  --allow-unauthenticated
```

---

## 🧪 Verification & Test Suite

All modules adhere to Tier 1 Safe Rust standards with zero compiler warnings and zero clippy lints across all 28 unit, regression, and integration test fixtures (190+ tests passing with 100% green status).

```powershell
# Run the complete test suite (all unit & full-funnel integration tests)
cargo test --tests -j 2

# Run strict clippy verification
cargo clippy --all-targets -- -D warnings
```


