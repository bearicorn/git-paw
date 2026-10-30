## Why

`SpecBackend` has one method, `scan`. Everything else a spec format decides is scattered and keyed
three different ways (verified 2026-10-18):

- **Matches on `SpecBackendKind`** (7): `skills::render_spec_path_doctrine`, the
  `matches!(…, OpenSpec)` opsx-region gate in `skills::render`, `main.rs::build_task_prompt`, and in
  `mcp/query/specs.rs` `backend_str`, `derive_title`, `get_spec` and `get_tasks` (the last with a `_`
  arm, so a new format silently gets checkbox parsing).
- **Raw type-string dispatch** (8): `specs::backend_for_type`, the conventional-directory branch in
  `specs::resolve_specs_config`, `resolved_spec_type(..) == Some("openspec")` for opsx role gating
  (`main.rs::cmd_dashboard`), `config::auto_wire_governance` (`!= "speckit"`), `init::SPEC_SYSTEMS`,
  `mcp/server.rs::validate_startup_config` (`VALID: [&str; 3]`), `mcp/tools/project.rs::get_skill`,
  and `mcp/query/specs.rs::resolve_dir`, which re-implements the private directory choice.
- **Three unconverted forms of the format**: the clap `SpecsFormat`, the config's `Option<String>`,
  and `SpecBackendKind`.

String dispatch escapes the compiler, which is how Superpowers shipped unsupported by the MCP server
(v0.17.0 `mcp-superpowers-support`, GP-44). And the Markdown task prompt
(`assets/boot-task-prompts/markdown.md`) still sends every Markdown worker to
`openspec/changes/<id>/`, a directory a Markdown spec never has — the defect GP-07 fixed for Spec Kit
in v0.16.0, left in place for Markdown by an explicit carve-out.

## What Changes

- `SpecBackend` becomes the Strategy for everything per-format: wire name, conventional directory,
  scan, path doctrine, task prompt, title artifact, artifact listing, task parsing, whether opsx role
  gating applies, and governance-document discovery. `SpecBackendKind::backend()` is the one
  exhaustive match.
- One format type. `SpecBackendKind` parses from and renders to the wire name; the config string is
  parsed once at the boundary, and `SpecsFormat` is reduced to a clap adapter that converts into it.
  No code outside `src/specs/` compares a spec-type string.
- Behaviour is byte-identical — the same prompts, doctrine, MCP payloads and errors — with one
  exception: the Markdown task prompt stops pointing at `openspec/changes/<id>/` and names the spec
  file instead.
- AGENTS.md §7's `SpecBackendKind` ripple list shrinks to the enum, its `backend()` arm and the new
  backend's impl.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `spec-providers`: *SpecBackend trait for format-specific scanning* — the trait carries every
  per-format behaviour; new requirements for one format type and dispatch only through the backend.
- `core-lang-agnostic`: *Spec Kit boot task-prompt does not point at the OpenSpec artifacts path* —
  generalised so no non-OpenSpec backend's task prompt names `openspec/changes/`.

## Impact

- **Code:** `src/specs/{mod,openspec,markdown,speckit,superpowers}.rs` (trait methods, `FromStr`,
  `ALL`, `default_dir`), `src/cli.rs` (`SpecsFormat` → `From` into `SpecBackendKind`),
  `src/main.rs` (`build_task_prompt`, `cmd_dashboard` role-gating check, `SpecsFormat` conversion),
  `src/skills.rs` (`render_spec_path_doctrine`, opsx region gate), `src/config/mod.rs`
  (`auto_wire_governance`), `src/init.rs` (`SPEC_SYSTEMS`), `src/mcp/server.rs`,
  `src/mcp/tools/project.rs`, `src/mcp/query/specs.rs`, `src/doctor.rs` (display only),
  `assets/boot-task-prompts/markdown.md`, `AGENTS.md` §7.
- **Enum-variant ripple (AGENTS.md §7):** the variant set of `SpecBackendKind` is unchanged; every
  site listed in §7 (`skills.rs::render_spec_path_doctrine`, `main.rs::build_task_prompt`,
  `mcp/query/specs.rs::{backend_str, derive_title, get_spec}`, `backend_for_type`, `SpecsFormat`)
  plus the string sites above move behind the trait. `BrokerMessage` untouched.
- **Config / wire:** unchanged — `[specs] type` stays a string with the same accepted values and the
  same unknown-type error; the MCP `backend` field keeps its strings.
- **Cross-version dependencies:** lands after v0.17.0 `mcp-superpowers-support` (which fixes the two
  MCP string sites and adds the §7 string-site list this change then retires) and
  `shipped-text-corrections` (edits the Spec Kit / Superpowers prompt assets). Rebases over v0.18.0
  `cmd-library-surface` (`build_task_prompt` and `cmd_dashboard` move out of `main.rs`) and
  `orchestrator-skill-export` (`get_skill` alias). No requirement modified here is modified by an
  earlier pending change.
- **Dependencies:** none.
