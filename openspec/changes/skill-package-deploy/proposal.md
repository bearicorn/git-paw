## Why

After `skill-package-layout`, bundled skills are packages in the source tree, but what git-paw
writes into a repository is still the old split: rendered skill text inlined into the sidecar,
helpers flattened into `.git-paw/scripts/`, and ~45 hand-typed `.git-paw/scripts/<helper>` literals
across the boot block and the skills. MILESTONE v0.20.0 asks that each package own its helper and
that the injected `AGENTS.md` block reference the packages, so the boot block and the skill export
stay in one place.

Moving the helper path is a security change, not just a rename. Since v0.17.0 git-paw approves no
permission prompt itself (`classifier-retirement`); the only pre-approval of a helper that git-paw
writes is the consented `git paw setup-cli claude --allowlist` (`cli-setup-consent`). It writes
exact-path allow rules for `.git-paw/scripts/<helper>` into the main checkout's
`.claude/settings.local.json`, together with a deny rule on edits under `**/.git-paw/scripts/**` so an agent in the CLI's
edit-accepting mode cannot rewrite a granted helper and then run it through the grant. Once bundled
prose invokes `.git-paw/skills/<package>/scripts/<helper>`, those grants no longer match, and a grant
for the new path written without new protection would reopen exactly the tamper path the deny rule
closes. The grant table, the protection rules, undo and the doctor setup check have to move with
the helpers, and the facts the grant relies on have to be re-verified for the new paths.

## What Changes

- **Packages are installed under `.git-paw/skills/<name>/`**: `SKILL.md` (rendered), `scripts/`
  (executable), and `references/` (rendered). `git paw init` installs every bundled package at the
  repository root; every launch path that creates or re-attaches an agent worktree (bare and
  spec-driven `start`, supervisor `start`, `add`) installs the selected set into the worktree
  before the agent boots. This also closes today's gap where the `start` paths never inject
  `docs-fetch` although `mcp-agent-docs` requires it.
- **The helper's invocation path is its package path**, e.g.
  `.git-paw/skills/coordination/scripts/broker.sh`. Bundled prose names helpers only through
  registry-rendered placeholders (`{{BROKER_HELPER}}`, `{{SWEEP_HELPER}}`, `{{DOCS_FETCH_HELPER}}`);
  no bundled template carries a literal helper path. The drive loop's learnings sink invokes the
  orchestrator package's helper.
- **`.git-paw/scripts/<helper>` becomes a deprecated compatibility copy**, still written
  byte-identical wherever its package is installed, so sessions, notes and user-written CLI allow
  rules keyed on the old path keep working through 0.x. Removal is a v1.0.0 deprecation-policy
  decision.
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
  matching; a missing or stale compatibility copy is ⚠, not ✗.

## Capabilities

### New Capabilities

<!-- none — extends `skill-packages` from `skill-package-layout` -->

### Modified Capabilities

- `skill-packages`: installation layout, package-path invocation through placeholders, the
  compatibility copy, and how requirements naming the legacy deployed path are read (never the
  `cli-setup` requirements); installing packages never touches setup grants.
- `cli-setup`: *Allowlist entries are path-scoped to git-paw's helper scripts* (package paths;
  ask rules at both paths), *The allowlist item guards the granted helpers against silent
  modification* (adds `Edit(**/.git-paw/skills/**)`; re-verification), *Undo removes exactly what
  git-paw added* (both path shapes), *The plan is printed before anything is written* and *Writes
  add only the planned entries* (scenario literals). Each delta starts from the latest earlier
  text: v0.17.0 `cli-setup-consent`'s (including *Undo removes exactly what git-paw added*); for the
  grant table v0.19.0 `verified-agent-send`'s (which carries v0.18.0 `orchestrator-read-surface`'s
  `events` grant and `agent-hook-and-tmux-guards`' full never-granted `sweep.sh` list, and adds
  `tell`).
- `boot-block`: *Boot block content requirements* — helper form names the package path; the
  helper-invocation scenarios are scoped to the helper form (the MCP form uses tools).
- `broker-agent-helper`: *Helpers provisioned into agent worktrees* — packages, every launch
  path, compatibility copy.
