## Why

Two production bugs surfaced in the v0.5.0 supervisor surface immediately after
`supervisor-as-pane-followups` archived (2026-05-20), plus the
`supervisor-as-pane-followups` change archived with **task lists left
unchecked** for two of its drift items (drift 68 §8c and drift 69 §8d). The
spec deltas for §8c and §8d propagated to the main specs during archive —
which means `openspec/specs/agent-skills/spec.md` and
`openspec/specs/broker-messages/spec.md` now describe behaviour the binary
does NOT implement. That is a spec-vs-code drift the v0.5.0 release cannot
ship with.

This change is the catch-up: implement the unchecked §8c (bundled sweep
helper installed via `git paw init`) and §8d (broker `agent_id` validation
+ placeholder rejection) tasks, and fix two newly-observed bugs in the
`git paw start` and `cmd_supervisor` flows.

### The four items

1. **Bug A — `git paw start` interactive prompt offers supervisor mode
   without a `[supervisor]` config section, then errors.** The flow:

   ```
   $ git paw start
   Start in supervisor mode? yes
   error: Config error: supervisor mode enabled but [supervisor] config missing
   ```

   `resolve_supervisor_mode` correctly prompts when no `[supervisor]`
   block exists (the prompt's design intent: ask the user when config is
   silent). `cmd_supervisor` then hard-errors with `config.supervisor.as_ref().ok_or_else(...)`.
   The two contradict. The fix is for `cmd_supervisor` to synthesise a
   `SupervisorConfig::default()` when `config.supervisor` is `None`, then
   resolve the supervisor CLI through the existing
   `[supervisor].cli > default_cli > error` chain on the synthetic
   default. Observed during 2026-05-20 release rehearsal.

2. **Bug B — `git paw start` on a stopped session spawns the resumed CLIs
   in the main repo folder instead of each agent's worktree folder.**
   Observed during 2026-05-20 dogfood: after `git paw stop` then
   `git paw start`, every coding agent pane's shell shows the main repo
   path as its cwd, not the worktree path. `recover_session` populates
   `PaneSpec.worktree = entry.worktree_path` correctly, so the bug is
   downstream — either `recover_bare_session` / `recover_supervisor_session`
   passes the worktree to a tmux-builder code path that doesn't honour it,
   or the tmux split-window-then-send-keys ordering races so the `cd`
   prefix lands in the wrong shell. Needs reproducer + RCA in this change.

3. **Drift 68 §8c — Bundled supervisor sweep helper installed via
   `git paw init`.** The `assets/agent-skills/supervisor.md` skill
   currently teaches the supervisor agent to write raw `tmux capture-pane
   -t paw-<project>:0.$p -p` and `curl -s -X POST .../publish -d '...'`
   commands. Both trip per-pattern Claude CLI approval prompts on every
   sweep (the `$p` shell-expansion warning and the `curl ... -H '...'`
   pattern). The fix is a generalized `sweep.sh` shell helper bundled
   via `assets/scripts/sweep.sh`, copied to `<repo>/.git-paw/scripts/sweep.sh`
   by `git paw init`, that the skill examples invoke for `snapshot`,
   `capture`, `approve`, `status`, `inbox`, `feedback-gate`, `verified`,
   and `status-publish`. The helper reads session name from
   `<repo>/.git-paw/sessions/*.json`, broker URL from
   `<repo>/.git-paw/config.toml`, and test command from
   `[supervisor].test_command`, so it's language-agnostic. Already
   specced in §8c.1-8c.10 of the archived `supervisor-as-pane-followups`
   change; tasks left unchecked at archive time.

4. **Drift 69 §8d — Broker `/publish` validates `agent_id` and rejects
   placeholder-shaped payload fields.** The broker currently accepts
   `agent_id: "a"`, `agent_id: "<agent-id>"`, `agent_id: ""`, and any
   other string. The bundled `supervisor.md` skill's curl examples use
   `<your specific question>` as placeholder syntax that, on accidental
   copy-paste without substitution, produces phantom agents in
   `/status`. Spec'd in §8d.1-8d.10 of the archived
   `supervisor-as-pane-followups` change; tasks left unchecked at
   archive time. Implements the broker-side validation as agreed there.

## What Changes

### 1. `cmd_supervisor` synthesises `SupervisorConfig::default()` when missing

