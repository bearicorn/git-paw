## Why

git-paw's design principle is that everything the binary exports to a consumer must be
project-agnostic, and the bundled skills already honour it — content lives in
`assets/agent-skills/*.md`, embedded via `include_str!`. But roughly 150 lines of
agent-facing prose are still assembled by compiled Rust `push_str`/`format!` calls, where
they are invisible to the export-agnosticism audits that guard the assets:

| Prose | Site |
|---|---|
| Inter-agent rules block | `src/agents.rs:147-181` |
| Drive-loop directive | `src/skills.rs:412-431` |
| Spec-path doctrine sentences | `src/skills.rs:798-815` |
| Governance section header | `src/skills.rs:877-879` |
| Boot-prompt framing per backend | `src/main.rs:348-376` |
| Supervisor framing prompt | `src/commands/supervisor.rs:742-748` |
| Spec Kit / Superpowers task-prompt sections | `src/specs/speckit.rs:391-442`, `src/specs/superpowers.rs:121-136` |
| Git hook bodies | `src/agents.rs:493-584` |

Two consequences. First, changing a sentence an agent reads requires a recompile and a
binary release. Second — and worse — this prose escapes the no-language-leak audit that
exists precisely to stop project-specific conventions reaching consumers. That is not
hypothetical: the inter-agent rules tell every consumer's agent that "the supervisor's
spec audit will reject mismatched names", exporting git-paw's spec workflow to projects
that have none. The audit would have caught it in an asset.

## What Changes

- Agent-facing prose moves from compiled string assembly into `assets/`, embedded at
  compile time so single-binary distribution is preserved exactly.
- Rust retains the **assembly**: placeholder substitution, conditional inclusion,
  per-backend selection, dedupe, and region gating. Only the sentences move.
- The git hook **bodies** move to `assets/hooks/*.sh`. The hook *installation* logic —
  marker chaining, mode bits, common-vs-linked gitdir resolution — stays compiled, since
  that is mechanism and a marker-chaining bug is a correctness problem, not a wording one.
- The no-language-leak audit is extended to cover the newly-externalised prose and to
  flag **vendor product names** (e.g. `Claude Code`, `.claude/`) outside an explicitly
  allowed span, so the class of leak that produced GP-01 becomes detectable.

**Explicitly out of scope (owned by in-flight changes):**
`peer-cherry-pick-removal` rewrites the *content* of the inter-agent rules and must land
first — this change moves whatever that change leaves. `speckit-spec-path-pointer` fixes
where the spec-path doctrine *points*; this change moves the sentences without altering
their routing. `worker-guidance-skill-export` *adds* new skill sections; this change
relocates existing ones. None of the three is redone here.

**Not breaking:** the rendered output an agent sees is unchanged. This is a relocation,
verified by asserting the rendered text is byte-identical before and after.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-lang-agnostic`: agent-facing prose SHALL be authored in bundled assets rather
  than assembled from compiled string literals; the no-leak audit is extended to cover
  that prose and to flag vendor product names outside allowed spans.
- `git-hook-injection`: the installed hooks' script bodies are sourced from bundled
  assets; the installation mechanism (marker chaining, permissions, gitdir resolution)
  is unchanged.

## Impact

- **Code:** `src/agents.rs`, `src/skills.rs`, `src/main.rs`,
  `src/commands/supervisor.rs`, `src/specs/speckit.rs`, `src/specs/superpowers.rs` —
  each replaces literal prose with an asset reference, keeping its assembly logic.
- **Assets:** new files under `assets/` for the relocated prose and `assets/hooks/` for
  the hook bodies; `DRIVE_LOOP_DIRECTIVE` becomes a marker-delimited region inside
  `supervisor.md`, reusing the existing region machinery (`render_opsx_regions`,
  `src/skills.rs:692`) rather than a new mechanism.
- **MUST stay compiled:** `render_dev_allowlist_preset` (`src/skills.rs:738-769`)
  generates its prose *from* the allowlist constant, which is what stops the documented
  allowlist drifting from what is actually auto-approved. A hand-editable asset there
  would silently widen the approval surface. Likewise the hook installation logic and
  the `{{CHANGE_ID}}` placeholder whitelist (`src/skills.rs:641`).
- **Tests:** the existing substring assertions in `src/skills.rs`, `src/agents.rs` and
  `tests/*_skill_content.rs` port unchanged, since the rendered text does not change.
  Add byte-identical before/after assertions for each relocated block.
- **Enum-variant ripple:** none. `SpecBackendKind` is *matched* by several of these sites
  (`render_spec_path_doctrine`, `build_task_prompt`) but no variant is added or removed.
- **Backward compatibility:** rendered output identical; `include_str!` embedding
  preserved, so a binary with no filesystem assets still renders.
- **Docs:** the contributing guide's "where does agent-facing text live" guidance, and
  the export-agnosticism dev skill's surface inventory.
