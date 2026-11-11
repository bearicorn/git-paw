## Context

Per-CLI knowledge in bundled prose today (post v0.19.0):
- `assets/agent-skills/coordination.md` (798 lines) → "Per-CLI mechanism" table: a `claude` row
  (`/compact`, `/clear`) and an `other` row (look for `/compact`, `/save`, `/reset`). Pinned by
  `skill-agent`'s *Per-CLI compact mechanism table*, which already forbids naming a custom variant
  such as `claude-oss` (that row was dropped before v0.19.0). The `claude` row is vendor text
  served to every agent whatever its CLI.
- `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` → "### PASTE HANDLING":
  "When you paste text, Claude may collapse it into `[Pasted text #N]`. After any paste operation,
  send an additional Enter key…", pinned by `boot-block`'s *Paste handling instructions* (scenario
  *Paste instruction format* asserts the Claude explanation for every boot block).
- The only other per-CLI branch at render time is the boot-block form. v0.19.0 already resolves it
  from the record: callers pass `CliProfile::resolve(cli, clis).mcp_capable()` (or the profile they
  already hold) into `build_boot_block(branch_id, broker_url, mcp_capable: bool)`; the
  `skills::cli_speaks_mcp` wrapper delegates to the same record and has no production caller left.
  This change does not alter the form rule, only where `build_boot_block` reads it from.

Per-CLI facts the code encodes for `codex` / `agy` live on their `CliProvider` impls in
`src/agent_cli/mod.rs`: approval flags (`codex` → `--sandbox workspace-write` / bypass, `claude`
and `agy` → `--dangerously-skip-permissions`), native sandbox args, session templates, MCP
capability and prompt profiles. Nothing about their compaction commands or paste behavior — those
are **to verify** before any fragment states them.

## Goals / Non-Goals

**Goals:**
- Each agent's skill text and boot block carry its own CLI's notes and no other vendor's.
- No forked copies of the ~800-line coordination skill.
- Unknown and custom CLIs degrade to a correct, vendor-neutral `generic`.

**Non-Goals:**
- Changing *how* the MCP-vs-helper form is decided (v0.19.0's capability record). This change only
  makes `build_boot_block` read that decision from the same profile it reads the family from.
- Per-CLI variants of the orchestrator skill (the orchestrator is the operator's own session; its
  CLI is not known to git-paw).
- Per-CLI prompt-shape profiles for the drive loop (shipped in v0.19.0 `cli-prompt-profiles`).
- The Claude-shaped `/clear to save <N>k tokens` wording in the coordination skill's context-bloat
  section: it describes what the detector's `context_bloat_pattern` (a per-CLI prompt-profile
  field since v0.19.0) matches for the default profile, so it stays with that detector and out of
  the fragments.
- A new family type. `CliFamily` is the closed set; this change reads it.

## Decisions

### D1 — Base + fragments, not whole per-CLI templates

MILESTONE names `claude.md`, `codex.md`, `agy.md`, `generic.md`. Four full copies of the
coordination skill would differ in a handful of lines and drift in everything else. A fragment per
family per slot (`skill`, `boot`) carries only what differs, rendered into `{{CLI_NOTES}}` /
`{{CLI_BOOT_NOTES}}`. The four names survive as the family directories.
*Alternative:* full per-CLI templates as MILESTONE literally states — rejected on drift; listed as
a decision for the user.

### D2 — Selection key: the profile's `CliFamily`, Null Object fallback

`select_fragment(family: CliFamily, slot, package) -> &'static str` is pure: return the fragment
registered at `assets/cli/<family.as_str()>/<slot>.md` if the package has one, else the `generic`
fragment. The family is `CliProfile::family()` of the agent's resolved v0.19.0 profile; a custom
`[clis.<name>]` CLI resolves the family it declares with `family`, else `generic` — never inferred
from `command` — so no CLI name is matched in rendering code. A render with no agent (repository
root, MCP `get_skill`) passes `CliFamily::Generic`.

No parallel `SkillFamily` type: v0.19.0 made `CliFamily` the one closed set of families, with
`CliFamily::provider` its only exhaustive match. Selection does not match on it either — it uses
`as_str()` as a directory name and falls back on a missing entry — so a family without a fragment
needs no code, and adding a fragment for an existing family is an asset only. This also keeps
`src/skills.rs` clear of the `cli-provider` source audit (*Per-CLI names are compared only inside
the agent CLI module*), which forbids comparing values against `"claude"`, `"codex"` etc. outside
`src/agent_cli/`.

`build_boot_block(branch_id, broker_url, cli: &CliProfile)` reads both `cli.mcp_capable()` (form)
and `cli.family()` (fragment). Callers already hold or resolve the profile; tests build one with
`CliProfile::for_family(..)`.

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
templates and the `generic` fragment must be vendor-free. `skill-package-layout`'s per-file audit
of every package file reads a file under `assets/cli/<family>/` as that family's fragment.

### D6 — The mechanism table requirement is replaced, not modified

*Per-CLI compact mechanism table* pins a table with a `claude` row and an `other` row. Once each
agent sees only its own CLI's mechanism there is no table and no `other` row, so the requirement is
REMOVED and an ADDED requirement states the per-agent mechanism. The surviving assertions carry
over: a Claude-family agent sees `/compact` / `/clear`, the fallback points at
`/compact` / `/save` / `/reset`, and no custom variant name such as `claude-oss` appears.
*Paste handling instructions* survives as MODIFIED: its *Paste instruction format* scenario keeps
its name, scoped to a Claude-family agent.

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
   reproduces today's Claude lines (the table's `other` row becomes the `generic` text).
2. Thread the profile family through `render`/`build_boot_block` callers.
3. Audit per family; docs.
Rollback: revert; rendering returns to the single text.

## Open Questions

- **Decision for the user:** fragments (recommended) vs four full per-CLI templates.
- Whether `codex` / `agy` fragments ship in v0.20.0 depends on task 2.3's verification; if neither
  verifies, the release ships `claude` + `generic` only and says so.
