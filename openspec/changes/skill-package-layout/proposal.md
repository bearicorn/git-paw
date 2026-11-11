## Why

git-paw's bundled skills are split across two trees that only prose ties together: the skill
text lives in flat `assets/agent-skills/{coordination,orchestrator,docs-fetch,cli-setup}.md`
(`cli-setup` from v0.17.0, with no helper), and the helper each of the first three drives lives in `assets/scripts/{broker,sweep,docs-fetch}.sh` plus a shared
`_paw_common.sh`. Every consumer of "the bundled skills" re-lists that set by hand:
`skills::embedded_default` (4 arms), `init::bundled_scripts` (4 entries, which
`init::refresh_root_scripts` installs for `run_init` and every supervised launch),
`init::provision_worktree_helpers` (its own gate logic), `init::EXPORTED_SKILL_DIRS` /
`init::export_skills` (the root-only `orchestrator` and `cli-setup` skills v0.18.0 exports to
`.git-paw/skills/`), the doctor bundled-scripts check, and v0.17.0 `git paw setup-cli`'s grant
table, whose coverage guard enumerates the bundled helpers. Adding a skill or moving a helper means
editing all of them and ~45 test files (plus the test modules of `src/agents.rs` and
`src/cli_setup/facts.rs`) that `include_str!`, read or copy an asset path. v0.20.0 makes skills
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
  gate (root only / broker enabled / docs base URL configured). `embedded_default`,
  `bundled_scripts` (and so `refresh_root_scripts`), worktree provisioning, the root-only skill
  export, the doctor check and the `setup-cli` grant table's helper coverage all derive from it.
  Which packages an agent worktree receives becomes one pure function.
- **Behavior-preserving:** deployed paths stay `.git-paw/scripts/<helper>` and
  `.git-paw/skills/{orchestrator,cli-setup}/SKILL.md`; installed helper bytes, rendered skills,
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
  selection), `src/init.rs` (`bundled_scripts`, `provision_worktree_helpers` and
  `EXPORTED_SKILL_DIRS` / `export_skills` read the registry; `refresh_root_scripts` follows
  `bundled_scripts`), the three `provision_worktree_helpers` call sites
  (`src/commands/start.rs` bare and `launch_spec_session` paths, `src/commands/launch.rs::attach_agent`,
  which `add` and the supervisor launch use), `src/doctor.rs` (bundled-scripts probe),
  `src/cli_setup/facts.rs` (the grant table's helper coverage guard enumerates the registry) and
  `src/cli_setup/doctor.rs` (helper drift check over `bundled_scripts`). No public API removed;
  `bundled_scripts()` keeps its shape for doctor. (`src/worktree_provision.rs` holds only the
  env-file / port / hook provisioning and is untouched.)
- **Assets:** `git mv` into `assets/agent-skills/<name>/…`; `assets/scripts/` disappears;
  `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` move into the
  coordination package's `assets/`.
- **Tests:** ~45 files under `tests/` (plus the test modules of `src/agents.rs`,
  `src/cli_setup/facts.rs`, and `src/skills.rs`'s frontmatter parity test, which reads asset paths
  from `tests/fixtures/skill_frontmatter_expected.json`) that `include_str!`, read or copy from `assets/scripts/` /
  `assets/agent-skills/` switch to a test-support accessor over the registry
  (install-a-package-into-a-tempdir), so they stop pinning source paths.
- **Docs:** contributor-facing only — `docs/src/architecture.md` (Skills row: the registry and
  package layout), the export-agnosticism and concept-decoupling dev skills
  (`.agents/skills/{export-agnosticism,concept-decoupling}/SKILL.md`, which name
  `assets/scripts/` and `assets/agent-skills/*.md`), the source paths named in
  `docs/src/user-guide/{coordination,supervisor}.md` and in doc comments of
  `src/{session,config/mod,supervisor/approval_gate}.rs`, and the
  `docs/src/user-guide/skill-templates.md` bundled-skill section. No user-visible change.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched). No field is added
  to an existing widely constructed struct; `BundledPackage` / `BundledFile` / `ProvisionGate` are
  new and constructed only in the registry.
- **Cross-version dependencies:**
  - v0.17.0 `bundled-helper-discovery` — routes every launch path through
    `init::provision_worktree_helpers` (called from `commands/start.rs` and
    `commands/launch.rs::attach_agent`); this change re-points that function at the registry.
    v0.19.0 refreshes the root helpers on every supervised launch through
    `init::refresh_root_scripts` (`supervisor-launch` *A supervised launch refreshes the bundled
    root helper scripts*), which iterates `bundled_scripts()` and so follows the registry
    unchanged. v0.17.0 `drop-serde-yaml` — the frontmatter parser this
    change's registry validation uses. v0.17.0 `classifier-retirement` deletes the classifier's
    `MANAGED_HELPER_SCRIPTS`, so there is no classifier helper set to derive. v0.17.0
    `cli-setup-consent` adds the `cli-setup` skill (`assets/agent-skills/cli-setup.md`) and the
    setup grant table, whose helper names come from `init::bundled_scripts()`.
  - v0.18.0 `orchestrator-skill-export` — renamed `supervisor` → `orchestrator` (alias kept; the
    asset is already `assets/agent-skills/orchestrator.md` with frontmatter `name: orchestrator`)
    and exports the root-only `orchestrator` and `cli-setup` skills to `.git-paw/skills/` through
    `init::export_skills`; the package is created under the new name and the export list derives
    from the registry's root-only gate.
  - v0.19.0 `cli-provider-record` (capability `cli-provider`) — not required here;
    `per-cli-skill-templates` builds on both.
  - Foundation for v0.20.0 `skill-package-deploy`, `per-cli-skill-templates`,
    `orchestrator-gate-references`.
- **Dependencies:** none.
