---
title: "Postbin Ultra: native HTTP request inspector"
description: "Native macOS, Linux, and Windows app for capturing and inspecting HTTP requests. JSON tree view, syntax highlighting, forward proxy with replay. No accounts, no cloud."
slug: ""
layout: home
---

<section class="hero">
  <span class="hero-eyebrow"><span class="badge">v{{version}}</span> Native HTTP inspector</span>
  <h1>Capture every request. <span class="accent">Inspect it like a native.</span></h1>
  <p class="lede">Postbin Ultra is a native desktop app that catches every HTTP request landing on a port on your machine and renders it the way you actually want to read it — JSON tree view with collapse/expand, syntax-highlighted XML and HTML, forward proxy with one-click replay, attempt history. No accounts, no tunnels, no data leaving your laptop.</p>

  <div class="hero-actions">
    <a class="btn primary" href="{{base}}/install/">Install Postbin Ultra</a>
    <a class="btn ghost" href="{{repo}}" rel="noopener noreferrer">
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8a8 8 0 0 0 5.47 7.59c.4.07.55-.17.55-.38v-1.33c-2.23.48-2.7-1.07-2.7-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.06-.49.06-.49.81.06 1.23.83 1.23.83.72 1.23 1.88.88 2.34.67.07-.52.28-.88.51-1.08-1.78-.2-3.65-.89-3.65-3.95 0-.87.31-1.59.83-2.15-.08-.21-.36-1.02.08-2.13 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.92.08 2.13.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.66 3.95.29.25.54.73.54 1.48v2.2c0 .21.15.46.55.38A8 8 0 0 0 16 8c0-4.42-3.58-8-8-8Z"/></svg>
      View on GitHub
    </a>
  </div>

  <div class="hero-meta">
    <span><strong>Native Rust + egui</strong></span>
    <span><strong>macOS · Linux · Windows</strong></span>
    <span><strong>MIT</strong> licensed</span>
    <span><strong>0 telemetry</strong> · runs offline</span>
  </div>

  <figure class="hero-figure">
    <img src="{{base}}/img/screenshot.png" alt="Postbin Ultra desktop app showing a captured Stripe webhook with the JSON body rendered as a collapsible tree, headers grid, and the Forwarded tab with attempt history" width="1600" height="1000" />
  </figure>
</section>

<section class="section">
  <div class="section-eyebrow">Why Postbin Ultra</div>
  <h2>The local-first alternative to webhook.site.</h2>
  <p class="section-lede">Most request inspectors are SaaS tools — sign up, get a random URL, copy it into the system you're debugging, wait for traffic to round-trip through someone else's cloud. Postbin Ultra is a native app that does the same job on <code>localhost</code>, with stronger formatting, no accounts, no rate limits, and no data leaving your machine. Every captured request lives in a bounded ring buffer in RAM and disappears when you close the app.</p>

  <div class="feature-grid">
    <div class="feature">
      <h3><span class="feature-icon">⚡</span> Real-time</h3>
      <p>Captures arrive instantly in the sidebar. Click any row to inspect headers, query, body, and the forwarded response.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">⌨</span> Any method, any path</h3>
      <p>Catch-all router. <span class="method-badge GET">GET</span> <span class="method-badge POST">POST</span> <span class="method-badge PUT">PUT</span> <span class="method-badge PATCH">PATCH</span> <span class="method-badge DELETE">DELETE</span> <span class="method-badge OPTIONS">OPTIONS</span> + custom verbs on any URL.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">⇄</span> Forward + replay</h3>
      <p>Turn Postbin into a transparent proxy with one click. Every forwarded request stores the upstream response. <a href="{{base}}/forward/">Click "Replay" to fire it again</a> — every attempt lands in an attempt-history table.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">{ }</span> JSON tree view</h3>
      <p>Collapsible objects and arrays, syntax-highlighted JSON / XML / HTML, hex view for binary, decoded form-urlencoded, multipart-aware. Expand all / Collapse all in one click.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">▶</span> Replay history</h3>
      <p>Re-fire any captured request through the current forward target. The new attempt lands in a table; click any row to compare 200 → 500 → 200 over time.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">🎚</span> Method-chip filter</h3>
      <p>Toggle GET / POST / PUT / PATCH / DELETE / OPTIONS / HEAD / OTHER chips alongside a free-text filter. See only the requests that matter right now.</p>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-eyebrow">30-second tour</div>
  <h2>Open it. Send a request. See it.</h2>
  <p class="section-lede">No setup. Default capture port is <code>127.0.0.1:9000</code>. Click the capture pill in the top bar to copy the URL.</p>

  <div class="tour">
    <div class="tour-step">
      <span class="step-num">01</span>
      <h3>Open it</h3>
      <p>The capture URL is shown in the top bar. Click to copy.</p>
