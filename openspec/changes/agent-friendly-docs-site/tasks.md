## 1. Docs-metadata generator (D1/D2)

- [ ] 1.1 Add a build-tooling generator (a `docs/` script + a `just` recipe) that runs after `mdbook build docs/`, taking the built `docs/book/` directory, `docs/src/SUMMARY.md`, and an explicit build-date argument; add NO new Cargo dependency (it is build tooling, not runtime)
- [ ] 1.2 Parse `SUMMARY.md` into an ordered page list (title, source path → canonical absolute URL); read each page's optional leading `<!-- summary: ... -->` override, else derive the first-sentence fallback

## 2. Discovery + index artifacts

- [ ] 2.1 Emit `docs/book/llms.txt`: H1 title, a one-line blockquote summary of git-paw, then grouped sections of `- [Title](absolute-url): summary` entries ordered by `SUMMARY.md`
- [ ] 2.2 Emit `docs/book/sitemap.xml`: well-formed XML with one `<url><loc>` (canonical absolute URL) per page
- [ ] 2.3 Emit `docs/book/robots.txt`: allow all user agents, do not disallow doc paths, include a `Sitemap:` line to the absolute `sitemap.xml` URL
- [ ] 2.4 Make generation reproducible — take the date from the build-date argument (never the wall clock); two runs over unchanged sources produce byte-identical `llms.txt`/`sitemap.xml`/`robots.txt`

## 3. Per-page structured metadata (D4)

- [ ] 3.1 Post-process each built HTML page to inject into `<head>` a `<meta name="description">` and a machine-readable metadata block (page title, canonical URL, and the anchor ids of its headings)

## 4. Build + CI wiring

- [ ] 4.1 Wire the generator into the `just` docs recipe so a local `mdbook build docs/` produces the four artifacts
- [ ] 4.2 Wire it into the CI `docs` job (`.github/workflows/ci.yml`) after `mdbook build docs/` and before `upload-pages-artifact`, so the artifacts deploy with the Pages upload

## 5. Tests + docs

- [ ] 5.1 Add a build smoke check: `llms.txt`/`sitemap.xml`/`robots.txt` exist and are non-empty, cover every `SUMMARY.md` page, and a sample built page carries the `<head>` metadata block + anchors
- [ ] 5.2 Add a reproducibility test: two generations over unchanged sources yield byte-identical `llms.txt`/`sitemap.xml`/`robots.txt`
- [ ] 5.3 Add a short docs section noting the agent-friendly surface (`llms.txt`, sitemap, per-page metadata) and its URL
