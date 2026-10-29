## Context

- `multi-repo-registry` adds `src/registry/` and `list_repos`. Entries carry `id`, `path`,
  `available`, `home` and an optional `reason`.
- A repository's CLI choice lives in its merged config: `PawConfig.default_cli` and
  `PawConfig.default_spec_cli` (`src/config/mod.rs`).
- v0.19.0 `cli-provider-record` (capability `cli-provider`) resolves each agent CLI into one record
  whose id is the `[clis.<name>]` key or known binary name. It has no folder-trust query.
- git-paw has no notion of persisted trust state. `tmux/readiness.rs` recognizes Claude Code's
  "trust the files in this folder" dialog on screen and answers it.
- The `mcp-server` guardrail forbids spawning any agent CLI.

## Goals / Non-Goals

**Goals:**
- An all-repos client can see each repository's preferred CLIs.
- Keep this thin: read the existing config loader, and store nothing.

**Non-Goals:**
- Folder-trust state. Dropped from this change: no CLI's trust-store location or format is
  verified, and the v0.19.0 provider layer has no trust query. It may return later on the per-CLI
  facts the v0.23.0 audit (`cli-support-audit`) verifies.
- Writing trust markers or pre-accepting trust.
- Using the profile to launch anything across repositories. Cross-repo writes are refused by
  `multi-repo-registry` D8.
- CLI authentication state (the post-1.0 `auth_ok()` follow-on in MILESTONE v0.19).

## Decisions

### D1 — Resolve, don't store

The profile is a pure function of the repository's merged config, evaluated when `list_repos`
runs. Storing it in `repos.json` would duplicate the repository's config, which can change without
`git paw init` running, so a stored copy would go stale silently.

Alternative considered: caching the profile in the registry at `init`. Rejected because it goes
stale and turns the registry into a second config.

### D2 — The preferred CLIs are `default_cli` and `default_spec_cli`

These are the CLIs a repository's worker panes launch with: bare launches and spec-driven
launches. Each is reported as configured: the CLI id that v0.19.0's resolved record carries (the
`[clis.<name>]` key or known binary name). The profile does not resolve the record itself, so a
CLI that is absent from PATH where the server runs is still reported by its id, and resolution
cannot fail.

Alternative considered: also reporting `[orchestrator] cli`, the headless orchestrator CLI from v0.18
`orchestrator-escalation-handoff`. Deferred: no client need has been observed for it.

### D3 — No trust half

The original draft also reported, per preferred CLI, whether that CLI already trusts the repository
root (`trusted`, `untrusted` or `unknown`), through a read-only folder-trust port on the v0.19
provider trait. It is dropped. The trust-store facts it needs are unverified, the v0.19.0
`cli-provider` capability has no such port, and adding one here would encode third-party file
formats ahead of the per-CLI audit. The readiness gate remains the runtime handling of the dialog.

### D4 — No side effects, by construction

`resolve_profile` takes `&PawConfig` only. It has no writer and no `CommandRunner`. A test checks
that the registry file and the repository config are unchanged afterwards, and the existing
"no agent CLI process is spawned" guardrail test is extended to cover `list_repos`.

### D5 — Additive wire change

Each available `list_repos` entry gains `profile`. Existing fields are untouched, and unavailable
entries carry no `profile`. Clients that ignore unknown fields are unaffected. A later trust field
would be another additive key on `profile`.

## Risks / Trade-offs

- [A client wants trust state now] → It reads `profile` for the CLI names and relies on the
  readiness gate at launch. Trust can be added additively once its facts are verified.
- [A configured CLI id names a CLI that is not installed] → The profile reports configuration, not
  installation. This is documented.

## Migration Plan

Additive. No file format changes. Rollback: remove the `profile` field from the DTO.

## Open Questions

- Should `[orchestrator] cli` (the headless orchestrator CLI) join the profile? Recommendation: not
  until a client needs it.
