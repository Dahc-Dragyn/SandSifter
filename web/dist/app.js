/**
 * Sandsifter Studio - Embedded SPA Client Application
 * Zero external runtime dependencies - 100% vanilla ES6+
 */

(function () {
  'use strict';

  // State Management
  const state = {
    activeTab: 'audit',
    theme: localStorage.getItem('sandsifter_theme') || 'dark',
    watchlist: [],
    auditResult: null,
    hooksResult: null,
    scriptResult: null,
    status: null,
  };

  // DOM Elements
  const themeToggleBtn = document.getElementById('theme-toggle-btn');
  const navChips = document.querySelectorAll('.nav-chip');
  const tabPanes = document.querySelectorAll('.tab-pane');
  const daemonStatusDot = document.getElementById('daemon-status-dot');

  // Initialize Theme
  function applyTheme(theme) {
    state.theme = theme;
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('sandsifter_theme', theme);
    if (themeToggleBtn) {
      themeToggleBtn.textContent = theme === 'dark' ? '☀️ Light' : '🌙 Dark';
    }
  }

  // Initialize Route / Tab
  function switchTab(tabId) {
    state.activeTab = tabId;
    navChips.forEach(chip => {
      chip.classList.toggle('active', chip.dataset.tab === tabId);
    });
    tabPanes.forEach(pane => {
      pane.classList.toggle('active', pane.id === `tab-${tabId}`);
    });

    // Update URL without full reload
    if (window.location.pathname !== `/${tabId}` && tabId !== 'audit') {
      window.history.pushState(null, '', `/${tabId}`);
    } else if (tabId === 'audit') {
      window.history.pushState(null, '', '/');
    }

    // Lazy load tab data
    if (tabId === 'watchlist' && state.watchlist.length === 0) {
      loadWatchlist();
    } else if (tabId === 'status') {
      loadStatus();
    }
  }

  // Check Daemon Health
  async function checkHealth() {
    try {
      const res = await fetch('/health');
      if (res.ok) {
        daemonStatusDot.style.background = 'var(--yt-status-success)';
        daemonStatusDot.title = 'Sandsifter Daemon: Connected';
      } else {
        daemonStatusDot.style.background = 'var(--yt-status-warning)';
        daemonStatusDot.title = 'Sandsifter Daemon: Unhealthy';
      }
    } catch {
      daemonStatusDot.style.background = 'var(--yt-status-error)';
      daemonStatusDot.title = 'Sandsifter Daemon: Offline';
    }
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tab 1: Live Audit Handler
  // ─────────────────────────────────────────────────────────────────────────────
  const auditForm = document.getElementById('audit-form');
  const auditUrlInput = document.getElementById('audit-url-input');
  const auditBtn = document.getElementById('audit-btn');
  const auditResultsContainer = document.getElementById('audit-results');

  async function runAudit(url) {
    if (!url) return;
    auditBtn.disabled = true;
    auditBtn.innerHTML = '<span class="spinner-icon-sm"></span> Analyzing Telemetry...';
    auditResultsContainer.innerHTML = `
      <div class="studio-card" style="text-align: center; padding: 48px 24px;">
        <div class="spinner-icon"></div>
        <h3 style="font-size: 18px; font-weight: 600; margin-bottom: 8px;">Executing Multimodal Telemetry Audit</h3>
        <p style="color: var(--yt-spec-text-secondary); font-size: 13px; max-width: 540px; margin: 0 auto;">
          Ingesting media stream, calculating sub-second RMS VAD acoustic latency, detecting visual scene pacing (ASL), measuring EBU R128 loudness, and analyzing retention curves...
        </p>
        <div class="audit-warning-box">
          <span style="font-size: 20px; line-height: 1;">⚠️</span>
          <div>
            <strong>Processing Notice (Expected Duration: 30 to 90 seconds):</strong>
            Zero-disk deep media extraction and acoustic frequency analysis are running in the background. Please keep this tab open until the scorecard is ready.
          </div>
        </div>
      </div>
    `;

    try {
      const res = await fetch('/api/marginal-gains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ video_url: url }),
      });

      if (!res.ok) {
        throw new Error(`Audit failed with status ${res.status}: ${await res.text()}`);
      }

      const data = await res.json();
      state.auditResult = data;
      renderAuditScorecard(data);
    } catch (err) {
      auditResultsContainer.innerHTML = `
        <div class="studio-card" style="border-color: var(--yt-status-error);">
          <h3 style="color: var(--yt-status-error);">❌ Telemetry Audit Error</h3>
          <p style="margin-top: 8px; font-size: 13px;">${err.message}</p>
        </div>
      `;
    } finally {
      auditBtn.disabled = false;
      auditBtn.innerHTML = '⚡ Run Telemetry Audit';
    }
  }

  function escapeHtml(str) {
    if (str === null || str === undefined) return '';
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#039;');
  }

  function extractYouTubeVideoId(urlOrId) {
    if (!urlOrId) return null;
    const clean = String(urlOrId).trim();
    if (/^[a-zA-Z0-9_-]{11}$/.test(clean)) return clean;
    const m = clean.match(/(?:v=|\/embed\/|youtu\.be\/|\/v\/|watch\?v=|\&v=)([a-zA-Z0-9_-]{11})/);
    return m ? m[1] : null;
  }

  function parseTimecodeSeconds(tc) {
    if (!tc) return 0;
    const clean = tc.split('-')[0].trim();
    const parts = clean.split(':').map(Number);
    if (parts.length === 2 && !isNaN(parts[0]) && !isNaN(parts[1])) {
      return parts[0] * 60 + parts[1];
    } else if (parts.length === 3 && !isNaN(parts[0]) && !isNaN(parts[1]) && !isNaN(parts[2])) {
      return parts[0] * 3600 + parts[1] * 60 + parts[2];
    }
    return 0;
  }

  function formatSeconds(sec) {
    const total = Math.max(0, Math.round(sec));
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }

  function renderAuditScorecard(data) {
    const sc = data.scorecard || data;
    const videoTitle = sc.title || sc.target_id || data.video_title || 'Video Title';
    const healthGrade = Math.round(sc.series_system_reliability_pct ?? data.overall_video_health_pct ?? 75);
    const compositeScore = Math.round(sc.composite_brailsford_score ?? data.composite_score ?? 85);
    const tone = sc.tone_profile || sc.niche_profile || data.tone_profile || 'DocumentaryBuildProcess';
    const bottleneck = sc.single_point_of_failure || data.primary_bottleneck || 'None detected (System Balanced)';
    const vectors = sc.vectors || data.telemetry_matrix || [];
    const topGains = sc.top_3_marginal_gains || [];
    const cuttable = sc.cuttable_fat;
    const markdownReport = data.markdown || '';
    const predictedLift = sc.predicted_multiplier_lift ? Math.round((sc.predicted_multiplier_lift - 1.0) * 100) : 0;

    const circumference = 251.2;
    const offset = Math.max(0, circumference - (circumference * healthGrade) / 100);

    let gaugeColor = 'var(--yt-status-success)';
    if (healthGrade < 70) gaugeColor = 'var(--yt-brand-red)';
    else if (healthGrade < 85) gaugeColor = '#f1a23a';

    let vectorsHtml = vectors.map(v => {
      const vName = v.name || v.vector_name || 'Telemetry Vector';

      let measuredVal = 'N/A';
      if (typeof v.measured_value === 'number') {
        const decimals = (v.unit === 's' || v.unit === 'cuts/min') ? 1 : 0;
        measuredVal = `${v.measured_value.toFixed(decimals)} ${v.unit || ''}`.trim();
      } else if (v.measured_value !== undefined && v.measured_value !== null) {
        measuredVal = String(v.measured_value);
      }

      let targetVal = 'N/A';
      if (typeof v.target_benchmark === 'number') {
        const decimals = (v.unit === 's') ? 1 : 0;
        targetVal = `${v.target_benchmark.toFixed(decimals)} ${v.unit || ''}`.trim();
      } else if (v.editor_target) {
        targetVal = String(v.editor_target);
      }

      const scoreVal = Math.round(v.score_0_to_100 ?? v.score ?? 0);
      const statusRaw = String(v.status || 'Optimal');

      let badgeClass = 'badge-optimal';
      let statusLabel = statusRaw;
      if (statusRaw.toLowerCase().includes('elite')) {
        badgeClass = 'badge-elite';
        statusLabel = '🟢 ELITE';
      } else if (statusRaw.toLowerCase().includes('optimal')) {
        badgeClass = 'badge-optimal';
        statusLabel = '✅ OPTIMAL';
      } else if (statusRaw.toLowerCase().includes('drag')) {
        badgeClass = 'badge-drag';
        statusLabel = '⚠️ DRAG';
      } else if (statusRaw.toLowerCase().includes('critical') || statusRaw.toLowerCase().includes('friction')) {
        badgeClass = 'badge-drag';
        statusLabel = '🚨 FRICTION';
      }

      const tweak = v.one_percent_fix || v.actionable_tweak || 'Maintain current retention benchmark.';
      const liftNum = v.estimated_retention_lift_pct ?? v.projected_lift ?? 0;
      const liftText = (typeof liftNum === 'number')
        ? (liftNum > 0 ? `+${liftNum.toFixed(1)}%` : '0.0%')
        : String(liftNum);

      return `
        <tr>
          <td style="font-weight: 600;">${escapeHtml(vName)}</td>
          <td><code>${escapeHtml(measuredVal)}</code></td>
          <td style="color: var(--yt-spec-text-secondary);">${escapeHtml(targetVal)}</td>
          <td><strong>${scoreVal}/100</strong></td>
          <td><span class="badge ${badgeClass}">${statusLabel}</span></td>
          <td style="font-size: 13px;">${escapeHtml(tweak)}</td>
          <td style="font-weight: 600; color: ${liftNum > 0 ? 'var(--yt-status-success)' : 'var(--yt-spec-text-secondary)'};">${liftText}</td>
        </tr>
      `;
    }).join('');

    let topGainsHtml = '';
    if (topGains && topGains.length > 0) {
      topGainsHtml = `
        <div class="studio-card" style="margin-top: 24px; border-left: 4px solid var(--yt-status-success);">
          <h3>🎯 Top 3 High-ROI Marginal Gain Tweaks (${predictedLift > 0 ? `+${predictedLift}% Compounded Retention Lift` : 'Optimized'})</h3>
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; margin-top: 14px;">
            ${topGains.map((g, idx) => `
              <div style="background: var(--yt-spec-additive-background); padding: 14px; border-radius: 8px;">
                <div style="font-size: 11px; font-weight: 700; color: var(--yt-brand-red); text-transform: uppercase;">Priority #${idx + 1}</div>
                <div style="font-weight: 600; margin-top: 4px; font-size: 14px;">${escapeHtml(g.name || 'Vector')}</div>
                <p style="font-size: 13px; color: var(--yt-spec-text-primary); margin: 6px 0;">${escapeHtml(g.one_percent_fix || '')}</p>
                <div style="font-size: 12px; font-weight: 600; color: var(--yt-status-success);">
                  Est. Retention Lift: +${(g.estimated_retention_lift_pct || 0).toFixed(1)}%
                </div>
              </div>
            `).join('')}
          </div>
        </div>
      `;
    }

    let cuttableHtml = '';
    if (cuttable && cuttable.cumulative_cuttable_seconds > 0) {
      cuttableHtml = `
        <div style="background: var(--yt-spec-additive-background); padding: 12px 16px; border-radius: 8px; margin-top: 14px; font-size: 13px; display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px;">
          <span>✂️ <strong>Editorial Fat Budget</strong>: ${cuttable.cumulative_cuttable_seconds.toFixed(1)}s dead weight (${(cuttable.cuttable_percentage * 100).toFixed(1)}% of timeline)</span>
          <span style="font-weight: 600; color: var(--yt-status-success);">Pacing Lift: +${cuttable.target_pacing_lift_pct.toFixed(1)}%</span>
        </div>
      `;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Retention Curve & Interactive Scrubber Synthesis
    // ─────────────────────────────────────────────────────────────────────────
    const retentionCurve = sc.retention_curve || [];
    const attentionBreaches = sc.attention_breaches || [];

    let durationSeconds = sc.duration_seconds;
    if (!durationSeconds || durationSeconds <= 0) {
      if (retentionCurve.length > 0) {
        durationSeconds = retentionCurve[retentionCurve.length - 1].end_time;
      } else {
        durationSeconds = 600.0;
      }
    }

    let curvePoints = retentionCurve;
    if (!curvePoints || curvePoints.length === 0) {
      curvePoints = [];
      const steps = 100;
      const dt = durationSeconds / steps;
      let r = 1.0;
      for (let i = 0; i <= steps; i++) {
        const t = i * dt;
        r = Math.max(0.2, r - 0.005);
        curvePoints.push({ start_time: t, end_time: t + dt, value: r });
      }
    }

    const svgWidth = 1000;
    const graphBottom = 195;
    const graphTop = 20;
    const graphHeight = graphBottom - graphTop; // 175

    let pathD = '';
    let areaD = '';
    const coords = [];

    curvePoints.forEach((pt, idx) => {
      const t = (pt.start_time + pt.end_time) / 2;
      const x = Math.min(svgWidth, Math.max(0, (t / durationSeconds) * svgWidth));
      const val = Math.max(0, Math.min(1, pt.value));
      const y = graphBottom - (val * graphHeight);
      coords.push({ x, y, t, val });
      if (idx === 0) {
        pathD += `M ${x.toFixed(1)},${y.toFixed(1)}`;
        areaD += `M ${x.toFixed(1)},${graphBottom} L ${x.toFixed(1)},${y.toFixed(1)}`;
      } else {
        pathD += ` L ${x.toFixed(1)},${y.toFixed(1)}`;
        areaD += ` L ${x.toFixed(1)},${y.toFixed(1)}`;
      }
    });
    if (coords.length > 0) {
      const last = coords[coords.length - 1];
      areaD += ` L ${last.x.toFixed(1)},${graphBottom} Z`;
    }

    // Grid lines
    let gridHtml = '';
    [0.25, 0.50, 0.75, 1.0].forEach(val => {
      const gy = graphBottom - (val * graphHeight);
      gridHtml += `
        <line x1="0" y1="${gy.toFixed(1)}" x2="${svgWidth}" y2="${gy.toFixed(1)}" stroke="rgba(255, 255, 255, 0.08)" stroke-width="1" stroke-dasharray="2 2" />
        <text x="6" y="${(gy - 4).toFixed(1)}" fill="rgba(255, 255, 255, 0.3)" font-size="9" font-family="monospace">${Math.round(val * 100)}%</text>
      `;
    });

    // Time ticks
    let timeTicksHtml = '';
    const numTicks = 6;
    for (let i = 0; i <= numTicks; i++) {
      const t = (i / numTicks) * durationSeconds;
      const tx = (i / numTicks) * svgWidth;
      const anchor = (i === 0) ? 'start' : (i === numTicks) ? 'end' : 'middle';
      timeTicksHtml += `
        <text x="${tx.toFixed(1)}" y="214" fill="rgba(255, 255, 255, 0.4)" font-size="10" font-family="monospace" text-anchor="${anchor}">${formatSeconds(t)}</text>
      `;
    }

    // Checkpoints
    let checkpointsHtml = '';
    if (durationSeconds >= 180.0) {
      const x3 = (180.0 / durationSeconds) * svgWidth;
      checkpointsHtml += `
        <g class="checkpoint-line">
          <line x1="${x3.toFixed(1)}" y1="${graphTop}" x2="${x3.toFixed(1)}" y2="${graphBottom}" stroke="#38bdf8" stroke-dasharray="4 4" stroke-width="1.5" />
          <text x="${(x3 + 4).toFixed(1)}" y="${graphTop + 14}" fill="#38bdf8" font-size="10" font-weight="600">03:00 Gate</text>
        </g>
      `;
    }
    if (durationSeconds >= 360.0) {
      const x6 = (360.0 / durationSeconds) * svgWidth;
      checkpointsHtml += `
        <g class="checkpoint-line">
          <line x1="${x6.toFixed(1)}" y1="${graphTop}" x2="${x6.toFixed(1)}" y2="${graphBottom}" stroke="#38bdf8" stroke-dasharray="4 4" stroke-width="1.5" />
          <text x="${(x6 + 4).toFixed(1)}" y="${graphTop + 14}" fill="#38bdf8" font-size="10" font-weight="600">06:00 Gate</text>
        </g>
      `;
    }

    // Attention Breach Pins (Red Markers)
    let markersHtml = '';
    attentionBreaches.forEach((breach, idx) => {
      const t = breach.timestamp_sec;
      const bx = Math.min(svgWidth, Math.max(0, (t / durationSeconds) * svgWidth));
      let by = graphBottom - 0.60 * graphHeight;
      for (let i = 0; i < coords.length - 1; i++) {
        if (coords[i].x <= bx && coords[i + 1].x >= bx) {
          const ratio = (coords[i + 1].x === coords[i].x) ? 0 : (bx - coords[i].x) / (coords[i + 1].x - coords[i].x);
          by = coords[i].y + ratio * (coords[i + 1].y - coords[i].y);
          break;
        }
      }
      const causeLabel = breach.root_cause
        ? (breach.root_cause.SlowVisualPacing ? 'Lingering Camera Take' : breach.root_cause.DeadAirSilence ? 'Cold-Open Dead Air' : typeof breach.root_cause === 'string' ? breach.root_cause : 'Attention Breach')
        : 'Retention Cliff';
      const tcFormatted = breach.formatted_timestamp || formatSeconds(t);
      const rPrimeFormatted = (typeof breach.r_prime === 'number') ? breach.r_prime.toFixed(1) : '-2.5';
      const remedyText = breach.tactical_remedy || 'Trim or insert visual reset';

      markersHtml += `
        <g class="breach-marker" data-index="${idx}" data-time="${t}" data-tc="${escapeHtml(tcFormatted)}" data-rprime="${rPrimeFormatted}" data-cause="${escapeHtml(causeLabel)}" data-remedy="${escapeHtml(remedyText)}">
          <circle class="marker-pulse" cx="${bx.toFixed(1)}" cy="${by.toFixed(1)}" r="6" fill="rgba(239, 68, 68, 0.45)" />
          <circle cx="${bx.toFixed(1)}" cy="${by.toFixed(1)}" r="5.5" fill="#ef4444" stroke="#ffffff" stroke-width="1.5" />
          <text x="${bx.toFixed(1)}" y="${(by - 9).toFixed(1)}" fill="#ef4444" font-size="10" font-weight="700" text-anchor="middle">▼</text>
        </g>
      `;
    });

    // Player embed container
    const targetSource = sc.target_id || data.video_url || '';
    const ytId = extractYouTubeVideoId(targetSource);
    let playerContainerHtml = '';
    if (ytId) {
      const origin = (window.location && window.location.origin && window.location.origin !== 'null') ? window.location.origin : '';
      const originParam = origin ? `&origin=${encodeURIComponent(origin)}` : '';
      playerContainerHtml = `
        <iframe id="studio-youtube-iframe"
          src="https://www.youtube-nocookie.com/embed/${ytId}?enablejsapi=1${originParam}&autoplay=0&rel=0"
          allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
          allowfullscreen>
        </iframe>
      `;
    } else if (sc.is_local_draft || targetSource) {
      const streamSrc = `/api/media-stream?path=${encodeURIComponent(targetSource)}`;
      playerContainerHtml = `
        <video id="studio-local-video" controls preload="metadata">
          <source src="${streamSrc}" type="video/mp4">
          Your browser does not support HTML5 video streaming.
        </video>
      `;
    } else {
      playerContainerHtml = `
        <div class="video-placeholder-state">
          <span style="font-size: 32px;">🎬</span>
          <div><strong>Video Preview Ready</strong></div>
          <span>Provide a YouTube URL or select a local video to enable interactive video scrubbing.</span>
        </div>
      `;
    }

    const scrubberSuiteHtml = `
      <div class="studio-card interactive-scrubber-card" id="retention-scrubber-suite">
        <div class="scrubber-header-row">
          <div class="scrubber-title-wrap">
            <h3><span>🎬</span> Interactive Retention Scrubber & NLE Video Sync</h3>
            <p>Click any red attention breach or click anywhere on the curve to jump the video playhead to that exact second.</p>
          </div>
          <div class="scrubber-badge-row">
            <div class="timecode-badge" id="scrubber-timecode-badge">00:00 / ${formatSeconds(durationSeconds)}</div>
          </div>
        </div>

        <div class="scrubber-layout-grid">
          <div class="video-preview-wrapper" id="video-preview-box">
            ${playerContainerHtml}
          </div>

          <div class="curve-container-wrapper">
            <div class="svg-canvas-box" id="svg-canvas-box">
              <svg class="retention-svg" id="retention-svg" viewBox="0 0 1000 220" preserveAspectRatio="none">
                <defs>
                  <linearGradient id="retention-gradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color="#22c55e" stop-opacity="0.45" />
                    <stop offset="50%" stop-color="#38bdf8" stop-opacity="0.2" />
                    <stop offset="100%" stop-color="#ef4444" stop-opacity="0.05" />
                  </linearGradient>
                </defs>

                ${gridHtml}
                ${checkpointsHtml}

                <!-- Area Fill -->
                <path d="${areaD}" fill="url(#retention-gradient)" />

                <!-- Retention Curve Line -->
                <path d="${pathD}" fill="none" stroke="#22c55e" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />

                <!-- Attention Breach Red Pins -->
                ${markersHtml}

                <!-- Playhead Line & Thumb -->
                <line id="scrubber-playhead-line" x1="0" y1="12" x2="0" y2="${graphBottom}" class="scrubber-playhead-line" />
                <polygon id="scrubber-playhead-thumb" points="0,12 -6,2 6,2" class="scrubber-playhead-thumb" />

                <!-- Time Axis Labels -->
                ${timeTicksHtml}

                <!-- Interactive Hitbox Rect -->
                <rect class="scrubber-hit-rect" id="scrubber-hitbox" x="0" y="0" width="1000" height="220" fill="transparent" />
              </svg>
              <div class="scrubber-tooltip" id="scrubber-tooltip"></div>
            </div>

            <div class="scrubber-legend">
              <div class="legend-items">
                <div class="legend-item"><span class="legend-dot green"></span> Audience Retention R(t)</div>
                <div class="legend-item"><span class="legend-dot red"></span> Attention Breach Cliff R'(t)</div>
                <div class="legend-item"><span class="legend-dot cyan"></span> Re-Engagement Gate (03:00 / 06:00)</div>
              </div>
              <span>💡 Click or drag playhead to scrub</span>
            </div>
          </div>
        </div>
      </div>
    `;

    // ─────────────────────────────────────────────────────────────────────────
    // NLE Timeline Punch List Table
    // ─────────────────────────────────────────────────────────────────────────
    const punchList = (data.flight_sheet && data.flight_sheet.timeline_punch_list) || [];
    let punchListHtml = '';
    if (punchList.length > 0) {
      punchListHtml = `
        <div class="studio-card" style="margin-top: 24px; margin-bottom: 24px;">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; flex-wrap: wrap; gap: 8px;">
            <h3>📋 Timeline Punch List (NLE Editing Commands)</h3>
            <span style="font-size: 12px; color: var(--yt-spec-text-secondary);">Click any timecode or red graph marker to jump playhead</span>
          </div>
          <div class="table-container">
            <table class="studio-table" id="punch-list-table">
              <thead>
                <tr>
                  <th>Timecode</th>
                  <th>Action Required</th>
                  <th>Editing Command & Prescription</th>
                  <th>Projected Impact</th>
                  <th style="text-align: center;">Seek</th>
                </tr>
              </thead>
              <tbody>
                ${punchList.map((cue, idx) => {
                  const tc = cue.timecode_range || '00:00';
                  const actionBadgeClass = cue.action_type.includes('DEAD AIR')
                    ? 'badge-optimal'
                    : cue.action_type.includes('B-ROLL')
                    ? 'badge-elite'
                    : 'badge-drag';
                  return `
                    <tr id="punch-row-${idx}" data-timecode="${escapeHtml(tc)}" class="punch-table-row">
                      <td><code>${escapeHtml(tc)}</code></td>
                      <td><span class="badge ${actionBadgeClass}">${escapeHtml(cue.action_type)}</span></td>
                      <td>${escapeHtml(cue.remedy_instruction)}</td>
                      <td style="font-weight: 600; color: var(--yt-status-success);">${escapeHtml(cue.impact_summary)}</td>
                      <td style="text-align: center;">
                        <button type="button" class="btn-secondary jump-to-time-btn" data-timecode="${escapeHtml(tc)}" style="padding: 4px 10px; font-size: 11px;">▶ Seek</button>
                      </td>
                    </tr>
                  `;
                }).join('')}
              </tbody>
            </table>
          </div>
        </div>
      `;
    }

    let markdownSection = '';
    if (markdownReport) {
      markdownSection = `
        <div class="studio-card" style="margin-top: 24px;">
          <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
            <h3>📄 NLE Timeline Flight Sheet & Audio Directives</h3>
            <button class="btn-secondary" id="copy-flight-sheet-btn">📋 Copy Flight Sheet</button>
          </div>
          <details style="margin-top: 12px;">
            <summary style="cursor: pointer; color: var(--yt-spec-text-secondary); font-size: 13px; font-weight: 500;">
              Click to view complete Markdown report
            </summary>
            <pre style="background: var(--yt-spec-additive-background); padding: 16px; border-radius: 8px; font-size: 12px; overflow-x: auto; margin-top: 8px; white-space: pre-wrap; font-family: monospace;">${escapeHtml(markdownReport)}</pre>
          </details>
        </div>
      `;
    }

    auditResultsContainer.innerHTML = `
      <div class="card-grid" style="grid-template-columns: 2fr 1fr;">
        <div class="studio-card">
          <div class="gauge-container">
            <svg class="gauge-svg" viewBox="0 0 100 100">
              <circle class="gauge-bg" cx="50" cy="50" r="40"></circle>
              <circle class="gauge-fill" cx="50" cy="50" r="40" style="stroke: ${gaugeColor}; stroke-dashoffset: ${offset};"></circle>
            </svg>
            <div>
              <div class="metric-value">${healthGrade}%</div>
              <div class="metric-label">Overall Video Health (Weakest-Link Grade)</div>
              <div style="margin-top: 6px; font-size: 13px; color: var(--yt-spec-text-secondary);">
                Tone Expectation: <strong>${escapeHtml(tone)}</strong> | Composite Score: <strong>${compositeScore}/100</strong>
              </div>
            </div>
          </div>
          <div style="background: var(--yt-spec-additive-background); padding: 12px; border-radius: 8px; font-size: 13px; border-left: 4px solid var(--yt-brand-red);">
            🚨 <strong>Primary Bottleneck</strong>: ${escapeHtml(bottleneck)}
          </div>
          ${cuttableHtml}
        </div>

        <div class="studio-card">
          <h3>📦 Packaging Quick-Copy</h3>
          <div class="hook-box" style="margin-top: 12px;">
            <div>
              <div class="hook-text">${escapeHtml(videoTitle)}</div>
              <div class="hook-meta">${escapeHtml(sc.niche_profile || 'Viral Outlier Target')}</div>
            </div>
          </div>
          <div style="display: flex; gap: 8px; margin-top: 14px; flex-wrap: wrap;">
            <button class="btn-secondary" id="copy-audit-title-btn">📋 Copy Title</button>
            <button class="btn-secondary" onclick="window.print()">🖨️ Print Report</button>
          </div>
        </div>
      </div>

      ${scrubberSuiteHtml}
      ${punchListHtml}
      ${topGainsHtml}

      <div class="studio-card" style="margin-top: 24px; margin-bottom: 24px;">
        <h3>📊 6-Point Telemetry Vector Audit</h3>
        <div class="table-container" style="margin-top: 12px;">
          <table class="studio-table">
            <thead>
              <tr>
                <th>Telemetry Vector</th>
                <th>Measured</th>
                <th>Editor Target</th>
                <th>Score</th>
                <th>Status</th>
                <th>1% Actionable Tweak</th>
                <th>Est. Lift</th>
              </tr>
            </thead>
            <tbody>
              ${vectorsHtml || '<tr><td colspan="7" style="text-align: center;">No vector metrics returned.</td></tr>'}
            </tbody>
          </table>
        </div>
      </div>

      ${markdownSection}
    `;

    // ─────────────────────────────────────────────────────────────────────────
    // Attach Listeners for Scrubber and Copy Buttons
    // ─────────────────────────────────────────────────────────────────────────
    setupInteractiveScrubber(durationSeconds, ytId);

    const copyTitleBtn = document.getElementById('copy-audit-title-btn');
    if (copyTitleBtn) {
      copyTitleBtn.addEventListener('click', () => {
        navigator.clipboard.writeText(videoTitle);
        copyTitleBtn.textContent = '✅ Copied!';
        setTimeout(() => { copyTitleBtn.textContent = '📋 Copy Title'; }, 2000);
      });
    }

    const copyFlightBtn = document.getElementById('copy-flight-sheet-btn');
    if (copyFlightBtn && markdownReport) {
      copyFlightBtn.addEventListener('click', () => {
        navigator.clipboard.writeText(markdownReport);
        copyFlightBtn.textContent = '✅ Copied!';
        setTimeout(() => { copyFlightBtn.textContent = '📋 Copy Flight Sheet'; }, 2000);
      });
    }
  }

  function setupInteractiveScrubber(durationSeconds, ytId) {
    const svg = document.getElementById('retention-svg');
    const hitbox = document.getElementById('scrubber-hitbox');
    const playheadLine = document.getElementById('scrubber-playhead-line');
    const playheadThumb = document.getElementById('scrubber-playhead-thumb');
    const timecodeBadge = document.getElementById('scrubber-timecode-badge');
    const tooltip = document.getElementById('scrubber-tooltip');
    const localVideo = document.getElementById('studio-local-video');
    const ytIframe = document.getElementById('studio-youtube-iframe');

    if (!svg || !hitbox) return;

    let isDragging = false;

    function seekPlayer(targetSeconds) {
      const clampedSec = Math.max(0, Math.min(durationSeconds, targetSeconds));
      if (localVideo) {
        localVideo.currentTime = clampedSec;
      }
      if (ytIframe && ytIframe.contentWindow) {
        try {
          ytIframe.contentWindow.postMessage(JSON.stringify({
            event: 'command',
            func: 'seekTo',
            args: [clampedSec, true]
          }), '*');
        } catch (e) {
          console.warn('YouTube postMessage seek failed:', e);
        }
      }
      updatePlayheadUI(clampedSec / durationSeconds, clampedSec);
    }

    function updatePlayheadUI(pct, sec) {
      const clampedPct = Math.max(0, Math.min(1, pct));
      const x = clampedPct * 1000;
      if (playheadLine) {
        playheadLine.setAttribute('x1', x.toFixed(1));
        playheadLine.setAttribute('x2', x.toFixed(1));
      }
      if (playheadThumb) {
        playheadThumb.setAttribute('transform', `translate(${x.toFixed(1)}, 0)`);
      }
      if (timecodeBadge) {
        timecodeBadge.textContent = `${formatSeconds(sec)} / ${formatSeconds(durationSeconds)}`;
      }
    }

    function onScrubEvent(e) {
      const rect = svg.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const targetSec = pct * durationSeconds;
      seekPlayer(targetSec);
    }

    hitbox.addEventListener('mousedown', (e) => {
      isDragging = true;
      onScrubEvent(e);
    });

    window.addEventListener('mousemove', (e) => {
      if (isDragging) {
        onScrubEvent(e);
      }
    });

    window.addEventListener('mouseup', () => {
      isDragging = false;
    });

    // Tooltip tracking on SVG
    hitbox.addEventListener('mousemove', (e) => {
      if (!tooltip) return;
      const rect = svg.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const hoverSec = pct * durationSeconds;
      tooltip.style.left = `${e.clientX - rect.left}px`;
      tooltip.style.top = `${e.clientY - rect.top}px`;
      tooltip.innerHTML = `<strong>⏱️ ${formatSeconds(hoverSec)}</strong> (Click to scrub)`;
      tooltip.classList.add('visible');
    });

    hitbox.addEventListener('mouseleave', () => {
      if (tooltip) tooltip.classList.remove('visible');
    });

    // Attention Breach Markers
    document.querySelectorAll('.breach-marker').forEach(marker => {
      marker.addEventListener('click', (e) => {
        e.stopPropagation();
        const sec = parseFloat(marker.dataset.time);
        seekPlayer(sec);

        const tc = marker.dataset.tc;
        highlightPunchRow(tc, sec);
      });

      marker.addEventListener('mouseenter', (e) => {
        if (!tooltip) return;
        const rect = svg.getBoundingClientRect();
        const markerRect = marker.getBoundingClientRect();
        tooltip.style.left = `${markerRect.left - rect.left + markerRect.width / 2}px`;
        tooltip.style.top = `${markerRect.top - rect.top}px`;
        tooltip.innerHTML = `
          <div style="font-weight: 700; color: #ef4444; margin-bottom: 2px;">🚨 ${escapeHtml(marker.dataset.cause)} (${escapeHtml(marker.dataset.tc)})</div>
          <div>Drop Rate: <strong>${escapeHtml(marker.dataset.rprime)}%/s</strong></div>
          <div style="color: #94a3b8; font-size: 10px; margin-top: 3px;">💡 ${escapeHtml(marker.dataset.remedy)}</div>
        `;
        tooltip.classList.add('visible');
      });

      marker.addEventListener('mouseleave', () => {
        if (tooltip) tooltip.classList.remove('visible');
      });
    });

    function highlightPunchRow(tc, targetSec) {
      document.querySelectorAll('.punch-table-row').forEach(row => {
        row.classList.remove('punch-row-highlight');
      });

      let matchedRow = null;
      document.querySelectorAll('.punch-table-row').forEach(row => {
        const rowTc = row.dataset.timecode || '';
        const rowSec = parseTimecodeSeconds(rowTc);
        if (rowTc === tc || Math.abs(rowSec - targetSec) <= 3.0) {
          if (!matchedRow) matchedRow = row;
        }
      });

      if (matchedRow) {
        matchedRow.classList.add('punch-row-highlight');
        matchedRow.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
    }

    document.querySelectorAll('.jump-to-time-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const tc = btn.dataset.timecode;
        const sec = parseTimecodeSeconds(tc);
        seekPlayer(sec);
        highlightPunchRow(tc, sec);
      });
    });

    if (localVideo) {
      localVideo.addEventListener('timeupdate', () => {
        if (!isDragging && durationSeconds > 0) {
          const current = localVideo.currentTime;
          updatePlayheadUI(current / durationSeconds, current);
        }
      });
    }

    if (ytIframe) {
      const ytPoller = setInterval(() => {
        if (!document.getElementById('studio-youtube-iframe')) {
          clearInterval(ytPoller);
          return;
        }
        if (window.YT && window.YT.get && typeof window.YT.get === 'function') {
          const p = window.YT.get('studio-youtube-iframe');
          if (p && typeof p.getCurrentTime === 'function') {
            const cur = p.getCurrentTime();
            if (!isDragging && cur !== undefined) {
              updatePlayheadUI(cur / durationSeconds, cur);
            }
          }
        }
      }, 150);
    }
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tab 2: Outlier Watchlist Handler
  // ─────────────────────────────────────────────────────────────────────────────
  async function loadWatchlist() {
    const listContainer = document.getElementById('watchlist-table-body');
    listContainer.innerHTML = '<tr><td colspan="6" style="text-align: center; padding: 20px;">⏳ Loading tracked creator baselines...</td></tr>';

    try {
      const res = await fetch('/api/watchlist');
      const data = await res.json();
      state.watchlist = data.watchlist || [];

      if (state.watchlist.length === 0) {
        listContainer.innerHTML = '<tr><td colspan="6" style="text-align: center; padding: 20px;">No channels tracked yet. Add one above!</td></tr>';
        return;
      }

      listContainer.innerHTML = state.watchlist.map(c => `
        <tr>
          <td><strong>${c.display_name || c.handle}</strong></td>
          <td><code>${c.handle}</code></td>
          <td><span class="badge badge-optimal">${c.platform}</span></td>
          <td>${(c.follower_count || 0).toLocaleString()}</td>
          <td><strong>${(c.baseline_median_views || 0).toLocaleString()}</strong></td>
          <td>${c.last_crawled_at ? new Date(c.last_crawled_at).toLocaleDateString() : 'Pending'}</td>
        </tr>
      `).join('');
    } catch (err) {
      listContainer.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--yt-status-error);">Error loading watchlist: ${err.message}</td></tr>`;
    }
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tab 3: The Hook Machine Handler
  // ─────────────────────────────────────────────────────────────────────────────
  const hookForm = document.getElementById('hook-form');
  const hookResultsContainer = document.getElementById('hook-results');

  if (hookForm) {
    hookForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      const channel = document.getElementById('hook-channel-input').value;
      const topic = document.getElementById('hook-topic-input').value;
      const btn = hookForm.querySelector('button[type="submit"]');

      btn.disabled = true;
      btn.innerHTML = '⏳ Synthesizing Hooks...';
      hookResultsContainer.innerHTML = '<div class="studio-card">Synthesizing 5 adapted & 5 scratch hooks with cognitive friction...</div>';

      try {
        const res = await fetch('/api/hook-machine', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ channel, topic }),
        });
        const data = await res.json();
        renderHooks(data);
      } catch (err) {
        hookResultsContainer.innerHTML = `<div class="studio-card" style="border-color: var(--yt-status-error);">❌ Hook Synthesis Failed: ${err.message}</div>`;
      } finally {
        btn.disabled = false;
        btn.innerHTML = '🎣 Synthesize Hooks';
      }
    });
  }

  function renderHooks(data) {
    const hooks = data.hooks || [];
    let html = `
      <div class="studio-card" style="margin-bottom: 20px;">
        <h3>🎯 Synthesized Hook Concepts for "${data.topic || 'Topic'}"</h3>
        <p style="color: var(--yt-spec-text-secondary); font-size: 13px; margin-bottom: 16px;">
          Niche Category: <strong>${data.niche_category || 'Universal'}</strong> | Rubric: <strong>${data.rubric_tier || 'Elite'}</strong>
        </p>
    `;

    hooks.forEach((h, idx) => {
      html += `
        <div class="hook-box">
          <div style="flex: 1;">
            <div class="hook-text">#${idx + 1}: ${h.hook_text || h}</div>
            <div class="hook-meta">${h.madlib_pattern || 'High-converting viral formula'}</div>
          </div>
          <button class="btn-secondary" onclick="navigator.clipboard.writeText('${(h.hook_text || h).replace(/'/g, "\\'")}')">Copy</button>
        </div>
      `;
    });

    html += '</div>';
    hookResultsContainer.innerHTML = html;
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tab 4: Pre-Production Shoot Sheet Handler
  // ─────────────────────────────────────────────────────────────────────────────
  const scriptForm = document.getElementById('script-form');
  const scriptResultsContainer = document.getElementById('script-results');

  if (scriptForm) {
    scriptForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      const topic = document.getElementById('script-title-input').value;
      const channel = document.getElementById('script-creator-input').value;
      const duration = parseInt(document.getElementById('script-duration-select').value, 10);
      const btn = scriptForm.querySelector('button[type="submit"]');

      btn.disabled = true;
      btn.innerHTML = '⏳ Generating Shoot Sheet...';
      scriptResultsContainer.innerHTML = '<div class="studio-card">Generating Moment Sentence, 4 Extreme Opposites, and 4 Ps Shot Clock...</div>';

      try {
        const res = await fetch('/api/script', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ channel, topic, duration_sec: duration }),
        });
        const data = await res.json();
        renderShootSheet(data);
      } catch (err) {
        scriptResultsContainer.innerHTML = `<div class="studio-card" style="border-color: var(--yt-status-error);">❌ Generation Failed: ${err.message}</div>`;
      } finally {
        btn.disabled = false;
        btn.innerHTML = '🎬 Generate Shoot Sheet';
      }
    });
  }

  function renderShootSheet(data) {
    const moment = data.peak_moment_sentence || 'The exact moment of peak tension.';
    const briefs = data.thumbnail_concepts || [];
    const beats = data.cold_open_beats || [];

    let briefsHtml = briefs.map(b => `
      <div class="studio-card" style="margin-bottom: 12px; background: var(--yt-spec-additive-background);">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
          <strong>Concept ${b.concept_id}: ${b.title}</strong>
          <span class="badge badge-elite">${b.overlay_text || 'OVERLAY'}</span>
        </div>
        <p style="font-size: 13px; color: var(--yt-spec-text-secondary); margin-bottom: 6px;">${b.description}</p>
        <div style="font-size: 12px; font-family: monospace; color: var(--yt-spec-call-to-action);">📷 ${b.camera_checklist || '24mm wide angle, crisp focal point'}</div>
      </div>
    `).join('');

    let beatsHtml = beats.map((b, i) => `
      <tr>
        <td><code>00:0${i * 2}.0 - 00:0${(i + 1) * 2}.4</code></td>
        <td><strong>${b.spoken_line || b[0] || ''}</strong></td>
        <td style="color: var(--yt-spec-text-secondary);">${b.visual_action || b[1] || ''}</td>
        <td><span class="badge badge-optimal">${b.overlay_slug || b[2] || 'BEAT'}</span></td>
      </tr>
    `).join('');

    scriptResultsContainer.innerHTML = `
      <div class="studio-card" style="margin-bottom: 20px; border-left: 4px solid var(--yt-brand-red);">
        <h3>🎬 Peak Moment Sentence (The Unstoppable Thumbnail Anchor)</h3>
        <p style="font-size: 16px; font-weight: 600; margin-top: 8px;">"${moment}"</p>
      </div>

      <div class="studio-card" style="margin-bottom: 20px;">
        <h3>📷 4 Extreme Opposite Thumbnail Concept Briefs</h3>
        <div style="margin-top: 14px;">${briefsHtml || '<p>No concept briefs generated.</p>'}</div>
      </div>

      <div class="studio-card" style="margin-bottom: 20px;">
        <h3>⏱️ 4 Ps Opening Shot Clock (0.0s - 10.0s)</h3>
        <div class="table-container" style="margin-top: 12px;">
          <table class="studio-table">
            <thead>
              <tr>
                <th>Timecode</th>
                <th>Spoken Dialogue Beat</th>
                <th>Visual B-Roll / Action</th>
                <th>Overlay Slug</th>
              </tr>
            </thead>
            <tbody>
              ${beatsHtml || '<tr><td colspan="4">No opening beats recorded.</td></tr>'}
            </tbody>
          </table>
        </div>
      </div>
      <button class="btn-cta" onclick="window.print()" style="margin-bottom: 20px;">🖨️ Print 1-Page Shoot Sheet</button>
    `;
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tab 5: Daemon Health & Status Handler
  // ─────────────────────────────────────────────────────────────────────────────
  async function loadStatus() {
    const container = document.getElementById('status-metrics');
    try {
      const res = await fetch('/api/status');
      const data = await res.json();
      state.status = data;

      container.innerHTML = `
        <div class="card-grid">
          <div class="studio-card">
            <div class="metric-value">${data.tracked_channels || 0}</div>
            <div class="metric-label">Tracked Channels</div>
          </div>
          <div class="studio-card">
            <div class="metric-value">${data.videos_analyzed || 0}</div>
            <div class="metric-label">Videos Ingested</div>
          </div>
          <div class="studio-card">
            <div class="metric-value">${data.indexed_hooks || 0}</div>
            <div class="metric-label">Semantic Hooks Stored</div>
          </div>
          <div class="studio-card">
            <div class="metric-value">${data.mcp_version || '2024-11-05'}</div>
            <div class="metric-label">MCP Protocol Spec</div>
          </div>
        </div>
        <div class="studio-card">
          <h3>⚙️ System Diagnostic Details</h3>
          <p style="font-size: 13px; margin-top: 8px;"><strong>Database Status:</strong> ${data.database_status || 'Mounted SQLite'}</p>
          <p style="font-size: 13px; margin-top: 4px;"><strong>Service:</strong> sandsifter v0.1.0 (Native 100% Rust Single-Binary)</p>
          <p style="font-size: 13px; margin-top: 4px;"><strong>HTTP / SSE Daemon:</strong> Port 8080 Active</p>
        </div>
      `;
    } catch (err) {
      container.innerHTML = `<div class="studio-card" style="border-color: var(--yt-status-error);">❌ Failed to load status: ${err.message}</div>`;
    }
  }

  // Event Listeners
  if (themeToggleBtn) {
    themeToggleBtn.addEventListener('click', () => {
      applyTheme(state.theme === 'dark' ? 'light' : 'dark');
    });
  }

  navChips.forEach(chip => {
    chip.addEventListener('click', () => {
      switchTab(chip.dataset.tab);
    });
  });

  if (auditForm) {
    auditForm.addEventListener('submit', (e) => {
      e.preventDefault();
      runAudit(auditUrlInput.value.trim());
    });
  }

  // Browse Local Video from Hard Drive Handler
  const browseLocalBtn = document.getElementById('browse-local-btn');
  const localVideoFilePicker = document.getElementById('local-video-file-picker');

  if (browseLocalBtn) {
    browseLocalBtn.addEventListener('click', async () => {
      const originalText = browseLocalBtn.innerHTML;
      browseLocalBtn.disabled = true;
      browseLocalBtn.innerHTML = '⏳ Opening File Picker...';

      try {
        const res = await fetch('/api/browse-local-video', { method: 'POST' });
        if (res.ok) {
          const data = await res.json();
          if (data.status === 'success' && data.file_path) {
            auditUrlInput.value = data.file_path;
            runAudit(data.file_path);
            return;
          } else if (data.status === 'cancelled') {
            console.log('File selection cancelled by user.');
            return;
          } else if (data.status === 'error') {
            alert('File dialog error: ' + (data.message || 'Unknown error'));
            return;
          }
        }
        const errText = await res.text();
        console.warn('Backend file dialog returned error:', errText);
        const manualPath = prompt('Native dialog unavailable. Please enter local video file path (e.g. C:\\Videos\\render.mp4):');
        if (manualPath && manualPath.trim()) {
          auditUrlInput.value = manualPath.trim();
          runAudit(manualPath.trim());
        }
      } catch (err) {
        console.warn('Native picker fetch failed:', err);
        const manualPath = prompt('Could not connect to file dialog. Please enter local video file path:');
        if (manualPath && manualPath.trim()) {
          auditUrlInput.value = manualPath.trim();
          runAudit(manualPath.trim());
        }
      } finally {
        browseLocalBtn.disabled = false;
        browseLocalBtn.innerHTML = originalText;
      }
    });
  }

  if (localVideoFilePicker) {
    localVideoFilePicker.addEventListener('change', () => {
      if (localVideoFilePicker.files && localVideoFilePicker.files[0]) {
        const file = localVideoFilePicker.files[0];
        const localPath = file.path || file.name;
        auditUrlInput.value = localPath;
        runAudit(localPath);
      }
    });
  }

  // Sample Audit Buttons
  document.querySelectorAll('.sample-url-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      auditUrlInput.value = btn.dataset.url;
      runAudit(btn.dataset.url);
    });
  });

  // Handle Browser Back/Forward
  window.addEventListener('popstate', () => {
    const path = window.location.pathname.replace('/', '') || 'audit';
    switchTab(path);
  });

  // Initial Boot
  applyTheme(state.theme);
  const initialPath = window.location.pathname.replace('/', '') || 'audit';
  switchTab(initialPath);
  checkHealth();
  setInterval(checkHealth, 15000);

})();