- `preflight-diagnostics`: *Bundled-scripts check* — per-package checks; *Doctor reports CLI setup
  state* (from v0.18.0 `setup-cli-adopted-entries`'s text) — pre-package entries are reported out of date,
  and the modified-helper warning follows the paths the applied entries name.
- `boot-agents-md`: the managed block carries a generated skills index.

## Impact

- **Code:** `src/skills.rs` (install/render of packages, helper placeholders, index generation),
  `src/init.rs` (`run_init` installs packages + compatibility copies), `src/worktree_provision.rs`
  (provisioning seam), `src/main.rs::attach_agent`, `src/commands/{start,add,supervisor}.rs`
  (one selection for skills, provisioning and injection), `src/agents.rs` (`WorktreeAssignment`
  gains the package index), `src/cli_setup/facts.rs` (grant table and protection rules at the
  package paths, from the registry), `src/cli_setup/undo.rs` (shape check accepts both paths),
  `src/doctor.rs` (package checks; setup check follows the applied entries' paths),
  `src/supervisor/drive.rs` (`SweepLearningSink` path).
- **Assets:** every literal `.git-paw/scripts/<helper>` in bundled prose becomes a placeholder.
- **Tests:** setup-cli grant-table, protection, undo and doctor tests re-pointed and extended
  (package-path entries, both deny rules, undo of a pre-package record, doctor ⚠ for a pre-package
  record), `tests/cli_init_writes_*`, provisioning e2e across launch paths, boot-block and
  skill-content tests re-pointed to the rendered package path, doctor e2e.
- **Docs:** `docs/src/user-guide/{skill-templates,agents-md,coordination,supervisor,docs-fetch,
  doctor,mcp,learnings}.md`, `docs/src/configuration/README.md`, `docs/src/architecture.md`,
  `docs/src/cli-reference.md` (`init` output), `docs/src/user-guide/cli-setup.md` and the
  security-posture chapter (grant table and protection rules at the package paths), upgrade note:
  re-run `git paw setup-cli claude --undo` then `--allowlist`; hand-written CLI allow rules keyed on
  `.git-paw/scripts/` should name the package path.
- **Security / safety:** no new grant: the same twelve subcommand grants move to new paths, the
  compatibility path loses its allow entries, and the tamper protection widens to cover the
  package directory. git-paw writes nothing into the CLI's settings except through the consented
  command. No new writable location outside `.git-paw/`; reviewed with
  `security-and-safety-review`.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:**
  - Requires v0.20.0 `skill-package-layout` (registry, `packages_for_worktree`).
  - v0.17.0 `bundled-helper-discovery` MODIFIES *Helpers provisioned into agent worktrees*; this
    change MODIFIES that requirement again on top of its text. v0.17.0 `start-spec-launch-parity`
    (spec-driven launch provisioning) and `shipped-text-corrections` (boot-block text) land first.
  - v0.18.0 `orchestrator-skill-export` already installs `.git-paw/skills/orchestrator/SKILL.md`,
    gitignores `.git-paw/skills/`, and pins `.git-paw/scripts/sweep.sh verified` in an ADDED
    requirement; this change generalises the install to every package, and its legacy-path
    mapping requirement re-points that pin. v0.18.0 `broker-verdict-authority` (`sweep.sh verified`
    via `__authority`) is path-independent.
  - v0.17.0 `classifier-retirement` removes the managed-helper classifier rule this change used to
    tighten, so nothing here classifies. v0.17.0 `cli-setup-consent` owns the `cli-setup`
    capability; v0.18.0 `orchestrator-read-surface` (grants `sweep.sh events`),
    `agent-hook-and-tmux-guards` (the full never-granted `sweep.sh` list; no setup-cli rule) and
    v0.19.0 `verified-agent-send` (`sweep.sh tell` on an `ask` rule) modify the grant-table
    requirement before this change, and v0.18.0 `setup-cli-adopted-entries` modifies *Doctor
    reports CLI setup state* (a planned entry already in the target file counts as applied); all
    must archive first. Re-diff each `cli-setup` and `preflight-diagnostics` delta against the
    archived text before archiving this change.
  - v0.19.0 `cli-provider-record` does not take over the setup fact table; the row stays keyed by
    CLI identity.
- **Dependencies:** none.
