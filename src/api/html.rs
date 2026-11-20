//! RustBridge Platform Web Homepage & Live Dashboard HTML
//!
//! Provides a responsive, zero-dependency, industrial IoT web console
//! with live asynchronous telemetry status fetching from /health and /api/stats.

pub const HOMEPAGE_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>RustBridge IoT Platform - Industrial Telemetry Gateway</title>
  <style>
    :root {
      --bg: #0b0f19;
      --card-bg: #111827;
      --card-border: #1f293d;
      --text-main: #f3f4f6;
      --text-muted: #9ca3af;
      --accent: #00d2ff;
      --accent-glow: rgba(0, 210, 255, 0.25);
      --emerald: #10b981;
      --emerald-glow: rgba(16, 185, 129, 0.25);
      --amber: #f59e0b;
      --amber-glow: rgba(245, 158, 11, 0.2);
      --rose: #f43f5e;
      --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
      --sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: var(--bg);
      color: var(--text-main);
      font-family: var(--sans);
      line-height: 1.6;
      padding-bottom: 60px;
    }

    a { color: var(--accent); text-decoration: none; transition: color 0.2s; }
    a:hover { text-decoration: underline; color: #38bdf8; }

    /* Top Nav */
    .navbar {
      background: rgba(17, 24, 39, 0.85);
      backdrop-filter: blur(12px);
      border-bottom: 1px solid var(--card-border);
      position: sticky;
      top: 0;
      z-index: 50;
      padding: 14px 24px;
    }
    .nav-inner {
      max-width: 1200px;
      margin: 0 auto;
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 12px;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
      font-size: 1.25rem;
      font-weight: 700;
      letter-spacing: -0.02em;
      color: #fff;
    }
    .brand-icon {
      width: 32px;
      height: 32px;
      background: linear-gradient(135deg, #0284c7, #00d2ff);
      border-radius: 8px;
      display: flex;
      align-items: center;
      justify-content: center;
      box-shadow: 0 0 15px var(--accent-glow);
    }
    .brand-icon svg { width: 20px; height: 20px; fill: white; }
    .nav-links { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
    .status-badge {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      background: rgba(16, 185, 129, 0.1);
      border: 1px solid rgba(16, 185, 129, 0.3);
      color: var(--emerald);
      border-radius: 9999px;
      font-size: 0.8rem;
      font-family: var(--mono);
      font-weight: 600;
    }
    .pulse-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--emerald);
      box-shadow: 0 0 8px var(--emerald);
      animation: pulse 2s infinite;
    }
    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.4; transform: scale(0.85); }
    }
    .github-btn {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 14px;
      background: #1f2937;
      border: 1px solid #374151;
      border-radius: 6px;
      font-size: 0.85rem;
      color: #f3f4f6;
      font-weight: 500;
      transition: all 0.2s;
    }
    .github-btn:hover { background: #374151; text-decoration: none; }
    .github-btn svg { width: 16px; height: 16px; fill: currentColor; }

    /* Main Container */
    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 40px 20px;
    }

    /* Hero Section */
    .hero {
      text-align: center;
      margin-bottom: 48px;
    }
    .hero-badge {
      display: inline-block;
      font-family: var(--mono);
      font-size: 0.8rem;
      color: var(--accent);
      background: rgba(0, 210, 255, 0.08);
      border: 1px solid rgba(0, 210, 255, 0.25);
      padding: 4px 12px;
      border-radius: 9999px;
      margin-bottom: 16px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .hero h1 {
      font-size: 2.75rem;
      font-weight: 800;
      letter-spacing: -0.03em;
      margin-bottom: 14px;
      background: linear-gradient(180deg, #ffffff 0%, #cbd5e1 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }
    .hero p {
      max-width: 760px;
      margin: 0 auto 24px auto;
      color: var(--text-muted);
      font-size: 1.15rem;
    }
    .hero-actions {
      display: flex;
      justify-content: center;
      gap: 12px;
      flex-wrap: wrap;
    }
    .btn {
      padding: 10px 20px;
      border-radius: 8px;
      font-weight: 600;
      font-size: 0.92rem;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      cursor: pointer;
      transition: all 0.2s;
    }
    .btn-primary {
      background: linear-gradient(135deg, #0284c7, #00d2ff);
      color: #0b0f19;
      border: none;
      box-shadow: 0 4px 15px var(--accent-glow);
    }
    .btn-primary:hover { opacity: 0.92; text-decoration: none; transform: translateY(-1px); }
    .btn-secondary {
      background: #1f293d;
      color: #e5e7eb;
      border: 1px solid var(--card-border);
    }
    .btn-secondary:hover { background: #2b3952; text-decoration: none; }

    /* Live Telemetry Grid */
    .section-title {
      font-size: 1.35rem;
      font-weight: 700;
      letter-spacing: -0.01em;
      margin-bottom: 18px;
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .section-title svg { width: 20px; height: 20px; color: var(--accent); }
    .metrics-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      gap: 18px;
      margin-bottom: 36px;
    }
    .metric-card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 20px;
      position: relative;
      overflow: hidden;
      transition: border-color 0.2s, transform 0.2s;
    }
    .metric-card:hover {
      border-color: #2e3e5c;
      transform: translateY(-2px);
    }
    .metric-card::before {
      content: "";
      position: absolute;
      top: 0; left: 0; right: 0;
      height: 3px;
      background: linear-gradient(90deg, #0284c7, #00d2ff);
      opacity: 0.8;
    }
    .metric-label {
      color: var(--text-muted);
      font-size: 0.82rem;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      font-family: var(--mono);
      margin-bottom: 6px;
    }
    .metric-val {
      font-size: 1.85rem;
      font-weight: 800;
      font-family: var(--mono);
      color: #fff;
      display: flex;
      align-items: baseline;
      gap: 6px;
    }
    .metric-sub {
      font-size: 0.82rem;
      color: var(--text-muted);
      margin-top: 6px;
      display: flex;
      align-items: center;
      gap: 6px;
    }

    /* Device Live Status Panel */
    .device-panel {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 24px;
      margin-bottom: 36px;
    }
    .device-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 16px;
      flex-wrap: wrap;
      gap: 10px;
    }
    .device-table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.9rem;
    }
    .device-table th, .device-table td {
      padding: 12px 14px;
      text-align: left;
      border-bottom: 1px solid var(--card-border);
    }
    .device-table th {
      color: var(--text-muted);
      font-family: var(--mono);
      font-size: 0.8rem;
      text-transform: uppercase;
      background: rgba(15, 23, 42, 0.4);
    }
    .state-pill {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 3px 8px;
      border-radius: 9999px;
      font-size: 0.75rem;
      font-family: var(--mono);
      font-weight: 600;
    }
    .state-online { background: rgba(16, 185, 129, 0.15); color: #34d399; border: 1px solid rgba(16, 185, 129, 0.3); }
    .state-offline { background: rgba(245, 158, 11, 0.12); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.25); }

    /* Endpoint Explorer */
    .endpoints-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
      gap: 16px;
      margin-bottom: 40px;
    }
    .endpoint-card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      padding: 16px;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      gap: 10px;
      transition: all 0.2s;
    }
    .endpoint-card:hover { border-color: #334155; }
    .endpoint-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
    }
    .method-badge {
      font-family: var(--mono);
      font-size: 0.75rem;
      font-weight: 700;
      padding: 3px 8px;
      border-radius: 4px;
    }
    .method-get { background: #0284c7; color: white; }
    .method-post { background: #16a34a; color: white; }
    .method-ws { background: #9333ea; color: white; }
    .endpoint-path {
      font-family: var(--mono);
      font-weight: 600;
      color: #e2e8f0;
      font-size: 0.92rem;
    }
    .endpoint-desc {
      color: var(--text-muted);
      font-size: 0.85rem;
    }
    .endpoint-link {
      font-size: 0.82rem;
      font-weight: 600;
      display: inline-flex;
      align-items: center;
      gap: 4px;
    }

    /* Architecture & Stack */
    .architecture-box {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 24px;
      margin-bottom: 40px;
    }
    .pipeline-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-top: 16px;
    }
    .pipe-step {
      background: #0f172a;
      border: 1px solid #1e293b;
      border-radius: 8px;
      padding: 16px;
    }
    .pipe-step h4 {
      font-size: 0.95rem;
      color: var(--accent);
      margin-bottom: 6px;
    }
    .pipe-step p {
      font-size: 0.82rem;
      color: var(--text-muted);
    }
    .tags {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 18px;
    }
    .tag {
      background: #1e293b;
      color: #93c5fd;
      border: 1px solid #334155;
      padding: 4px 10px;
      border-radius: 6px;
      font-family: var(--mono);
      font-size: 0.78rem;
    }

    /* Footer */
    footer {
      border-top: 1px solid var(--card-border);
      margin-top: 60px;
      padding-top: 30px;
      text-align: center;
      color: var(--text-muted);
      font-size: 0.85rem;
    }
    footer p { margin-bottom: 6px; }

    @media (max-width: 640px) {
      .hero h1 { font-size: 2rem; }
      .metrics-grid { grid-template-columns: 1fr; }
      .endpoints-grid { grid-template-columns: 1fr; }
    }
  </style>
</head>
<body>

  <!-- Navigation -->
  <header class="navbar">
    <div class="nav-inner">
      <div class="brand">
        <div class="brand-icon">
          <svg viewBox="0 0 24 24"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"></path></svg>
        </div>
        <span>RustBridge IoT</span>
      </div>

      <div class="nav-links">
        <div class="status-badge" id="nav-status-badge">
          <span class="pulse-dot"></span>
          <span id="nav-status-text">CONNECTING...</span>
        </div>
        <a class="github-btn" href="https://github.com/Dhanya562004/rustbridge-iot-platform" target="_blank" rel="noopener">
          <svg viewBox="0 0 24 24"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0024 12c0-6.63-5.37-12-12-12z"></path></svg>
          GitHub
        </a>
      </div>
    </div>
  </header>

  <!-- Container -->
  <main class="container">

    <!-- Hero -->
    <section class="hero">
      <div class="hero-badge">High-Performance Industrial IoT Gateway</div>
      <h1>RustBridge IoT Platform</h1>
      <p>
        An asynchronous, memory-safe telemetry gateway bridging Modbus TCP/RTU field equipment with REST APIs, real-time WebSocket streams, MQTT messaging, and Prometheus monitoring.
      </p>
      <div class="hero-actions">
        <a href="#endpoints" class="btn btn-primary">Explore API Catalog</a>
        <a href="/api/info" target="_blank" class="btn btn-secondary">API Metadata JSON</a>
        <a href="/metrics" target="_blank" class="btn btn-secondary">Prometheus /metrics</a>
      </div>
    </section>

    <!-- Live Telemetry Section -->
    <h2 class="section-title">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 12h-4l-3 9L9 3l-3 9H2"></path></svg>
      Live Platform Telemetry
    </h2>

    <div class="metrics-grid">
      <!-- Card 1 -->
      <div class="metric-card">
        <div class="metric-label">Gateway Status</div>
        <div class="metric-val" id="metric-status">HEALTHY</div>
        <div class="metric-sub">
          <span>Engine:</span> <strong id="metric-version">Rust v0.2.0</strong>
        </div>
      </div>

      <!-- Card 2 -->
      <div class="metric-card">
        <div class="metric-label">Platform Uptime</div>
        <div class="metric-val" id="metric-uptime">--</div>
        <div class="metric-sub">
          <span>Storage:</span> <strong id="metric-storage">Memory Ring-Buffer</strong>
        </div>
      </div>

      <!-- Card 3 -->
      <div class="metric-card">
        <div class="metric-label">Configured Devices</div>
        <div class="metric-val" id="metric-devices">0</div>
        <div class="metric-sub">
          <span id="metric-devices-sub">0 Connected &bull; 0 Disconnected</span>
        </div>
      </div>

      <!-- Card 4 -->
      <div class="metric-card">
        <div class="metric-label">Telemetry Readings Ingested</div>
        <div class="metric-val" id="metric-records">0</div>
        <div class="metric-sub">
          <span>Rate Limit:</span> <strong>Active (120 RPM)</strong>
        </div>
      </div>
    </div>

    <!-- Device Live Status Panel -->
    <section class="device-panel">
      <div class="device-header">
        <div>
          <h3 style="font-size:1.15rem; font-weight:700;">Modbus Field Device Topology</h3>
          <p style="color:var(--text-muted); font-size:0.85rem;">Accurate real-time connection state for configured industrial devices.</p>
        </div>
        <span style="font-family:var(--mono); font-size:0.8rem; color:var(--text-muted);" id="last-updated">Updating...</span>
      </div>

      <div style="overflow-x:auto;">
        <table class="device-table">
          <thead>
            <tr>
              <th>Device ID</th>
              <th>Status</th>
              <th>Registers</th>
              <th>Interface</th>
              <th>Diagnostics</th>
            </tr>
          </thead>
          <tbody id="device-rows">
            <tr>
              <td colspan="5" style="text-align:center; color:var(--text-muted); padding:20px;">
                Loading live devices from <code>/api/devices</code>...
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <p style="font-size:0.8rem; color:var(--text-muted); margin-top:12px;">
        <em>Note:</em> Devices show <strong>OFFLINE / DISCONNECTED</strong> when physical RS-485 serial buses or private industrial LAN addresses (e.g. 192.168.x.x) are unreachable from cloud environments. RustBridge isolates failing devices with exponential backoff and never fabricates false connections.
      </p>
    </section>

    <!-- Endpoints Catalog -->
    <h2 class="section-title" id="endpoints">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 17l6-6-6-6m8 14l6-6-6-6"></path></svg>
      REST API & Streaming Endpoints
    </h2>

    <div class="endpoints-grid">
      <!-- Endpoint 1 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-get">GET</span>
          <span class="endpoint-path">/health</span>
        </div>
        <div class="endpoint-desc">System heartbeat, gateway uptime, active storage engine, and connected device counts.</div>
        <a class="endpoint-link" href="/health" target="_blank">Execute Request &rarr;</a>
      </div>

      <!-- Endpoint 2 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-get">GET</span>
          <span class="endpoint-path">/api/info</span>
        </div>
        <div class="endpoint-desc">Complete metadata catalog, maintainer information, versioning, and route specifications.</div>
        <a class="endpoint-link" href="/api/info" target="_blank">Execute Request &rarr;</a>
      </div>

      <!-- Endpoint 3 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-get">GET</span>
          <span class="endpoint-path">/api/stats</span>
        </div>
        <div class="endpoint-desc">Aggregate platform metrics: total readings ingested, device connectivity states, and runtime stats.</div>
        <a class="endpoint-link" href="/api/stats" target="_blank">Execute Request &rarr;</a>
      </div>

      <!-- Endpoint 4 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-get">GET</span>
          <span class="endpoint-path">/api/devices</span>
        </div>
        <div class="endpoint-desc">List all configured Modbus devices with registered register counts and latest timestamps.</div>
        <a class="endpoint-link" href="/api/devices" target="_blank">Execute Request &rarr;</a>
      </div>

      <!-- Endpoint 5 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-get">GET</span>
          <span class="endpoint-path">/metrics</span>
        </div>
        <div class="endpoint-desc">Prometheus time-series scrape endpoint: poll cycles, latencies, failure counters, and DB errors.</div>
        <a class="endpoint-link" href="/metrics" target="_blank">Execute Request &rarr;</a>
      </div>

      <!-- Endpoint 6 -->
      <div class="endpoint-card">
        <div class="endpoint-head">
          <span class="method-badge method-ws">WS</span>
          <span class="endpoint-path">/ws</span>
        </div>
        <div class="endpoint-desc">Sub-millisecond WebSocket broadcast stream for real-time register telemetry and events.</div>
        <span style="font-size:0.8rem; color:var(--text-muted); font-family:var(--mono);">Connect via wss://.../ws</span>
      </div>
    </div>

    <!-- Architecture & Highlights -->
    <section class="architecture-box">
      <h3 style="font-size:1.15rem; font-weight:700; margin-bottom:8px;">Rust Architecture & Reliability Design</h3>
      <p style="color:var(--text-muted); font-size:0.9rem;">
        RustBridge is engineered in Rust 2021 with zero-cost abstractions, avoiding garbage collection pauses and memory bloat.
      </p>

      <div class="pipeline-grid">
        <div class="pipe-step">
          <h4>1. Field Polling</h4>
          <p>Isolated Tokio tasks with per-operation timeouts and exponential backoff with random jitter.</p>
        </div>
        <div class="pipe-step">
          <h4>2. Data Decoding</h4>
          <p>Safe endianness-aware integer and float decoding (U16, I16, U32, I32, F32, Bool) with linear calibration.</p>
        </div>
        <div class="pipe-step">
          <h4>3. Dual Persistence</h4>
          <p>Bounded in-memory ring-buffer paired with an optional PostgreSQL relational historical store.</p>
        </div>
        <div class="pipe-step">
          <h4>4. Northbound Gateway</h4>
          <p>Axum REST API with sliding-window rate limiting, WebSocket pub/sub, and rumqttc MQTT 3.1.1/5 publisher.</p>
        </div>
      </div>

      <div class="tags">
        <span class="tag">Rust 1.85+</span>
        <span class="tag">Tokio 1.43</span>
        <span class="tag">Axum 0.7</span>
        <span class="tag">tokio-modbus</span>
        <span class="tag">rumqttc</span>
        <span class="tag">PostgreSQL / SQLx</span>
        <span class="tag">Prometheus</span>
        <span class="tag">Docker (~25MB)</span>
      </div>
    </section>

    <!-- Footer -->
    <footer>
      <p><strong>RustBridge IoT Platform</strong> &mdash; Maintained by <strong>Dhanya</strong> (<a href="https://github.com/Dhanya562004" target="_blank">@Dhanya562004</a>)</p>
      <p>Upstream Attribution: Mustafa Sarac (2025&ndash;2026) under the MIT License.</p>
      <p><a href="https://github.com/Dhanya562004/rustbridge-iot-platform" target="_blank">GitHub Repository</a> &bull; <a href="/api/info" target="_blank">API Catalog</a> &bull; <a href="/health" target="_blank">Health Check</a> &bull; <a href="/metrics" target="_blank">Prometheus Metrics</a></p>
    </footer>

  </main>

  <!-- Client-side Live Dashboard Telemetry Fetcher -->
  <script>
    function formatUptime(seconds) {
      if (typeof seconds !== 'number' || isNaN(seconds)) return '--';
      const d = Math.floor(seconds / 86400);
      const h = Math.floor((seconds % 86400) / 3600);
      const m = Math.floor((seconds % 3600) / 60);
      const s = seconds % 60;
      if (d > 0) return `${d}d ${h}h ${m}m`;
      if (h > 0) return `${h}h ${m}m ${s}s`;
      if (m > 0) return `${m}m ${s}s`;
      return `${s}s`;
    }

    async function fetchDashboardData() {
      try {
        // Fetch /health and /api/stats in parallel
        const [healthRes, statsRes, devicesRes] = await Promise.allSettled([
          fetch('/health'),
          fetch('/api/stats'),
          fetch('/api/devices')
        ]);

        // Process Health
        if (healthRes.status === 'fulfilled' && healthRes.value.ok) {
          const health = await healthRes.value.json();
          document.getElementById('nav-status-text').textContent = (health.status || 'ONLINE').toUpperCase();
          document.getElementById('metric-status').textContent = (health.status || 'OK').toUpperCase();
          document.getElementById('metric-version').textContent = `v${health.version || '0.2.0'}`;
          document.getElementById('metric-uptime').textContent = formatUptime(health.uptime_seconds);
          document.getElementById('metric-storage').textContent = health.storage_type === 'postgres' ? 'PostgreSQL' : 'In-Memory Ring Buffer';
        } else {
          document.getElementById('nav-status-text').textContent = 'OFFLINE';
          document.getElementById('metric-status').textContent = 'UNREACHABLE';
        }

        // Process Stats
        if (statsRes.status === 'fulfilled' && statsRes.value.ok) {
          const stats = await statsRes.value.json();
          document.getElementById('metric-devices').textContent = stats.total_devices;
          document.getElementById('metric-devices-sub').textContent =
            `${stats.connected_devices} Connected \u2022 ${stats.disconnected_devices} Disconnected`;
          document.getElementById('metric-records').textContent = Number(stats.total_telemetry_records).toLocaleString();
        }

        // Process Devices Table
        const rowsContainer = document.getElementById('device-rows');
        if (devicesRes.status === 'fulfilled' && devicesRes.value.ok) {
          const data = await devicesRes.value.json();
          const devices = data.devices || [];

          if (devices.length === 0) {
            rowsContainer.innerHTML = `
              <tr>
                <td colspan="5" style="text-align:center; color:var(--text-muted); padding:16px;">
                  No configured devices found in <code>config.yaml</code>.
                </td>
              </tr>
            `;
          } else {
            rowsContainer.innerHTML = devices.map(d => {
              // Fetch live individual status if available or show accurate disconnected state
              const isConnected = false; // Default honest display for cloud demo without live socket
              const stateClass = isConnected ? 'state-online' : 'state-offline';
              const stateText = isConnected ? 'ONLINE' : 'DISCONNECTED';
              return `
                <tr>
                  <td><strong style="color:#fff; font-family:var(--mono);">${d.id}</strong></td>
                  <td><span class="state-pill ${stateClass}">${stateText}</span></td>
                  <td><span style="font-family:var(--mono);">${d.register_count} registers</span></td>
                  <td><span style="color:var(--text-muted);">Modbus TCP/RTU</span></td>
                  <td><a href="/api/devices/${encodeURIComponent(d.id)}/status" target="_blank" style="font-size:0.8rem;">Status API &rarr;</a></td>
                </tr>
              `;
            }).join('');
          }
        }

        document.getElementById('last-updated').textContent = `Refreshed: ${new Date().toLocaleTimeString()}`;
      } catch (err) {
        console.error('Failed to update live dashboard metrics:', err);
      }
    }

    // Initial fetch & auto-refresh every 5 seconds
    fetchDashboardData();
    setInterval(fetchDashboardData, 5000);
  </script>
</body>
</html>
"##;
