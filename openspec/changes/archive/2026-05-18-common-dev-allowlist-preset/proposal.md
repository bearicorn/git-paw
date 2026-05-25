## Why

During the v0.5.0 dogfood (10-agent supervisor session), the supervisor
hand-approved **100+ permission prompts over multiple hours**, the vast
majority of which were trivial repetitions of the same dev-loop commands
agents run dozens of times each: `cargo build`, `cargo test`, `cargo
test --lib`, `cargo clippy`, `cargo fmt --check`, `git commit -m "..."`,
`git commit -am "..."`, `git push`, `git push --force-with-lease`, `just
check`, `just deny`, `mdbook build docs/`, `openspec validate
<change>`, `find . -name "*.rs"`, `grep -rn "pattern" src/`.

Claude Code's "Yes, don't ask again for X" prompts are **exact-string
allowlist entries**, not pattern rules. That means `cargo test` and
`cargo test --lib` are two different allowlist entries; approving one
does not approve the other. Across a multi-hour session with multiple
agents each running variants of the same commands, the approval queue
becomes the supervisor's full-time job and drowns out the prompts that
*actually* deserve human judgment (file edits in unfamiliar paths,
network calls outside the broker, destructive git operations).

Drift 27 (v1.0.0 *Per-CLI Broker-Curl Allowlist Seeding*) already covers
this pattern for broker round-trips — git-paw seeds
`.claude/settings.json::allowed_bash_prefixes` with the broker endpoints
on session start so the first `curl http://127.0.0.1:9119/...` doesn't
prompt. That mechanism works. **This change extends the same mechanism
to common dev-loop commands** as the v0.5.0 mitigation. The full v1.0.0
hook-providers work (per-CLI placement for Codex, Gemini, opencode,
etc.) is out of scope here; v0.5.0 ships Claude (and `~/.claude-oss` for
the alt-config dogfood pattern) and defers the rest.

The goal is **dogfood signal-to-noise**: the supervisor's pane should
show prompts that matter, not 80% noise. A built-in preset plus
user-extensible `extra` field gets us there for v0.5.0 without
hand-crafting a 40-entry allowlist per project.

## What Changes

- **New config sub-table**
  `[supervisor.common_dev_allowlist]` with two fields:
  - `enabled: bool` — defaults to `true` (the dogfood needed this on by
    default; opt-out is one TOML line).
  - `extra: Vec<String>` — additional project-specific prefix patterns
    appended to the built-in preset. Defaults to empty.
- **Built-in preset patterns (hard-coded in code, not config-driven).**
  The standard preset covers prefix patterns from the dogfood
  observation — see "Standard preset" below for the exact list. The
  patterns are committed as a `pub const &[&str]` constant so they're
  reviewable in source and not drifting via config.
- **Seed on supervisor start.** When `cmd_supervisor()` boots a session
  AND `[supervisor.common_dev_allowlist] enabled = true`, git-paw merges
  the built-in preset + `extra` patterns into
  `.claude/settings.json::allowed_bash_prefixes` using the same merge
  semantics as the existing curl-allowlist seeding (preserve existing
  entries; append missing ones; no duplicates; create parent dir if
  missing). When a `~/.claude-oss/settings.json` is detected
  (alt-config dogfood pattern documented in `prompt-submit-fix`), the
  same merge applies there too. Failures are logged but **non-fatal**
  (matches the existing curl-allowlist seeding behaviour at
  `src/main.rs:746`).
- **No cleanup on `git paw stop`.** The preset patterns are kept after
  the session ends (Decision D5 in design). They are harmless globally;
  re-seeding on next session start is idempotent.
- **Per-CLI placement is Claude-only in v0.5.0.** Other CLIs
  (Codex, Gemini, opencode) have different settings shapes and a
  designed extension point lands with the v1.0.0 hook-providers
  capability. This change is the **v0.5.0 mitigation** scoped to
  Claude / `~/.claude-oss`; v1.0.0 extends it.

### Standard preset (committed in code)

The built-in preset is the exact list below. Patterns are **prefix
matches** consumed by Claude's `allowed_bash_prefixes`:

| Domain      | Prefix pattern                                          |
| ----------- | ------------------------------------------------------- |
| Cargo       | `cargo build`                                           |
| Cargo       | `cargo test`                                            |
| Cargo       | `cargo clippy`                                          |
| Cargo       | `cargo fmt`                                             |
| Cargo       | `cargo check`                                           |
| Cargo       | `cargo tree`                                            |
| Cargo       | `cargo deny`                                            |
| Cargo       | `cargo update`                                          |
| Git (read)  | `git status`                                            |
| Git (read)  | `git log`                                               |
| Git (read)  | `git diff`                                              |
| Git (read)  | `git show`                                              |
| Git (read)  | `git fetch`                                             |
| Git (write) | `git commit`                                            |
| Git (write) | `git push`                                              |
| Git (write) | `git pull`                                              |
| Git (write) | `git merge`                                             |
| Git (write) | `git stash`                                             |
| Git (write) | `git add`                                               |
| Git (write) | `git restore`                                           |
| Git (write) | `git rm`                                                |
| Just        | `just`                                                  |
| mdBook      | `mdbook build`                                          |
| OpenSpec    | `openspec validate`                                     |
| OpenSpec    | `openspec new`                                          |
| OpenSpec    | `openspec archive`                                      |
| OpenSpec    | `openspec list`                                         |
| OpenSpec    | `openspec status`                                       |
| OpenSpec    | `openspec instructions`                                 |
| Search      | `find`                                                  |
| Search      | `grep`                                                  |
| Search      | `sed -n`                                                |

