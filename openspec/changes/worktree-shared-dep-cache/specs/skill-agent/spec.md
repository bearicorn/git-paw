## MODIFIED Requirements

### Requirement: Coordination skill — worktree environment orientation

The embedded coordination skill SHALL include a worktree-environment orientation section so a worker does not burn budget investigating expected-policy conditions in a fresh, possibly FS-confined worktree. In stack-agnostic terms it SHALL cover:

- **Install artifacts.** A fresh worktree's per-stack install artifacts (dependency directories, build/restore caches) are gitignored — absent from the checkout — and may already be provisioned by an operator-configured worktree `on_create` hook before the CLI starts. The repo-root copy is not writable by the worker, so the worker SHALL NOT reach for it or symlink it; if the artifacts are genuinely missing, the worker SHALL run its stack's install/restore step in its own worktree — expected setup, not a misconfiguration to diagnose.
- **Shared caches.** A directory in the worktree that git-paw created as a symlink to the main working tree (an operator-configured shared download cache) is shared by every worktree and is read-mostly: the worker SHALL let its stack's install step use it, and SHALL NOT delete, clean, prune, or replace it, nor turn it into a regular directory.
- **FS-confinement is policy.** If the worktree is FS-confined by an operator-configured sandbox, writes are permitted inside the worktree (and shared `.git`, the CLI's caches, `TMPDIR`) and denied elsewhere (`$HOME`, the repo root, `.git/hooks` / `.git/config` / `.git/info/exclude`). An `Operation not permitted` on a path outside the worktree is policy, not a broken machine, and the worker SHALL NOT investigate it with `xattr` / `id` / `ls -lO@` / write-probes — it SHALL adapt and work inside its worktree.
- **setuid binaries under a sandbox.** Some setuid-root system binaries cannot exec inside a sandbox — e.g. `ps` fails with `operation not permitted` because the kernel refuses to exec a setuid binary in a sandbox. This is expected, not fixable, and not a symptom of anything else; everyday tools (`git` and the stack's toolchain) work normally.

The section SHALL NOT hard-code any stack's install command or artifact names (per the export-agnosticism principle); stack-specific steps SHALL be phrased generically (e.g. "your stack's install step") or sourced from the resolved stack. git-paw's own surface (the `on_create` hook, the sandbox, `.git-paw/` paths, the shared-cache links) MAY be named.

#### Scenario: The coordination skill teaches worktree-environment orientation

- **WHEN** the coordination skill is rendered
- **THEN** it SHALL state that an `Operation not permitted` outside the worktree is policy, not a fault, and direct the worker to adapt rather than probe (e.g. with `xattr` / `ls -lO@`)
- **AND** it SHALL note that fresh-worktree install artifacts are gitignored (possibly provisioned by an `on_create` hook) and to run the stack's install step if genuinely missing
- **AND** it SHALL note that a setuid binary such as `ps` cannot exec under a sandbox and that this is expected

#### Scenario: The coordination skill protects shared caches

- **WHEN** the coordination skill is rendered
- **THEN** it SHALL state that a git-paw-created shared-cache symlink is shared by every worktree and SHALL NOT be deleted, cleaned, pruned or replaced

#### Scenario: The orientation section is stack-agnostic

- **WHEN** the rendered coordination skill's worktree-environment section is inspected
- **THEN** it SHALL NOT contain a hard-coded stack package-manager command or artifact name (it SHALL refer to "your stack's install step" rather than a specific package-manager invocation), so it passes the export-agnosticism conformance audit
