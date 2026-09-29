## Context

v0.16.0 removed settings-file permission seeding and added *No permission grants are seeded into
agent CLI settings files*, *Prompt-free operation is attributed to the permission mode and
classifier*, and *Settings path remains available to memory isolation*. The archive did not
remove the older seeding requirements, so `approval-command-safety` now says both "seed X into
settings" and "never seed". Separately, *MCP get_learnings consumes the variant* was specified
and never built. The shipped, documented `get_learnings` reads only the file.

What the code does today (verified):
- No writer for `settings.json`, `allowed_bash_prefixes` or a worktree `.claude/` exists under
  `src/`. `attach_agent_leaves_a_pre_existing_settings_file_byte_identical` (`src/main.rs`)
  confirms existing files are left alone.
- The presets live in `src/supervisor/dev_allowlist.rs` (`DEV_ALLOWLIST_PRESET`, the
  `*_STACK_PRESET` constants, `stack_preset`, `effective_patterns`). Two things consume them:
  `SupervisorConfig::effective_whitelist` (`src/config/supervisor.rs` ~:927) and
  `render_dev_allowlist_preset` (`src/skills.rs` ~:776, universal tier only).
  `CommonDevAllowlistConfig::enabled` is documented "Parsed for config compatibility; has no
  effect".
- `get_learnings` (`src/mcp/tools/session.rs` ~:94) returns `{ sections }` parsed from the file.

## Goals / Non-Goals

**Goals:** every SHALL left in these capabilities describes shipped behaviour. The preset
requirements keep their contract (contents, exclusions, single source of truth, dedup and
ordering) and now name their real consumers.

**Non-Goals:** any behaviour change other than the learning-id hash (D5); adding a broker path
to `get_learnings`; renaming capabilities; removing the inert `enabled` field (that would be a
config-schema change).

## Decisions

**D1 — REMOVE the ten seeding requirements instead of rewriting them.** Every one of them
describes writing a settings file. No rewrite could keep their subject matter without
contradicting the no-seeding requirement. Behaviour they used to imply that still holds is
already specified elsewhere: preserving files byte-identical (*No permission grants are
seeded…*), the settings path for memory isolation, and prompt-free helpers (*git-paw managed
helper-script invocations classify as safe*).

**D2 — MODIFY, not remove, the three preset requirements.** The presets still exist and feed the
classifier. Their content lists, exclusion rubric and dedup rules are a live contract with tests
(`dev_allowlist.rs`, `config/tests.rs`). Only the "seeder / `allowed_bash_prefixes`" framing
changes, to "`effective_patterns` → auto-approve whitelist / skill guidance". The lists are kept
word-for-word. One clause is new and matches existing behaviour and an existing test: an unknown
stack name contributes nothing.

**D3 — REMOVE *MCP get_learnings consumes the variant*; do not MODIFY it to file-only.**
`mcp-server` already owns the file-only `get_learnings` contract, with scenarios. Rewriting this
requirement to say the same thing would create two owners of one contract, which can drift.

**D4 — Also fix three stale "seed" sentences outside `approval-command-safety`**
(`supervisor-config` *Common dev allowlist sub-table*, `cli-resolution` *End-to-end boot
registration*, the `sweep.sh learn` scenario in `supervisor-learnings`). Leaving them would keep
contradictions this change exists to remove. Each edit is limited to the stale clause; the rest
of the requirement is copied unchanged.

**D5 — Pin the learning id to a hand-rolled FNV-1a 64-bit hash.** The spec said SHA-256; the
code uses std `DefaultHasher`. Neither is acceptable as is: `DefaultHasher`'s algorithm is
explicitly unspecified and may change between Rust releases, so a toolchain bump could change
every id and break dedup across a rolling upgrade; SHA-256 needs a crate (`sha2`) that is not on
the approved dependency list, and the id is a dedup key, not a security primitive, so a
cryptographic hash buys nothing. FNV-1a 64-bit is ~10 lines, has published test vectors, keeps the
existing 16-hex-character id shape, and is trivial to mirror in the bundled Python helper
(`sweep.sh learn`, via `bundled-helper-discovery`). The canonical serialisation (`canonical_value`
+ `hour_bucket`) is unchanged and is now written into the requirement, so the requirement — not
the code — is the single statement of the scheme. A golden-value scenario pins one concrete id.
Alternatives rejected: SipHash with fixed keys via `std::hash::SipHasher` (deprecated, and still
std-internal); keeping `DefaultHasher` and documenting the instability (leaves the defect).

The same `DefaultHasher` pattern computes the `agent.advanced-main` event id
(`src/broker/messages.rs` ~:430). It is fixed here (task 2b.5) with the same FNV-1a scheme,
so no persisted or published id depends on the toolchain.

## Risks / Trade-offs

- [Ids change once on upgrade] → A record emitted by the old binary and the same record emitted by
  the new one do not dedupe. Learnings are per-session and the hour bucket already rotates ids
  hourly, so the effect is at most one duplicate per record across the upgrade. Stated in the
  requirement and the learnings chapter.

- [The requirement count drops by 11] → Intended. REMOVED deltas carry the Reason and the
  Migration, and the archive keeps the audit trail.
- [The `approval-command-safety` `## Purpose` still describes seeding after archive] → A delta
  cannot change a Purpose. The archiver rewrites that paragraph by hand right after
  `openspec archive` (task 4.2), and `tests/spec_purpose_backfilled.rs` still passes because the
  Purpose stays non-placeholder.