`src/main.rs::cmd_supervisor` SHALL replace the existing `ok_or_else`
hard-error on `config.supervisor.is_none()` with a fallback to
`SupervisorConfig::default()`. The existing `[supervisor].cli >
default_cli > error` chain remains as the source of the supervisor CLI;
the error path is reached only when **both** the `[supervisor].cli`
override and the top-level `default_cli` are missing. A
`SupervisorConfig::default()` has `enabled = false`, `cli = None`,
`test_command = None`, `agent_approval = ApprovalLevel::default()`, no
`auto_approve`, default `conflict`, `learnings = false`, default
`learnings_config`, default `common_dev_allowlist`. None of those values
require explicit user opt-in beyond the prompt; the supervisor session
launches with sensible defaults.

The same fix applies to `recover_supervisor_session` (`src/main.rs:1620+`)
which has the same `config.supervisor.as_ref().ok_or_else(...)` pattern
and would re-trigger the bug after a `git paw stop && git paw start`
cycle.

### 2. `recover_session` cwd bug — investigate and fix

The implementing agent SHALL first reproduce bug B with an integration
test (`tests/cli_recover_cwd.rs` or extension of `tests/recover_integration.rs`):
launch a 2-branch session, stop it, restart it, capture each pane's
`#{pane_current_path}` via `tmux display-message -p`, and assert each
coding-agent pane's cwd equals its `worktree_path` from the session
JSON. The expected failure provides the precise symptom for RCA.

Likely cause (informed by code read): `recover_bare_session` builds a
`TmuxSessionBuilder` with the worktree per pane, but the first agent
pane in the supervisor layout (pane 2 in the supervisor layout, the
"agent area") is created by a `split-window` without `-c <agent.worktree>`
and then receives `cd <worktree> && <cli>` via `send-keys`. If the
`send-keys` race with shell readiness loses (per the existing
`-c` race comment in `src/tmux.rs`), the `cd` is dropped. The fix is
to pass `-c <first_agent.worktree>` on the first agent split so the
pane is born in the right cwd, removing the `cd ... &&` race entirely.
Subsequent agent splits already use `-c <agent.worktree>` correctly.

For bare-session recovery, `src/tmux.rs::build` (the
`TmuxSessionBuilder` path) already uses `-c first_worktree` on
`new-session` but subsequent panes use `cd <worktree> && <cli>` via
`send-keys` — same race shape. Subsequent-pane splits SHALL pass `-c
<pane.worktree>` directly.

### 3. Bundle `sweep.sh` and install via `git paw init` (drift 68 §8c implementation)

- Move (or rewrite for generalization) the user's
  `~/.claude-oss/scripts/paw-supervisor-sweep.sh` into
  `assets/scripts/sweep.sh`. Generalize: read session name from
  `<repo>/.git-paw/sessions/*.json` (or the only session if exactly one
  exists); read broker URL from `<repo>/.git-paw/config.toml`
  `[broker].port`; read test command from `[supervisor].test_command`.
  Drop hardcoded values (`paw-git-paw`, `/Users/jieli/...`, `just check`).
- `src/init.rs::run_init` writes the bundled script (via `include_str!`)
  to `<repo>/.git-paw/scripts/sweep.sh` and chmod 0o755.
- Rewrite `assets/agent-skills/supervisor.md` so every tmux-capture,
  raw curl `/publish`, and `for p in ...` loop example invokes
  `.git-paw/scripts/sweep.sh <subcommand>` instead.
- `sweep.sh status` filters `agent_id` values that don't match
  `^(supervisor|feat[-/].+)$` (phantom debris suppression) with `--all`
  to bypass.

This is the §8c implementation pass; the spec deltas already landed in
`openspec/specs/agent-skills/spec.md` from the
`supervisor-as-pane-followups` archive.

### 4. Broker `agent_id` + placeholder validation (drift 69 §8d implementation)

`src/broker/server.rs::publish` handler validates the deserialized
`BrokerMessage`'s top-level `agent_id`:

- Reject (HTTP 400) when `agent_id` does NOT match
  `^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$`.
- Reject (HTTP 400) when any of `payload.question`, `payload.message`,
  `payload.needs`, or any string element of `payload.errors[]` exactly
  matches `^<.*>$` (placeholder syntax).

