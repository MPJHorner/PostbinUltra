---
title: "Postbin Ultra: local HTTP request inspector"
description: "Capture, inspect, replay, and proxy any HTTP request on a local port. Single Rust binary, live web UI, no accounts, no cloud."
slug: ""
layout: home
---

<section class="hero">
  <span class="hero-eyebrow"><span class="badge">v{{version}}</span> Local-first HTTP inspector</span>
  <h1>Capture every request. <span class="accent">Right where you work.</span></h1>
  <p class="lede">Postbin Ultra is a local HTTP request inspector for developers. Point any webhook, SDK, or test client at a port on your machine and watch every request appear in your terminal and a live web UI. No accounts, no tunnels, no data leaving your laptop.</p>

  <div class="hero-actions">
    <a class="btn primary" href="{{base}}/install/">Install Postbin Ultra</a>
    <a class="btn ghost" href="{{repo}}" rel="noopener noreferrer">
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8a8 8 0 0 0 5.47 7.59c.4.07.55-.17.55-.38v-1.33c-2.23.48-2.7-1.07-2.7-1.07-.36-.92-.89-1.17-.89-1.17-.73-.5.06-.49.06-.49.81.06 1.23.83 1.23.83.72 1.23 1.88.88 2.34.67.07-.52.28-.88.51-1.08-1.78-.2-3.65-.89-3.65-3.95 0-.87.31-1.59.83-2.15-.08-.21-.36-1.02.08-2.13 0 0 .67-.21 2.2.82a7.6 7.6 0 0 1 4 0c1.53-1.04 2.2-.82 2.2-.82.44 1.11.16 1.92.08 2.13.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.66 3.95.29.25.54.73.54 1.48v2.2c0 .21.15.46.55.38A8 8 0 0 0 16 8c0-4.42-3.58-8-8-8Z"/></svg>
      View on GitHub
    </a>
  </div>

  <div class="hero-meta">
    <span><strong>Rust</strong> · single binary, ~5 MB</span>
    <span><strong>macOS · Linux · Windows</strong></span>
    <span><strong>MIT</strong> licensed</span>
    <span><strong>0 telemetry</strong> · runs offline</span>
  </div>

  <figure class="hero-figure">
    <img src="{{base}}/img/screenshot.png" alt="Postbin Ultra web UI showing a captured POST request with formatted JSON body, headers, and replay tab" width="1600" height="1000" />
  </figure>
</section>

<section class="section">
  <div class="section-eyebrow">Why Postbin Ultra</div>
  <h2>The local alternative to cloud request bins.</h2>
  <p class="section-lede">Most request bins are SaaS tools. You sign up, get a random URL, copy it into the system you're debugging, and wait for traffic to round-trip through someone else's cloud. Postbin Ultra is a single binary that does the same job on <code>localhost</code>, with stronger formatting, no accounts, no rate limits, and no data leaving your machine.</p>

  <div class="feature-grid">
    <div class="feature">
      <h3><span class="feature-icon">⚡</span> Real-time</h3>
      <p>Live CLI stream and a Server-Sent-Events web UI. Requests appear the instant they hit the port.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">⌨</span> Any method, any path</h3>
      <p>Catch-all router. <span class="method-badge GET">GET</span> <span class="method-badge POST">POST</span> <span class="method-badge PUT">PUT</span> <span class="method-badge PATCH">PATCH</span> <span class="method-badge DELETE">DELETE</span> and any custom verb on any URL.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">⇄</span> Proxy mode</h3>
      <p><code>--forward</code> turns Postbin into a transparent man-in-the-middle. Capture, then relay to a real upstream.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">{ }</span> Smart formatters</h3>
      <p>Collapsible JSON, form-encoded tables, multipart parts, image previews, hex dumps with ASCII gutter.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">▶</span> Replay</h3>
      <p>Re-fire any captured request to a target URL from the browser. Headers and body intact.</p>
    </div>
    <div class="feature">
      <h3><span class="feature-icon">🗎</span> NDJSON log</h3>
      <p><code>--log-file</code> tails into a structured log so AI assistants can watch live traffic alongside you.</p>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-eyebrow">30-second tour</div>
  <h2>Run it. Send a request. See it.</h2>
  <p class="section-lede">No flags needed. Defaults bind <code>127.0.0.1:9000</code> for capture and <code>127.0.0.1:9001</code> for the UI. If a port is busy, the next free one is used and the actual URL is printed.</p>

  <div class="tour">
    <div class="tour-step">
      <span class="step-num">01</span>
      <h3>Run it</h3>
      <p>One binary, no config. Banner shows the URLs it bound.</p>
<pre>$ postbin-ultra
  ▶ Postbin Ultra v{{version}}
    Capture  <span class="t-mute">http://127.0.0.1:9000</span>
    Web UI   <span class="t-mute">http://127.0.0.1:9001</span></pre>
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
      <p>Terminal stream + live web UI. Replay, copy as curl, jump to the next request with <kbd>j</kbd>.</p>
<pre><span class="t-mute">14:23:45.123</span>  <span class="t-method-post">POST</span>  /webhook        45 B  application/json</pre>
    </div>
  </div>
</section>

<section class="section">
  <div class="section-eyebrow">Built for the way you actually debug</div>
  <h2>Use cases.</h2>

  <div class="use-grid">
    <div class="use">
      <h3>Webhook debugging</h3>
      <p>Stripe, GitHub, Shopify, Slack, Twilio, Sentry. See exactly what they send, formatted.</p>
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
      <h3>Replay against staging</h3>
      <p>Capture a request once, then re-fire it from the UI to your dev server.</p>
    </div>
    <div class="use">
      <h3>AI-assistant pairing</h3>
      <p><code>--log-file</code> + <code>--forward</code> lets a coding agent watch live traffic while you keep working.</p>
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

<div class="code-block"><span class="code-lang">sh</span><button class="copy-btn" type="button" aria-label="Copy code">copy</button><pre><code class="language-sh">curl -L -o postbin-ultra.tar.gz \
  https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz
tar -xzf postbin-ultra.tar.gz
./postbin-ultra</code></pre></div>

  <p>Other platforms, package managers, and source builds are listed on the <a href="{{base}}/install/">install page</a>.</p>

  <div class="cta-card">
    <div>
      <h3>Read the full reference.</h3>
      <p>Every flag, every API endpoint, every shortcut. Searchable, mobile-friendly, and always in sync with the code.</p>
    </div>
    <div>
      <a class="btn primary" href="{{base}}/cli/">CLI reference</a>
      <a class="btn ghost" href="{{base}}/api/">API reference</a>
    </div>
  </div>
</section>
