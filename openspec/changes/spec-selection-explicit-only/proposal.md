## Why

git-paw chose the spec system (OpenSpec / Markdown / Spec Kit / Superpowers) two
ways: an explicit `[specs]` config section or `--specs-format` flag, OR
filesystem auto-detection (probing `.specify/specs/` → Spec Kit; and, added
in-flight, `docs/superpowers/plans/` → Superpowers). Auto-detection is
implicit magic: it makes the resolved format depend on directory layout, needs
precedence rules when layouts co-exist, and drifts from the config that is
supposed to be authoritative.

Make the **config `[specs]` section (or the `--specs-format` CLI flag) the sole
source of truth**. Remove all filesystem auto-detection of the spec system. When
neither is set, launching from specs fails with an actionable error. `git paw
init` asks the user to choose the spec system and records it, so a configured
project always runs the correct backend deterministically.

## What Changes

- **REMOVE** the `spec-scanning` requirement *"Auto-detection of Spec Kit
  projects"* — git-paw no longer probes the filesystem to pick the spec system.
  (The in-flight `superpowers-spec-format` change likewise drops its own
  auto-detection requirement.)
- **ADD** a `spec-scanning` requirement that spec-system resolution is explicit:
  `--specs-format` (highest) → config `[specs]` → else `None` (an actionable
  error at scan time). No filesystem probing.
- **ADD** a `project-initialization` requirement: `git paw init` prompts the user
  to choose the spec system and writes `[specs]`; a non-interactive init writes
  a commented `[specs]` template. Init never auto-detects the spec system.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `spec-scanning`: filesystem auto-detection removed; resolution is config or
  `--specs-format` only, error otherwise.
- `project-initialization`: init records the spec system via an explicit prompt
  (or a commented template when non-interactive) instead of detecting it.

## Impact

- **Code:** `src/specs/mod.rs` (`resolve_specs_config` — dropped the `.specify/`
  and `docs/superpowers/plans/` probes and the `repo_root` param; sharpened the
  "not configured" error); `src/init.rs` (removed `detect_speckit_section` /
  `detect_superpowers_section`; added `prompt_specs_section`).
- **BREAKING (behaviour):** an existing Spec Kit project that relied on
  `.specify/`-only auto-detection with no `[specs]` section will now error on
  `git paw start --from-specs`. **Migration:** add `[specs] type = "speckit"`,
  `dir = ".specify/specs"` to `.git-paw/config.toml` (or re-run `git paw init`),
  or pass `--specs-format speckit`. No release has shipped Superpowers
  auto-detection, so only Spec Kit users are affected.
- **Docs:** spec-driven-launch, configuration, and CLI references updated to
  describe config/CLI selection (no auto-detection).
- The constitution-path probe and governance constitution auto-wiring are
  UNCHANGED — they are gated on an already-configured `type = "speckit"` and do
  not select the spec system.
