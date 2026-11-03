## Why

The v0.19 post-merge audits of `cli-provider-record`, `cli-profile-threading` and `worker-sandbox-mode` left small gaps. Some behaviour has tests but no spec, some spec text names a syntax that does not exist or frames a rule relative to "before this change", some user-visible behaviour is undocumented, and one dry-run output is misleading. None is large enough for its own change, so they are grouped here in three task groups.

## What Changes

**cli-provider** (from `cli-provider-record`):
- Specify that the headless orchestrator one-shot carries the CLI's `args`: whitespace-split, after the executable and before the native sandbox arguments, the approval flags and the headless arguments. The test `the_entry_args_precede_the_approval_flags_and_headless_form` in `src/supervisor/escalation.rs` already pins this.
- State plainly what an identity's "leading token" is: the record id comes from the leading whitespace-delimited token of the CLI *identity* (`claude --model x` → `claude`), never from the resolved launch command. Today `boot-block` says "use the leading token of the identity" and `cli-provider` says "never re-derive from a launch-command string or its leading token", which reads as a contradiction.
- Add a scenario: a `family = "claude"` custom entry gets the MCP boot-block form end to end, on a real launch path.
- Docs: `supported-clis.md` says that `--cli "claude --model x"` now gets the base CLI's approval flags and session ids. The `add-cli` docs say in one line that `family` and `args` are set in the config file. The `start --dry-run` help mentions the `runs:` lines.

**cli-prompt-profiles** (from `cli-profile-threading`):
- Add requirements, already implemented and tested in `tests/prompt_profile_threading.rs`, for three load-time and helper failures:
  - an empty or whitespace-only marker fails the load
  - a shell-matched pattern that uses a construct outside the `grep -E` subset (`(?…)` flags such as `(?i)`, `\d`, `\s`, `\w`, `\p`, `\A`, `\z`, lazy quantifiers) fails the load
  - `__prompt-profile` exits non-zero on a configuration error
- Add a scenario: a CLI swap on a kept worktree re-resolves the pane's profile.
- Fix the phantom syntax `__prompt-profile <name> <field>` → `__prompt-profile --cli <name> --field <field>`.
- Replace "as before this change" / "current" framing with the rule itself.
- State the one place the helper and the compiled loop differ: a pane with no recorded CLI resolves the session's configured CLI in the helper, but `generic` in the loop.
- Docs: a user-guide sentence that configs using those constructs or empty markers now fail to load (**BREAKING** for those configs), a link from the configuration-reference row to the subset, and a note on the no-recorded-CLI difference.

**worker-sandbox-mode**:
- `start --dry-run` prints a worker's `runs:` plan line before the native-sandbox refusal error, so the plan shows a command that would never run. Print the refusal in place of that line.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-provider`: new requirement "The headless hand-off carries the CLI's launch arguments". Modified "Each agent CLI resolves into one record" (identity vs launch-command wording), "The boot-block form follows the record's MCP capability" (end-to-end scenario), and "Full-auto never disables a declared native sandbox" (dry-run reports the refusal).
- `cli-prompt-profiles`: three new requirements (empty markers, the `grep -E` subset, the helper command's failure exit). Modified "A CLI prompt-shape profile describes one CLI's terminal surface" (helper syntax; body shortened under the strict limit), "The embedded default profile is read only by profile resolution" and "The helper script resolves the profile per pane" (framing, and the no-recorded-CLI difference), and "A surface's CLI is taken from git-paw's launch record" (CLI-swap scenario).

`cli-provider` is already listed in `docs/src/specifications/README.md`. No new capability is added, so the index does not change.

## Impact

- **Code**:
  - `src/commands/launch.rs`: `dry_run_launch_line` applies the native-sandbox refusal before formatting a `runs:` line.
  - `src/cli.rs`: the `start --dry-run` help string.
  - `src/supervisor/escalation.rs`: no behaviour change; the headless-args test gains its scenario reference.
- **Tests**:
  - map the three new `cli-prompt-profiles` requirements to the existing tests in `tests/prompt_profile_threading.rs` (`an_empty_marker_fails_the_load_naming_the_field`, `a_shell_matched_pattern_outside_grep_e_fails_the_load`, `prompt_profile_subcommand_fails_on_an_invalid_config`)
  - new: a CLI-swap profile test, an end-to-end boot-block test for a `family = "claude"` entry, and a dry-run refusal test
- **Docs**:
  - `docs/src/supported-clis.md` (identity with trailing flags, `add-cli` note)
  - `docs/src/cli-reference.md` (`--dry-run` `runs:` lines, refusal in the plan)
  - `docs/src/user-guide/cli-prompt-profiles.md` (breaking load failures, the CLI swap, the no-recorded-CLI difference)
  - `docs/src/configuration/README.md` (prompt-profile row links the `grep -E` subset)
- **No struct-literal or enum ripple**: no field or variant is added.
- **Breaking (already shipped in v0.19)**: configs with empty markers or non-ERE constructs in shell-matched patterns fail to load. This change only documents it.
