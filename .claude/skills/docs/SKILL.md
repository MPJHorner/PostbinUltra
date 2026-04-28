---
name: docs
description: >-
  Use BEFORE committing any change that alters user-visible behavior in Postbin Ultra:
  CLI flags, environment variables, API endpoints/responses, web UI features, body
  formatters, keyboard shortcuts, install/build steps, error messages, or configuration
  defaults. Walks through which docs to update (README + the site at site/), how to
  write them (voice, structure, SEO), and how to verify the rebuild before committing.
---

# Postbin Ultra docs maintenance

This skill keeps the README and the GitHub Pages docs site (`site/`) in sync with the code. **Invoke this skill before staging any commit that ships a user-visible change.** Documentation drift is a release-blocker; the docs site goes out on every push to `main` and every `v*` tag, so anything stale becomes public immediately.

## When to invoke

Required for any change that touches:

- `src/cli.rs` (flags, defaults, validation)
- `src/ui.rs` (HTTP routes, JSON shapes, SSE events)
- `src/capture.rs` (forward semantics, headers, errors)
- `src/output.rs` (terminal output the user sees)
- `ui/` (web UI features, keyboard shortcuts, formatters)
- The startup banner, error messages, or anything printed to the user
- Install / build / release flow
- New or removed environment variables

Skip for: pure refactors with no user-visible change, internal tests, code-comment edits, build-system tweaks that don't change `make` targets.

## What to update — page-by-page rules

Each rule is "if you change X, update Y". Don't update other pages out of habit; respect the page boundaries below.

