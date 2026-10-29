## Context

The bundled skill set today (symbols, not line numbers):

- `src/skills.rs` — `COORDINATION_DEFAULT` / `SUPERVISOR_DEFAULT` / `DOCS_FETCH_DEFAULT`, plus
  v0.17.0's `cli-setup` default (`include_str!("../assets/agent-skills/*.md")`),
  `embedded_default` (a four-arm `match` once `cli-setup` is in),
  `build_boot_block` (`include_str!` of `assets/boot-block-template.md` /
  `assets/boot-block-mcp-template.md`). The loader for directory-form skills
  (`load_skill_from_directory`, `parse_standardized_metadata`) already exists and already collects
  `scripts/`, `references/`, `assets/` as `resource_paths` — but only for `.agents/skills/` and the
  config-dir override, never for the embedded defaults.
- `src/init.rs` — `COMMON_SCRIPT` / `SWEEP_SCRIPT` / `BROKER_SCRIPT` / `DOCS_FETCH_SCRIPT`,
  `bundled_scripts()` (consumed by doctor), `run_init` (four hand-written install blocks) and
  `provision_worktree_helpers(root, broker_enabled, docs_base_url_configured)` (its own gate
  logic). v0.17.0 `bundled-helper-discovery` moves the call into `worktree_provision::provision_worktree`.
- `src/doctor.rs` — `check_bundled_scripts` over `init::bundled_scripts()`.
- `src/cli_setup/facts.rs` (v0.17.0 `cli-setup-consent`) — the `setup-cli` grant table, one row
  per helper with a guard that every bundled script has a row, a fourth listing of the set. (The
  classifier's `MANAGED_HELPER_SCRIPTS`, an earlier copy, is deleted by v0.17.0
  `classifier-retirement`.) `assets/agent-skills/cli-setup.md` (also v0.17.0) is a fourth flat
  skill file, with no helper.
- Skill-to-worktree composition is repeated in `commands/start.rs` (bare and spec-driven paths),
  `commands/add.rs`, `commands/supervisor.rs`: each resolves `coordination` (and sometimes
  `docs-fetch`) behind its own `if broker_config.enabled` / `docs_base_url` checks — the bare
  `start` path never resolves `docs-fetch`, the others do.
- Tests: ~30 files `include_str!` or copy from `assets/scripts/` or `assets/agent-skills/`; the
  shell-helper tests copy `sweep.sh` + `_paw_common.sh` side by side into a tempdir.

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
`(relative path, include_str!(…))` pairs plus a `ProvisionGate` (`Always`, `BrokerEnabled`,
`DocsBaseUrlConfigured`). A conformance test walks `assets/agent-skills/` at test time and fails
if a file on disk is not registered (or vice versa), so the static list cannot silently drift.
*Alternatives:* a `build.rs` that generates the list (build-script complexity for three
packages); an embed-a-directory crate (new dependency, not approved).

### D3 — `_paw_common.sh` has one source, every package lists it

The preamble lives once at `assets/agent-skills/_shared/scripts/_paw_common.sh` (`_shared` has no
`SKILL.md` and is never a package). The registry lists that one embedded constant in each
package's `scripts/`, so every installed package is self-contained and the helpers' existing
sibling sourcing (`$(dirname "${BASH_SOURCE[0]}")/_paw_common.sh`) keeps working unchanged.
*Alternatives:* a copy per package in the repo (three files to keep identical); sourcing
`../../_shared/` (breaks the package once it is installed on its own).
Consequence: helpers no longer run from the source tree as-is (their sibling preamble is not
there). Nothing ships that relies on it; tests install a package into a tempdir first (D7). The
`core-init` preamble requirement is reworded accordingly.

### D4 — Which packages a worktree gets is a pure decision

`skills::packages_for_worktree(inputs) -> Vec<&'static BundledPackage>` where `inputs` carries
`broker_enabled` and `docs_base_url_configured` (and nothing I/O). `provision_worktree_helpers`
becomes the shell that writes what it returns; the per-command composition in `start`/`add`/
`supervisor` resolves skill text for exactly that set. This is the functional-core split
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
`orchestrator-skill-export` uses for the supervisor → orchestrator rename. The affected
requirements (listed in tasks 6.2) are queued for the v1.0.0 pre-freeze wording sweep.

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
- [`git mv` loses `git log` continuity for some tools] → use `git mv` so rename detection works.

## Migration Plan

1. Add the registry over the *current* paths; derive `embedded_default`, `bundled_scripts`,
   `run_init`, provisioning, doctor and the `setup-cli` grant table's coverage guard from it. Suite green, no output
   change.
2. `git mv` assets into packages; update the registry's `include_str!` paths only.
3. Re-point tests to the registry helpers; add the conformance and audit-coverage tests.
4. Docs/inventory.
Rollback: revert; nothing on disk in consumer repos changes.

## Open Questions

- **The config-dir user override is unreachable in production.** `skills::resolve` calls
  `resolve_with_config_dir(name, None)`, so `<config_dir>/git-paw/agent-skills/<name>/SKILL.md` is
  only consulted by a unit test, while `skill-agent`'s *Skill resolution order* names a flat
  `<config_dir>/git-paw/agent-skills/<name>.md` as step 1 and `docs/src/user-guide/
  skill-templates.md` documents `.agents/skills/` → embedded only. Reported, not fixed here (this
  change preserves resolution exactly); needs a decision on which of the three is the contract.
- Whether agentskills.io requires a skill's frontmatter `name` to equal its directory name (to
  verify against the published spec). It matters for v0.18.0's `name: git-paw-orchestrator`
  under an `orchestrator/` directory.
