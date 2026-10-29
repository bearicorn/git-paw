## Why

Every agent gets the same coordination skill and boot block whatever CLI it runs, so the
per-CLI facts git-paw does know are written for everyone at once: the coordination skill's
"Per-CLI mechanism" table lists `claude` and `claude-oss` (a maintainer's private variant, in a
bundled asset), and every boot block tells a Codex or Antigravity agent that "Claude may collapse"
pasted text. MILESTONE v0.20.0 asks for per-CLI skill templates (`claude`, `codex`, `agy`, a
`generic` fallback) selected for each pane's CLI. v0.19.0's per-CLI provider layer resolves one
CLI profile per surface (Strategy with a `generic` Null Object), which is the selection key this
needs; v0.20.0's packages give the variants a home.

## What Changes

- **One base template plus per-CLI fragments, not four copies.** A package may carry
  `assets/cli/<family>/skill.md` and `assets/cli/<family>/boot.md`; the coordination `SKILL.md` gains
  `{{CLI_NOTES}}` where the per-CLI mechanism table sits today, and both boot-block forms gain
  `{{CLI_BOOT_NOTES}}` where the paste-handling paragraph sits.
- **Selection is a pure function of the agent's resolved CLI profile**: its skill family
  (`claude`, `codex`, `agy`), falling back to `generic` when the family ships no fragment or the CLI
  is unrecognised. A custom `[clis.<name>]` variant uses the family its profile declares, so a
  Claude-family variant gets the Claude fragment without being listed by name.
- **Rendered per agent**: the sidecar, the installed `SKILL.md` in the worktree, and the boot block
  each use that agent's fragment; renders with no agent CLI (repository root, MCP `get_skill`) use
  `generic`.
- **Content comes only from facts the code already encodes or that are verified at
  implementation**: `claude` gets `/compact` / `/clear` and the `[Pasted text #N]` note; `generic`
  gets the vendor-neutral fallback row and paste advice. `codex` / `agy` fragments ship only for
  facts verified against those CLIs' documentation; unverified ⇒ no fragment ⇒ `generic`.
- **Export guard**: vendor naming is legal only in the matching family's fragment; the base
  templates and `generic` name no vendor.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `skill-agent`: new requirement for per-CLI fragment selection; *Per-CLI compact mechanism
  table* becomes "the agent's own CLI's mechanism, from its fragment" (the `claude-oss` row goes).
- `boot-block`: *Paste handling instructions* — the vendor-specific explanation moves into the
  Claude fragment; every form still carries the second-Enter instruction. *Helper function
  signature* — `build_boot_block` takes the resolved CLI profile (the spec still shows the
  two-argument form although the code already takes a third `mcp_capable` flag).
- `core-lang-agnostic`: vendor names confined to the matching CLI fragment, audited per family.

## Impact

- **Code:** `src/skills.rs` (`render` and `build_boot_block` take the agent's CLI profile, or a
  `SkillFamily` read from it; fragment lookup in the registry), the callers that render per agent
  (`commands/{start,add,supervisor}.rs`, `main.rs::attach_agent`, provisioning from
  `skill-package-deploy`), `src/mcp/tools/project.rs` (`get_skill` renders `generic`).
- **Assets:** `assets/agent-skills/coordination/assets/cli/{claude,generic}/{skill,boot}.md`
  (+ `codex`, `agy` when verified); coordination `SKILL.md` and both boot-block forms gain the
  placeholders.
- **Tests:** per-family render tests (skill + both boot forms), selection table test, custom
  variant test, audit per family.
- **Docs:** `docs/src/user-guide/skill-templates.md` (fragments, families, override behavior),
  `docs/src/user-guide/coordination.md` (context-budget section), `docs/src/user-guide/agents-md.md`.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched).
- **Cross-version dependencies:**
  - Requires v0.19.0 `cli-provider-record` (capability `cli-provider`): the resolved `CliProfile`
    carries the provider family this change uses as the skill family, with the `generic` Null
    Object and a custom variant's declared `[clis.<name>].family` (GP-29). The record's MCP
    capability already picks the boot-block form (`cli-provider` *The boot-block form follows the
    record's MCP capability*); v0.19.0 does not modify `boot-block`'s *Helper function signature*,
    so this change's delta is the one that replaces the `mcp_capable` flag with the profile.
  - Requires v0.20.0 `skill-package-layout` (fragments live in the package's `assets/`); composes
    with `skill-package-deploy` (the installed `SKILL.md` is rendered per agent).
  - v0.17.0 `shipped-text-corrections` edits boot-block text; land it first.
- **Dependencies:** none.
