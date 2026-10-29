## Why

An all-repos MCP client (`multi-repo-registry`) can list every registered repository, but it cannot
tell which agent CLI each repository launches its workers with. MILESTONE asks the registry to
resolve per-repo CLI preferences so that it composes with the v0.19.0 per-CLI provider layer. This
change is the thin, read-only version of that.

MILESTONE also asked for each CLI's folder-trust state (whether the CLI would stop on a first-run
"trust this folder?" dialog in that repository). That half is dropped from this change: no CLI's
trust-store location or format is verified yet, and v0.19.0 `cli-provider-record` ships no trust
query to compose with. Trust may return later, built on the per-CLI facts the v0.23.0 per-CLI audit
(`cli-support-audit`) verifies. Until then the readiness gate (`tmux/readiness.rs`, GP-03b) stays
the only handling of the dialog.

## What Changes

- **Repository profile resolution.** For an available registry entry, git-paw resolves the
  preferred CLIs, `default_cli` and `default_spec_cli`, from that repository's merged config. Each
  is reported as configured: the CLI id in the sense of the v0.19.0 resolved CLI record (the
  `[clis.<name>]` key or known binary name).
- **Nothing new is stored.** The registry file gains no field, and the profile is computed when
  asked. The repository's config stays the single source of truth.
- **Read-only.** Resolving a profile never writes a config file, never spawns an agent CLI, and
  never modifies the registry.
- **`list_repos` reports the profile.** Each available entry gains an additive
  `profile: { default_cli, default_spec_cli }`. Unavailable entries carry no profile.
- **No folder-trust state.** The profile carries no trust field, and git-paw reads no CLI's trust
  store.

## Capabilities

### New Capabilities

<!-- None. `core-repo-registry` is introduced by `multi-repo-registry`. -->

### Modified Capabilities

- `core-repo-registry`: adds repository profile resolution and its no-side-effect guarantee. These
  are ADDED requirements on the capability that `multi-repo-registry` creates.
- `mcp-server`: `list_repos` entries carry the profile. This is an ADDED requirement, because the
  listing requirement it extends is itself still pending in `multi-repo-registry`.

## Impact

- **Code:**
  - `src/registry/profile.rs`: a pure `resolve_profile(&PawConfig) -> RepoProfile`.
  - `src/mcp/query/repos.rs` and `src/mcp/tools/repos.rs`: the profile DTO.
- **Tests:**
  - Unit tables over in-memory configs.
  - A side-effect test: the registry and the repository config are unchanged and no CLI is spawned.
  - `tests/mcp_e2e.rs`: `list_repos` profile fields.
- **Docs:** the `list_repos` reference in `docs/src/user-guide/mcp.md`, and a profile note in the
  registry section of the configuration docs.
- **Enum-variant ripple:** none.
- **Dependencies:** none new.
- **Cross-version dependencies:**
  - **Depends on** `multi-repo-registry` (v1.1.0, this cycle) and must land after it.
  - **Gated on v0.19.0 `cli-provider-record`** (capability `cli-provider`), which owns CLI identity
    as a resolved record rather than a config key (GP-25, GP-29). The profile reports the record's
    CLI id and needs nothing else from the provider layer.
  - v0.18 `cmd-library-surface` / `orchestrator-escalation-handoff`: the orchestrator CLI
    (`[orchestrator] cli`) is deliberately not part of the profile (see design).
  - v0.21.0 `supervisor-vocabulary-split` / v1.0.0 `remove-supervisor-vocabulary`: this change
    uses only the post-1.0 configuration names (`[orchestrator] cli`).
- **Security / safety:** strictly read-only. No config or registry write, and no read of any
  third-party CLI's store. No agent CLI is spawned, which keeps the `mcp-server` guardrail.