<pre>Postbin Ultra v{{version}}
  Capture  <span class="t-mute">http://127.0.0.1:9000</span>
  Forward  <span class="t-mute">not set</span></pre>
    </div>
    <div class="tour-step">
      <span class="step-num">02</span>
      <h3>Send a request</h3>
      <p>Anything HTTP works. Webhooks, SDKs, scripts, browsers.</p>
<pre>$ curl -X POST <span class="t-mute">http://127.0.0.1:9000/webhook</span> \
  -H 'content-type: application/json' \
  -d '{"event":"user.created"}'</pre>
    </div>
    <div class="tour-step">
      <span class="step-num">03</span>
      <h3>Inspect it</h3>
      <p>Click the row in the sidebar. Body / Headers / Query / Raw / Forwarded tabs are all one keystroke away.</p>
<pre><span class="t-method-post">POST</span>  /webhook        45 B  now</pre>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-eyebrow">Built for the way you actually debug</div>
  <h2>Use cases.</h2>

  <div class="use-grid">
    <div class="use">
      <h3>Webhook debugging</h3>
      <p>Stripe, GitHub, Shopify, Slack, Twilio, SendGrid. See exactly what they send, formatted.</p>
    </div>
    <div class="use">
      <h3>SDK inspection</h3>
      <p>Find out what an HTTP client or generated SDK actually puts on the wire.</p>
    </div>
    <div class="use">
      <h3>Reverse-engineering</h3>
      <p>Point a third-party integration at Postbin and decode the protocol from the captures.</p>
    </div>
    <div class="use">
      <h3>Forward to a real upstream</h3>
      <p>Capture <em>and</em> relay to staging in one step. The Forwarded tab shows the upstream response so you can debug both sides at once.</p>
    </div>
    <div class="use">
      <h3>Replay until it works</h3>
      <p>Hit Replay to re-fire a captured request through the current forward target. Each attempt lands in the history table — compare 500 → 200 across deploys.</p>
    </div>
    <div class="use">
      <h3>Learning HTTP</h3>
      <p>Headers, query strings, multipart, content encodings, all rendered in a way you can actually read.</p>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-eyebrow">Get started</div>
  <h2>Install in 30 seconds.</h2>

<div class="code-block"><span class="code-lang">sh</span><button class="copy-btn" type="button" aria-label="Copy code">copy</button><pre><code class="language-sh">curl -sSL https://raw.githubusercontent.com/MPJHorner/PostbinUltra/main/scripts/install.sh | bash</code></pre></div>

  <p>Or grab the platform package manually from the <a href="{{base}}/install/">install page</a> — <code>.dmg</code> for macOS, <code>.tar.gz</code> for Linux, <code>.zip</code> for Windows.</p>

  <div class="cta-card">
    <div>
      <h3>Read the docs.</h3>
      <p>Forward setup, attempt history, every settings tab, every keyboard shortcut. Always in sync with the latest release.</p>
    </div>
    <div>
      <a class="btn primary" href="{{base}}/quick-start/">Quick start</a>
      <a class="btn ghost" href="{{base}}/forward/">Forward + replay</a>
    </div>
  </div>
</section>
