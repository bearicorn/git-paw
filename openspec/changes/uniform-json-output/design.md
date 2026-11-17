## Context

Three commands have `--json` today, and each declares it the same way in clap
(`#[arg(long, help = "Emit machine-readable JSON")] json: bool`):

| Command | Built by | Shape | Format |
|---|---|---|---|
| `status` | `json!` in `commands/status.rs::cmd_status` | `{session, status, tmux_running, worktrees[{branch, cli, worktree_path, cli_session_id}]}`, or `{"session": null}` | compact |
| `approvals` | `json!` in `commands/approvals.rs::cmd_approvals` | `{session, approvals[{pattern, count, suggested_target, first_seen, last_seen}]}` | pretty |
| `doctor` | private `Report` in `doctor.rs::render_json` | `{status, checks[{group, name, status, detail, remedy}]}` | pretty |

None of them has a version. Doctor checks are identified only by display text
(`group` + `name`). Errors go through `PawError::exit` (`error: …` on stderr, exit 1, or 2 for
cancellation). `doctor` exits through `PawError::DoctorFailed` after it prints its document.
`list` does not exist: the read commands are `status`, `list-clis`, `approvals`,
`replay --list` and `doctor`. `add` and `remove` mutate state and print one confirmation line.
The MCP server has its own response DTOs (`SessionSnapshot`, …), and they deliberately differ
from the CLI shapes.

## Goals / Non-Goals

**Goals:**
- One documented contract that every `--json` follows, with a version.
- Programmatic results for the commands an orchestrator scripts: `add`, `remove`,
  `list-clis`.
- Every existing key keeps its name, type and meaning, except `status`'s `tmux_running`, which
  `terminal_alive` replaces (D4).
- Typed output DTOs, so a schema change is a compile-visible edit and not a `json!` literal.

**Non-Goals:**
- `--json` for `replay --list`, which v0.22.0 rethinks.
- `--json` for `start`, `stop`, `pause` and `purge`. These are lifecycle commands that attach,
  block or prompt. The v0.18 MCP lifecycle tools are their programmatic surface.
- A global `--json` flag.
- A JSON error envelope on stdout.
- Sharing DTOs with MCP. Those are different contracts with different consumers.
- Publishing JSON Schema files.

## Decisions

### D1 — `schema_version` at the top level of every document
The key is `schema_version: 1`, a top-level integer. The name `version` is avoided because
broker `/status` already uses `version` for the git-paw package version, and readers would
confuse the two. The version is bumped only for an incompatible change (a removed or renamed
key, or a changed type or meaning), which the deprecation policy reserves for a major release.
Added keys never bump it. Consumers must ignore unknown keys.

*Alternative:* a version per command family, or a version in HTTP-style headers. Neither has a
place to live in a CLI.

### D2 — Typed DTOs in one `src/output.rs`; formatting is not contract
Each command renders a `#[derive(Serialize)]` DTO through `output::print_json(&T)`, which
writes exactly one document and a trailing newline to stdout. Whitespace is not part of the
contract. Today's formatting stays as it is (compact for `status`, pretty for the others), so
any test that matches substrings keeps passing. The DTOs live at the `entry` layer and are
filled from the command's result. After v0.18 `cmd-library-surface`, that result is the typed
outcome the library returns.

### D3 — Stdout discipline and failure semantics
In JSON mode, stdout carries only the document. Progress, warnings and hints already go to
stderr (`eprintln!`) or are suppressed. When a command fails, stdout is empty, the error stays
on stderr unchanged, and the exit code is the same as in human mode. Doctor is not a failure in
this sense: a ✗ check is data, so doctor always prints the document and then exits non-zero.
That is today's behavior, and the `error: doctor found N failing check(s)` line stays on
stderr.

*Alternative:* a `{"schema_version":1,"error":{…}}` document on stdout. It is useful, but it
adds a second shape every consumer must branch on, and the exit code already carries the
signal. Out of scope (resolved question 2).

### D4 — `status`: `terminal_alive` replaces `tmux_running`; `broker` object
`terminal_alive` carries the value `tmux_running` carried: whether the terminal session exists. That is the has-session fact, which v0.17.0's `status-and-picker-truth` keeps separate from "agent CLIs exited"; that state shows as `status: "stopped"` with `terminal_alive: true`.
After v0.18 it comes from the `TerminalBackend`'s session liveness rather than a tmux probe.
`tmux_running` is removed in 0.21.0 (**BREAKING**). It names one terminal implementation, and
keeping it would freeze that name in 1.0.0. Removing it now, before the freeze, costs one rename
for consumers; removing it later would need a major release. No deprecation entry is registered
for it, because nothing is left to deprecate. `schema_version` stays 1: no versioned document
ever carried `tmux_running`, because both land in this release. The MODIFIED *Liveness reflects dead agent panes*
requirement names `terminal_alive` where it named `tmux_running`.

