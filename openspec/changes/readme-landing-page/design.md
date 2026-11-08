## Context

Four existing requirements pin README content, and four tests enforce them. A 110–125 line README cannot hold what they pin: the enumeration of every flag and config table, a CLI table, a `[specs]` excerpt, and a supervisor quick start with `--force`. Each pinned fact already lives, or will live, on a docs page.

## Decisions

### 1. Pin the link, and pin the detail on the page it links

Each README-content requirement keeps its name and every scenario, as archive requires. Its scenarios now check two things: the README names the topic and links its page, and that page contains the substring the old scenario checked in the README. Discoverability stays "from a single page", now one click deep, and the substring guards keep working.

### 2. `## Highlights` replaces `## Features`

`tests/readme_features.rs` parses the section by heading and guards against a truncated parse with a >500-byte check. Eight linked one-liners run to roughly 700–900 bytes, so the guard stays meaningful. If the final text comes in under the guard, lower the threshold with a comment, but never below the length of one Highlights line.

### 3. The one-line CLI list keeps the binary code spans

`tests/supported_clis_docs_sync.rs` parses a table column. For the README it switches to collecting `` `<binary>` `` code spans from the line that follows `## Supported CLIs`. The same set comparison runs against the roster, so the guard is no weaker.

### 4. Voice

The README is written with the `tone-of-voice` skill's technical-docs rules: lead with the point, use lists and tables over prose, no warm-up and no hedging. The maintainer reviews the draft before it lands.

## Risks / Trade-offs

- [`git paw remove supervisor` sentence] → the brief calls it a v0.18 leftover, but `src/commands/remove.rs:65` still refuses `remove supervisor` with a pointer to `git paw stop`. Keep the refusal fact and reword the reason: no supervisor pane exists, and the orchestrator runs outside the session. Do not delete the sentence as if the behaviour were gone.
- [A reader who wants every feature on one page] → `docs/src/introduction.md`'s Key Features was a v0.6-era list. This change refreshes it to the current surface, one line and one link per feature with no version tags, and the first Highlight links it.
