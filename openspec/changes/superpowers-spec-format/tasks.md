## 1. Backend skeleton + dispatch

- [x] 1.1 Add `SpecBackendKind::Superpowers` in `src/specs/mod.rs` and a dispatch arm (`"superpowers" => Box::new(SuperpowersBackend)`); extend the unknown-type error to list `superpowers`
- [x] 1.2 Create `src/specs/superpowers.rs` with `SuperpowersBackend` implementing `SpecBackend`
- [x] 1.3 Test: `specs.type = "superpowers"` dispatches to `SuperpowersBackend`; existing types unaffected; unknown-type error lists superpowers

## 2. Plan-document parser

- [x] 2.1 Tests for the parser: task heading + steps, complete-step case-insensitivity, Files/Run/code-block/prose leniency
- [x] 2.2 Implement the line-oriented parser (`Goal`/`Architecture`/`Tech Stack`, `### Task N`, `- [ ]`/`- [x]` steps counted within the tasks body)
- [x] 2.3 Run parser tests — green

## 3. Scan → one SpecEntry per incomplete plan

- [x] 3.1 Test: flat `*.md` files scanned (subdirs/non-md ignored); empty dir → empty Vec
- [x] 3.2 Test: in-scope plan (≥1 `- [ ]`) → one entry; fully-`- [x]` plan → skipped; no-task file → skipped silently; other plans still scan
- [x] 3.3 Test: entry `id` = plan file stem, `owned_files = None`, branch = `plan/<slugify_branch(stem)>` with safe chars
- [x] 3.4 Implement `scan`: parse each plan, emit one `SpecEntry` per in-scope plan (no per-task fan-out)

## 4. Boot-prompt assembly

- [x] 4.1 Test: prompt carries Plan Context (Goal/Architecture/Tech Stack) + Your Tasks (descriptions, `Files:`, `Run:`) + writeback/`agent.done` instruction
- [x] 4.2 Implement boot-prompt assembly (sections joined by `\n\n---\n\n`)

## 5. Auto-detection + CLI flag

- [x] 5.1 Test: unconfigured repo with `docs/superpowers/plans/*.md` auto-selects superpowers (`specs.dir = docs/superpowers/plans`); empty/missing dir does not activate
- [x] 5.2 Test: precedence when `.specify/specs/` and `docs/superpowers/plans/` co-exist → speckit chosen
- [x] 5.3 Implement the auto-detect probe (`dir_has_md`) + deterministic precedence (speckit before superpowers) + format-override default dir
- [x] 5.4 Add `SpecsFormat::Superpowers` (clap ValueEnum auto-lists it + rejects unknowns) + `as_str`; tests `start_with_specs_format_superpowers`, `specs_format_superpowers_maps_to_backend_string`

## 6. Docs

- [x] 6.1 Added a "Superpowers Format" section to `spec-driven-launch.md` + `--specs-format`/`[specs] type` value lists in the config + CLI references; `--help` auto-lists via ValueEnum
- [x] 6.2 `mdbook build docs/` passes

## 7. Verification

- [x] 7.1 `openspec validate superpowers-spec-format --strict` passes
- [x] 7.2 `just check` — specs tests 167 green, fmt clean, clippy (below); every scenario maps to a test
- [x] 7.3 Confirmed the parser makes no CLI-specific assumptions (plain markdown; export-agnostic). NOTE: `SpecBackendKind` variant rippled into 4 exhaustive matches (`skills.rs::render_spec_path_doctrine`, `mcp/query/specs.rs` ×3) — same class as the BrokerMessage variant-ripple; worth adding to the AGENTS.md checklist