Notable **exclusions** (intentional, see design D3):

- `cargo install` — fetches arbitrary crates from crates.io; security
  surface too broad for a default preset.
- `cargo run` — agents executing arbitrary repo binaries should hit a
  prompt; user adds via `extra` if they want it.
- `find ... -exec` — `find` itself is read-only, but `-exec` invokes
  arbitrary commands. The prefix is `find` only; `find -exec` patterns
  would have to be added through `extra` (and the user takes the
  responsibility).
- `sed` without `-n` — write-mode `sed` (e.g. `sed -i`) edits files;
  the prefix is locked to `sed -n` which is the read-only invocation.
- `git rebase`, `git reset`, `git checkout`, `git branch -D` —
  destructive; excluded per CLAUDE.md "git safety protocol".

The exclusions are part of the design contract (D3) so future PRs
adding to the preset have to justify against the same rubric.

## Capabilities

### New Capabilities

- `dev-command-allowlist` — the built-in preset, the seeding function,
  and the merge semantics for `allowed_bash_prefixes`. Distinct from
  `curl-allowlist` because the inputs (preset constant vs broker URL),
  the trigger (supervisor start vs broker enabled), and the
  user-extension surface (`extra` field vs none) differ; co-located in
  `.claude/settings.json` but separately configurable.

### Modified Capabilities

- `supervisor-config` — add the `[supervisor.common_dev_allowlist]`
  sub-table (`enabled`, `extra`).

## Impact

**Code**:
- `src/config.rs`: `SupervisorConfig` gains `common_dev_allowlist:
  CommonDevAllowlistConfig` with `#[serde(default)]`. The new struct
  carries `enabled: bool` (default `true`) and `extra: Vec<String>`
  (default empty).
- `src/supervisor/dev_allowlist.rs` (new module, peer to
  `curl_allowlist.rs`): the `DEV_ALLOWLIST_PRESET: &[&str]` constant,
  the `effective_patterns(extra: &[String]) -> Vec<String>` helper,
  and `setup_dev_allowlist(extra, settings_path) -> Result<(),
  PawError>` (mirrors `setup_curl_allowlist`).
- `src/main.rs::cmd_supervisor()`: invoke
  `setup_dev_allowlist(...)` after the existing curl-allowlist call
  (around line 746-757). Same non-fatal failure handling. Also invoked
  in the recovery path (around line 1353).
- `docs/src/user-guide/supervisor.md`: a short subsection
  "Common dev-command allowlist" describing the preset, the opt-out,
  and the `extra` field.
- `docs/src/configuration.md`: document the new sub-table.

**Tests**:
- `src/config.rs` tests: round-trip for the new sub-table; defaults
  when absent; `extra` parses correctly; pre-v0.4 / pre-v0.5 configs
  load with `enabled = true` and `extra = []`.
- `src/supervisor/dev_allowlist.rs` tests (mirror the
  `curl_allowlist.rs` test shape):
  - Writes preset entries to a fresh `settings.json`.
  - Merges with existing user entries (preserves them).
  - No duplicates on re-seed.
  - `extra` patterns appended after preset.
  - Invalid JSON returns error, not panic.
  - Creates parent directory when missing.
  - Skipped entirely when `enabled = false`.
- Integration test (`tests/dev_allowlist_integration.rs` or extension
  of an existing supervisor-launch test): launch the supervisor in a
  tempdir; assert `.claude/settings.json::allowed_bash_prefixes`
  contains the preset entries after launch; assert opt-out
  (`enabled = false`) leaves the file untouched by this code path.

**Backward compatibility**: additive. Configs without
`[supervisor.common_dev_allowlist]` default to `enabled = true` and
empty `extra`. **This changes the on-disk state** of
`.claude/settings.json` for users who already have one — but the change
is strictly *additive* (entries are appended, never removed or
modified) and matches the existing curl-allowlist precedent. Users on
v0.4 who upgrade to v0.5.0 and start a supervisor session will see the
preset entries appear in `.claude/settings.json` on first start.
Documented in the release notes.

**Mismatches surfaced**: none new. Drift 27 (v1.0.0 per-CLI seeding)
remains the longer-term plan for non-Claude CLIs; this change
intentionally scopes to Claude/`~/.claude-oss` to ship the v0.5.0
mitigation without blocking on the hook-providers refactor.
