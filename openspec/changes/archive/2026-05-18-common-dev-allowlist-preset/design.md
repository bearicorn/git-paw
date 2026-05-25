## Context

git-paw already seeds `.claude/settings.json::allowed_bash_prefixes`
with broker-curl prefixes when supervisor mode starts a session
(`src/supervisor/curl_allowlist.rs`, called from
`src/main.rs::cmd_supervisor()` at lines 746-757 and 1353-1358). That
mechanism solves drift 27's narrow case: agents `curl`-ing the broker
don't trigger Claude permission prompts.

The v0.5.0 dogfood surfaced a much wider noise floor: 100+ permission
prompts in a single multi-hour session, dominated by trivial dev-loop
commands (`cargo build`, `cargo test`, `git commit`, `git push`, `just
check`, `mdbook build`, `openspec validate`, `find`, `grep`). Claude's
"Yes, don't ask again for X" is an **exact-string** rule, so each
slight variation (`cargo test` vs `cargo test --lib`) registers as a
new prompt. Supervisor approval time became the bottleneck.

This change generalises the curl-allowlist seeding mechanism to a
**common dev-command allowlist**: a hard-coded preset of prefix patterns
git-paw writes into the same `.claude/settings.json` on supervisor start,
plus a user-extensible `extra: Vec<String>` field for per-project
additions. The full per-CLI specialisation work (Codex `auto-approve`,
Gemini `auto-approve-shell`, etc.) is deferred to v1.0.0 hook providers.
v0.5.0 scopes to Claude / `~/.claude-oss`.

## Goals / Non-Goals

**Goals:**
- Cut the supervisor's manual-approval workload by ~80% for the
  observed dogfood-command distribution.
- Re-use the existing `curl_allowlist.rs` merge semantics — same
  file, same JSON shape, same idempotency contract.
- Keep the preset reviewable in source (`pub const &[&str]`), not
  config-driven, so the security-sensitive surface is read once at
  PR-review time.
- Give the user a single line to opt out
  (`[supervisor.common_dev_allowlist] enabled = false`).
- Give the user a list to opt in to project-specific additions
  (`extra = ["..."]`).

**Non-Goals:**
- Per-CLI placement for Codex / Gemini / opencode / Cursor. Deferred to
  v1.0.0 hook providers.
- Pattern-language richness (regex, glob, exec-tree analysis). Claude's
  `allowed_bash_prefixes` is prefix-only; we don't pretend to offer more.
- Removing patterns on `git paw stop` or anywhere else. The settings
  file is global to the user's Claude install; per-session cleanup
  would race with concurrent sessions.
- Negative entries / denylist. Out of scope; `allowed_bash_prefixes`
  is allowlist-only.
- Validation of `extra` entries. The user owns what they put there;
  garbage prefixes are harmless (no Claude command matches them).

## Decisions

### D1. Config shape: enabled + extra, preset is in code

```toml
[supervisor.common_dev_allowlist]
enabled = true                              # default true
extra = ["pnpm test", "deno fmt"]           # project-specific additions
```

**Alternatives considered:**
- *Per-pattern config (every preset entry toggleable individually).*
  Rejected: explodes the config surface; users don't want to maintain
  a 30-entry allowlist; the curated preset is the *value* of the
  feature.
- *Per-domain config (`cargo = true, git = false, ...`).* Rejected:
  granularity is below the noise floor — users either want the preset
  or they don't. Edge cases use `enabled = false` + their own `extra`.

**Rationale**: the preset is the curated value-add and lives in
source (one `pub const` constant per design D3). `extra` is the
escape hatch for project-specific commands (`pnpm`, `deno`, `bun`,
`uv`, `make`, etc. — git-paw is Rust-centric in its preset). Two
fields, one decision per user.

### D2. Per-CLI placement: Claude + `~/.claude-oss` only in v0.5.0

Claude's allowlist lives at `<repo>/.claude/settings.json` under the
`allowed_bash_prefixes` array. The alt-config dogfood pattern from
`prompt-submit-fix` documents the same shape under `~/.claude-oss/`
when the user sets `CLAUDE_CONFIG_DIR=~/.claude-oss`. v0.5.0 writes
to:

- `<repo>/.claude/settings.json` — always.
- `~/.claude-oss/settings.json` — only if the directory exists (lets
  the alt-config dogfood pattern inherit the allowlist; doesn't create
  the directory when the user isn't using that pattern).

Other CLIs are **not** addressed:

- **Codex** — has an approval-mode flag (`--approval-mode`) and a
  config file (`~/.codex/config.toml`) but no per-command allowlist
  equivalent to Claude's `allowed_bash_prefixes`. Codex users today
  use `--approval-mode=auto-edit` or `full-auto` to bypass prompts;
  v0.5.0 leaves this path unchanged.
- **Gemini** — has `auto-approve-shell` boolean (all-or-nothing).
- **opencode / Cursor / others** — unknown / no documented shape.

v1.0.0's per-CLI hook-providers capability will introduce a trait
covering "where does this CLI store its allowlist and how does git-paw
add to it" for every supported CLI. This change ships the v0.5.0
mitigation for the CLI git-paw users overwhelmingly run (Claude).

**Alternatives considered:**
- *Add Codex/Gemini in v0.5.0 too.* Rejected: their shapes diverge from
  the prefix-allowlist model. Bolting them on without the hook-providers
  abstraction creates per-CLI conditional branches in `cmd_supervisor()`
  that v1.0.0 has to delete and rewrite.

### D3. Standard preset content

The preset is a `pub const DEV_ALLOWLIST_PRESET: &[&str]` in
`src/supervisor/dev_allowlist.rs`. The exact list:

```rust
pub const DEV_ALLOWLIST_PRESET: &[&str] = &[
    // Cargo (read + build + test)
    "cargo build", "cargo test", "cargo clippy", "cargo fmt",
    "cargo check", "cargo tree", "cargo deny", "cargo update",

    // Git read-only
    "git status", "git log", "git diff", "git show", "git fetch",

    // Git write (project-local; supervisor approves merge-time ops)
    "git commit", "git push", "git pull", "git merge",
    "git stash", "git add", "git restore", "git rm",

    // Just (any recipe)
    "just",

    // mdBook
    "mdbook build",

    // OpenSpec
    "openspec validate", "openspec new", "openspec archive",
    "openspec list", "openspec status", "openspec instructions",

    // Search (read-only)
    "find", "grep", "sed -n",
];
```

**Inclusion rubric** (a pattern qualifies if all three hold):

1. Observed in the v0.5.0 dogfood as a repeated prompt source.
2. Side-effects are bounded to the repo / local working tree / read-only
   filesystem; no arbitrary network or arbitrary code execution.
3. Aligns with CLAUDE.md's existing safety protocols (e.g. no destructive
   git operations).

**Exclusion rubric** (explicit calls vs notable absences):

- `cargo install` — fetches and builds arbitrary crates from crates.io.
  Network + code-execution surface too wide. User opts in via `extra`.
- `cargo run` — executes the project's built binary, which may take
  arbitrary input. Supervisor should see these.
- `cargo bench` — same as `cargo run` (executes user code).
- `git rebase`, `git reset`, `git checkout`, `git branch -D` —
  destructive per CLAUDE.md's git safety protocol; the supervisor must
  see these.
- `git push --force`, `git push -f` — same.
- `find ... -exec` — `find` itself is read-only, but `-exec` invokes
  arbitrary commands. Prefix is plain `find` only; the agent invoking
  `find -exec rm ...` still hits a prompt because the *full command
  line* contains `-exec` which forces Claude's matcher to re-evaluate.
  (Claude's prefix match is on the literal start of the command; a
  user wanting `find -exec` patterns must add the exact `find ... -exec`
  prefix to `extra`.)
- `sed` (write mode) — `sed -i` and `sed` without `-n` can edit files.
  Locked to `sed -n` (the read-only invocation).
- `npm` / `pnpm` / `yarn` / `deno` / `bun` / `uv` / `pip` / `pipx` /
  `gem` / `cargo install` — package-manager networks are out of scope
  for the default preset. Users on JS / Python / Ruby stacks add via
  `extra`.

The rubric is part of the design contract: any future PR adding to the
preset MUST cite which of the three inclusion criteria holds and
defend the absence of the corresponding exclusion criteria.

### D4. When to apply: on `cmd_supervisor()` start, before any agent pane boots

The seeding call happens after worktree pruning and before tmux session
construction (where the broker URL is exported and agent panes are
queued). Specifically:

- `cmd_supervisor()` in `src/main.rs` — after the existing
  `git::prune_worktrees(...)` call, after the existing
  `setup_curl_allowlist(...)` call (when broker is enabled), the new
  `setup_dev_allowlist(...)` call runs **unconditionally on the
  `enabled` config flag** (broker enable status does not gate it — even
  non-broker supervisor sessions benefit from suppressing dev-command
  prompts).
- The recovery path (around `src/main.rs:1353`) makes the same call so
  re-attached sessions get the updated preset.

The agent panes inherit the merged `settings.json` when Claude starts
in each worktree (Claude reads `<repo>/.claude/settings.json` on launch).

**Alternative considered**: seed lazily on first prompt from a
known-class command. Rejected: complicates the implementation
considerably (would need a tmux pane content watcher + classifier)
versus a one-shot pre-launch file write.

### D5. Cleanup: keep patterns after session ends

Arguments **for keeping**:
- Patterns are globally harmless (Claude only applies them when
  matching, and matching is allowlist-only).
- Multiple concurrent sessions in the same repo (or across repos using
  shared `~/.claude-oss`) would race if cleanup ran per-session.
- Idempotent re-seed on next session start guarantees the patterns are
  present when needed.