Error body shape per the spec delta already in
`openspec/specs/broker-messages/spec.md`.

This is the §8d implementation pass; the spec deltas already landed.

## Capabilities

### New Capabilities

*(none — extends existing capabilities)*

### Modified Capabilities

- `supervisor-launch` — `cmd_supervisor` synthesises
  `SupervisorConfig::default()` when `config.supervisor.is_none()` rather
  than hard-erroring; `recover_session` re-uses worktree-cwd correctly so
  resumed coding-agent panes start in their worktree, not the main repo.
- `agent-skills` — bundled `sweep.sh` is installed by `git paw init`;
  the supervisor skill teaches the helper subcommands instead of raw
  tmux+curl pipelines.
- `broker-messages` — `/publish` validates `agent_id` and rejects
  placeholder-shaped payload fields.

## Impact

**Code:**

- `src/main.rs::cmd_supervisor` — replace `ok_or_else(... config
  missing)` with `unwrap_or(&SupervisorConfig::default())`.
- `src/main.rs::recover_supervisor_session` — same fix.
- `src/tmux.rs::TmuxSessionBuilder::build` — pass `-c <pane.worktree>`
  on subsequent agent splits in `build_supervisor_session` and the bare
  builder; drop the `cd <worktree> && <cli>` race path.
- `src/init.rs::run_init` — write `<repo>/.git-paw/scripts/sweep.sh`
  from embedded `assets/scripts/sweep.sh` via `include_str!`; chmod 0o755.
- `src/broker/server.rs::publish` — validate `agent_id` regex and
  payload placeholder-syntax; return HTTP 400 on rejection.
- `assets/scripts/sweep.sh` (new) — generalized helper.
- `assets/agent-skills/supervisor.md` — rewrite tmux/curl examples to
  invoke `.git-paw/scripts/sweep.sh`.

**Tests:**

- `tests/cli_supervisor_no_config.rs` — `git paw start --supervisor`
  without `[supervisor]` config section exits 0 and prints the
  Supervisor-session-launched line.
- `tests/cli_recover_cwd.rs` (or extension) — after `stop && start`,
  every coding-agent pane's `pane_current_path` equals its
  `worktree_path`.
- `tests/cli_init_writes_sweep_script.rs` — `git paw init` writes
  `<repo>/.git-paw/scripts/sweep.sh` and marks it executable.
- `tests/broker_agent_id_validation.rs` — POST `agent_id: "a"`,
  `agent_id: "<agent-id>"`, `payload.question: "<your question>"` each
  return HTTP 400 with explanatory JSON body; valid `agent_id`s
  (`supervisor`, `feat-x`, `feat/x`) and real content return 200/204.
- Skill-content tests — the rendered supervisor skill contains
  `.git-paw/scripts/sweep.sh` invocations and does NOT contain raw
  `for p in ... ; do tmux capture-pane ...` loops or raw
  `curl -X POST .../publish -d '...'` examples for the verified /
  feedback / status families.

**Docs:**

- `docs/src/user-guide/supervisor.md` — mention `.git-paw/scripts/sweep.sh`
  and the canonical subcommands.
- `docs/src/user-guide/init.md` — note the new script written by
  `git paw init`.
- `CHANGELOG.md` — autogenerated, no manual edit.

**Backward compatibility:**

- v0.4-saved sessions with no `[supervisor]` block load without error
  (today they error if the user picks supervisor mode at the prompt;
  this change makes that path work).
- Existing `.git-paw/config.toml` files are unaffected — the broker
  validation is on the request side, not the on-disk config.
- Existing `.git-paw/scripts/` content is overwritten by
  `git paw init` only on explicit invocation; the script is not
  re-installed on `git paw start`.
- Broker-side `agent_id` validation rejects requests that prior
  versions silently accepted. The only known consumers producing those
  values are the supervisor skill's curl examples with unfilled
  placeholders — the rejection is the desired effect.

**Mismatches surfaced:**

- The spec deltas for drift 68 §8c.1-10 and drift 69 §8d.1-10 in
  `openspec/specs/agent-skills/spec.md` and
  `openspec/specs/broker-messages/spec.md` (propagated to main specs
  by the `supervisor-as-pane-followups` archive) now match the
  implementation after this change lands.
