## ADDED Requirements

### Requirement: Coordination skill — worktree environment orientation

The embedded coordination skill SHALL include a worktree-environment orientation section so a worker does not burn budget investigating expected-policy conditions in a fresh, possibly FS-confined worktree. In stack-agnostic terms it SHALL cover:

- **Install artifacts.** A fresh worktree's per-stack install artifacts (dependency directories, build/restore caches) are gitignored — absent from the checkout — and may already be provisioned by an operator-configured worktree `on_create` hook before the CLI starts. The repo-root copy is not writable by the worker, so the worker SHALL NOT reach for it or symlink it; if the artifacts are genuinely missing, the worker SHALL run its stack's install/restore step in its own worktree — expected setup, not a misconfiguration to diagnose.
- **FS-confinement is policy.** If the worktree is FS-confined by an operator-configured sandbox, writes are permitted inside the worktree (and shared `.git`, the CLI's caches, `TMPDIR`) and denied elsewhere (`$HOME`, the repo root, `.git/hooks` / `.git/config` / `.git/info/exclude`). An `Operation not permitted` on a path outside the worktree is policy, not a broken machine, and the worker SHALL NOT investigate it with `xattr` / `id` / `ls -lO@` / write-probes — it SHALL adapt and work inside its worktree.
- **setuid binaries under a sandbox.** Some setuid-root system binaries cannot exec inside a sandbox — e.g. `ps` fails with `operation not permitted` because the kernel refuses to exec a setuid binary in a sandbox. This is expected, not fixable, and not a symptom of anything else; everyday tools (`git` and the stack's toolchain) work normally.

The section SHALL NOT hard-code any stack's install command or artifact names (per the export-agnosticism principle); stack-specific steps SHALL be phrased generically (e.g. "your stack's install step") or sourced from the resolved stack. git-paw's own surface (the `on_create` hook, the sandbox, `.git-paw/` paths) MAY be named.

#### Scenario: The coordination skill teaches worktree-environment orientation

- **WHEN** the coordination skill is rendered
- **THEN** it SHALL state that an `Operation not permitted` outside the worktree is policy, not a fault, and direct the worker to adapt rather than probe (e.g. with `xattr` / `ls -lO@`)
- **AND** it SHALL note that fresh-worktree install artifacts are gitignored (possibly provisioned by an `on_create` hook) and to run the stack's install step if genuinely missing
- **AND** it SHALL note that a setuid binary such as `ps` cannot exec under a sandbox and that this is expected

#### Scenario: The orientation section is stack-agnostic

- **WHEN** the rendered coordination skill's worktree-environment section is inspected
- **THEN** it SHALL NOT contain a hard-coded stack package-manager command or artifact name (it SHALL refer to "your stack's install step" rather than a specific package-manager invocation), so it passes the export-agnosticism conformance audit

### Requirement: Supervisor skill — tiered permission model and safe-command policy

The embedded supervisor skill SHALL document the tiered permission model the supervisor applies when deciding whether a worker's blocked command may be approved: (1) the worker's CLI-native permission check runs first; (2) if the CLI would prompt, the supervisor consults git-paw's safe-command policy, evaluated authoritatively via `git paw __classify <command>` — the single source shared with the mechanical drive loop, so the skill never maintains a parallel authoritative list that could drift; (3) if the command is not classified safe, the supervisor escalates to the human. The skill SHALL summarise the policy classes as orientation — safe (git-paw's managed helper scripts, worktree-confined dev/test commands, read-mostly verbs) and danger (writes under `.git/` or protected paths, the curated danger-list) — while directing the supervisor to `git paw __classify` as the authoritative check. The skill SHALL be stack-agnostic: it SHALL NOT enumerate a consumer's toolchain verbs as universally safe; safe dev/test commands are sourced from the resolved stack.

#### Scenario: The supervisor skill documents the tiered permission ladder

- **WHEN** the supervisor skill is rendered
- **THEN** it SHALL describe the order — CLI-native permission check, then git-paw's safe-command classification via `git paw __classify`, then escalation to the human
- **AND** it SHALL direct the supervisor to use `git paw __classify` as the authoritative safe-command check

#### Scenario: The safe-command policy is single-sourced and stack-agnostic

- **WHEN** the rendered supervisor skill's permission section is inspected
- **THEN** it SHALL direct the supervisor to `git paw __classify` rather than a hand-maintained authoritative allowlist
- **AND** it SHALL NOT hard-code a consumer's stack toolchain verbs as universally safe