| If you change… | Update |
| --- | --- |
| A CLI flag, default, or validation | `site/content/cli.md` (the flag's own `### {#flag-name}` block) AND `site/content/configuration.md` (the flags table) |
| A JSON API endpoint, request body, response, or status code | `site/content/api.md` |
| An SSE event type or payload | `site/content/api.md` (Stream new captures section) |
| Forward/proxy header handling, error codes, or timeout behavior | `site/content/proxy.md` |
| A keyboard shortcut | `site/content/web-ui.md` (Keyboard shortcuts table) |
| A body formatter (JSON tree toolbar, hex view, etc.) | `site/content/web-ui.md` (Body formatters list) |
| `--log-file` shape or NDJSON behavior | `site/content/logging.md` |
| Install/build/release steps, supported platforms | `site/content/install.md` AND `README.md` |
| A behavior described in `CHANGELOG.md` for the new release | bump `Cargo.toml`, add the changelog entry; the site auto-renders it on rebuild |
| Module map, coverage policy, or contributor flow | `site/content/contributing.md` |
| The screenshot | `docs/screenshot.png` (the site symlinks it; no path change needed) |

If the change adds a new top-level concept that doesn't fit any existing page, add it to the most adjacent page instead of creating a new one. New pages need a nav link, footer link, and sitemap entry — only justified for genuinely standalone topics.

## Site layout (so you put content in the right place)

```
site/
  build.mjs                  # the static-site builder
  content/                   # one .md per page; frontmatter sets title/description/slug
  templates/
    layout.html              # default page shell (head, nav, prose, footer)
    home.html                # home-only shell (full-width)
    partials/{nav,footer}.html
  static/                    # CSS, JS, icons. Copied verbatim into dist/
```

Pages:
- `index.md` — home, hero + sections + CTA. Custom HTML inside markdown is fine here.
- `install.md`, `quick-start.md`, `cli.md`, `proxy.md`, `logging.md`, `web-ui.md`, `api.md`, `configuration.md`, `use-cases.md`, `comparison.md`, `contributing.md`, `changelog.md`, `404.md` — standard prose pages.

The nav order is set in `build.mjs` (`NAV` array). Footer links are in the `FOOTER_LINKS` array. Adding a new page means editing both arrays.

## Page structure (every prose page)

```markdown
---
title: "Short title"                        # 30–60 chars, used in <title> and OG
description: "One-sentence summary."        # 120–160 chars, used in <meta description> + OG
slug: "page-slug"                           # the URL path, no leading/trailing slash
---

# Page title          ← matches frontmatter title

One-paragraph intro that restates the page's purpose in plain terms. This is the line a search-engine snippet shows; make it stand alone.

## Section heading    ← every section is h2; use h3 for sub-blocks; never skip levels

Body content.

## Related

Cross-link to 1–3 sibling pages where relevant.
```

Rules the builder enforces (or you should match by hand):

- One `<h1>` per page, set by the leading `# Title` line.
- Internal links use root-relative paths starting with `/`: `[Proxy mode]({{base}}/proxy/)`. The builder rewrites `{{base}}` → `/PostbinUltra` at build time. Don't hard-code the prefix.
- Custom anchors: `## Forward management {#forward}` — the `{#id}` syntax is supported by the builder.
- Code blocks must specify a language: `sh`, `json`, `text`, `rust`, `toml`. The renderer adds a copy button.
- Tables go in pipe-syntax. The builder wraps them in `.table-wrap` for horizontal scroll on mobile.

## Writing style

The voice is the same as the README, the CLI banner, and `--help`. Match it.

**Always:**
- Lead with the verb. "Forwards captured requests…" beats "This feature lets you forward…".
- Show one concrete example for every concept introduced. Examples > prose.
- Sentences ≤ 25 words. Paragraphs ≤ 4 sentences.
- Spell out defaults inline. "default `30`" beats "30 (configurable)".
- Use active voice. "Postbin returns 502" not "A 502 will be returned".
- Use product/protocol terms exactly. `Content-Type`, `application/json`, `127.0.0.1`, `Server-Sent Events`. Don't mix casings.
- Reference flags as `--forward`, env vars as `RUST_LOG`, paths as `site/content/cli.md` — always inside backticks.

**Never (these are blocking; the build pipeline doesn't catch them, you must):**
- **No em-dashes (`—`) anywhere in user-facing text.** Use a comma, semicolon, parentheses, or split into two sentences. Em-dashes read as AI-generated copy.
- **No AI-slop adjectives.** Banned words/phrases:
  - "blazing-fast", "blazing fast", "lightning-fast", "lightning fast"
  - "beautiful", "elegant", "delightful", "powerful", "robust", "seamless"
  - "leverage" (use "use"), "utilize" (use "use"), "facilitate"
  - "dive into", "deep dive", "unlock", "unleash"
  - "in today's world", "in the modern era", any "world of X" framing
  - "best-in-class", "cutting-edge", "state-of-the-art"
  - "we're excited to", "we're thrilled", any first-person marketing voice
- No emoji in prose (the home-page feature icons in `index.md` are an intentional exception, kept minimal).
- No exclamation marks.
- No "you'll love it", "you'll be amazed", or any reader-mood claim.
- No filler intros: "It's worth noting that…", "Of course…", "As you might expect…". Just say the thing.

If you're tempted to write "powerful flag", you mean "the flag does X, Y, Z" — write that instead.

## SEO conventions

Every page must have:

- A unique, accurate `title` (frontmatter). Format: `"Topic"` — the builder appends ` · Postbin Ultra`. Home is the exception: it stands alone.
- A unique `description` between 120–160 characters. Lead with what the page covers; do not repeat the title.
- One `<h1>`, set by the leading `# Title` line.
- A canonical link (auto-emitted by the builder; nothing to do here).

The builder auto-generates: `<title>`, `<meta description>`, `<link rel="canonical">`, OpenGraph tags (`og:title`, `og:description`, `og:url`, `og:image`, `og:site_name`), Twitter card, JSON-LD (`SoftwareApplication` on home, `TechArticle` elsewhere), `sitemap.xml`, `robots.txt`, RSS feed, and a `<link rel="alternate">` for the changelog. Don't duplicate these by hand in markdown.

For images: include a real `alt` describing what the image *shows for sighted readers*. Decorative images use `alt=""`. The renderer adds `loading="lazy"` automatically.

For internal links: prefer descriptive text over "click here" or "this page". `See [proxy mode]({{base}}/proxy/)` beats `See [this page]({{base}}/proxy/) for proxy mode`.

For external links: the renderer adds `rel="noopener noreferrer"` automatically.

## Auto-sourced content (do not duplicate by hand)

These are wired up by `build.mjs` and update themselves from the source of truth:

| What | Source | Placeholder |
| --- | --- | --- |
| Current version (in headings, hero badge, JSON-LD, examples) | `Cargo.toml` `version = "..."` | `{{version}}` |
| Full `--help` block on the CLI page | `site/cli-help.txt`, captured in CI from the live binary | `{{cli_help}}` |
| Changelog entries | `CHANGELOG.md` parsed at build time | `{{changelog_html}}` |
| Site root path prefix | `BASE_URL` env var | `{{base}}` |
| Repo URL | hardcoded in `build.mjs` | `{{repo}}` |

If you're tempted to copy the version into prose ("works as of v0.6.1…"), use `{{version}}` instead. If you reference the GitHub repo, use `{{repo}}`.

## Workflow before each commit

Run this checklist. It takes under two minutes.

1. **Identify the change**. Is it user-visible? If yes, continue; if no, skip the skill.
2. **Update the relevant page(s)** per the table above. One change = at most two pages, usually one.
3. **Update README.md** only if install steps, badges, or the headline pitch changed. The README is intentionally short; resist the urge to expand it. Bulk content lives on the site.
4. **Refresh `site/cli-help.txt` if a CLI flag changed**:
   ```sh
   cargo build --release --quiet
   ./target/release/postbin-ultra --help > site/cli-help.txt
   ```
5. **Build the site locally and inspect**:
   ```sh
   cd site
   npm ci  # only the first time
   BASE_URL=/PostbinUltra SITE_URL=https://mpjhorner.github.io npm run build
   ```
   The build is sub-second. Check `site/dist/` for the page you changed; open it in a browser if the change is layout-sensitive.
6. **Sweep for slop**. Search the diff for em-dashes (`—`) and the banned adjective list above. The builder won't catch these; they will land on the public site if you don't.
7. **Stage and commit**. Per project policy in `CLAUDE.md`, a user-visible change also bumps `Cargo.toml` and adds a `CHANGELOG.md` entry in the same commit (`feat:` → minor, `fix:`/no prefix → patch, `[major]`/`BREAKING CHANGE:` → major). Documentation-only changes skip the bump.

## Common pitfalls

- **Forgetting `cli-help.txt`.** If you change a flag, the live `--help` text changes but the cached file in the repo doesn't. CI re-captures it on every push, so the deployed site is right; the *local* build will lag until you refresh the cache. Refresh it before committing so reviewers see what the live page will actually contain.
- **Hardcoding `/PostbinUltra/` in links.** Always use `{{base}}/page/`. The local build uses `BASE_URL=/PostbinUltra`; CI does the same. Hardcoded paths break local preview and any future custom-domain switch.
- **Adding a "v0.6.1" reference in prose.** Use `{{version}}` so it tracks the next release without a manual edit.
- **Adding a new page without nav/footer entries.** The page won't be reachable from the main nav. Either edit `NAV` and `FOOTER_LINKS` in `build.mjs` or stop and ask whether the content fits an existing page.
- **Adding "you'll love this" copy on the home page.** The home page is the highest-stakes voice surface; resist marketing tone there above all.

## Verification before commit

The build pipeline catches: missing files, malformed YAML, missing markdown, unbuildable code blocks. It does **not** catch: drift from the source code, em-dashes, banned adjectives, broken cross-references, factually wrong defaults.

You are the last line of defense for those four.
