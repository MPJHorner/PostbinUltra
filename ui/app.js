// PostbinUltra UI — vanilla JS, single file. No build step.

(() => {
  'use strict';

  // ───────── state ─────────
  const state = {
    requests: [],        // newest-first, mirrors server order
    selectedId: null,
    activeTab: 'body',
    paused: false,
    filter: '',
    captureUrl: null,
    bodyCache: new Map(),
    lastReplayUrl: '',
    forward: { enabled: false, url: null, timeout_secs: 30, insecure: false },
  };

  const $ = (sel) => document.querySelector(sel);
  const $$ = (sel) => Array.from(document.querySelectorAll(sel));

  // ───────── boot ─────────
  document.addEventListener('DOMContentLoaded', init);

  async function init() {
    detectTheme();
    wireUi();
    await loadHealth();
    await loadInitial();
    await loadForward();
    connectStream();
  }

  function detectTheme() {
    const saved = localStorage.getItem('pbu-theme');
    if (saved === 'light' || saved === 'dark') {
      document.documentElement.dataset.theme = saved;
    }
  }

  function wireUi() {
    $('#theme-toggle').addEventListener('click', toggleTheme);
    $('#clear-btn').addEventListener('click', confirmClear);
    $('#pause-toggle').addEventListener('click', togglePause);
    $('#copy-url').addEventListener('click', () => copy(state.captureUrl || '', 'Copied capture URL'));
    $('#copy-curl').addEventListener('click', () => copy($('#curl-snippet').textContent, 'Copied'));
    $('#search').addEventListener('input', onSearch);

    $$('.tab').forEach((b) => {
      b.addEventListener('click', () => switchTab(b.dataset.tab));
    });

    $('#help-close')?.addEventListener('click', () => $('#help-dialog').close());
    $('#shortcuts-btn')?.addEventListener('click', () => $('#help-dialog').showModal());

    $('#forward-pill')?.addEventListener('click', openForwardDialog);
    $('#forward-cancel')?.addEventListener('click', () => $('#forward-dialog').close());
    $('#forward-disable')?.addEventListener('click', disableForward);
    $('#forward-form')?.addEventListener('submit', (e) => {
      e.preventDefault();
      saveForward();
    });

    document.addEventListener('keydown', onKeydown);
  }

  // ───────── forward (proxy) ─────────
  async function loadForward() {
    try {
      const res = await fetch('/api/forward');
      if (!res.ok) return;
      state.forward = await res.json();
      renderForwardChip();
    } catch {}
  }

  function renderForwardChip() {
    const chip = $('#forward-pill');
    const code = $('#forward-url-display');
    if (!chip || !code) return;
    const f = state.forward;
    if (!f || !f.enabled) {
      chip.classList.remove('forward-on');
      code.textContent = 'off';
      chip.title = 'Click to enable proxy forward';
    } else {
      chip.classList.add('forward-on');
      code.textContent = f.url;
      const sec = f.insecure ? ' · insecure' : '';
      chip.title = `Forwarding to ${f.url} (timeout ${f.timeout_secs}s${sec}). Click to edit.`;
    }
  }

  function openForwardDialog() {
    const f = state.forward || { enabled: false, url: '', timeout_secs: 30, insecure: false };
    $('#forward-input-url').value = f.url || '';
    $('#forward-input-timeout').value = f.timeout_secs || 30;
    $('#forward-input-insecure').checked = !!f.insecure;
    showForwardError(null);
    $('#forward-disable').hidden = !f.enabled;
    $('#forward-dialog').showModal();
    setTimeout(() => $('#forward-input-url').focus(), 0);
  }

  async function saveForward() {
    const url = $('#forward-input-url').value.trim();
    const timeout_secs = Number($('#forward-input-timeout').value) || 30;
    const insecure = $('#forward-input-insecure').checked;
    if (!url) {
      showForwardError('URL is required');
      return;
    }
    try {
      const res = await fetch('/api/forward', {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ url, timeout_secs, insecure }),
      });
      if (!res.ok) {
        let msg = `${res.status}`;
        try {
          const err = await res.json();
          msg = err.reason || err.error || msg;
        } catch {}
        showForwardError(msg);
        return;
      }
      state.forward = await res.json();
      renderForwardChip();
      renderDetail();
      $('#forward-dialog').close();
      toast('Forward enabled');
    } catch (e) {
      showForwardError(String(e));
    }
  }

  async function disableForward() {
    try {
      const res = await fetch('/api/forward', { method: 'DELETE' });
      if (!res.ok && res.status !== 204) {
        showForwardError(`failed to disable (${res.status})`);
        return;
      }
      state.forward = { enabled: false, url: null, timeout_secs: 30, insecure: false };
      renderForwardChip();
      renderDetail();
      $('#forward-dialog').close();
      toast('Forward disabled');
    } catch (e) {
      showForwardError(String(e));
    }
  }

  function showForwardError(msg) {
    const e = $('#forward-error');
    if (!e) return;
    if (msg) {
      e.textContent = msg;
      e.hidden = false;
    } else {
      e.textContent = '';
      e.hidden = true;
    }
  }

  /** Mirror of the server-side path-prefix join in `forward_request`. */
  function composeForwardUrl(forwardUrl, path, query) {
    if (!forwardUrl) return null;
    let u;
    try {
      u = new URL(forwardUrl);
    } catch {
      return null;
    }
    const basePath = (u.pathname || '').replace(/\/+$/, '');
    u.pathname = basePath + (path || '');
    u.search = query ? '?' + query : '';
    return u.toString();
  }

  // ───────── data ─────────
  async function loadHealth() {
    try {
      const res = await fetch('/api/health');
      const j = await res.json();
      $('#version').textContent = 'v' + (j.version || '');
    } catch {}
  }

  async function loadInitial() {
    try {
      const res = await fetch('/api/requests?limit=1000');
      const list = await res.json();
      state.requests = list;
      render();
      const port = await guessCapturePort();
      const host = window.location.hostname;
      state.captureUrl = `http://${host}:${port}`;
      $('#capture-url').textContent = state.captureUrl;
      $('#curl-snippet').textContent = `curl -X POST ${state.captureUrl}/hello -d 'world'`;
    } catch (e) {
      toast('Failed to load requests');
    }
  }

  /* The UI is hosted on the UI port; the capture port is different. The server
     reports it on /api/health (so port-fallback still gives the right URL).
     If that's missing we probe ui_port ± 1; ?capture=<port> overrides both. */
  async function guessCapturePort() {
    const params = new URLSearchParams(window.location.search);
    const override = params.get('capture');
    if (override) return override;
    try {
      const res = await fetch('/api/health');
      if (res.ok) {
        const j = await res.json();
        if (j && Number.isInteger(j.capture_port)) return j.capture_port;
      }
    } catch {}
    const uiPort = Number(window.location.port || 80);
    const candidates = [uiPort - 1, uiPort + 1];
    for (const p of candidates) {
      try {
        const ok = await Promise.race([
          fetch(`http://${window.location.hostname}:${p}`, { method: 'HEAD', mode: 'no-cors' })
            .then(() => true)
            .catch(() => false),
          new Promise((r) => setTimeout(() => r(false), 400)),
        ]);
        if (ok) return p;
      } catch {}
    }
    return uiPort - 1;
  }

  function connectStream() {
    const es = new EventSource('/api/stream');
    setStatus('connecting');
    es.addEventListener('hello', () => setStatus('connected'));
    es.addEventListener('request', (e) => {
      const req = JSON.parse(e.data);
      if (state.paused) return;
      const existingIdx = state.requests.findIndex((r) => r.id === req.id);
      if (existingIdx >= 0) state.requests.splice(existingIdx, 1);
      state.requests.unshift(req);
      const wasEmpty = state.requests.length === 1;
      render({ flashId: req.id });
      if (wasEmpty) selectRequest(req.id);
    });
    es.addEventListener('cleared', () => {
      state.requests = [];
      state.selectedId = null;
      state.bodyCache.clear();
      render();
    });
    es.addEventListener('resync', async () => {
      await loadInitial();
    });
    es.onerror = () => setStatus('disconnected');
    es.onopen = () => setStatus('connected');
  }

  function setStatus(s) {
    const dot = $('#status');
    dot.className = 'status-dot ' + (s === 'connected' ? 'connected' : '');
    if (state.paused) dot.classList.add('paused');
    dot.title = s.charAt(0).toUpperCase() + s.slice(1);
  }

  // ───────── rendering ─────────
  function render(opts = {}) {
    renderCount();
    renderList(opts);
    renderDetail();
    if (state.requests.length === 0) {
      $('#empty-state').classList.remove('hidden');
    } else {
      $('#empty-state').classList.add('hidden');
    }
  }

  function renderCount() {
    $('#count').textContent = state.requests.length;
  }

  function renderList({ flashId } = {}) {
    const ol = $('#request-list');
    ol.replaceChildren();
    const filtered = applyFilter(state.requests);
    for (const r of filtered) {
      const li = document.createElement('li');
      li.className = 'request-row';
      if (r.id === state.selectedId) li.classList.add('selected');
      if (r.id === flashId) li.classList.add('flash');
      li.dataset.id = r.id;
      li.setAttribute('role', 'option');

      const m = document.createElement('span');
      m.className = 'method-badge ' + r.method;
      m.textContent = r.method;
      li.appendChild(m);

      const path = document.createElement('span');
      path.className = 'row-path';
      path.textContent = r.path + (r.query ? '?' + r.query : '');
      path.title = path.textContent;
      li.appendChild(path);

      const meta = document.createElement('span');
      meta.className = 'row-meta';
      const when = document.createElement('span');
      when.className = 'row-time';
      when.textContent = relativeTime(r.received_at);
      when.title = new Date(r.received_at).toLocaleString();
      const size = document.createElement('span');
      size.className = 'row-size';
      size.textContent = formatBytes(r.body_size);
      meta.appendChild(when);
      meta.appendChild(size);
      li.appendChild(meta);

      li.addEventListener('click', () => selectRequest(r.id));
      ol.appendChild(li);
    }
  }

  function applyFilter(list) {
    const q = state.filter.trim().toLowerCase();
    if (!q) return list;
    return list.filter((r) => {
      if (r.method.toLowerCase().includes(q)) return true;
      if (r.path.toLowerCase().includes(q)) return true;
      if ((r.query || '').toLowerCase().includes(q)) return true;
      if (r.headers.some(([k, v]) => k.toLowerCase().includes(q) || (v || '').toLowerCase().includes(q))) return true;
      if ((r.body || '').toLowerCase().includes(q)) return true;
      return false;
    });
  }

  function renderDetail() {
    const r = state.requests.find((x) => x.id === state.selectedId);
    const detail = $('#detail');
    const empty = $('#detail-empty');
    if (!r) {
      detail.hidden = true;
      empty.classList.remove('hidden');
      return;
    }
    detail.hidden = false;
    empty.classList.add('hidden');

    const m = $('#d-method');
    m.textContent = r.method;
    m.className = 'method-badge ' + r.method;
    $('#d-path').textContent = r.path + (r.query ? '?' + r.query : '');
    $('#d-time').textContent = new Date(r.received_at).toLocaleString();
    $('#d-from').textContent = 'from ' + r.remote_addr;

    $('#t-body-meta').textContent = formatBytes(r.body_size);
    $('#t-headers-meta').textContent = String(r.headers.length);
    const queryCount = countQuery(r.query);
    $('#t-query-meta').textContent = queryCount === 0 ? '' : String(queryCount);

    renderBody(r);
    renderHeaders(r);
    renderQuery(r);
    renderRaw(r);
    renderReplay(r);
    showTab(state.activeTab);
  }

  function selectRequest(id) {
    state.selectedId = id;
    renderList();
    renderDetail();
  }

  // ───────── tab content ─────────
  function renderBody(r) {
    const pane = $('#pane-body');
    pane.replaceChildren();
    pane.appendChild(bodyBanner(r));

    if (r.body_bytes_received === 0) {
      const empty = document.createElement('div');
      empty.className = 'empty-body';
      empty.textContent = '(no body)';
      pane.appendChild(empty);
      return;
    }

    const ct = headerValue(r, 'content-type') || '';
    const ctLow = ct.toLowerCase();
    const bodyText = decodeBodyAsText(r);
    const bytes = decodeBodyAsBytes(r);

    if (ctLow.includes('application/json') || looksLikeJson(bodyText)) {
      try {
        const parsed = JSON.parse(bodyText);
        pane.appendChild(jsonTree(parsed));
        pane.appendChild(downloadRow(r, () => JSON.stringify(parsed, null, 2)));
        return;
      } catch {}
    }

    if (ctLow.includes('application/x-www-form-urlencoded')) {
      pane.appendChild(kvTable(parseFormEncoded(bodyText)));
      pane.appendChild(downloadRow(r, bodyText));
      return;
    }

    if (ctLow.startsWith('image/')) {
      const img = document.createElement('img');
      img.className = 'image-preview';
      img.src = bytesToObjectUrl(bytes, ct);
      pane.appendChild(img);
      // Binary image — no copy button, just the download link.
      pane.appendChild(downloadRow(r));
      return;
    }

    if (ctLow.includes('multipart/form-data')) {
      pane.appendChild(renderMultipart(bytes, ct, r));
      return;
    }

    if (r.body_encoding === 'utf8' || isMostlyText(bytes)) {
      const pre = document.createElement('pre');
      pre.className = 'text-block';
      pre.textContent = bodyText;
      pane.appendChild(pre);
      pane.appendChild(downloadRow(r, bodyText));
      return;
    }

    pane.appendChild(hexView(bytes));
    // Hex view: skip copy. The "Download raw body" link is the right exit.
    pane.appendChild(downloadRow(r));
  }

  function bodyBanner(r) {
    const wrap = document.createElement('div');
    wrap.className = 'body-banner';
    const size = document.createElement('span');
    size.className = 'chip';
    size.textContent = formatBytes(r.body_size);
    wrap.appendChild(size);
    const ct = headerValue(r, 'content-type');
    if (ct) {
      const c = document.createElement('span');
      c.className = 'chip';
      c.textContent = ct;
      wrap.appendChild(c);
    }
    if (r.body_truncated) {
      const t = document.createElement('span');
      t.className = 'chip warn';
      t.textContent = `truncated — original ${formatBytes(r.body_bytes_received)}`;
      wrap.appendChild(t);
    }
    return wrap;
  }

  /**
   * Action row at the bottom-right of a body pane. Always renders a download
   * link; if `copyText` is provided (a string or a () => string thunk), also
   * renders a Copy button. Binary bodies omit the copy button.
   */
  function downloadRow(r, copyText) {
    const row = document.createElement('div');
    row.className = 'copy-row';
    if (copyText) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'btn btn-ghost';
      btn.textContent = 'Copy body';
      btn.addEventListener('click', () => {
        const text = typeof copyText === 'function' ? copyText() : copyText;
        copy(text, 'Body copied');
      });
      row.appendChild(btn);
    }
    const dl = document.createElement('a');
    dl.className = 'btn btn-ghost';
    dl.href = `/api/requests/${r.id}/raw`;
    dl.download = `body-${r.id}`;
    dl.textContent = 'Download raw body';
    row.appendChild(dl);
    return row;
  }

  function renderHeaders(r) {
    const pane = $('#pane-headers');
    pane.replaceChildren();
    if (r.headers.length === 0) {
      const e = document.createElement('div');
      e.className = 'empty-body';
      e.textContent = '(no headers)';
      pane.appendChild(e);
      return;
    }
    const tbl = document.createElement('table');
    tbl.className = 'kv-table';
    for (const [k, v] of r.headers) {
      const tr = document.createElement('tr');
      const th = document.createElement('th');
      th.textContent = k;
      const td = document.createElement('td');
      td.textContent = v;
      tr.appendChild(th);
      tr.appendChild(td);
      tbl.appendChild(tr);
    }
    pane.appendChild(tbl);
    pane.appendChild(
      copyButton('Copy headers', () =>
        r.headers.map(([k, v]) => `${k}: ${v}`).join('\n')
      )
    );
  }

  function renderQuery(r) {
    const pane = $('#pane-query');
    pane.replaceChildren();
    if (!r.query) {
      const e = document.createElement('div');
      e.className = 'empty-body';
      e.textContent = '(no query string)';
      pane.appendChild(e);
      return;
    }
    const params = new URLSearchParams(r.query);
    const rows = [];
    for (const [k, v] of params) rows.push([k, v]);
    pane.appendChild(kvTable(rows));
    pane.appendChild(copyButton('Copy query', () => r.query));
  }

  function renderRaw(r) {
    const pane = $('#pane-raw');
    pane.replaceChildren();

    const curlSection = sectionHeader('curl');
    pane.appendChild(curlSection);
    const curlPre = document.createElement('pre');
    curlPre.className = 'text-block';
    curlPre.textContent = buildCurl(r);
    pane.appendChild(curlPre);
    pane.appendChild(copyButton('Copy curl', () => curlPre.textContent));

    const httpSection = sectionHeader('Raw HTTP');
    pane.appendChild(httpSection);
    const httpPre = document.createElement('pre');
    httpPre.className = 'text-block';
    httpPre.textContent = buildRawHttp(r);
    pane.appendChild(httpPre);
    pane.appendChild(copyButton('Copy HTTP', () => httpPre.textContent));
  }

  function sectionHeader(text) {
    const h = document.createElement('div');
    h.className = 'section-h';
    h.textContent = text;
    return h;
  }

  function copyButton(label, getText) {
    const wrap = document.createElement('div');
    wrap.className = 'copy-row';
    const btn = document.createElement('button');
    btn.className = 'btn btn-ghost';
    btn.textContent = label;
    btn.addEventListener('click', () => copy(getText(), label.replace(/^Copy /, '') + ' copied'));
    wrap.appendChild(btn);
    return wrap;
  }

  function renderReplay(r) {
    const pane = $('#pane-replay');
    pane.replaceChildren();
    const intro = document.createElement('p');
    intro.style.color = 'var(--text-muted)';
    intro.style.margin = '0 0 12px';
    const proxyUrl = state.forward && state.forward.enabled
      ? composeForwardUrl(state.forward.url, r.path, r.query)
      : null;
    intro.textContent = proxyUrl
      ? 'Replay this captured request. Prefilled from the current proxy upstream — edit to send anywhere else.'
      : 'Replay this captured request to a target URL. Browser CORS rules apply.';
    pane.appendChild(intro);

    const form = document.createElement('div');
    form.className = 'replay-form';

    const urlInput = document.createElement('input');
    urlInput.type = 'url';
    urlInput.placeholder = 'https://example.com' + r.path + (r.query ? '?' + r.query : '');
    urlInput.value = proxyUrl || state.lastReplayUrl || '';
    form.appendChild(urlInput);

    const btn = document.createElement('button');
    btn.className = 'btn';
    btn.textContent = `Send ${r.method}`;
    form.appendChild(btn);

    const result = document.createElement('div');
    result.className = 'replay-result';
    result.style.display = 'none';
    form.appendChild(result);

    btn.addEventListener('click', async () => {
      const target = urlInput.value.trim();
      if (!target) return;
      state.lastReplayUrl = target;
      btn.disabled = true;
      btn.textContent = 'Sending…';
      result.style.display = 'block';
      result.className = 'replay-result';
      result.textContent = '…';
      try {
        const headers = {};
        for (const [k, v] of r.headers) {
          const lk = k.toLowerCase();
          if (
            lk === 'host' ||
            lk === 'content-length' ||
            lk === 'connection' ||
            lk.startsWith('proxy-') ||
            lk.startsWith('sec-')
          ) continue;
          headers[k] = v;
        }
        const init = { method: r.method, headers };
        if (!['GET', 'HEAD'].includes(r.method)) {
          init.body = decodeBodyAsBytes(r);
        }
        const start = performance.now();
        const res = await fetch(target, init);
        const elapsed = Math.round(performance.now() - start);
        result.className = 'replay-result success';
        result.textContent = `${res.status} ${res.statusText} · ${elapsed}ms`;
      } catch (e) {
        result.className = 'replay-result error';
        result.textContent = String(e);
      } finally {
        btn.disabled = false;
        btn.textContent = `Send ${r.method}`;
      }
    });
    pane.appendChild(form);
  }

  // ───────── tab switching ─────────
  function switchTab(tab) {
    state.activeTab = tab;
    showTab(tab);
  }

  function showTab(tab) {
    $$('.tab').forEach((b) => b.classList.toggle('active', b.dataset.tab === tab));
    $$('.pane').forEach((p) => p.classList.toggle('active', p.id === 'pane-' + tab));
  }

  // ───────── controls ─────────
  function toggleTheme() {
    const cur = document.documentElement.dataset.theme || 'dark';
    const next = cur === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = next;
    localStorage.setItem('pbu-theme', next);
  }

  function togglePause() {
    state.paused = !state.paused;
    $('#pause-toggle .btn-label').textContent = state.paused ? 'Resume' : 'Pause';
    setStatus(state.paused ? 'paused' : 'connected');
  }

  async function confirmClear() {
    if (state.requests.length === 0) return;
    if (!window.confirm(`Clear ${state.requests.length} captured requests?`)) return;
    try {
      await fetch('/api/requests', { method: 'DELETE' });
      toast('Cleared');
    } catch {
      toast('Clear failed');
    }
  }

  function onSearch(e) {
    state.filter = e.target.value;
    renderList();
  }

  function onKeydown(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
      if (e.key === 'Escape') e.target.blur();
      return;
    }
    // Never hijack browser/OS shortcuts (cmd+c, ctrl+r, alt+left, …).
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    switch (e.key) {
      case 'j':
      case 'ArrowDown':
        e.preventDefault();
        moveSelection(1);
        break;
      case 'k':
      case 'ArrowUp':
        e.preventDefault();
        moveSelection(-1);
        break;
      case 'g':
        if (state.requests.length) selectRequest(state.requests[0].id);
        break;
      case 'G':
        if (state.requests.length) selectRequest(state.requests[state.requests.length - 1].id);
        break;
      case '/':
        e.preventDefault();
        $('#search').focus();
        break;
      case 'p':
        togglePause();
        break;
      case 'X':
        confirmClear();
        break;
      case 't':
        toggleTheme();
        break;
      case '?':
        $('#help-dialog').showModal();
        break;
      case '1':
      case '2':
      case '3':
      case '4':
      case '5': {
        const tabs = ['body', 'headers', 'query', 'raw', 'replay'];
        const idx = Number(e.key) - 1;
        if (tabs[idx]) switchTab(tabs[idx]);
        break;
      }
      case 'Escape':
        if ($('#help-dialog').open) $('#help-dialog').close();
        break;
    }
  }

  function moveSelection(delta) {
    const visible = applyFilter(state.requests);
    if (visible.length === 0) return;
    const idx = visible.findIndex((r) => r.id === state.selectedId);
    let next = idx + delta;
    if (idx === -1) next = 0;
    next = Math.max(0, Math.min(visible.length - 1, next));
    selectRequest(visible[next].id);
    const row = $(`.request-row[data-id="${visible[next].id}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }

  // ───────── helpers ─────────
  function headerValue(r, name) {
    const lname = name.toLowerCase();
    const found = r.headers.find(([k]) => k.toLowerCase() === lname);
    return found ? found[1] : null;
  }

  function decodeBodyAsText(r) {
    if (r.body_encoding === 'utf8') return r.body;
    try {
      const bin = atob(r.body);
      const arr = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
      return new TextDecoder('utf-8', { fatal: false }).decode(arr);
    } catch {
      return '';
    }
  }

  function decodeBodyAsBytes(r) {
    if (r.body_encoding === 'utf8') {
      return new TextEncoder().encode(r.body);
    }
    try {
      const bin = atob(r.body);
      const arr = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
      return arr;
    } catch {
      return new Uint8Array(0);
    }
  }

  function looksLikeJson(s) {
    const t = (s || '').trim();
    return t.startsWith('{') || t.startsWith('[');
  }

  function bytesToObjectUrl(bytes, mime) {
    return URL.createObjectURL(new Blob([bytes], { type: mime || 'application/octet-stream' }));
  }

  function isMostlyText(bytes) {
    if (!bytes || bytes.length === 0) return false;
    let printable = 0;
    const limit = Math.min(bytes.length, 1024);
    for (let i = 0; i < limit; i++) {
      const b = bytes[i];
      if (b === 9 || b === 10 || b === 13 || (b >= 32 && b < 127)) printable++;
    }
    return printable / limit > 0.85;
  }

  function parseFormEncoded(s) {
    const params = new URLSearchParams(s);
    const out = [];
    for (const [k, v] of params) out.push([k, v]);
    return out;
  }

  function kvTable(rows) {
    const tbl = document.createElement('table');
    tbl.className = 'kv-table';
    for (const [k, v] of rows) {
      const tr = document.createElement('tr');
      const th = document.createElement('th');
      th.textContent = k;
      const td = document.createElement('td');
      td.textContent = v;
      tr.appendChild(th);
      tr.appendChild(td);
      tbl.appendChild(tr);
    }
    return tbl;
  }

  function renderMultipart(bytes, contentType, r) {
    const wrap = document.createElement('div');
    const note = document.createElement('div');
    note.className = 'empty-body';
    note.textContent = 'multipart/form-data — full part parsing not implemented; raw bytes shown below.';
    wrap.appendChild(note);
    wrap.appendChild(hexView(bytes));
    wrap.appendChild(downloadRow(r));
    return wrap;
  }

  // JSON tree with collapsible nodes & syntax highlighting.
  // Returns a wrapper containing a toolbar (expand/collapse all) plus the tree.
  function jsonTree(value) {
    const wrap = document.createElement('div');
    wrap.className = 'json-tree-wrap';

    const tree = document.createElement('pre');
    tree.className = 'json-tree';
    tree.appendChild(renderJsonNode(value, 0));

    // Only show controls when there are toggleable nodes (i.e. non-empty
    // objects/arrays). For a primitive root they'd be inert.
    const hasToggles = tree.querySelector('.json-toggle') !== null;
    if (hasToggles) {
      wrap.appendChild(jsonTreeToolbar(tree));
    }
    wrap.appendChild(tree);
    return wrap;
  }

  function jsonTreeToolbar(tree) {
    const bar = document.createElement('div');
    bar.className = 'json-tree-toolbar';

    const collapseAll = document.createElement('button');
    collapseAll.type = 'button';
    collapseAll.className = 'btn btn-ghost btn-xs';
    collapseAll.textContent = 'Collapse all';

    const expandAll = document.createElement('button');
    expandAll.type = 'button';
    expandAll.className = 'btn btn-ghost btn-xs';
    expandAll.textContent = 'Expand all';

    collapseAll.addEventListener('click', () => setAllJsonCollapsed(tree, true));
    expandAll.addEventListener('click', () => setAllJsonCollapsed(tree, false));

    bar.appendChild(collapseAll);
    bar.appendChild(expandAll);
    return bar;
  }

  function setAllJsonCollapsed(tree, collapsed) {
    tree.querySelectorAll('.json-toggle').forEach((toggle) => {
      const target = toggle.parentElement;
      if (!target) return;
      target.classList.toggle('json-collapsed', collapsed);
      toggle.textContent = collapsed ? '▸' : '▾';
    });
  }

  function renderJsonNode(value, depth) {
    if (value === null) return jsonSpan('null', 'json-null');
    if (typeof value === 'boolean') return jsonSpan(String(value), 'json-bool');
    if (typeof value === 'number') return jsonSpan(String(value), 'json-number');
    if (typeof value === 'string') {
      return jsonSpan('"' + escapeJsonString(value) + '"', 'json-string');
    }
    if (Array.isArray(value)) {
      const wrap = document.createElement('span');
      wrap.appendChild(makeToggle(wrap));
      wrap.appendChild(jsonSpan('[', 'json-punct'));
      const summary = jsonSpan(`…${value.length} items]`, 'json-summary');
      wrap.appendChild(summary);
      const children = document.createElement('span');
      children.className = 'json-children';
      for (let i = 0; i < value.length; i++) {
        children.appendChild(document.createTextNode('\n' + indent(depth + 1)));
        children.appendChild(renderJsonNode(value[i], depth + 1));
        if (i < value.length - 1) children.appendChild(jsonSpan(',', 'json-punct'));
      }
      children.appendChild(document.createTextNode('\n' + indent(depth)));
      wrap.appendChild(children);
      wrap.appendChild(jsonSpan(']', 'json-punct'));
      return wrap;
    }
    if (typeof value === 'object') {
      const wrap = document.createElement('span');
      wrap.appendChild(makeToggle(wrap));
      wrap.appendChild(jsonSpan('{', 'json-punct'));
      const keys = Object.keys(value);
      wrap.appendChild(jsonSpan(`…${keys.length} keys}`, 'json-summary'));
      const children = document.createElement('span');
      children.className = 'json-children';
      keys.forEach((k, i) => {
        children.appendChild(document.createTextNode('\n' + indent(depth + 1)));
        children.appendChild(jsonSpan('"' + escapeJsonString(k) + '"', 'json-key'));
        children.appendChild(jsonSpan(': ', 'json-punct'));
        children.appendChild(renderJsonNode(value[k], depth + 1));
        if (i < keys.length - 1) children.appendChild(jsonSpan(',', 'json-punct'));
      });
      children.appendChild(document.createTextNode('\n' + indent(depth)));
      wrap.appendChild(children);
      wrap.appendChild(jsonSpan('}', 'json-punct'));
      return wrap;
    }
    return jsonSpan(String(value), 'json-punct');
  }

  function jsonSpan(text, cls) {
    const s = document.createElement('span');
    s.className = cls;
    s.textContent = text;
    return s;
  }

  function makeToggle(target) {
    const t = document.createElement('span');
    t.className = 'json-toggle';
    t.textContent = '▾';
    t.addEventListener('click', () => {
      target.classList.toggle('json-collapsed');
      t.textContent = target.classList.contains('json-collapsed') ? '▸' : '▾';
    });
    return t;
  }

  function escapeJsonString(s) {
    return s.replace(/[\\"\n\r\t]/g, (c) => ({
      '\\': '\\\\',
      '"': '\\"',
      '\n': '\\n',
      '\r': '\\r',
      '\t': '\\t',
    })[c]);
  }

  function indent(n) {
    return '  '.repeat(n);
  }

  // Hex dump: 16 bytes / line with addr + ASCII gutter — built via DOM nodes
  function hexView(bytes) {
    const wrap = document.createElement('pre');
    wrap.className = 'hex-view';
    const limit = Math.min(bytes.length, 16384);
    for (let i = 0; i < limit; i += 16) {
      const slice = bytes.slice(i, Math.min(i + 16, limit));

      const addr = document.createElement('span');
      addr.className = 'hex-addr';
      addr.textContent = i.toString(16).padStart(8, '0');
      wrap.appendChild(addr);

      const hex = document.createElement('span');
      hex.className = 'hex-bytes';
      const hexStr = Array.from(slice)
        .map((b) => b.toString(16).padStart(2, '0'))
        .join(' ')
        .padEnd(48, ' ');
      hex.textContent = hexStr;
      wrap.appendChild(hex);

      const ascii = document.createElement('span');
      ascii.className = 'hex-ascii';
      let asciiStr = '';
      for (const b of slice) asciiStr += b >= 32 && b < 127 ? String.fromCharCode(b) : '·';
      ascii.textContent = asciiStr;
      wrap.appendChild(ascii);

      wrap.appendChild(document.createTextNode('\n'));
    }
    if (bytes.length > limit) {
      const more = document.createElement('span');
      more.style.color = 'var(--text-dim)';
      more.textContent = `\n…${formatBytes(bytes.length - limit)} more truncated in display.`;
      wrap.appendChild(more);
    }
    return wrap;
  }

  // curl / raw HTTP serialisation
  function buildCurl(r) {
    const parts = ['curl'];
    if (r.method !== 'GET') parts.push('-X', r.method);
    const url = (state.captureUrl || '') + r.path + (r.query ? '?' + r.query : '');
    parts.push(`'${url}'`);
    for (const [k, v] of r.headers) {
      const kl = k.toLowerCase();
      if (kl === 'host' || kl === 'content-length') continue;
      parts.push(`-H '${k}: ${v.replace(/'/g, "'\\''")}'`);
    }
    if (r.body && r.body_size > 0) {
      const text = decodeBodyAsText(r);
      const escaped = text.replace(/'/g, "'\\''");
      parts.push(`--data-raw '${escaped}'`);
    }
    return parts.join(' \\\n  ');
  }

  function buildRawHttp(r) {
    let out = `${r.method} ${r.path}${r.query ? '?' + r.query : ''} ${r.version}\r\n`;
    for (const [k, v] of r.headers) out += `${k}: ${v}\r\n`;
    out += '\r\n';
    out += decodeBodyAsText(r);
    return out;
  }

  function countQuery(q) {
    if (!q) return 0;
    return new URLSearchParams(q).size;
  }

  function relativeTime(iso) {
    const d = new Date(iso).getTime();
    const diff = (Date.now() - d) / 1000;
    if (diff < 1) return 'now';
    if (diff < 60) return Math.floor(diff) + 's ago';
    if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
    if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
    return Math.floor(diff / 86400) + 'd ago';
  }

  function formatBytes(n) {
    if (n == null) return '—';
    if (n < 1024) return n + ' B';
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KiB';
    if (n < 1024 * 1024 * 1024) return (n / (1024 * 1024)).toFixed(1) + ' MiB';
    return (n / (1024 * 1024 * 1024)).toFixed(2) + ' GiB';
  }

  async function copy(text, msg) {
    try {
      await navigator.clipboard.writeText(text);
      toast(msg || 'Copied');
    } catch {
      toast('Copy failed');
    }
  }

  let toastTimer = null;
  function toast(msg) {
    const el = $('#toast');
    el.textContent = msg;
    el.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => el.classList.remove('show'), 1800);
  }

  // refresh relative timestamps every couple of seconds
  setInterval(() => {
    $$('.row-time').forEach((el) => {
      const id = el.closest('.request-row')?.dataset.id;
      const r = state.requests.find((x) => x.id === id);
      if (r) el.textContent = relativeTime(r.received_at);
    });
  }, 2000);
})();
