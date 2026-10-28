## Why

The roadmap's highest-leverage friction-killer is "a sandboxed worker should run in skip-permissions":
with the v0.15.0 FS-scoped sandbox confining a worker's writes, the approval treadmill mostly guards
what the kernel already guards. Since v0.17.0 `classifier-retirement` git-paw approves no permission
prompt on its own: at `auto` a Claude worker runs with `--permission-mode acceptEdits`, so its file
edits go through, but every other command it runs prompts and, under `--unattended`, is escalated for
a human decision. The agent CLI's own allow rules or `full-auto` inside a sandbox are the only routes
to a wave that does not wait on a human. Today git-paw cannot tell a sandboxed worker from an
unsandboxed one, so it cannot help a user get there safely:

- The sandbox exists only as a user-written wrapper behind `[clis.<name>].command`; git-paw does not
  know it is there, and `git paw doctor` only checks that `sandbox-exec` / `bwrap` is installed.
- A CLI's *own* sandbox is never used on purpose. Codex's `--sandbox workspace-write` is reached only
  as its `auto` approval row, and its `full-auto` row is `--dangerously-bypass-approvals-and-sandbox`
  — so asking for less friction silently switches the sandbox off.
- Nothing warns that a worker is running at `full-auto` with no sandbox at all — the GP-10 failure
  shape (a worker launched unsandboxed while `doctor` stayed green).

## What Changes

- **`[clis.<name>].sandbox`** declares how a CLI's workers are confined: `native` (git-paw passes the
  CLI's own sandbox arguments) or `wrapper` (the `command` is the user's FS-sandbox wrapper). Absent
  means unsandboxed, as today.
- **Native sandbox arguments** are a provider facet with a `[clis.<name>].sandbox_args` override.
  The Codex provider carries `--sandbox workspace-write` (already encoded and verified as its `auto`
  row); no other provider has one until the v0.23.0 audit verifies it. `native` with nothing to pass
  fails config load.
- **Full-auto never switches a declared native sandbox off.** A built-in approval row that disables
  the CLI's sandbox is refused for a `native`-sandboxed CLI, with an error naming the override key.
- **Skip-permissions stays explicit.** Declaring a sandbox does not raise anyone's approval level;
  the recipe is `agent_approval = "full-auto"` on sandboxed workers. `git paw doctor` reports each
  worker CLI's sandbox mode and warns when the effective worker level is `full-auto` and the CLI
  declares no sandbox.
- **Posture:** the security chapter presents sandboxed full-auto as the prompt-free route for an
  unattended wave and states what a sandboxed full-auto worker can still do.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-provider`: sandbox declaration, native sandbox arguments, and the rule that full-auto never
  disables a declared native sandbox (requirements added to the capability `cli-provider-record`
  introduces).
- `preflight-diagnostics`: a per-CLI sandbox-mode report and the unsandboxed-full-auto warning.
- `core-security-posture`: the posture chapter documents sandboxed full-auto and its residual risks.

## Impact

- **Code:** `src/agent_cli/` (sandbox facet, sandbox-disabling marks on approval rows),
  `src/config/cli.rs` + `src/config/mod.rs` (`sandbox`, `sandbox_args`, validation), the launch
  builder (sandbox arguments after `args`, before approval flags, not duplicated), `src/doctor.rs`
  (new check beside `check_sandbox`).
- **Config:** two optional fields; commented examples in `git paw init`. Existing configs unchanged.
- **Behaviour:** none unless `sandbox` is set; `doctor` gains a warning for full-auto without a
  declared sandbox (a warning, not a failure).
- **Enum-variant ripple:** none.
- **Security / safety (reviewed with `security-and-safety-review`):** the change adds no grant and
  no implicit escalation — it can only append arguments that restrict, refuse a launch, or warn.
  Under skip-permissions no prompt is raised, so no command of that worker is escalated to a human
  (git-paw approves nothing itself since v0.17.0); only the CLI's own deny rules and the sandbox
  remain. The FS sandbox confines writes outside the worktree but not network egress, reads of
  secrets (unless read-hardened), git operations against the shared repository's refs, or pushes
  made with the user's credentials. The posture chapter must say so. A `sandbox` value is the user's own assertion about their config and is reported as such,
  never as verified containment.
- **Cross-version dependencies:** requires `cli-provider-record` (record, launch builder, the
  `cli-provider` capability) — this change adds to that capability, so it must archive after it.
  Builds on v0.15.0's sandbox workflow and doctor check, v0.16.0's bypass-acceptance dialog
  handling (GP-03b), and v0.17.0 `classifier-retirement` (every prompt escalated, none approved;
  its posture text names the CLI's permission system and the sandbox as the controls). No
  requirement modified here is modified by an earlier pending change; this change only ADDs.
- **Dependencies:** none.
