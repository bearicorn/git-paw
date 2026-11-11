## Context

The bundled skill set today (symbols, not line numbers):

- `src/skills.rs` — `COORDINATION_DEFAULT` / `ORCHESTRATOR_DEFAULT` / `DOCS_FETCH_DEFAULT` /
  `CLI_SETUP_DEFAULT` (`include_str!("../assets/agent-skills/*.md")`),
  `embedded_default` (a four-arm `match`),
  `build_boot_block` → `boot_block_for_id` (`include_str!` of `assets/boot-block-template.md` /
  `assets/boot-block-mcp-template.md`). The loader for directory-form skills
  (`load_skill_from_directory`, `parse_standardized_metadata`) already exists and already collects
  `scripts/`, `references/`, `assets/` as `resource_paths` — but only for `.agents/skills/` and the
  config-dir override, never for the embedded defaults.
- `src/init.rs` — `COMMON_SCRIPT` / `SWEEP_SCRIPT` / `BROKER_SCRIPT` / `DOCS_FETCH_SCRIPT`,
  `bundled_scripts()` (consumed by doctor, `cli_setup`, and `refresh_root_scripts`, which
  `run_init` and — since v0.19.0 — every supervised launch call to install the root helpers),
  `provision_worktree_helpers(root, broker_enabled, docs_base_url_configured)` (its own gate
  logic), and `EXPORTED_SKILL_DIRS` / `export_skills` (v0.18.0: writes the root-only
  `orchestrator` and `cli-setup` skills to `.git-paw/skills/`, from `run_init` and the supervised
  launch). v0.17.0 `bundled-helper-discovery` calls `provision_worktree_helpers` on every launch
  path: `commands/start.rs` (bare and `launch_spec_session`) and `commands/launch.rs::attach_agent`
  (used by `add` and the supervisor launch). `worktree_provision::provision_worktree` handles only
  env files, ports and hooks.
- `src/doctor.rs` — `check_bundled_scripts` over `init::bundled_scripts()`;
  `src/cli_setup/doctor.rs` — the helper drift check (`helper_paths` / `helper_state`) over the same.
- `src/cli_setup/facts.rs` (v0.17.0 `cli-setup-consent`) — the `setup-cli` grant table, one row
  per helper with a guard that every bundled script has a row, a fourth listing of the set. (The
  classifier's `MANAGED_HELPER_SCRIPTS`, an earlier copy, is deleted by v0.17.0
  `classifier-retirement`.) `assets/agent-skills/cli-setup.md` (also v0.17.0) is a fourth flat
  skill file, with no helper.
- Skill-to-worktree composition is repeated in `commands/start.rs` (bare and spec-driven paths),
  `commands/add.rs`, `commands/supervisor.rs`: each resolves `coordination` (and sometimes
  `docs-fetch`) behind its own `if broker_config.enabled` / `docs_base_url` checks — neither
  `start` path resolves `docs-fetch` (though both provision `docs-fetch.sh`), `add` and the
  supervisor launch do and pass it to `attach_agent`.
- Tests: ~45 files under `tests/` plus the test modules of `src/agents.rs` and
  `src/cli_setup/facts.rs` `include_str!`, read or copy from `assets/scripts/` or
  `assets/agent-skills/`; the shell-helper tests copy `sweep.sh` + `_paw_common.sh` side by side
  into a tempdir.

Constraints: single self-contained binary (`include_str!`, no runtime asset directory); no new
crate (an `include_dir`-style crate is not on the approved list); `skills::resolve` and its
override order stay as they are; export-agnosticism applies to every packaged file.

## Goals / Non-Goals

**Goals:**
- One directory per bundled skill in the source tree, in the agentskills.io shape.
- One compiled registry that every consumer of the bundled set reads.
- Zero observable change: same files written to the same paths, same rendered text.
- Tests stop pinning source asset paths.

