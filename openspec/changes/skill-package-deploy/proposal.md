## Why

After `skill-package-layout`, bundled skills are packages in the source tree, but what git-paw
writes into a repository is still the old split: rendered skill text inlined into the sidecar,
helpers flattened into `.git-paw/scripts/`, and ~99 hand-typed `.git-paw/scripts/<helper>` literals
across seven bundled prose assets (67 in the orchestrator skill alone, the rest in the
coordination and docs-fetch skills, both boot-block forms, the orchestrator session context and
the escalation framing). MILESTONE v0.20.0 asks that each package own its helper and that the
injected `AGENTS.md` block reference the packages, so the boot block and the skill export stay in
one place.

Moving the helper path is a security change, not just a rename. Since v0.17.0 git-paw approves no
permission prompt itself (`classifier-retirement`); the only pre-approval of a helper that git-paw
writes is the consented `git paw setup-cli claude --allowlist` (`cli-setup`). It writes
exact-path allow rules for `.git-paw/scripts/<helper>` into the main checkout's
`.claude/settings.local.json`, together with a deny rule on edits under `**/.git-paw/scripts/**` so an agent in the CLI's
edit-accepting mode cannot rewrite a granted helper and then run it through the grant. Once bundled
prose invokes `.git-paw/skills/<package>/scripts/<helper>`, those grants no longer match, and a grant
for the new path written without new protection would reopen exactly the tamper path the deny rule
closes. The grant table, the protection rules, undo and the doctor setup and broad-grant checks
have to move with the helpers, and the facts the grant relies on have to be re-verified for the
new paths.

Three deployment defects found in v0.19.0 dogfooding belong to the same seam and are folded in:
a fresh clone has no root helpers until someone runs `git paw init` (DF-06); `sweep.sh` run from
inside an agent's worktree finds no session, because the shared preamble resolves `.git-paw/` from
`git rev-parse --show-toplevel`, which is the worktree (DF-54); and every binary-managed write is
an in-place `fs::write`, so a refresh can hand a running `sweep.sh` a half-written script (v0.19.0
release review).

## What Changes

- **Packages are installed under `.git-paw/skills/<name>/`**: `SKILL.md` (rendered), `scripts/`
  (executable), and `references/` (rendered). `git paw init` installs every bundled package at the
  repository root; every launch path that creates or re-attaches an agent worktree (bare and
  spec-driven `start`, supervisor `start`, `add`) installs the selected set into the worktree
  before the agent boots. This also closes today's gap where the bare and spec-driven `start`
  paths never inject `docs-fetch` although `mcp-agent-docs` requires it (`attach_agent`, used by
  supervisor `start` and `add`, already does).
- **Every `git paw start` installs the root packages (DF-06).** v0.19.0's supervised launch
  already refreshes the root helpers (`init::refresh_root_scripts`) and the exported orchestrator
  and `cli-setup` skills; that refresh now covers every package and compatibility copy, and the
  bare and spec-driven `start` paths do the same, so a fresh clone needs no `git paw init` for its
  root helpers. Only missing or differing files are written; config, `.gitignore` and `AGENTS.md`
  stay `init`'s.
- **The helper's invocation path is its package path**, e.g.
  `.git-paw/skills/coordination/scripts/broker.sh`. Bundled prose names helpers only through
  registry-rendered placeholders (`{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}`, `{{DOCS_FETCH_HELPER}}`),
  substituted by every renderer (`render`, `render_standardized`, `build_boot_block`, the
  orchestrator session context, the escalation framing); no bundled prose carries a literal helper
  path. The drive loop's learnings sink invokes the orchestrator package's helper.
- **`.git-paw/scripts/<helper>` becomes a deprecated compatibility copy**, still written
  byte-identical wherever its package is installed, so sessions, notes and user-written CLI allow
  rules keyed on the old path keep working through 0.x. Removal is a v1.0.0 deprecation-policy
  decision.
- **Installed files are replaced atomically**: package files and compatibility copies are written
  to a temporary file in the same directory, given their mode, and renamed into place, so a running
  `sweep.sh` never reads a half-written script after a refresh. The symlink refusal stays.
- **Helpers resolve session state from the main checkout (DF-54)**: the discovery files under
  `.git-paw/sessions/` and `sweep.sh`'s progress, dedup and approve-claim files are resolved in the
  repository's main working tree (via the git common directory), so a helper run from inside a
  linked worktree finds the session and shares the approve claim with the binary. `broker.sh`
  keeps deriving its agent id from the current worktree.
