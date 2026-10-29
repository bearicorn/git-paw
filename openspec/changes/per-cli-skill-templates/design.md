## Context

Per-CLI knowledge in bundled prose today:
- `assets/agent-skills/coordination.md` → "Per-CLI mechanism" table: `claude` (`/compact`,
  `/clear`), `claude-oss` (same), `other` (look for `/compact`, `/save`, `/reset`). Pinned by
  `skill-agent`'s *Per-CLI compact mechanism table*, which requires the `claude-oss` row.
- `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` → "PASTE HANDLING: When
  you paste text, Claude may collapse it into `[Pasted text #N]`…", pinned by `boot-block`'s
  *Paste handling instructions*.
- The only other per-CLI branch at render time is `skills::cli_speaks_mcp` (the `[clis.<name>] mcp`
  override, then a name match against `MCP_CAPABLE_CLIS`), which picks the boot-block form. v0.19.0 replaces name matching with a
  resolved capability record (GP-29); this change does not touch the form choice.

Per-CLI facts the code encodes for `codex` / `agy`: approval flags
(`config::supervisor` maps `codex` → `--sandbox workspace-write` / bypass, `claude|agy` →
`--dangerously-skip-permissions`) and MCP capability. Nothing about their compaction commands or
paste behavior — those are **to verify** before any fragment states them.

## Goals / Non-Goals

**Goals:**
- Each agent's skill text and boot block carry its own CLI's notes and no other vendor's.
- No forked copies of the 720-line coordination skill.
- Unknown and custom CLIs degrade to a correct, vendor-neutral `generic`.

**Non-Goals:**
- Changing *how* the MCP-vs-helper form is decided (v0.19.0's capability record). This change only
  makes `build_boot_block` read that decision from the same profile it reads the family from.
- Per-CLI variants of the orchestrator skill (the orchestrator is the operator's own session; its
  CLI is not known to git-paw).
- Per-CLI prompt-shape profiles for the drive loop (v0.19.0).
- The Claude-shaped `/clear to save <N>k tokens` wording in the context-bloat section: it describes
  what `sweep.sh`'s detector matches and moves with v0.19.0's per-CLI prompt profiles.

## Decisions

### D1 — Base + fragments, not whole per-CLI templates

MILESTONE names `claude.md`, `codex.md`, `agy.md`, `generic.md`. Four full copies of the
coordination skill would differ in a handful of lines and drift in everything else. A fragment per
family per slot (`skill`, `boot`) carries only what differs, rendered into `{{CLI_NOTES}}` /
`{{CLI_BOOT_NOTES}}`. The four names survive as the family directories.
*Alternative:* full per-CLI templates as MILESTONE literally states — rejected on drift; listed as
a decision for the user.

### D2 — Selection key: the profile's skill family, Null Object fallback

`select_fragment(family, slot, package) -> &'static str` is pure: return the family's fragment for
that slot if the package registers one, else the `generic` fragment. The family comes from the
agent's resolved v0.19.0 CLI profile; a custom `[clis.<name>]` variant resolves to the family its
profile declares (the GP-29 shape), so no CLI name is ever matched in rendering code. A render with
no agent (repository root, MCP `get_skill`) passes `generic`. The family is a string key that
matches the asset directory name, so adding a family is an asset plus a profile value — no code
branch. (`architecture-direction.md`: an enum is for a small fixed set where callers handle each
case; here no caller does.)

### D3 — Only verified facts ship

`claude` and `generic` fragments are authored from text already in the bundled assets. `codex` and
`agy` fragments are written only for facts checked against those CLIs' current documentation at
implementation time (task 2.3); until then those families render `generic`, which is correct for
them.

### D4 — Overrides still work

A user/`.agents/skills/` override of the coordination skill may use `{{CLI_NOTES}}` or not; the
placeholder renders the same way. Fragments themselves are not overridable in this change (they
are registry assets) — an override that wants different per-CLI prose writes it inline.

### D5 — Audit per family

The no-leak audit renders each template once per registered family plus `generic`, and allows a
vendor token only where the rendered text came from that family's own fragment. The base
templates and the `generic` fragment must be vendor-free.

## Risks / Trade-offs

- [v0.19.0's provider families (`claude`, `codex`, `agy`, `qwen`, `cursor-agent`, `generic`)
  outnumber the fragments] → a family with no fragment renders `generic`; the selection key is
  still the resolved profile, never a name match.
- [An agent's CLI changes mid-session (re-attach with another `--cli`)] → provisioning re-renders
  on attach, so the installed `SKILL.md`, the sidecar and the next boot block follow the new CLI.
- [Fragments make the coordination skill harder to read as one file] → `get_skill` and the docs
  show the `generic` rendering; the fragments are tiny.

## Migration Plan

1. Placeholders + `generic` fragments that reproduce today's vendor-neutral text; Claude fragment
   reproduces today's Claude lines; `claude-oss` row dropped.
2. Thread the profile family through `render`/`build_boot_block` callers.
3. Audit per family; docs.
Rollback: revert; rendering returns to the single text.

## Open Questions

- **Decision for the user:** fragments (recommended) vs four full per-CLI templates.
- Whether `codex` / `agy` fragments ship in v0.20.0 depends on task 2.3's verification; if neither
  verifies, the release ships `claude` + `generic` only and says so.
