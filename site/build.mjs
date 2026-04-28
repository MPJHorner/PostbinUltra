// Postbin Ultra documentation site builder.
// Reads markdown from content/, wraps it in templates/, writes static HTML to dist/.
// Source-of-truth values (version, CLI --help, CHANGELOG) are pulled from the repo
// at build time so the site cannot drift from the code that ships.

import { readFile, writeFile, mkdir, readdir, copyFile, stat, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { dirname, join, relative, resolve, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import matter from 'gray-matter';
import { marked } from 'marked';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');
const SITE = __dirname;
const DIST = join(SITE, 'dist');

const SITE_URL = process.env.SITE_URL || 'https://mpjhorner.github.io/PostbinUltra';
const BASE = process.env.BASE_URL || '/PostbinUltra';
const REPO = 'https://github.com/MPJHorner/PostbinUltra';

const NAV = [
  { href: '/install/', label: 'Install' },
  { href: '/quick-start/', label: 'Quick start' },
  { href: '/cli/', label: 'CLI' },
  { href: '/proxy/', label: 'Proxy' },
  { href: '/web-ui/', label: 'Web UI' },
  { href: '/api/', label: 'API' },
  { href: '/changelog/', label: 'Changelog' },
];

const FOOTER_LINKS = [
  { href: '/install/', label: 'Install' },
  { href: '/quick-start/', label: 'Quick start' },
  { href: '/cli/', label: 'CLI reference' },
  { href: '/proxy/', label: 'Proxy mode' },
  { href: '/logging/', label: 'Logging' },
  { href: '/web-ui/', label: 'Web UI' },
  { href: '/api/', label: 'API' },
  { href: '/configuration/', label: 'Configuration' },
  { href: '/use-cases/', label: 'Use cases' },
  { href: '/comparison/', label: 'Comparison' },
  { href: '/contributing/', label: 'Contributing' },
  { href: '/changelog/', label: 'Changelog' },
];

// ───────── helpers ─────────

const withBase = (href) => {
  if (/^https?:/.test(href) || href.startsWith('mailto:')) return href;
  if (href.startsWith('#')) return href;
  if (BASE && (href === BASE || href.startsWith(BASE + '/'))) return href;
  if (href === '/') return BASE + '/';
  return BASE + href;
};

const escapeHtml = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const slugify = (s) =>
  String(s)
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-');

async function readFirst(...paths) {
  for (const p of paths) {
    if (existsSync(p)) return readFile(p, 'utf8');
  }
  return null;
}

async function copyDir(src, dest) {
  await mkdir(dest, { recursive: true });
  for (const entry of await readdir(src, { withFileTypes: true })) {
    const s = join(src, entry.name);
    const d = join(dest, entry.name);
    if (entry.isDirectory()) await copyDir(s, d);
    else await copyFile(s, d);
  }
}

// ───────── data sources ─────────

async function loadVersion() {
  const cargo = await readFile(join(ROOT, 'Cargo.toml'), 'utf8');
  const m = cargo.match(/^version\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error('Could not extract version from Cargo.toml');
  return m[1];
}

async function loadCliHelp() {
  const live = process.env.CLI_HELP_PATH;
  if (live && existsSync(live)) return readFile(live, 'utf8');
  const cached = join(SITE, 'cli-help.txt');
  if (existsSync(cached)) return readFile(cached, 'utf8');
  return null;
}

async function loadChangelogEntries() {
  const raw = await readFile(join(ROOT, 'CHANGELOG.md'), 'utf8');
  const lines = raw.split('\n');
  const entries = [];
  let current = null;
  const pushBlock = () => {
    if (current) entries.push(current);
  };
  for (const line of lines) {
    const m = line.match(/^##\s*\[([^\]]+)\]\s*-\s*(\S+)\s*$/);
    if (m) {
      pushBlock();
      current = { version: m[1], date: m[2], lines: [] };
    } else if (current) {
      current.lines.push(line);
    }
  }
  pushBlock();
  for (const e of entries) {
    e.body = e.lines.join('\n').trim();
    delete e.lines;
  }
  return entries;
}

// ───────── markdown rendering ─────────

const renderer = new marked.Renderer();
renderer.heading = (text, level, raw) => {
  // Support `## Heading {#custom-id}` syntax: extract the id, drop the marker.
  const customMatch = (raw || '').match(/\{#([\w-]+)\}\s*$/);
  let cleanText = text;
  let cleanRaw = raw || '';
  let id;
  if (customMatch) {
    id = customMatch[1];
    cleanText = text.replace(/\s*\{#[\w-]+\}\s*$/, '');
    cleanRaw = cleanRaw.replace(/\s*\{#[\w-]+\}\s*$/, '');
  } else {
    id = slugify(cleanRaw);
  }
  if (level === 1) return `<h1>${cleanText}</h1>\n`;
  return `<h${level} id="${id}"><a class="anchor" href="#${id}" aria-label="Anchor">#</a>${cleanText}</h${level}>\n`;
};
renderer.link = (href, title, text) => {
  const isExternal = /^https?:/.test(href);
  const safeHref = href.startsWith('/') ? withBase(href) : href;
  const attrs = isExternal ? ' rel="noopener noreferrer"' : '';
  const titleAttr = title ? ` title="${escapeHtml(title)}"` : '';
  return `<a href="${escapeHtml(safeHref)}"${titleAttr}${attrs}>${text}</a>`;
};
renderer.image = (href, title, text) => {
  const safeHref = href.startsWith('/') ? withBase(href) : href;
  const titleAttr = title ? ` title="${escapeHtml(title)}"` : '';
  return `<img src="${escapeHtml(safeHref)}" alt="${escapeHtml(text || '')}"${titleAttr} loading="lazy" decoding="async" />`;
};
renderer.code = (code, lang) => {
  const cls = lang ? ` class="language-${escapeHtml(lang)}"` : '';
  const langLabel = lang ? `<span class="code-lang">${escapeHtml(lang)}</span>` : '';
  return `<div class="code-block">${langLabel}<button class="copy-btn" type="button" aria-label="Copy code">copy</button><pre><code${cls}>${escapeHtml(code)}</code></pre></div>\n`;
};
renderer.table = (header, body) =>
  `<div class="table-wrap"><table>\n<thead>${header}</thead>\n<tbody>${body}</tbody>\n</table></div>\n`;

marked.setOptions({ renderer, gfm: true, breaks: false });

const renderMarkdown = (md) => marked.parse(md);

function applyPlaceholders(text, data) {
  return text.replace(/\{\{\s*([\w.]+)\s*\}\}/g, (_, key) => {
    const value = key.split('.').reduce((acc, k) => (acc == null ? acc : acc[k]), data);
    return value == null ? '' : String(value);
  });
}

// ───────── templates ─────────

async function loadTemplates() {
  const layout = await readFile(join(SITE, 'templates/layout.html'), 'utf8');
  const home = await readFile(join(SITE, 'templates/home.html'), 'utf8');
  const nav = await readFile(join(SITE, 'templates/partials/nav.html'), 'utf8');
  const footer = await readFile(join(SITE, 'templates/partials/footer.html'), 'utf8');
  return { layout, home, nav, footer };
}

function renderNav(navHtml, navItems, currentSlug) {
  const links = navItems
    .map((item) => {
      const isActive = item.href === `/${currentSlug}/` || (item.href === '/' && currentSlug === '');
      const cls = isActive ? ' class="nav-link active" aria-current="page"' : ' class="nav-link"';
      return `<a${cls} href="${withBase(item.href)}">${escapeHtml(item.label)}</a>`;
    })
    .join('\n          ');
  return navHtml.replace('{{nav_links}}', links).replace(/\{\{base\}\}/g, BASE).replace(/\{\{repo\}\}/g, REPO);
}

function renderFooter(footerHtml, footerLinks) {
  const cols = footerLinks
    .map((item) => `<li><a href="${withBase(item.href)}">${escapeHtml(item.label)}</a></li>`)
    .join('\n              ');
  const year = new Date().getFullYear();
  return footerHtml
    .replace('{{footer_links}}', cols)
    .replace(/\{\{base\}\}/g, BASE)
    .replace(/\{\{repo\}\}/g, REPO)
    .replace(/\{\{year\}\}/g, String(year));
}

// ───────── changelog rendering ─────────

function renderChangelogHtml(entries) {
  return entries
    .map((e) => {
      const body = renderMarkdown(e.body);
      return `<section class="changelog-entry" id="v${e.version}">
  <header class="changelog-header">
    <h2><a class="anchor" href="#v${e.version}" aria-label="Anchor">#</a>v${escapeHtml(e.version)}</h2>
    <time datetime="${escapeHtml(e.date)}">${escapeHtml(e.date)}</time>
  </header>
  ${body}
</section>`;
    })
    .join('\n');
}

function renderRss(entries, version) {
  const now = new Date().toUTCString();
  const items = entries
    .map((e) => {
      const url = `${SITE_URL}${BASE}/changelog/#v${e.version}`;
      const pubDate = new Date(e.date + 'T00:00:00Z').toUTCString();
      const desc = renderMarkdown(e.body);
      return `    <item>
      <title>v${e.version}</title>
      <link>${url}</link>
      <guid isPermaLink="true">${url}</guid>
      <pubDate>${pubDate}</pubDate>
      <description><![CDATA[${desc}]]></description>
    </item>`;
    })
    .join('\n');
  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Postbin Ultra changelog</title>
    <link>${SITE_URL}${BASE}/changelog/</link>
    <description>Releases of Postbin Ultra. Latest: v${version}.</description>
    <language>en</language>
    <lastBuildDate>${now}</lastBuildDate>
${items}
  </channel>
</rss>
`;
}

// ───────── page build ─────────

function buildHead({ title, description, slug, version }) {
  const fullTitle = slug === '' ? `${title}` : `${title} · Postbin Ultra`;
  const canonical = `${SITE_URL}${BASE}/${slug}${slug ? '/' : ''}`;
  const ogImage = `${SITE_URL}${BASE}/img/screenshot.png`;
  const jsonLd = slug === ''
    ? {
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        name: 'Postbin Ultra',
        operatingSystem: 'macOS, Linux, Windows',
        applicationCategory: 'DeveloperApplication',
        description,
        softwareVersion: version,
        license: 'https://opensource.org/licenses/MIT',
        downloadUrl: `${REPO}/releases/latest`,
        url: `${SITE_URL}${BASE}/`,
        offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
      }
    : {
        '@context': 'https://schema.org',
        '@type': 'TechArticle',
        headline: title,
        description,
        url: canonical,
        author: { '@type': 'Person', name: 'MPJHorner' },
      };
  return `
  <title>${escapeHtml(fullTitle)}</title>
  <meta name="description" content="${escapeHtml(description)}" />
  <link rel="canonical" href="${escapeHtml(canonical)}" />
  <meta property="og:type" content="website" />
  <meta property="og:title" content="${escapeHtml(fullTitle)}" />
  <meta property="og:description" content="${escapeHtml(description)}" />
  <meta property="og:url" content="${escapeHtml(canonical)}" />
  <meta property="og:image" content="${escapeHtml(ogImage)}" />
  <meta property="og:site_name" content="Postbin Ultra" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="${escapeHtml(fullTitle)}" />
  <meta name="twitter:description" content="${escapeHtml(description)}" />
  <meta name="twitter:image" content="${escapeHtml(ogImage)}" />
  <link rel="alternate" type="application/rss+xml" title="Postbin Ultra changelog" href="${escapeHtml(BASE + '/changelog.xml')}" />
  <script type="application/ld+json">${JSON.stringify(jsonLd)}</script>`;
}

async function buildPages({ version, cliHelp, changelogEntries, templates }) {
  const contentDir = join(SITE, 'content');
  const files = (await readdir(contentDir)).filter((f) => f.endsWith('.md'));

  const pages = [];

  for (const file of files) {
    const raw = await readFile(join(contentDir, file), 'utf8');
    const fm = matter(raw);
    const slug = fm.data.slug ?? (basename(file, '.md') === 'index' ? '' : basename(file, '.md'));
    const title = fm.data.title || 'Postbin Ultra';
    const description = fm.data.description || 'Local HTTP request inspector for developers.';
    const layout = fm.data.layout || (slug === '' ? 'home' : 'page');

    const replacements = {
      version,
      cli_help: cliHelp ? escapeHtml(cliHelp) : '<em>The CLI help text is rendered at build time. Run `postbin-ultra --help` locally to see it.</em>',
      changelog_html: renderChangelogHtml(changelogEntries),
      base: BASE,
      repo: REPO,
      site_url: SITE_URL,
    };

    const body = renderMarkdown(applyPlaceholders(fm.content, replacements));

    const head = buildHead({ title, description, slug, version });
    const navRendered = renderNav(templates.nav, NAV, slug);
    const footerRendered = renderFooter(templates.footer, FOOTER_LINKS);
    const layoutTemplate = layout === 'home' ? templates.home : templates.layout;

    const html = applyPlaceholders(layoutTemplate, {
      head,
      nav: navRendered,
      footer: footerRendered,
      content: body,
      title: escapeHtml(title),
      description: escapeHtml(description),
      version,
      base: BASE,
      repo: REPO,
    });

    pages.push({ slug, title, description, html, fileName: file });
  }

  return pages;
}

// ───────── output ─────────

async function writePage(page) {
  if (page.slug === '404') {
    await writeFile(join(DIST, '404.html'), page.html);
    return;
  }
  const dir = page.slug === '' ? DIST : join(DIST, page.slug);
  await mkdir(dir, { recursive: true });
  await writeFile(join(dir, 'index.html'), page.html);
}

async function writeSitemap(pages) {
  const now = new Date().toISOString().slice(0, 10);
  const urls = pages
    .filter((p) => p.slug !== '404')
    .map((p) => {
      const loc = `${SITE_URL}${BASE}/${p.slug}${p.slug ? '/' : ''}`;
      const priority = p.slug === '' ? '1.0' : '0.7';
      return `  <url><loc>${loc}</loc><lastmod>${now}</lastmod><priority>${priority}</priority></url>`;
    })
    .join('\n');
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>
`;
  await writeFile(join(DIST, 'sitemap.xml'), xml);
}

async function writeRobots() {
  const txt = `User-agent: *
Allow: /
Sitemap: ${SITE_URL}${BASE}/sitemap.xml
`;
  await writeFile(join(DIST, 'robots.txt'), txt);
}

async function copyStatic() {
  const staticDir = join(SITE, 'static');
  if (existsSync(staticDir)) await copyDir(staticDir, DIST);
  const screenshot = join(ROOT, 'docs/screenshot.png');
  if (existsSync(screenshot)) {
    await mkdir(join(DIST, 'img'), { recursive: true });
    await copyFile(screenshot, join(DIST, 'img/screenshot.png'));
  }
}

// ───────── main ─────────

async function main() {
  console.time('build');
  await rm(DIST, { recursive: true, force: true });
  await mkdir(DIST, { recursive: true });

  const [version, cliHelp, changelogEntries, templates] = await Promise.all([
    loadVersion(),
    loadCliHelp(),
    loadChangelogEntries(),
    loadTemplates(),
  ]);

  if (!cliHelp) {
    console.warn('  ! cli-help.txt missing — CLI page will show a fallback notice');
  }

  const pages = await buildPages({ version, cliHelp, changelogEntries, templates });

  await Promise.all(pages.map(writePage));
  await writeSitemap(pages);
  await writeRobots();
  await writeFile(join(DIST, 'changelog.xml'), renderRss(changelogEntries, version));
  await copyStatic();

  console.timeEnd('build');
  console.log(`  ✓ wrote ${pages.length} pages to ${relative(ROOT, DIST)} (v${version})`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
