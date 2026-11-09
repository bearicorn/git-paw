## Context

The v0.19 conformance audit compared the code to `docs/src/architecture-direction.md` and the `code-standards` / `concept-decoupling` skills. The findings are independent and small. Each is a seam move under the `code-architecture` contract: output stays byte-identical and the full suite gates each step. The one exception is F2, which makes two prompt-profile fields overridable. File:line references are as of the audit and will be re-located after the rebase onto the in-flight v0.19 branches.

## Goals / Non-Goals

**Goals:**
- Make the source audits see every production line, and add the matching audit for CLI names.
- Take the last one-CLI screen literals out of `commands/` and put them behind the prompt profile.
- Remove ambient I/O and global state from `agent_cli`.
- Resolve each agent's CLI once per command.
- Pin `capture_styled`, delete a redundant dispatcher wrapper, and bring the status table up to date.

**Non-Goals:**
- The `agent_cli` ↔ `config` module cycle (roadmap F12).
- Moving `doctor.rs`'s per-CLI workflow-definition table into `agent_cli` (roadmap F10). It is allowlisted.
- Any change to the `config::resolve_submit_delay_ms` shim's other callers (`commands/supervisor.rs`), or removing the shim.

## Decisions

**D1 — Share the audit filter through `tests/helpers/`.** Each integration test is its own crate, so `production_code` moves into a module in `tests/helpers/` (already used by other tests through `mod helpers;`). Both `source_audit.rs` and `command_library_audit.rs` use it, and `production_part` is deleted. Two fixes go in while it moves:
- A `#[cfg(test)]` that appears inside a `//` comment (as on `src/main.rs:7`) must not start a blank-out. Strip comment lines before scanning for the marker; the line count is kept so reported line numbers stay right.
- An attribute applied to a `;`-terminated item (`#[cfg(test)] mod tests;`, `#[cfg(test)] use …;`) blanks only up to its `;`. The current loop already does this.

Alternative considered: keep `production_part` and teach it to resume after the first test item. Rejected, because two filters would drift apart again.