- Matches the existing `curl_allowlist.rs` precedent (which also does
  not clean up).

Arguments **for removing**:
- Per-session isolation: a user who turns the feature off and runs a
  session expecting prompts won't see them for previously-seeded
  patterns.

**Decision: keep.** The dogfood priority is signal-to-noise, not
isolation. The user can manually prune `.claude/settings.json` if they
need a clean slate. (We document this in the supervisor user guide.)

### D6. Backward compatibility: enabled by default

Arguments **for `enabled = true` default**:
- The dogfood evidence is concrete: 100+ approvals across one session.
  Shipping disabled-by-default makes the feature invisible.
- Opt-out is one TOML line.
- Existing `curl_allowlist.rs` is also unconditional (no opt-out at all).

Arguments **for `enabled = false` default**:
- Conservative: don't modify the user's `.claude/settings.json`
  without explicit consent.
- Matches the `[supervisor.auto_approve]` precedent, where the
  presence of the table opts in.

**Decision: `enabled = true`.** The feature exists *because* the
defaults are wrong on a fresh install. Disabling-by-default reproduces
the exact bug the dogfood surfaced. The `enabled = false` line gives
users full control with one TOML edit, and the patterns appended are
narrow and reviewable (D3).

Existing v0.2.0 / v0.3.0 / v0.4.0 configs without the table fall
through to the default (`enabled = true`, `extra = []`), so the
upgrade path is: `git paw start` on v0.5.0 -> first supervisor session
appends the preset to `.claude/settings.json` -> documented in release
notes.

### D7. Module placement: peer to `curl_allowlist.rs`

The new module is `src/supervisor/dev_allowlist.rs`. Reasons:

- Same target file (`.claude/settings.json`).
- Same merge semantics required (preserve existing, append missing, no
  duplicates, parent-dir creation, non-fatal failure).
- Different inputs (preset constant + extra Vec vs broker URL).
- Different trigger (always-on supervisor seeding vs broker-enabled).

The shared semantic surface (`allowed_bash_prefixes` merge) is small
enough that we **do not** extract it into a shared helper in this
change; the two modules each contain ~30 lines of merge logic that
read independently. If a third such seeder appears we extract.

## Risks / Trade-offs

- **[Risk] Preset entries surprise users on first v0.5.0 supervisor
  session.** Mitigated by release-notes call-out + the preset being
  reviewable in source. The `.claude/settings.json` change is
  contained, additive, and one-line-revertible (delete the lines).
- **[Risk] User runs `cargo install some-malicious-crate` thinking
  cargo is allowlisted.** `cargo install` is **not** in the preset
  (D3 exclusion rubric); only safe cargo sub-commands are.
- **[Risk] `git push` to a wrong remote.** `git push` is allowlisted
  because it's high-frequency in agent loops, but per CLAUDE.md the
  pre-push hook already blocks worktree pushes when broker is enabled
  — supervisor still owns merge-time pushes. The preset's `git push`
  entry doesn't bypass that hook.
- **[Trade-off] No per-CLI support in v0.5.0.** Users on Codex/Gemini
  see no benefit from this change; they use those CLIs' native
  bypass flags. v1.0.0 hook-providers closes the gap.
- **[Trade-off] Curated preset means git-paw decides what's "safe".**
  The inclusion/exclusion rubric (D3) is the only defence; reviewers
  enforce it on PRs that touch the constant.
- **[Trade-off] Settings file state persists across sessions.** Users
  who want a clean slate prune `.claude/settings.json` manually. We
  document the file path in the user guide.

## Migration Plan

Additive.

1. Land this change behind `enabled = true` default.
2. v0.5.0 release notes call out:
   - New `[supervisor.common_dev_allowlist]` config section, defaults
     enabled.
   - Lists the preset patterns in full.
   - Tells users where to opt out
     (`[supervisor.common_dev_allowlist] enabled = false`) and where
     to extend (`extra = [...]`).
3. Rollback path: set `enabled = false` in `.git-paw/config.toml` AND
   delete the appended entries from `.claude/settings.json` (or just
   delete the file — Claude rebuilds it). No code rollback required.

## Open Questions

- **Should `extra` patterns be validated (e.g. min length, no
  shell metacharacters)?** Decision: no. The user owns what they
  write; bad entries are harmless to Claude's matcher.
- **Should we de-duplicate against entries the user manually added to
  `.claude/settings.json` outside the preset?** The existing
  `curl_allowlist.rs` merge logic does this (entry already present →
  skip). The dev-allowlist seeder reuses the same logic, so yes,
  automatically.
- **Should `enabled = false` actively remove preset entries on session
  start?** Decision: no (D5). `enabled = false` means "git-paw does
  not write entries this session"; existing entries (whether seeded
  by a prior session or hand-added by the user) are left alone. Users
  who want a clean slate prune `.claude/settings.json` manually.
