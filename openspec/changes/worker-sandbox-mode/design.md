## Context

v0.15.0 shipped the sandbox as documentation: a wrapper script around the CLI, pointed at by a
`[clis.<name>]` entry, plus a read-only `doctor` check that `sandbox-exec` / `bwrap` is installed
(`doctor::check_sandbox`). With `cli-provider-record` the wrapper can be named directly by `command`,
and each CLI has a provider record — so git-paw can now know, per CLI, whether its workers are meant
to be confined and how.

The only native-sandbox fact the code encodes is Codex's: `approval_flags("codex", Auto)` is
`--sandbox workspace-write`, verified against upstream 2026-07-15, and its `FullAuto` row is
`--dangerously-bypass-approvals-and-sandbox`, whose name states that it disables the sandbox. The
roadmap names a Claude Code `--sandbox`; nothing in the code encodes it.

## Goals / Non-Goals

**Goals:**

- Let a user declare that a CLI's workers are sandboxed, and use a CLI's own sandbox when it has one.
- Make it impossible for a configured friction level to silently disable a declared native sandbox.
- Make an unsandboxed full-auto worker visible before a run, not after.
- State the residual risk of sandboxed full-auto honestly.

**Non-Goals:**

- Raising any worker's approval level automatically because a sandbox is declared.
- Verifying that a wrapper actually confines anything (git-paw does not read or run it).
- Per-CLI approval levels (see Decisions D3).
- Native sandbox rows for CLIs other than Codex — to verify by the v0.23.0 audit.

## Decisions

**D1 — Two declared modes.** `sandbox = "native"` appends the provider's native sandbox arguments
(or `[clis.<name>].sandbox_args`); a `native` declaration with neither fails config load naming both
keys. `sandbox = "wrapper"` appends nothing — the executable *is* the sandbox. A single
`sandboxed = true` flag was rejected: git-paw must know whether it is responsible for passing the
arguments.

**D2 — Sandbox-disabling rows are refused, not rewritten.** Providers mark built-in approval rows
that turn the CLI's sandbox off (Codex `FullAuto`). For a `native` CLI at such a level, launch is
refused with an error naming `[clis.<name>].approval_args`. Rewriting the row to a sandbox-preserving
form was rejected: that form is not encoded or verified for any CLI (Codex's prompt-free-but-sandboxed
form is to verify by the v0.23.0 audit). An `approval_args` override is the user's explicit choice
and is used as given; the docs say an override can itself disable the sandbox.

**D3 — Skip-permissions stays an explicit level.** The roadmap's goal is reached with
`agent_approval = "full-auto"` on sandboxed workers. Alternatives: (a) a declared sandbox implies
full-auto — rejected, it turns a self-declared config value into a privilege escalation; (b) a
per-CLI approval level so sandboxed and unsandboxed workers can mix in one session — deferred until
a user needs mixed sessions, since `agent_approval` already covers the single-CLI tiered setup.

**D4 — Visibility, not enforcement.** `doctor` reports every configured worker CLI's sandbox mode and
warns (⚠, not ✗) when the effective worker level is `full-auto` and that CLI declares no sandbox.
A hard refusal of unsandboxed full-auto was rejected as a breaking change for existing users; the
warning closes the "green while unsandboxed" gap that GP-10 exposed.

**D5 — Argument order and duplicates.** The builder places sandbox arguments after `args` and before
approval flags, and omits them when the resolved approval flags already contain them verbatim (Codex
at `auto` already passes `--sandbox workspace-write`).

## Risks / Trade-offs

- **A false declaration** (`sandbox = "wrapper"` on an unwrapped CLI) silences the doctor warning →
  the report says the mode is declared, not verified; the posture chapter says the same.
- **Sandboxed full-auto puts no command to a human** → with no prompt, nothing is escalated, and
  git-paw has had no command check of its own since v0.17.0 `classifier-retirement`; the CLI's own
  deny rules and the sandbox are what remain. The FS sandbox blocks writes outside the worktree and, with
  the documented profile, to `.git/hooks` and `.git/config`, but not network egress, reads of
  secrets without read-hardening, ref updates in the shared `.git` (another agent's branch can be
  rewritten or deleted), or pushes with the user's credentials. Documented as residual risk; the
  read-hardening tiers and pushing from outside the sandbox are the mitigations.
- **Native sandbox semantics are the CLI's** (what Codex's `workspace-write` allows, whether it
  blocks network) → git-paw passes the flag and claims nothing beyond "the CLI's own sandbox is on";
  specifics to verify by the v0.23.0 audit.
