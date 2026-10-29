## Why

git-paw's bundled skills are split across two trees that only prose ties together: the skill
text lives in flat `assets/agent-skills/{coordination,supervisor,docs-fetch,cli-setup}.md`
(`cli-setup` from v0.17.0, with no helper), and the helper each of the first three drives lives in `assets/scripts/{broker,sweep,docs-fetch}.sh` plus a shared
`_paw_common.sh`. Every consumer of "the bundled skills" re-lists that set by hand:
`skills::embedded_default` (4 arms after v0.17.0), `init::bundled_scripts` (4 entries) and `run_init`'s install
sequence, `init::provision_worktree_helpers` (its own gate logic), the doctor bundled-scripts
check, and v0.17.0 `git paw setup-cli`'s grant table, whose coverage guard enumerates the bundled
helpers. Adding a skill or moving a helper means
editing all of them and ~30 test files that `include_str!` an asset path. v0.20.0 makes skills
agentskills.io packages; this change is the seam that has to come first — restructure the
source tree and give the set one owner, with no change in what git-paw writes or does.

## What Changes

- **Source layout becomes one directory per skill**, the agentskills.io shape:
  `assets/agent-skills/<name>/SKILL.md` plus that skill's own `scripts/`, and `references/` /
  `assets/` when it has them. `coordination` owns `broker.sh` and the boot-block templates,
  `orchestrator` (named by v0.18.0's rename) owns `sweep.sh`, `docs-fetch` owns `docs-fetch.sh`.
  The shared `_paw_common.sh` has a single source file and is listed in every package's scripts.
  v0.17.0's `cli-setup` skill becomes a package too, with no helper, and is root-only like
  `orchestrator`: it is never provisioned into an agent worktree.
- **A compiled package registry** (`skills::bundled_packages()`) is the single source for the
  bundled set: name, embedded `SKILL.md`, embedded scripts/references/assets, and the provisioning
  gate (always / broker enabled / docs base URL configured). `embedded_default`, `run_init`,
  `bundled_scripts`, worktree provisioning, the doctor check and the `setup-cli` grant table's
  helper coverage all derive from it. Which packages an agent worktree receives becomes one pure function.
- **Behavior-preserving:** deployed paths stay `.git-paw/scripts/<helper>`; rendered skills,
  boot blocks and sidecars are byte-identical; init/doctor output is unchanged.
- **Overrides keep their scope:** a user or `.agents/skills/` override replaces a package's skill
  text only. Helper scripts always come from the binary's embedded package — an override can
  never supply a script git-paw will install or trust.
- **Export guard follows the registry:** the no-leak audit enumerates every text file in every
  bundled package, so a new package, reference or asset is audited without a test edit.

## Capabilities

### New Capabilities

- `skill-packages`: bundled skills as agentskills.io packages — source layout (including the
  root-only `cli-setup` package), the compiled registry as the single source of the bundled set,
  the override boundary (text yes, scripts no), and the mapping from pre-package asset paths that
  existing requirements still name.

### Modified Capabilities

- `core-init`: *Init installs the shared shell preamble* — the preamble's single source moves into
  the package tree, and a helper's sibling sourcing must work wherever its package is installed
  rather than "from the bundled `assets/scripts/` tree".
- `core-lang-agnostic`: the no-leak audit covers every text file of every registered package.

## Impact

- **Code:** `src/skills.rs` (registry, `embedded_default` derived from it, worktree-package
  selection), `src/init.rs` (`run_init`, `bundled_scripts`, `provision_worktree_helpers` read the
  registry), `src/doctor.rs` (bundled-scripts probe), `src/cli_setup/facts.rs` (the grant
  table's helper coverage guard enumerates the registry), `src/worktree_provision.rs` (the provisioning seam
  `bundled-helper-discovery` introduces). No public API removed; `bundled_scripts()` keeps its
  shape for doctor.
- **Assets:** `git mv` into `assets/agent-skills/<name>/…`; `assets/scripts/` disappears;
  `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` move into the
  coordination package's `assets/`.
- **Tests:** ~30 files that `include_str!` or copy from `assets/scripts/` / `assets/agent-skills/`
  switch to a test-support accessor over the registry (install-a-package-into-a-tempdir), so they
  stop pinning source paths.
- **Docs:** contributor-facing only — `docs/src/architecture.md` (asset layout), the
  export-agnosticism dev skill's inventory (`.agents/skills/export-agnosticism/SKILL.md`),
  `docs/src/user-guide/skill-templates.md` bundled-skill section. No user-visible change.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched).
- **Cross-version dependencies:**
  - v0.17.0 `bundled-helper-discovery` — moves provisioning into `provision_worktree`; this change
    re-points that seam at the registry. v0.17.0 `drop-serde-yaml` — the frontmatter parser this
    change's registry validation uses. v0.17.0 `classifier-retirement` deletes the classifier's
    `MANAGED_HELPER_SCRIPTS`, so there is no classifier helper set to derive. v0.17.0
    `cli-setup-consent` adds the `cli-setup` skill (`assets/agent-skills/cli-setup.md`) and the
    setup grant table, whose helper names come from `init::bundled_scripts()`.
  - v0.18.0 `orchestrator-skill-export` — renamed `supervisor` → `orchestrator` (alias kept); the
    package is created under the new name.
  - v0.19.0 `cli-provider-record` (capability `cli-provider`) — not required here;
    `per-cli-skill-templates` builds on both.
  - Foundation for v0.20.0 `skill-package-deploy`, `per-cli-skill-templates`,
    `orchestrator-gate-references`.
- **Dependencies:** none.