**Non-Goals:**
- Changing where helpers are deployed or how agents invoke them (`skill-package-deploy`).
- Per-CLI variants (`per-cli-skill-templates`).
- Splitting any skill's content into `references/` (`orchestrator-gate-references`).
- Changing `skills::resolve`'s order or wiring the config-dir override into `resolve()` (see Open
  Questions — the override is unreachable in production today).

## Decisions

### D1 — `assets/agent-skills/<name>/` is the package root

Keep the existing `agent-skills` directory name and turn each file into a directory:
`coordination/{SKILL.md, scripts/broker.sh, assets/boot-block.md, assets/boot-block-mcp.md}`,
`orchestrator/{SKILL.md, scripts/sweep.sh}`, `docs-fetch/{SKILL.md, scripts/docs-fetch.sh}`.
The boot-block templates join the coordination package because they are its boot entry point —
MILESTONE asks that "the boot block and the skill export stay in one place".
*Alternative:* a new `assets/skills/` root — rejected as churn with no gain.

### D2 — A static registry, not a directory walk

`skills::bundled_packages() -> &'static [BundledPackage]`, each entry an explicit list of
`(relative path, include_str!(…))` pairs plus a `ProvisionGate` (`RootOnly` for `orchestrator`
and `cli-setup`, `BrokerEnabled` for `coordination`, `DocsBaseUrlConfigured` for `docs-fetch`).
The gate governs worktree selection only: `git paw init` installs every package's helpers at the
root as it does today, and `export_skills` writes the `RootOnly` packages' skills to
`.git-paw/skills/` in registry order (today's `EXPORTED_SKILL_DIRS` order). A conformance test walks `assets/agent-skills/` at test time and fails
if a file on disk is not registered (or vice versa), so the static list cannot silently drift.
*Alternatives:* a `build.rs` that generates the list (build-script complexity for three
packages); an embed-a-directory crate (new dependency, not approved).

### D3 — `_paw_common.sh` has one source, every package lists it

The preamble lives once at `assets/agent-skills/_shared/scripts/_paw_common.sh` (`_shared` has no
`SKILL.md` and is never a package). The registry lists that one embedded constant in each
package's `scripts/`, so every installed package is self-contained and the helpers' existing
sibling sourcing (`$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)/_paw_common.sh`) keeps
working unchanged.
*Alternatives:* a copy per package in the repo (three files to keep identical); sourcing
`../../_shared/` (breaks the package once it is installed on its own).
Consequence: helpers no longer run from the source tree as-is (their sibling preamble is not
there). Nothing ships that relies on it; tests install a package into a tempdir first (D7). The
`core-init` preamble requirement is reworded accordingly.

### D4 — Which packages a worktree gets is a pure decision

`skills::packages_for_worktree(inputs) -> Vec<&'static BundledPackage>` where `inputs` carries
`broker_enabled` and `docs_base_url_configured` (and nothing I/O). `provision_worktree_helpers`
becomes the shell that writes what it returns; the per-command composition in `start`/`add`/
`supervisor` (and `attach_agent`, which the latter two feed) resolves skill text for exactly that
set. This is the functional-core split
`docs/src/architecture-direction.md` asks for (see Risks for the one divergence it exposes).

### D5 — The override boundary: text, never scripts

An override (`.agents/skills/<name>/` or the config-dir form) replaces the package's skill text
for rendering. Scripts, references and assets an override directory carries are ignored for
installation: helpers are always written from the binary's embedded package. This is the
existing behavior (scripts only ever came from `init.rs` constants), now stated as a contract
because v0.17.0 `git paw setup-cli` can pre-approve helper paths in the agent CLI's settings, so a
script an override supplied would run through that grant without a prompt.

### D6 — Old asset paths in existing requirements map, rather than ~20 MODIFIED blocks

About twenty requirements name a bundled file by its pre-package path (`assets/agent-skills/
coordination.md`, `assets/scripts/sweep.sh`, …) only to identify *which* asset they constrain.
Rewriting each as MODIFIED would copy large requirement bodies to change one path. Instead
`skill-packages` adds one requirement with the mapping table; the same device v0.18.0
`orchestrator-skill-export` uses for the supervisor → orchestrator rename (`skill-agent`
*Orchestrator skill is the supervisor skill relocated*). The affected requirements (listed in
tasks 6.2) are queued for the v1.0.0 pre-freeze wording sweep.

### D7 — Tests read the registry, not paths

A `tests/helpers` function installs a named package (scripts + `_paw_common.sh`) into a tempdir
from `bundled_packages()`; content tests read `bundled_package(name).skill_md`. Assertions stay
behavioral (rendered text, helper output) and stop failing on a file move.

## Risks / Trade-offs

- [Silent drift between the registry and files on disk] → D2's conformance test.
- [A test that executes a helper straight from `assets/scripts/` breaks] → every such test is
  re-pointed by task 4.x to the installer helper; `tests/paw_common_conventions.rs` keeps its
  "sources relative to own location" assertion.
- [D4 centralises gate logic that differs today — the bare and spec-driven `start` paths never
  inject `docs-fetch`, while `supervisor` and `add` do (contradicting `mcp-agent-docs`' *Gated
  injection into agent sessions*)] → this change keeps the current per-path behavior: the selector
  is called only where each path consults it today. Aligning every launch path on one package set
  is a behavior change and belongs to `skill-package-deploy`.
- [Contributors and dogfood notes that say "copy `assets/scripts/broker.sh`"] → architecture doc
  and the export-agnosticism inventory updated; MEMORY-style notes are the maintainer's.
- [The four shell helpers carry comments naming the source tree — `_paw_common.sh` says "edit
  `assets/scripts/_paw_common.sh`", and `broker.sh` / `sweep.sh` / `docs-fetch.sh` say sourcing
  "works from both the bundled `assets/scripts/` tree" and the deployed directory — and those
  comments are installed verbatim] → rewording them changes the installed bytes that *Packaging
  preserves deployed output* pins, so this change leaves them as they are; `skill-package-deploy`,
  which changes the deployed helpers anyway, rewords them. (Needs a maintainer call if the
  comments should be fixed here instead, with the byte-identity requirement scoped to
  non-comment lines.)
- [`git mv` loses `git log` continuity for some tools] → use `git mv` so rename detection works.

## Migration Plan

1. Add the registry over the *current* paths; derive `embedded_default`, `bundled_scripts` (and
   so `refresh_root_scripts`), worktree provisioning, the root-only skill export, doctor and the
   `setup-cli` grant table's coverage guard from it. Suite green, no output change.
2. `git mv` assets into packages; update the registry's `include_str!` paths only.
3. Re-point tests to the registry helpers; add the conformance and audit-coverage tests.
4. Docs/inventory.
Rollback: revert; nothing on disk in consumer repos changes.

## Open Questions

- **The config-dir user override is unreachable in production.** `skills::resolve` calls
  `resolve_with_config_dir(name, None)`, so `<config_dir>/git-paw/agent-skills/<name>/SKILL.md` is
  only consulted by a unit test. `skill-agent`'s *Skill resolution order* (as v0.18.0 restated it)
  states this production order — `.agents/skills/` from the cwd up four ancestors, then embedded —
  and `docs/src/user-guide/skill-templates.md` matches it, while `docs/src/architecture.md`,
  `mcp-server` *Project knowledge tools* and the `src/skills.rs` module doc still name a user
  level. Not fixed here (this change preserves resolution exactly); v0.20.0
  `skill-override-resolution` decides the contract.
- ~~Whether agentskills.io requires a skill's frontmatter `name` to equal its directory name.~~
  Settled by v0.18.0: the orchestrator skill ships `name: orchestrator`, equal to its directory
  name as the standardized format requires (`core-init` *Init installs the orchestrator skill in
  the standardized format*), and every bundled skill's `name` already equals its package name.
