## 1. Struct definitions

- [x] 1.1 In `src/config.rs`, add `pub struct GovernanceConfig` with five `pub <field>: Option<PathBuf>` fields (`adr`, `test_strategy`, `security`, `dod`, `constitution`). Derives: `Debug, Clone, Default, Deserialize, Serialize`. Each field has `#[serde(default, skip_serializing_if = "Option::is_none")]` matching local conventions.
- [x] 1.2 Add `pub governance: GovernanceConfig` as a top-level field on `PawConfig` with `#[serde(default)]`. The struct SHALL NOT contain a `gates` field or any nested `GovernanceGates` struct.

## 2. Constitution auto-wiring

- [x] 2.1 Implement `fn auto_wire_governance(config: &mut PawConfig, repo_root: &Path)`:
  - Short-circuit if `config.governance.constitution.is_some()`.
  - Short-circuit if `config.specs` is `None` or its `r#type` is not `"speckit"`.
  - Compute `specs_dir = repo_root.join(&config.specs.dir)`.
  - Call `git_paw::specs::speckit::detect_constitution(&specs_dir)` (provided by `spec-kit-format`).
  - If it returns `Some(path)`, assign `config.governance.constitution = Some(path)`.
- [x] 2.2 Wire the auto-wiring step into `PawConfig::load(repo_root)` — call after deserialisation, before returning.
- [x] 2.3 Verify the import `git_paw::specs::speckit::detect_constitution` is reachable from `src/config.rs`. Document the dependency ordering in `openspec/changes/_release-notes/v0.5.0-archive-order.md` (spec-kit-format first, then governance-config).

## 3. Config-load tests

- [x] 3.1 No `[governance]` section → `config.governance` present, all paths `None`.
- [x] 3.2 All paths populated → all `Some` matching TOML values.
- [x] 3.3 Partial paths → only set fields are `Some`.
- [x] 3.4 Absolute path preserved as-is.
- [x] 3.5 Non-existent path loads cleanly without error.
- [x] 3.6 Round-trip via save → load preserves all field values.
- [x] 3.7 v0.4 fixture config (no `[governance]`) loads with defaults.
- [x] 3.8 `GovernanceConfig::default()` exposes only the five path fields (no `gates` field) — compile-time assertion via field count or named-field check.

## 4. Auto-wiring tests

- [x] 4.1 Fixture: `.specify/memory/constitution.md` exists, `[specs] type = "speckit"`, `[specs] dir = ".specify/specs"`, no `governance.constitution` in TOML → `config.governance.constitution` populated with the detected path after `load`.
- [x] 4.2 Explicit `governance.constitution = "docs/principles.md"` → preserved unchanged even if `.specify/memory/constitution.md` exists.
- [x] 4.3 `[specs] type = "openspec"` (not speckit) → no auto-wiring, `governance.constitution` stays `None`.
- [x] 4.4 `[specs]` section absent → no auto-wiring, `governance.constitution` stays `None`.
- [x] 4.5 SpecKit backend active but `.specify/memory/constitution.md` absent → no auto-wiring, `governance.constitution` stays `None`, no error.
- [x] 4.6 Explicit empty-string constitution: `governance.constitution = ""` → preserved as `Some(PathBuf::from(""))`. Auto-wiring SHALL NOT override (since `is_some()` is true).

## 5. Documentation

- [x] 5.1 Update `docs/src/configuration.md` with:
  - The `[governance]` table and its 5 path fields, with examples.
  - The constitution auto-wiring behaviour and how to disable it.
  - A note that paths point at user-maintained docs; git-paw doesn't dictate structure or templates.
  - Forward-reference to `governance-context` for what the supervisor does with these paths.
- [x] 5.2 Add a "Governance" section or chapter to the user guide. Include user-guide *examples* (not vendored templates) showing what an ADR-0001, a DoD checklist, a security checklist, or a test strategy doc *might* look like. Frame these as illustrative; the project's actual conventions belong to the team's existing process.
- [x] 5.3 `mdbook build docs/` succeeds.

## 6. Release notes

- [x] 6.1 v0.5.0 release notes: announce `[governance]` table with optional paths to user-maintained governance docs. Note Spec Kit constitution auto-wiring. Forward-reference `governance-context` for runtime usage. Explicitly state git-paw provides path-pointer + injection, not templates or rubrics.

## 7. Quality gates

- [x] 7.1 `just check` — fmt, clippy, all tests green.
- [x] 7.2 `just deny` — supply chain clean.
- [x] 7.3 No new `unwrap()` / `expect()` in non-test code.
- [x] 7.4 `mdbook build docs/` succeeds.
- [x] 7.5 `openspec validate governance-config` passes.
- [x] 7.6 No supervisor verification logic added in this change. The capability boundary (D5 in design.md) is enforced — no `match config.governance.<field> { ... }` or similar consumption code lands here.