**D2 — The spec-format regex adds `if let` / `let … else`.** Add `\blet\s+(Some\()?SpecBackendKind::\w+` to the kind-match pattern, with a caught-example line for it. The positive control on `src/main.rs` asserts that a fixed production token (the `fn main` signature, which sits after line 20's `#[cfg(test)]`) survives the filter.

**D3 — The CLI-name audit's pattern.** It flags a quoted known name in these forms:
- an equality test: `(==|!=)\s*(Some\()?"name"` and `"name"\s*(==|!=)`
- a match arm: `"name"\s*(=>|\|)`, or a tuple arm whose first element is the name, `\(\s*"name"\s*,[^)]*\)\s*=>`
- inside `matches!(…)`

A bare literal used as an argument or a default value (for example `detect`'s known-binary list) is not a comparison and is not flagged. Files under `src/agent_cli/` and `*_tests.rs` are skipped. The allowlist is a `(path, trimmed line)` list holding the `src/doctor.rs` `("claude", "/opsx:apply") =>` arm, with a comment pointing at roadmap item F10. A stale allowlist entry (one that no longer matches any line) fails the test, so the list cannot rot.

**D4 — Menu-shape fields.** `CliPromptProfile` gains `menu_cursor_glyphs: Vec<String>` (default `["❯", "›", ">"]`) and `menu_footer_markers: Vec<String>` (default `["enter to select"]`). The override struct gains matching `Option<Vec<String>>` fields with `#[serde(default, skip_serializing_if = "Option::is_none")]`. These fields follow the same per-field fallback as the existing marker lists, and the empty-marker load check from `v019-review-followups` applies to them. They are literals, not regexes, so the `grep -E` subset check does not apply. `sweep.sh` does not read them, so no shell-side change is needed. The helper command can print them because it prints any marker-list field.

The pure readers `menu_showing`, `strip_decoration_keep_cursor`, `composer_text_is_dim`, `styled_cells` and `apply_sgr` move to a new `src/supervisor/screen.rs`. `permission_prompt.rs` stays focused on the live-prompt gate. `menu_showing` takes the profile and matches a cursor as any glyph in `menu_cursor_glyphs` followed by an option line. A footer matches when any marker is a case-insensitive substring of a tail line. The `send.rs` unit tests that cover these readers move with them. `send.rs` keeps the decisions about refusing a send.

**D5 — Warnings as values (F3).**
- `resolve_for_launch` returns `Result<Launchable, PawError>`, where `Launchable { profile: CliProfile, warning: Option<String> }`, or the tuple equivalent. The `static SEEN` and `first_warning_for` are deleted.
- Each launch path collects warnings into a per-launch `BTreeSet<String>` (or checks before emitting) and emits each one once through `out.warn(…)`. `start` resolves the same CLI in pre-flight and then per pane, so the dedupe set is owned by the launch, not by the function.
- `recover.rs` and `supervisor.rs` emit through their `Output`. `launch.rs` threads the warning back to its caller rather than printing.
- `AgentProfiles::record` returns `Option<String>`. The drive-loop shell, which already prints its diagnostics with `eprintln!`, prints it.
- `locate(command, path, cwd: &Path)` takes the working directory. `ensure_unambiguous_in` takes it too. `ensure_unambiguous` keeps its signature by reading `current_dir` at that one shell-side wrapper, or its callers pass the repo root. Pick whichever keeps the diff smaller; the core function no longer reads it.

A CLI run still shows the warning on stderr through `TerminalOutput::warn`, the same text once per run as today. The stderr-leak test runs a launch path through `CaptureOutput` in a child process (`assert_cmd` on a test binary, or a test that spawns `git paw` with the hidden MCP path) and asserts that stderr is empty while the captured warnings hold the line.

**D6 — One resolution (F4).** `send::deliver` resolves `let cli = CliProfile::resolve(&agent.cli, target.clis)` once and reads `cli.compiled_prompt_profile()?` and `cli.submit_delay_ms()`. `add.rs` already holds `agent_profile` (~178) and uses `agent_profile.submit_delay_ms()` at ~416. Both are byte-identical: the shim delegates to the same resolution.

**D7 — Keystroke dispatcher (F7).** `KeyDispatcher::send_key` / `send_text` change from `&mut self` to `&self`. The blanket `impl<T: TerminalBackend + ?Sized> KeyDispatcher for T` already forwards to `&self` helpers. The `dispatcher` fields on `SubmitPort` and the drive deps become `&'a dyn KeyDispatcher`. `BackendDispatcher` (send.rs) and `SharedFakeDispatcher` (drive tests) are deleted, and callers pass the backend itself. The test-only `RecordingDispatcher` keeps its log in a `RefCell<Vec<…>>`. This is a crate-internal trait, so no public surface changes.

**D8 — Styled capture in the fake (F6).** `Operation` gains `CaptureStyled(SurfaceId)`. `FakeBackend::capture_styled` records it and returns the scripted screen, as `capture_tail` does. Existing tests that assert operation sequences around `__send` are updated to expect `CaptureStyled` where they saw `CaptureTail`; that is the only test churn. The golden argv test sits beside the existing fixtures under `src/terminal/fixtures/`.

**D9 — F5 is conditional.** After the rebase, if `git.rs` imports `crate::agents::is_managed_path`, the rebase-skip decision takes the predicate as a parameter (`impl Fn(&Path) -> bool`), and the caller in `commands/` supplies `agents::is_managed_path`. If that touches more than the decision function and its one caller, a roadmap note is added instead.

**D10 — F16 and F17 are spec fixes first.** The MODIFIED requirements describe what the code already does: `create_worktree_with` rebases only in the branch's worktree, `budget::awaits_review` exempts the cost-cap notice, and `doctor::configured_worker_clis` defines the configured set. Each scenario keeps its name, so the archive keeps its traceability. Only the new scenarios need new tests. Where a test already exists, it gets a scenario reference instead of a duplicate.

## Risks / Trade-offs

- [Rebase churn: five branches touch `send.rs`, `start.rs`, `drive.rs` and `prompt_profile.rs`] → Implement only after the rebase, re-grep every site listed in Impact, and keep one commit per finding group so conflicts stay local.
- [The wider audit filter surfaces existing offenders in `drive.rs` and the other files] → Fix the real ones as part of F1; a pre-existing site that is a separate roadmap item is allowlisted with a pointer, not silently skipped.
- [`&mut self` → `&self` on `KeyDispatcher` hides mutation in test fakes] → Only the recording fakes mutate, and they use `RefCell`. Production backends already take `&self`.
- [F2 is a behaviour change for overridden profiles] → Defaults equal today's literals, and the default-profile scenario pins unchanged behaviour.