*Alternative:* keep `tmux_running` beside `terminal_alive` as a deprecated alias. Rejected by the
maintainer (2026-11-17): it would stay until 2.0.0.

`broker` is `null` when the session recorded no broker. Otherwise it is
`{"url": …, "state": "running" | "paused" | "not_responding" | "foreign"}`, decided by the same
probe the human output uses. `foreign` appears once `broker-repo-identity` gives the probe a
"git-paw broker for another repository" result. Until then that case reports `not_responding`,
as the human output does. The no-session document becomes
`{"schema_version": 1, "session": null}`.

### D5 — Doctor: stable check `id`
Each check gains `id: "<group-slug>.<check-slug>"`, for example `environment.tmux`,
`config.schema-version` or `hygiene.gitignore`. The ids are declared next to the check
constructors and never derived from display text, so rewording a check never changes its id.
A unit test asserts that ids are unique and match `^[a-z0-9-]+\.[a-z0-9-]+$`. `group`, `name`,
`detail` and `remedy` are unchanged.

A check's `status` is `ok`, `warn`, `fail` or `skipped`. `skipped` marks a check that was not
evaluated; `config-check-command` defines when (a config load error). The document's overall
`status` stays `ok`, `warn` or `fail`, computed over the evaluated checks only, so a skipped check
never changes it or the exit code. `skipped` is part of schema version 1 from its first release,
so no versioned document ever lacked it.

### D6 — `add` / `remove` / `list-clis` documents
- `add`: `{schema_version, session, branch, agent_id, cli, worktree_path, paused}`. `agent_id`
  uses the broker slug rule. No pane index is exposed: it is a tmux artifact that v0.18 turns
  into an opaque surface handle.
- `remove`: `{schema_version, session, branch, worktree_path, worktree_kept}`.
- `list-clis`: `{schema_version, clis: [{name, display_name, source: "detected" | "custom", path}]}`.
  An empty roster is `clis: []` with exit 0. The human output's install hint is not part of the
  JSON.

`add --json` and `remove --json` keep every safety behavior: `remove`'s uncommitted-work refusal
still fails the command. They never introduce a prompt.

### D7 — Roster guard
`tests/json_output_contract.rs` walks the clap command tree (`Cli::command()`), collects every
subcommand that defines `--json`, and asserts that the set equals the roster declared in the
`cli-machine-output` spec. The roster includes `config check`, which `config-check-command` (other half) adds under this contract, so whichever of the two changes lands second enables the full roster assertion. It mirrors that roster in a test constant and asserts the CLI
reference documents each command's JSON. For each roster command it then runs the binary in a
temporary repo and asserts: exactly one JSON object on stdout, an integer `schema_version`, and
no `\x1b` byte. This is behavioral and not a source grep. It follows the `spec-organization` §7
guard pattern.

## Risks / Trade-offs

- [The additive rule is violated later] → the roster guard checks presence. Schema golden tests
  per command pin the key set, so a removed key fails a test.
- [A consumer still reads `tmux_running`] → the key is gone, so the consumer sees a missing key
  and not a wrong value. The CHANGELOG marks the removal as breaking and names `terminal_alive`.
  In-tree readers switch in this change.
- [A command prints a stray stdout line in JSON mode] → the contract test parses stdout as one
  JSON value and fails on trailing data.

## Migration Plan

1. Add `output.rs` and switch `status`, `approvals` and `doctor` to DTOs with no key changes;
   existing tests stay green.
2. Add `schema_version`, `terminal_alive`, `broker`, and the doctor `id`; remove `tmux_running`
   and switch its in-tree readers to `terminal_alive`.
3. Add `--json` to `add`, `remove` and `list-clis`.
4. Add the contract test and the docs.

All steps are additive except the `tmux_running` removal in step 2. Rollback is a revert.

## Resolved Questions (2026-11-17)

1. `tmux_running`: dropped in 0.21.0. `terminal_alive` replaces it. This is an intentional break
   before the 1.0.0 freeze (D4).
2. Stdout JSON error envelope: out of scope. Errors stay on stderr with an empty stdout (D3).
3. Key name: `schema_version` in JSON output and on the wire, `version` in `config.toml`. The specs
   already say this; no spec change.
