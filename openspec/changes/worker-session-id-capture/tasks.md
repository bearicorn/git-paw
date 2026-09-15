## 1. Config templates

- [ ] 1.1 Add `session_id_args: Option<String>` and `resume_args: Option<String>` to `CustomCli` (`src/config/cli.rs`) with `#[serde(default, skip_serializing_if = "Option::is_none")]`; load-time validation rejects a template lacking `{{SESSION_ID}}`, naming the key
- [ ] 1.2 Add `resolve_session_id_args(cli, id, clis)` / `resolve_resume_args(cli, id, clis)` beside `resolve_approval_flags`: override → built-in (`claude`: `--session-id {{SESSION_ID}}` / `--resume {{SESSION_ID}}`) → none; verify the Claude flags against upstream docs
- [ ] 1.3 Unit tests: built-in Claude rows; override wins; unknown CLI → none; placeholder-less template rejected

## 2. Identifier + persistence

- [ ] 2.1 Add an OS-randomness v4-UUID helper (no new dependency); unit test the shape and distinctness
- [ ] 2.2 Add `WorktreeEntry.cli_session_id: Option<String>` (`serde(default, skip_serializing_if)`); round-trip, older-file-loads-None, and omitted-when-None tests
- [ ] 2.3 `attach_agent` (`src/main.rs`): when a `session_id_args` template resolves, generate the id, append the rendered args after approval flags, record it on the entry; unchanged command and `None` otherwise; tests for both paths and for flag ordering

## 3. Recovery + status

- [ ] 3.1 `recover_session` (`src/commands/recover.rs`): append rendered `resume_args` when the entry has an id and the CLI resolves a template; unchanged command otherwise; recovery never fails on a missing template; tests for the three scenarios
- [ ] 3.2 `git paw status --json`: add `cli_session_id` (string or `null`) per agent; human output unchanged (byte-identity test)

## 4. Docs + gates

- [ ] 4.1 Configuration reference: `session_id_args` / `resume_args` rows in the `[clis.<name>]` table with the `{{SESSION_ID}}` placeholder; `git paw init` commented examples
- [ ] 4.2 `docs/src/user-guide/session-lifecycle.md` (recovery resumes worker context), `docs/src/supported-clis.md` (which CLIs resume), `status --json` field in `cli-reference.md`; `mdbook build docs/` succeeds
- [ ] 4.3 Every scenario maps to a named test; `just check` + `just deny` green; existing session fixtures load unchanged
