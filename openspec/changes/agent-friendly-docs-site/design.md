## Context

git-paw's docs are an mdBook site built by `mdbook build docs/` and deployed to GitHub Pages by the CI `docs` job. mdBook emits human-facing HTML only — no `llms.txt`, no `sitemap.xml`, no `robots.txt`, and no per-page machine metadata. This change adds a machine-readable surface generated from the same sources, so it can never drift from the published content. Constraint: git-paw's approved-dependency set is fixed; a docs-build helper should avoid adding a runtime crate dependency.

## Goals / Non-Goals

**Goals:**
- Emit `llms.txt`, `sitemap.xml`, `robots.txt`, and per-page metadata into `docs/book/` as part of the normal docs build.
- Generation is deterministic from `docs/src/` (same input → same output; no manual upkeep).
- Runs in both local (`just docs` / `mdbook build`) and CI (`docs` job) paths, and the artifacts deploy with the Pages upload.

**Non-Goals:**
- No change to human-facing content, navigation, or theme.
- Not the consuming fetch skill (`docs-fetch-skill`, separate change).
- No hosted search/RAG index — just static discovery + retrieval surfaces.

## Decisions

**D1 — Generate via a post-build step over `docs/book/` + `docs/src/SUMMARY.md`, not an mdBook preprocessor.**
A post-build generator reads the rendered `docs/book/` and the `SUMMARY.md` table of contents, then writes the four artifacts. Rationale: a preprocessor runs *before* rendering (no final URLs/HTML yet) and couples us to mdBook's preprocessor protocol; a post-build pass sees final pages + canonical URLs and is trivial to run and test. *Alternative considered:* an mdBook preprocessor — rejected for coupling + not having final HTML.

**D2 — Generator is a small self-contained script invoked by the justfile + CI, adding no crate dependency.**
The docs build is not part of the shipped binary, so the generator lives in the repo's build tooling (a `docs/` script + a `just` recipe), keeping `Cargo.toml` untouched. Rationale: honors the fixed approved-dependency set; docs generation has no place in the runtime binary. *Alternative:* a Rust `xtask`/bin — heavier, adds a target, no benefit for a static-file emitter. *Open:* exact language (POSIX sh vs a scripting runtime already on CI) — resolved in tasks.

**D3 — `llms.txt` follows the llmstxt.org convention:** an H1 title, a one-line blockquote summary of git-paw, then grouped sections (`## User Guide`, `## Reference`, …) of `- [Page Title](absolute-url): one-line summary` entries, ordered by `SUMMARY.md`. Page summaries come from an optional first-line HTML comment in each source page (`<!-- summary: ... -->`), falling back to the first sentence of the page. Rationale: the de-facto standard maximizes agent compatibility; deriving from source keeps it in sync.

**D4 — Per-page metadata is injected into each built HTML page as a `<meta name="description">` + a small JSON block (title, canonical URL, summary, section anchor list).** Section anchors are read from the rendered heading `id`s. Rationale: lets an agent fetch one page and target a section without loading siblings. *Risk-aware:* keyed on stable rendered structure (heading ids, `<head>`), regenerated every build.

**D5 — `sitemap.xml` + `robots.txt` are standard:** sitemap lists every page's canonical URL (+ build date passed in, since `Date::now` isn't used in generation); `robots.txt` allows all user agents and advertises `Sitemap:`.

## Risks / Trade-offs

- **mdBook output structure changes across versions** → the HTML post-process could break. Mitigation: key on stable anchors (`<head>`, heading `id`s); the generator is regenerated every build so breakage surfaces immediately in CI, and a smoke check asserts the four files exist and are non-empty.
- **GitHub Pages deploy is already flaky** (upstream `upload-pages-artifact` retry-dup, see v0.10.0 CI riders) → the new files ride the *same* Pages artifact, so they add no new failure mode; they inherit the existing `workflow_dispatch` recovery.
- **Summary quality** (auto-derived first sentence can be weak) → allow the per-page `<!-- summary: -->` override for pages that need a better one.

## Migration Plan

Purely additive — new files at the site root, no removals, no redirects. Rollback = drop the generation step; the human site is unaffected. No data or config migration.

## Open Questions

- Generator language/runtime (POSIX sh vs a runtime guaranteed on the CI image) — decide in tasks; must run identically locally and in CI.
- Ship an optional `llms-full.txt` (whole-site concatenation) in this change or defer to `docs-fetch-skill`? Lean: defer; `llms.txt` + per-page fetch covers the need.
