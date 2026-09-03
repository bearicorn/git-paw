## Why

git-paw makes each consumer hand-author worker-orientation into their own `AGENTS.md`.
The peer-4-poker dogfood showed the cost: sandboxed workers **burned budget
investigating expected-policy conditions** — an `Operation not permitted` on a path
outside the worktree, a setuid `ps` that "the kernel refuses to exec inside a sandbox"
— probing with `xattr` / `id` / `ls -lO@` / write-probes to diagnose a non-bug; and
workers were confused by gitignored install artifacts. Separately, git-paw's
safe-command *policy* (tier 2 of the permission ladder) lives only in the Rust
classifier, invisible to the LLM supervisor. Both are **worker-guidance git-paw should
export as skills**, not leave to each consumer.

This is the reframe of GP-01: rather than seed an (inert, non-agnostic)
`allowed_bash_prefixes` allowlist into every CLI's settings file, rely on the CLI's own
permission mode + git-paw's classifier, and export the *guidance* as agnostic skills.

## What Changes

- The **embedded coordination skill** (worker-facing) gains a **worktree-environment
  orientation** section: install artifacts are gitignored and may already be
  provisioned by an operator `on_create` hook (don't reach for the repo-root copy or
  symlink it; if genuinely missing, run your stack's install step — expected, not a
  misconfiguration); FS-confinement is *policy, not a broken machine* (an
  `Operation not permitted` outside your worktree — do not probe it, adapt and work
  in-worktree); some setuid binaries (`ps`) cannot exec inside a sandbox and that is
  expected and not fixable.
- The **embedded supervisor skill** (supervisor-facing) documents the **tiered
  permission model**: the worker's CLI-native check first, then git-paw's safe-command
  policy evaluated authoritatively via `git paw __classify` (the single source shared
  with the mechanical drive loop — no parallel list that could drift), then escalate to
  the human.
- Both are **export-agnostic**: no hard-coded stack toolchain (generalized to "your
  stack's install step" / sourced from the resolved stack); the sandbox is framed
  conditionally ("if your operator configured one"). git-paw's own features
  (`on_create`, the sandbox, `git paw __classify`, `.git-paw/scripts`) may be named.

**Non-Goal (explicit):** removing the inert `allowed_bash_prefixes` /
`common_dev_allowlist` / curl-allowlist seeding. It is now redundant, but its removal
ripples across `approval-command-safety`, `core-configuration`, and
`broker-agent-helper` and is a separate deliberate decision — not this change.

## Capabilities

### New Capabilities

<!-- None — extends the existing embedded skills. -->

### Modified Capabilities

- `skill-agent`: the embedded coordination skill gains worktree-environment
  orientation; the embedded supervisor skill gains the tiered permission model and
  safe-command policy (deferring to `git paw __classify`).

## Impact

- **Assets:** `assets/agent-skills/coordination.md` (+ orientation section) and
  `assets/agent-skills/supervisor.md` (+ tiered permission model), rendered by the
  existing skill machinery.
- **Tests:** skill-content tests (`*_skill_content.rs`) for the new subsections, and
  the export-agnosticism conformance test (`tests/agent_skills_conform.rs`) — the new
  content MUST NOT introduce a hard-coded stack toolchain verb.
- **Export-agnosticism:** governed by the `export-agnosticism` dev skill — stack
  install/test steps are generic or sourced from the resolved stack; only git-paw's own
  surface is named. This is a bundled-asset change and must pass that audit.
- **Single source of truth:** the supervisor safe-command policy defers to
  `git paw __classify` (v0.14 `classifier-single-source`), so the skill never maintains
  a parallel authoritative list that could drift from the Rust classifier.
- **Enum-variant ripple:** none.
- **Docs:** the `coordination.md` and `supervisor.md` user-guide chapters mirror the
  skill additions (per the existing coordination-mirror requirement).
