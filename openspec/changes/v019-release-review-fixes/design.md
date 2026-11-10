## Context

Two reviews ran after every other v0.19 change merged: a whole-release regression review (R1–R3) and a post-merge audit of `architecture-conformance-fixes` (A1–A12). Every decision below was made in the review brief; this records them. File:line references are as of `d66114b`.

## Goals / Non-Goals

**Goals:**
- A repository upgraded from v0.18 gets helper scripts that match the skill the launch exports.
- No launch selection is ignored silently, and no MCP message tells a client to run a CLI command.
- The spec, docs, tests and audits agree with the code as merged.

**Non-Goals:**
- Refactoring the config loader's `eprintln!` + `static OnceLock` warning dedupe (`src/config/mod.rs` ~681-745, `src/config/broker.rs` ~46) — roadmap.
- Renaming or moving scenarios: the A5 "README Features section …" titles, A6, A8, and A10's git-operations title — roadmap. `--strict` rejects renames inside a MODIFIED block, so titles stay.

## Decisions

**D1 — Refresh root scripts where the skill is re-exported (R1).** `src/init.rs` gains `pub fn refresh_root_scripts(repo_root) -> Result<Vec<(String, WriteOutcome)>, PawError>`, which creates `.git-paw/scripts/` if missing and installs every `bundled_scripts()` entry through `install_script`. `install_script` already writes only when content differs (`write_managed_file`) and refuses a symlink, so the refresh is idempotent and no new guard is needed. `run_init` uses it (and keeps its per-file report), and `write_orchestrator_context` in `src/commands/supervisor.rs` calls it right after `export_skills`. That one function covers both the fresh supervised launch (~664) and the drive-loop bind (~1081). The set is the same one init installs, `_paw_common.sh` included, because `sweep.sh` sources it.

Alternative considered: refresh only `sweep.sh`. Rejected: the helpers share `_paw_common.sh`, and a partial refresh could pair a new script with an old preamble.

The upgrade note goes into `docs/src/user-guide/supervisor.md` next to the existing "Upgrading mid-run" note: a supervised launch refreshes `.git-paw/scripts/`, so local edits there are overwritten (as `git paw init` already does).

**D2 — Unattended selection notice (R2).** In `supervisor()` (~380), the `unattended` branch prints `selection_ignored_notice` through `out.warn` before returning `DriveExisting`, when a selection was given and the session records agents. A session with no agents is refused by the caller, so it gets no notice. `has_selection` is computed once above the branch and reused by the attended path.

**D3 — One shared CLI hint, stripped for MCP (R3).** `src/commands/start.rs` gains `pub(crate) const ADD_AGENTS_CLI_HINT: &str = " — add agents with `git paw add <branch>` or `git paw add --from-spec <name>`"`. Both `selection_ignored_notice` and the empty-session refusal end with it. The CLI wording changes from "add more with" / "add them with" to "add agents with"; specs only require that the message names `git paw add`, and the one test reading the notice matches its first half. `src/mcp/tools/session.rs` gets `fn mcp_wording(msg: &str) -> String`, which strips `REBASE_SKIP_CLI_HINT` (as now) and `ADD_AGENTS_CLI_HINT`. `start_session` applies it to every warning and to a `PawError::SessionError` the launch returns. Stripping keeps one source of the reason text, as `REBASE_SKIP_CLI_HINT` already does.

**D4 — Spec text only (A1, A4, A9, A10).** A1 narrows the requirement body to the agent-CLI module and launch resolution and keeps all six scenarios. A4 adds the menu fields to the field list. A9 adds one scenario; its test, `no_worker_cli_configured_raises_no_cost_cap_warning`, takes the existing "no CLI" assert out of `the_probe_flags_a_cost_cap_over_a_claude_fleet` so the scenario has its own test. A10 replaces "the same key operations as before this refactor" with the five operations the test asserts.

**D5 — A5 edits the main spec's Purpose directly.** A delta cannot change a Purpose, so `openspec/specs/core-project-conventions/spec.md` is edited in place: "README Features section" → "README Highlights section". Its scenario titles stay.

**D6 — Audit forms (A11).** `DISPATCH_FORMS` gains `.contains(` and `contains(&`; the positive controls add `cli.contains("claude")` and `names.contains(&"codex")`. If the current tree then has a finding outside `src/agent_cli/`, that is a real violation and is fixed by reaching the resolved record, not by an allowlist. `tests/helpers/source_scan.rs`'s `uncommented` blanks every line from one whose trimmed text starts with `/*` through the line holding the matching `*/`. It stays line-based like the `//` rule, so line numbers are kept. A `/*` that opens mid-line after code is left alone: the shared filter only ever removed whole comment lines.

**D7 — Small edits (A2, A3, A7, A12).** The configuration reference `prompt_profile` row and the prompt-profiles guide's validation list and "Breaking since v0.19" note name `menu_cursor_glyphs` (default `❯`, `›`, `>`) and `menu_footer_markers` (default `enter to select`), and say an empty list fails the load naming the field. `tests/readme_features.rs` comments cite "README Highlights section enumerates the current user-facing surface". The two stacked comments above `LaunchWarnings` in `start.rs` (~286) merge into one paragraph.

**D8 — Bound the helpers and the learning sink (H1).** The full suite showed `unattended_rebind_uses_the_recorded_broker_endpoint` hanging once R1 put a real `sweep.sh` in the test repository. `SweepLearningSink` waited on `sweep.sh learn` with no timeout. The helper read the broker from `config.toml`, which the test had pointed at a listener that accepts and never answers, and `curl` had no time bound. Production repositories already carry the script, so the hang was latent there. Decided by the orchestrator:
- `_paw_common.sh` defines `PAW_BROKER_CURL_BOUNDS=(--connect-timeout 2 --max-time 10)` and every broker `curl` passes it. The `events` long-poll passes a later `--max-time "$((wait + 10))"`, which curl takes over the shared one.
- `discover_broker_url` returns `GIT_PAW_BROKER_URL` when it is a loopback `http://host:port` (`127.0.0.1`, `localhost` or `[::1]`), and otherwise warns and reads the config. Every session already sets the variable for its panes, and a pane can rewrite the session environment; `sweep.sh`'s verdict verbs send the authority credential to this address, so a non-loopback value (or one starting with `-`, which curl would read as an option) must never be used.
- Tests that run a helper build the command through `tests/helpers/script_cmd.rs`, which clears the variable, and `TmuxTestEnv::apply` clears it too: a test run inside a live session otherwise published fixtures to that session's broker.
- `SweepLearningSink` spawns the helper with `GIT_PAW_BROKER_URL` set to the session's recorded broker, waits up to `LEARNING_TIMEOUT` (15s, above the helper's own 12s worst case), then kills it, warns and returns.

## Risks / Trade-offs

- [A supervised launch now overwrites local edits to `.git-paw/scripts/*`] → the directory is documented as binary-managed and `git paw init` already overwrites it; the upgrade note says so.
- [`.contains(` is a broad dispatch form] → it only fires on a line that also holds a quoted known CLI name; any hit in the current tree is a real per-CLI test.
- [A broker slower than 10s now fails a helper call] → the broker answers in milliseconds; only the long-poll waits, and it keeps its own longer bound.
- [Changing the CLI hint wording] → one shared suffix; docs and tests that quote it are updated in the same group.
