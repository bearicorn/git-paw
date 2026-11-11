## Why

Every agent gets the same coordination skill and boot block whatever CLI it runs, so the
per-CLI facts git-paw does know are written for everyone at once: the coordination skill's
"Per-CLI mechanism" table carries a `claude` row (`/compact`, `/clear`) next to a generic `other`
row, and every boot block tells a Codex or Antigravity agent that "Claude may collapse" pasted
text into `[Pasted text #N]`. MILESTONE v0.20.0 asks for per-CLI skill templates (`claude`, `codex`,
`agy`, a `generic` fallback) selected for each pane's CLI. v0.19.0's per-CLI provider layer
(`src/agent_cli/`) resolves one `CliProfile` per agent CLI, carrying a closed `CliFamily` with a
`generic` Null Object; that family is the selection key this needs, and v0.20.0's packages give
the variants a home.

## What Changes

- **One base template plus per-CLI fragments, not four copies.** A package may carry
  `assets/cli/<family>/skill.md` and `assets/cli/<family>/boot.md`, where `<family>` is the
  family's config name (`CliFamily::as_str()`: `claude`, `codex`, `agy`, `qwen`, `cursor-agent`,
  `generic`); the coordination `SKILL.md` gains `{{CLI_NOTES}}` where the per-CLI mechanism table
  sits today, and both boot-block forms gain `{{CLI_BOOT_NOTES}}` where the paste-handling
  paragraph sits.
- **Selection is a pure function of the agent's resolved `CliProfile`**: its `CliFamily`
  (`CliProfile::family()`), falling back to `generic` when the family ships no fragment. No new
  family type is introduced. A custom `[clis.<name>]` CLI resolves the family it declares with
  `family` (else `generic`, never inferred from `command`), so a Claude-family variant gets the
  Claude fragment without being listed by name.
- **Rendered per agent**: the sidecar, the installed `SKILL.md` in the worktree, and the boot block
  each use that agent's fragment; renders with no agent CLI (repository root, MCP `get_skill`) use
  `generic`.
- **`build_boot_block` takes the profile**: the third parameter becomes `&CliProfile` instead of
  v0.19.0's `mcp_capable: bool`; the profile decides both the helper-vs-MCP form (unchanged rule:
  `CliProfile::mcp_capable()`) and the boot fragment.
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

- `skill-agent`: new requirements for per-CLI fragment placeholders and selection; *Per-CLI compact
  mechanism table* is REMOVED and replaced by an ADDED requirement stating the agent's own CLI's
  mechanism from its fragment (the table and its `other` row go; the "no custom variant name" rule
  carries over).
- `boot-block`: *Paste handling instructions* — the vendor-specific explanation moves into the
  Claude fragment; every form still carries the second-Enter instruction. *Helper function
  signature* — `build_boot_block` takes the resolved `CliProfile` (the spec still shows the
  two-argument form although the code takes a third `mcp_capable` flag). *Template variable
  substitution* — its scenarios call the new signature.
- `core-lang-agnostic`: vendor names confined to the matching CLI fragment, audited per family.

## Impact

- **Code:** `src/skills.rs` (`render` / `render_for_agent_id` take the agent's `CliFamily`;
  `build_boot_block` / `boot_block_for_id` take `&CliProfile`; fragment lookup in the package
  registry keyed by `CliFamily::as_str()`; `cli_speaks_mcp`, which has no production caller left,
  stays as is). Per-agent render sites: `src/commands/start.rs` (`render` at the sidecar/skill
  sites, `build_boot_block` at both pane-boot sites, which already resolve a `CliProfile`),
  `src/commands/launch.rs` (`attach_agent`'s `render`, `compose_agent_launch`'s
  `build_boot_block`, reached by `git paw add` and supervisor-mode launches), provisioning from
  `skill-package-deploy`, `src/mcp/tools/project.rs` (`get_skill` renders `generic`).
  `tests/boot_block_bare_helper_calls.rs` and the boot-block tests in `src/skills.rs` /
  `src/commands/launch.rs` switch from a `bool` to `CliProfile::for_family(..)`.
- **Assets:** `assets/agent-skills/coordination/assets/cli/{claude,generic}/{skill,boot}.md`
  (+ `codex`, `agy` when verified); coordination `SKILL.md` and both boot-block forms gain the
  placeholders.
- **Tests:** per-family render tests (skill + both boot forms), selection table test, custom
  variant test, audit per family. `skills::tests::coordination_skill_per_cli_mechanism_table`
  and `tests/coordination_helper_only_skill_content.rs::coordination_skill_carries_no_git_paw_history_or_variant_names`
  are re-pointed to the replacement requirement.
- **Docs:** `docs/src/user-guide/skill-templates.md` (fragments, families, override behavior),
  `docs/src/user-guide/coordination.md` (context-budget section), `docs/src/user-guide/agents-md.md`.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched; `CliFamily` is read,
  not extended).
- **Source audit:** fragment selection lives in `src/skills.rs`, outside `src/agent_cli/`, so it
  SHALL NOT compare a value against a family or CLI name (`cli-provider` *Per-CLI names are
  compared only inside the agent CLI module*); it joins `CliFamily::as_str()` into a path and falls
  back on a missing entry.
- **Cross-version dependencies:**
  - Builds on v0.19.0 `cli-provider-record` + `cli-profile-threading` (capability `cli-provider`,
    shipped): `CliProfile::family()` is the selection key, `CliFamily::Generic` the Null Object, and
    `[clis.<name>].family` the custom-variant declaration. The record's MCP capability already
    picks the boot-block form (`cli-provider` *The boot-block form follows the record's MCP
    capability*); v0.19.0 did not modify `boot-block`'s *Helper function signature*, so this
    change's delta is the one that replaces the `mcp_capable` flag with the profile.
  - Requires v0.20.0 `skill-package-layout` (fragments live in the coordination package's
    `assets/`, which owns the boot-block templates); composes with `skill-package-deploy` (the
    installed `SKILL.md` is rendered per agent; package `assets/` are render-time inputs and are
    not installed).
- **Dependencies:** none.