- **`git paw setup-cli` grants move to the package paths (safety):** the allow entries name
  `.git-paw/skills/<package>/scripts/<helper>`, derived from the registry; none names the
  compatibility path. The `approve` and `tell` ask rules are planned at both paths, because the
  compatibility copy still runs. A second deny rule, `Edit(**/.git-paw/skills/**)`, joins
  `Edit(**/.git-paw/scripts/**)`. Undo accepts entries at either path, so grants written by
  v0.17.0–v0.19.0 stay removable. The `claude` row stays verified only if the fact gate is re-run
  for the package-path entries and the new deny rule; otherwise it ships plan-only.
- **No silent migration.** git-paw never rewrites recorded grants. After the upgrade
  `git paw doctor` reports the setup state as out of date, and the user re-runs
  `git paw setup-cli claude --undo` then `--allowlist`. Until then the agent's calls to the package
  path prompt (and are escalated under `--unattended`).
- **The managed `AGENTS.md` block indexes the packages**: a generated `## Skills` section lists
  each provisioned package's name, description, `SKILL.md` path and helper path. Skill bodies stay
  inlined so the one file the boot prompt names remains self-sufficient.
- **Doctor checks packages**: per package `SKILL.md` present; each helper present, executable and
  matching; a missing or stale compatibility copy is ⚠, not ✗. The broad-grant advisory recognises
  a wide helper grant at either path and prints the longer package path whole.

## Capabilities

### New Capabilities

<!-- none — extends `skill-packages` from `skill-package-layout` -->

### Modified Capabilities

