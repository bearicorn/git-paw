## Context

`PawConfig` (in `src/config.rs`) is the single Rust struct produced by parsing `.git-paw/config.toml`. v0.4 fields include `[broker]`, `[supervisor]`, `[specs]`, `[clis]`, and a few top-level options. Each section is its own struct with `Option<...>` fields and `#[serde(default)]` annotations so partial configs load cleanly.

This change adds one new struct (`GovernanceConfig`) following the same pattern. No new dependency, no new TOML parser; just `serde` derives.

The interesting wrinkle is the post-load auto-wiring of `governance.constitution` from `.specify/memory/constitution.md` when (a) the user hasn't set the field and (b) the SpecKit backend is active. This requires reaching into `git_paw::specs::speckit::detect_constitution()` (defined by the parallel `spec-kit-format` change) at the right moment in config-load — after raw deserialisation, before the config is handed to the rest of the system.

The runtime consumer (boot-prompt injection + supervisor skill update) lives in the next change `governance-context`. This change provides the slot only.

## Goals / Non-Goals

**Goals:**
- Add `[governance]` to the TOML schema with optional path fields, all defaulting cleanly when absent.
- Auto-wire `governance.constitution` from `spec_kit::detect_constitution()` when (a) the user hasn't set it and (b) the SpecKit backend is active. Explicit user values always win.
- Keep the change purely declarative: parse, expose in `PawConfig`, do nothing else. No reading of the documents themselves, no enforcement.
- Backward-compatible: v0.4 configs without `[governance]` continue to load unchanged with `governance = GovernanceConfig::default()`.

**Non-Goals:**
- Reading any governance document (the runtime capability owns this).
- Running any check or emitting `agent.feedback` on governance findings.
- A `[governance.gates]` table or any per-doc gating semantics.
- Validating that configured paths exist at load time. The runtime logic is responsible for handling missing files.
- Init-time scaffolding (`init-governance-scaffolding` was dropped during planning; user-guide examples replace it).
- Per-doc check rubrics in code or schema.

## Decisions

### D1. Single TOML table, paths-only

```toml
[governance]
adr = "docs/adr"
test_strategy = "docs/test-strategy.md"
security = "docs/security-checklist.md"
dod = "docs/definition-of-done.md"
constitution = ".specify/memory/constitution.md"
```

Earlier MILESTONE drafts had a parallel `[governance.gates]` table with per-doc boolean flags for opt-in enforcement. That was dropped because gating-per-doc requires git-paw to define what constitutes a "failure" for each doc — which is a process choice owned by the user's team, not by git-paw. The simpler stance: provide the paths, trust the supervisor LLM to read each doc and apply judgment as part of its existing audit flow.

### D2. Constitution auto-wiring at config-load post-processing

`PawConfig::load(repo_root)` (or whatever the existing entry point is named) currently deserialises the TOML and returns `PawConfig`. Inserting auto-wiring is a post-deserialise step:

```rust
fn auto_wire_governance(config: &mut PawConfig, repo_root: &Path) {
    if config.governance.constitution.is_none() {
        if let Some(specs_cfg) = &config.specs {
            if specs_cfg.r#type == "speckit" {
                let specs_dir = repo_root.join(&specs_cfg.dir);
                if let Some(detected) = git_paw::specs::speckit::detect_constitution(&specs_dir) {
                    config.governance.constitution = Some(detected);
                }
            }
        }
    }
}
```

Called once at the end of `load`, after deserialise.

If `spec-kit-format` ships before this change, `detect_constitution` exists and is callable. If this change ships first (unlikely given the dependency chain), the import resolves to a stub returning `None`. The pragmatic ordering: `spec-kit-format` lands first, then this change consumes its export. The release-prep `_release-notes/v0.5.0-archive-order.md` plan SHOULD encode this dependency.

### D3. Path resolution at use time

Configured paths are stored as `PathBuf` in `GovernanceConfig`. Relative paths are resolved against the repository root at *use time* (in `governance-context`), not at config-load time. Storing them as raw `PathBuf` (not pre-resolved) keeps the config struct clean and avoids needing the repo root in every config-load callsite.

Absolute paths are stored as-is. Symlinks are not resolved at config time (the runtime logic may resolve them when reading).

### D4. No filesystem checks at load time

A path that doesn't exist still loads cleanly. The runtime logic is the right place to handle missing files — that's a "user pointed at a doc that doesn't exist" scenario, surfaced via `agent.feedback` if the supervisor finds it relevant during audit.

This keeps `PawConfig::load` infallible for all sane TOML inputs.

### D5. Capability boundary: `governance-config` does not consume itself

This change ships the slot only. No code in this change *reads* `config.governance.adr` to do anything with the value. That's `governance-context`'s job. The test surface for this change is therefore narrow: deserialise correctly, expose correctly, auto-wire correctly. Behaviour-tier tests (supervisor sees DoD findings) live in the runtime capability.

This boundary is enforced by NOT touching the supervisor or any verification logic in this change.

## Risks / Trade-offs

- **[Risk] Constitution auto-wiring overrides a user's intent silently.** If a user explicitly sets `governance.constitution = ""` (empty string), the auto-wiring overwrites it. → **Mitigation:** the check is `is_none()`, not `is_none() || empty`. An empty string deserialises to `Some("")` which preserves the user's value. Documentation calls this out: "to disable auto-wiring without deleting the path, set the field to a literal empty string or to a path that doesn't exist."
- **[Trade-off] Paths-only with no gates feels too permissive.** Users coming from heavyweight governance frameworks may want git-paw to enforce checks. v0.5.0's stance is "trust the supervisor LLM"; gating semantics could land in v0.6.0+ if dogfood demand is real.
- **[Trade-off] No filesystem checks at load time.** A path that doesn't exist loads cleanly. Verification handles the failure if the user points the supervisor at a non-existent doc. → **Mitigation:** users adding paths manually should test by running `git paw start --dry-run`.

## Migration Plan

Additive. No migration step.

1. Land `spec-kit-format` first (provides `detect_constitution`).
2. Land this change. Existing v0.4 configs continue to load with no `[governance]` section; `config.governance == GovernanceConfig::default()`.
3. Users opt in by adding `[governance]` to their config, pointing at docs they already have.
4. Rollback: revert. The `governance` field disappears from `PawConfig`. Any user who added the section sees their config still parses (extra fields are ignored) — or they re-edit to drop the section.

Release-notes call-outs:
- New optional `[governance]` table with path fields.
- Spec Kit constitution auto-wires when path is unset and SpecKit backend is active.
- Runtime behaviour ships in `governance-context`.

## Open Questions

- **Should `governance.adr` accept a single file or a directory only?** Decision: directory only (matches the typical convention of one ADR per file in a directory). The runtime logic enumerates files under the directory; a single-file ADR is unusual.
- **Does `auto_wire_governance` need the repo root, or can it use `std::env::current_dir()`?** Decision: explicit `repo_root: &Path` parameter, matching the existing `PawConfig::load(repo_root)` signature. Avoids hidden environmental dependence.
