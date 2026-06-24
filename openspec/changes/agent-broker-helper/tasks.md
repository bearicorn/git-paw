# Tasks

## 1. Author the bundled `broker.sh` helper

- [ ] 1.1 Create `assets/scripts/broker.sh` (bash, `set -u`, bash
  shebang) mirroring `sweep.sh`'s structure: project-root discovery via
  `git rev-parse --show-toplevel`, Python-3 detection, and broker-URL
  discovery from `<repo>/.git-paw/config.toml [broker]` (port + bind,
  default `http://127.0.0.1:9119`).
- [ ] 1.2 Implement agent-id resolution: take `--agent <id>` (the
  pre-expanded branch id passed by the boot block) or fall back to
  slugifying the current worktree branch (mirror `sweep.sh`'s
  `resolve_agent_for_path` slug rules).
- [ ] 1.3 Implement publish subcommands assembling JSON internally and
  POSTing to `<broker-url>/publish`:
  - `status <message>` → `agent.status` (`status:"working"`, message,
    `modified_files:[]`)
  - `artifact [--exports a,b] [--files a,b]` → `agent.artifact`
    (`status:"done"`, `exports`, `modified_files`) — same shape as the
    prior raw done curl
  - `blocked <needs> <from>` → `agent.blocked`
  - `question <text>` → `agent.question`
  - `intent <summary> <files> [valid_for_seconds]` → `agent.intent`
- [ ] 1.4 Implement `poll [since]` → `GET
  <broker-url>/messages/<agent-id>?since=<n>` and emit returned
  messages.
- [ ] 1.5 Add a `usage`/`--help` block enumerating each subcommand and
  the payload it publishes; unknown subcommand exits non-zero with usage.
- [ ] 1.6 Follow the stdin discipline: any embedded interpreter script
  uses `-c "$(cat <<'EOF' … EOF)"`, never `interpreter - <<`.

## 2. Install the helper via `git paw init`

- [ ] 2.1 In `src/init.rs`, add
  `const BROKER_SCRIPT: &str = include_str!("../assets/scripts/broker.sh");`.
- [ ] 2.2 Write `broker.sh` to `<repo>/.git-paw/scripts/broker.sh` with
  mode `0o755`, overwriting any existing file — reuse the same
  install-and-chmod path as `install_sweep_script` (factor a shared
  installer if clean).
- [ ] 2.3 Report `Created`/`Updated .git-paw/scripts/broker.sh` in the
  init summary, exactly like the sweep install.

## 3. Rewrite the boot block to call the helper

- [ ] 3.1 In `assets/boot-block-template.md`, replace the four raw
  `curl -s -X POST {{GIT_PAW_BROKER_URL}}/publish …` blocks (REGISTER,
  DONE-fallback, BLOCKED, QUESTION) with `.git-paw/scripts/broker.sh`
  invocations carrying the pre-expanded `{{BRANCH_ID}}` (e.g.
  `broker.sh --agent {{BRANCH_ID}} status booting`).
- [ ] 3.2 Keep the DONE section's commit-first ordering and the
  uncommitted-changes warning; the manual fallback becomes a
  `broker.sh artifact` invocation with the unchanged `agent.artifact
  status:"done"` shape.
- [ ] 3.3 Remove broker-URL/JSON-shaping prose from the boot block; the
  block SHALL NOT inline a raw broker `curl` for any of the four events.
- [ ] 3.4 Update any boot-block builder code/tests in `src/agents.rs` /
  `src/skills.rs` that assert raw-curl content.

## 4. Seed the least-privilege helper-path allowlist

- [ ] 4.1 In `src/supervisor/curl_allowlist.rs` (and/or the
  custom-CLI seeding path), replace the per-endpoint
  `curl <broker-url><endpoint>` prefixes with the single
  `.git-paw/scripts/broker.sh` path grant (seed both the bare path and
  any `bash .git-paw/scripts/broker.sh` form the boot block emits).
- [ ] 4.2 Preserve every existing seeding property: config-driven
  targets (repo-local `.claude/settings.json` always + each
  `[clis.<name>].settings_path`), never create a CLI config directory,
  idempotent, deduped across supervisor/agent paths, non-fatal on write
  failure.
- [ ] 4.3 Ensure no `curl *` (broad curl) grant is seeded anywhere on
  the launch path.

## 5. Tests

- [ ] 5.1 Add `tests/broker_sh_conventions.rs` (analogous to
  `sweep_sh_conventions`): scan `assets/scripts/broker.sh` for the
  `interpreter - <<` heredoc shape; assert none on non-comment lines and
  that the scanner flags a synthetic `python3 - <<` body with the line.
- [ ] 5.2 Add an init test (analogous to
  `cli_init_writes_sweep_script`): `git paw init` writes
  `.git-paw/scripts/broker.sh`, first line is a shebang, mode has the
  execute bits, and a stale file is overwritten.
- [ ] 5.3 Boot-block content test: the rendered boot block calls
  `.git-paw/scripts/broker.sh` for all four events and contains no raw
  broker `curl`; the DONE fallback still publishes `agent.artifact
  status:"done"`.
- [ ] 5.4 Allowlist test: seeding produces a `.git-paw/scripts/broker.sh`
  path grant and no `curl *` grant; re-seeding is idempotent.
- [ ] 5.5 Behavioral helper test (mirroring
  `sweep_sh_session_discovery`): run the installed `broker.sh` against a
  config with a non-default broker port and assert it targets the
  configured URL.

## 6. Docs

- [ ] 6.1 Update the boot-block and `git paw init` mdBook chapters
  (`docs/src/`) to describe `broker.sh`, its install, and the
  helper-path allowlist (replacing the per-endpoint curl description).
- [ ] 6.2 Add a helper reference enumerating the `broker.sh`
  subcommands (`status`/`artifact`/`blocked`/`question`/`intent`/`poll`).
- [ ] 6.3 Update README/CLI/config references if the allowlist
  description changes; ensure `mdbook build docs/` succeeds.

## 7. Quality gates

- [ ] 7.1 `just check` (fmt + clippy + all tests) passes; no
  `unwrap()`/`expect()` in non-test code; all public items documented.
- [ ] 7.2 `just deny` passes (no new dependencies introduced).
- [ ] 7.3 Backward compatibility verified: a session carrying a
  pre-existing per-endpoint `curl` allowlist still launches; broker
  endpoints unchanged.
