## Why

v0.5.0's supervisor benefits from access to the project's existing governance documents (ADRs, test strategy, security checklist, DoD, Spec Kit constitution). Today the supervisor knows nothing about them; an agent can ship code that contradicts an ADR, fails a security checklist item, or skips a DoD step, and the supervisor has no way to flag it.

This change introduces the *path-pointer mechanism* for those documents: a `[governance]` TOML table that lets a project name where its ADR directory, test strategy doc, security checklist, DoD, and constitution live. All paths are optional. Projects opt in per document.

Critically, this change ships *only* the path-pointer slot and an auto-detection handshake with `spec-kit-format` for the constitution path. It does NOT define what each governance document should look like, what counts as a "failure" against any of them, or how the supervisor should enforce them. ADR conventions, DoD format, security checklists, and constitutions are owned by teams' existing processes (Scrum, XP, Nygard, `adr-tools`, OWASP, Spec Kit) — git-paw should not dictate that. The supervisor's runtime use of the configured docs lives in the parallel `governance-context` change.

## What Changes

- **New `[governance]` table** in `.git-paw/config.toml` with 5 optional path fields:
  - `adr: Option<PathBuf>` — directory containing ADR files (whatever convention the project uses).
  - `test_strategy: Option<PathBuf>` — single Markdown file describing the project's test strategy.
  - `security: Option<PathBuf>` — single Markdown file containing a security checklist.
  - `dod: Option<PathBuf>` — single Markdown file containing the Definition of Done.
  - `constitution: Option<PathBuf>` — single Markdown file (Spec Kit's `constitution.md` or any project's equivalent).
- **Spec Kit constitution auto-wiring.** When `governance.constitution` is unset AND the SpecKit backend is active for the session AND `spec_kit::detect_constitution()` returns `Some(path)`, the system SHALL populate `governance.constitution` with the detected path at session start. This is the consumer side of the `spec-kit-format` change's D7 handshake. If `governance.constitution` is set explicitly (even to a path that doesn't exist), auto-wiring SHALL NOT override it.
- **Path resolution.** Configured paths SHALL be stored as raw `PathBuf` and resolved against the repository root at use time, not at config-load time. Absolute paths are accepted as-is. Paths SHALL NOT be required to exist at config-load time.
- **Validation.** `[governance]` is optional. The system SHALL accept any combination — including an entirely absent `[governance]` table (the v0.4 baseline). No filesystem checks at load time.
- **No gates table.** Earlier MILESTONE drafts proposed a `[governance.gates]` boolean-per-doc table for opt-in enforcement. This is dropped: gating per doc would require git-paw to define what constitutes a "failure" for each governance doc type, which is a process choice owned by the user's team, not git-paw. The supervisor agent (LLM) reads the configured docs and applies judgment — no separate enforcement table needed.
- **No supervisor logic.** This change does NOT read any governance document, run any check, or emit any `agent.feedback`. The runtime consumer (`governance-context`) provides boot-prompt injection and a skill update; that's separate from this storage slot.

Not in scope:
- Any reading or interpretation of governance documents.
- Per-doc check rubrics or gating semantics.
- A `[governance.gates]` table.
- Init-time scaffolding of governance docs (out for v0.5.0; templates conflict with the "don't dictate structure" stance).
- Path discovery beyond Spec Kit's `constitution.md` auto-wiring.
- Per-language or per-format hooks. v0.5.0 governance is fully language-agnostic.

## Capabilities

### New Capabilities
- `governance-config`: the `[governance]` TOML table, its fields, their defaults, and the constitution auto-wiring handshake with `spec-kit-format`. Storage-only — no runtime supervisor logic.

### Modified Capabilities
*(none)*

## Impact

**Code**:
- `src/config.rs` (or wherever `PawConfig` lives) — new `GovernanceConfig` struct with the 5 `Option<PathBuf>` fields. `#[serde(default)]` on the parent and on each field so missing TOML produces `None` defaults.
- `src/config.rs` — `PawConfig` gains `pub governance: GovernanceConfig`.
- `src/config.rs` — config-load post-processing step: if `governance.constitution.is_none() && config.specs.type == "speckit"`, call `git_paw::specs::speckit::detect_constitution(&specs_dir)` and populate `governance.constitution` with the result if `Some`.
- `docs/src/configuration.md` — document the new table, the constitution auto-wiring handshake, and (for the user guide) link out to user-guide examples of governance docs the user might point at.

**Tests**:
- Config-load with no `[governance]` section → `governance` field present with all paths `None`.
- Config-load with all paths populated → all paths resolve to repository-relative `PathBuf`s.
- Config-load with absolute path for one field → preserved as-is.
- Config-load with non-existent path → loads cleanly without error.
- Spec Kit interop: with `[specs] type = "speckit"`, `[specs] dir = ".specify/specs"`, `governance.constitution = None` in TOML, AND a fixture `.specify/memory/constitution.md` present → `governance.constitution` is populated with that path post-load.
- Spec Kit interop precedence: explicit `governance.constitution = "docs/principles.md"` is preserved even if `.specify/memory/constitution.md` exists.
- Round-trip: a fully-populated `GovernanceConfig` survives save → load.
- v0.4 backward compat: pre-v0.5 configs (no `[governance]`) load with all paths `None`.

**Backward compatibility**: fully additive. v0.4 configs with no `[governance]` section load with `governance: GovernanceConfig::default()`, all fields `None`. Per-key `#[serde(default)]` ensures partial sections also load cleanly.

**Mismatches resolved**:
- MILESTONE drift item #18 (governance scope was over-reaching) — resolved by this change reducing to path-pointer only and dropping `[governance.gates]`.
