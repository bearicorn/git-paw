## 1. Backend skeleton + dispatch

- [ ] 1.1 Add `SpecBackendKind::Superpowers` in `src/specs/mod.rs` and a dispatch arm (`"superpowers" => Box::new(SuperpowersBackend)`); extend the unknown-type error to list `superpowers`
- [ ] 1.2 Create `src/specs/superpowers.rs` with `SuperpowersBackend` implementing `SpecBackend` (stub `scan` returning empty Vec)
- [ ] 1.3 Test: `specs.type = "superpowers"` dispatches to `SuperpowersBackend`; existing types unaffected; unknown-type error lists superpowers

## 2. Plan-document parser

- [ ] 2.1 Write failing tests for the parser: task heading + steps, complete-step case-insensitivity, Files/Run/code-block/prose leniency
- [ ] 2.2 Implement the line-oriented parser (header marker, `Goal`/`Architecture`/`Tech Stack`, `### Task N`, `- [ ]`/`- [x]` steps, `**Files:**` block), reusing the Spec Kit checkbox helpers
- [ ] 2.3 Run parser tests — green

## 3. Scan → one SpecEntry per incomplete plan

- [ ] 3.1 Test: flat `*.md` files scanned (subdirs/non-md ignored); empty dir → empty Vec
- [ ] 3.2 Test: in-scope plan (≥1 `- [ ]`) → one entry; fully-`- [x]` plan → skipped with warning; no-task file → skipped silently; other plans still scan
- [ ] 3.3 Test: entry `id` = plan file stem, `owned_files = None`, branch = `plan/<slugify_branch(stem)>` with safe chars
- [ ] 3.4 Implement `scan`: parse each plan, identify incomplete plans, emit one `SpecEntry` each

## 4. Boot-prompt assembly

- [ ] 4.1 Test: prompt carries Plan Context (Goal/Architecture/Tech Stack) + Your Tasks (descriptions, `Files:`, `Run:`) + writeback/`agent.done` instruction
- [ ] 4.2 Implement boot-prompt assembly (sections joined by `\n\n---\n\n`)

## 5. Auto-detection + CLI flag

- [ ] 5.1 Test: unconfigured repo with `docs/superpowers/plans/*.md` auto-selects superpowers (`specs.dir = docs/superpowers/plans`); explicit config/flag wins; empty/missing dir does not activate
- [ ] 5.2 Test: precedence when `.specify/specs/` and `docs/superpowers/plans/` co-exist → speckit chosen, selection reported on stderr
- [ ] 5.3 Implement the auto-detect probe + deterministic precedence + stderr report
- [ ] 5.4 Add `superpowers` to the `--specs-format` accepted values + help text; test valid/invalid values

## 6. Docs

- [ ] 6.1 Add an mdBook chapter for the superpowers backend + a row in the spec-format table; update `--specs-format` help text
- [ ] 6.2 `mdbook build docs/` passes

## 7. Verification

- [ ] 7.1 `openspec validate superpowers-spec-format --strict` passes
- [ ] 7.2 `cargo test` green; every scenario maps to a test
- [ ] 7.3 Confirm the parser makes no CLI-specific assumptions (export-agnostic)