- `skill-packages`: installation layout; init installs every package at the root; every `start`
  installs the root packages (DF-06); every launch path installs the worktree's selection;
  installing never touches setup grants; atomic replacement of installed files; package-path
  invocation through placeholders; helpers the system invokes itself use the package path; the
  compatibility copy; how requirements naming the legacy deployed path are read (never the
  `cli-setup` requirements or doctor's grant checks); and that `skill-package-layout`'s
  byte-identity pin governs the restructure only, not the installed output defined here.
- `cli-setup`: *Allowlist entries are path-scoped to git-paw's helper scripts* (package paths;
  ask rules at both paths), *The allowlist item guards the granted helpers against silent
  modification* (adds `Edit(**/.git-paw/skills/**)`; re-verification), *Undo removes exactly what
  git-paw added* (both path shapes), *The plan is printed before anything is written* and *Writes
  add only the planned entries* (scenario literals). Each delta copies the current main-spec text
  (post-v0.19.0, including `verified-agent-send`'s `tell` ask rule and its test reference).
- `boot-block`: *Boot block content requirements* — helper form names the package path; the
  helper-invocation scenarios are scoped to the helper form (the MCP form uses tools).
- `broker-agent-helper`: *Helpers provisioned into agent worktrees* — packages, every launch
  path, compatibility copy, docs-fetch injection on every launch path; ADDED *Helpers resolve
  session state from the main checkout* (DF-54).
- `supervisor-launch`: *A supervised launch refreshes the bundled root helper scripts* (v0.19.0
  `v019-release-review-fixes`) — the refresh installs the root packages and their compatibility
  copies.
- `preflight-diagnostics`: *Bundled-scripts check* — per-package checks; *Doctor reports CLI setup
  state* — pre-package entries are reported out of date, and the modified-helper warning follows
  the paths the applied entries name; *Doctor lists broad durable grants in the project's CLI
  settings* — shape 8 at either path, package path printed whole.
- `boot-agents-md`: the managed block carries a generated skills index.

## Impact

- **Code:** `src/skills.rs` (install/render of packages, helper placeholders in `render`,
  `render_standardized`, `build_boot_block` and the orchestrator session context, index
  generation), `src/supervisor/escalation.rs` (escalation framing placeholder),
  `src/init.rs` (`run_init`, `refresh_root_scripts` and `export_skills` install packages +
  compatibility copies; `write_managed_file` / `install_script` become temp-file-and-rename;
  `provision_worktree_helpers` installs the selected packages), the three
  `provision_worktree_helpers` call sites (`src/commands/start.rs` bare and spec-driven paths,
  `src/commands/launch.rs::attach_agent`, which `add` and supervisor `start` use),
  `src/commands/start.rs` (root install before panes on the bare and spec-driven paths),
  `src/commands/supervisor.rs::write_orchestrator_context` (root refresh covers packages),
  `src/commands/add.rs` (one selection for provisioning and injection), `src/agents.rs`
  (`WorktreeAssignment` gains the package index), `src/cli_setup/facts.rs` (grant table and
  protection rules at the package paths, from the registry), `src/cli_setup/undo.rs` (shape check
  accepts both paths), `src/cli_setup/doctor.rs` (setup check follows the applied entries' paths;
  broad-grant shape 8 at both paths; `MAX_WORD` keeps the longest package path whole),
  `src/doctor.rs` (package checks; the Supervisor check's `sweep.sh` probe at the package path),
  `src/supervisor/drive.rs` (`SweepLearningSink` path). `src/worktree_provision.rs`
  (env files, ports, `on_create` hook) is untouched: helper provisioning is not in it.
- **Assets:** every literal `.git-paw/scripts/<helper>` in bundled prose becomes a placeholder;
  `assets/…/_paw_common.sh` resolves the main working tree from `git rev-parse --git-common-dir`
  and `sweep.sh` keeps its session state there (DF-54).
- **Struct-literal ripple (GP-40):** `WorktreeAssignment` gains a field — 15 literal sites
  (`src/agents.rs` ×10 incl. tests, `src/commands/start.rs` ×2, `src/commands/launch.rs` ×1,
  `src/commands/supervisor.rs` ×1, `tests/git_integration.rs` ×1).
- **Tests:** setup-cli grant-table, protection, undo and doctor tests re-pointed and extended
  (package-path entries, both deny rules, undo of a pre-package record, doctor ⚠ for a pre-package
  record, wide package-path grant listed whole), `tests/cli_init_writes_*`,
  `tests/worktree_helper_provisioning.rs` across launch paths, `tests/e2e_supervisor_launch.rs`
  (root package refresh), a bare-start fresh-clone e2e, `tests/sweep_sh_session_discovery.rs` (run
  from a linked worktree), boot-block and skill-content tests re-pointed to the rendered package
  path (34 test files name `.git-paw/scripts/` today), doctor e2e.
- **Docs:** the 88 `.git-paw/scripts/` mentions across 16 chapters — `docs/src/{architecture,
  cli-reference,quick-start-orchestrator}.md`, `docs/src/configuration/README.md`,
  `docs/src/user-guide/{agents-md,cli-setup,conflict-detection,coordination,docs-fetch,doctor,
  learnings,mcp,remote-control,security-posture,skill-templates,supervisor}.md` — re-pointed;
  grant table and protection rules at the package paths; upgrade note: re-run
  `git paw setup-cli claude --undo` then `--allowlist`; hand-written CLI allow rules keyed on
  `.git-paw/scripts/` should name the package path; `start` now installs the root packages.
- **Security / safety:** no new grant: the same twelve allow entries move to new paths, the
  compatibility path loses its allow entries, and the tamper protection widens to cover the
  package directory. git-paw writes nothing into the CLI's settings except through the consented
  command. No new writable location outside `.git-paw/`. Atomic replacement keeps the symlink
  refusal and creates the temporary file in the target's own directory (no cross-directory
  rename, no predictable path outside `.git-paw/`). Resolving session state from the main checkout
  makes the approve claim span the root, the worktrees and the binary again. Reviewed with
  `security-and-safety-review`.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:**
  - Requires v0.20.0 `skill-package-layout` (registry, `packages_for_worktree`) and is ordered
    after it; `skill-override-resolution` reads the installed `.git-paw/skills/` never.
  - Every v0.17.0–v0.19.0 change this delta builds on (`bundled-helper-discovery`,
    `start-spec-launch-parity`, `cli-setup-consent`, `classifier-retirement`,
    `orchestrator-skill-export`, `orchestrator-read-surface`, `agent-hook-and-tmux-guards`,
    `setup-cli-adopted-entries`, `verified-agent-send`, `v019-release-review-fixes`) is archived;
    each MODIFIED block was re-copied from `openspec/specs/` on 2026-11-11.
  - v0.19.0 `cli-provider-record` does not take over the setup fact table; the row stays keyed by
    CLI identity.
- **Dependencies:** none.
