## Why

`git paw mcp` serves exactly one repository, fixed at startup by `--repo` or the current directory
(`mcp::resolve_repo`, `RepoContext::for_root`). A user with N git-paw repositories therefore keeps N
entries in every MCP client config, and clients that spawn from a fixed directory (Claude Desktop,
ChatGPT Desktop) need a hand-written absolute `--repo` per entry. Nothing on the machine knows which
repositories git-paw manages, so there is no way to serve them from one entry, and nothing for the
later cross-repo work (v1.2.0 A2A, v1.4.0 orchestrator-to-orchestrator) to build on.

## What Changes

- **Repository registry.** A versioned JSON file, `<data_dir>/git-paw/repos.json`, in the same
  platform data directory as the session receipts (`crate::dirs::data_dir()`; not a `~/.git-paw/`
  dotdir). It lists each registered repository's stable `id`, canonical `path` and `registered_at`.
  Updates take an exclusive lock and replace the file atomically. The file is created with mode
  `0600`. Entries whose repository is gone are pruned on the next write and reported as unavailable
  until then.
- **`git paw init` registers the repository.** It skips linked worktrees and repos that set
  `[mcp] register = false`, which also removes an existing entry. A registry failure is a warning
  and never fails init.
- **`git paw mcp --all-repos`** (new, opt-in). One server serves every registered repository. It
  adds a `list_repos()` tool and an optional `repo` parameter (a registry id) on every tool. The
  home repository (from `--repo` or the current directory, as today) becomes optional.
- **Least-privilege access policy.**
  - `repo` resolves only to a registered, available id, never to a path.
  - Cross-repository access is read-only. Write tools act only on the home repository.
  - A worktree session (an agent's server) never reaches another repository: `--all-repos` is
    ignored there, with a warning.
- **Back-compat.** Without `--all-repos` the tool set and every tool's parameters are unchanged.
  The only difference: a call that supplies a `repo` argument is now refused instead of being
  silently ignored. Omitting `repo` in `--all-repos` mode means the home repository.
- **New config key `[mcp] register`** (bool, default `true`). Setting it to `false` excludes the
  repository from the registry and from `--all-repos` serving.

## Capabilities

### New Capabilities

- `core-repo-registry`: the machine-wide registry of git-paw repositories, covering the file
  location, versioned schema, identifiers, which repositories are eligible, stale entries,
  concurrent updates, file integrity and the per-repo opt-out.

### Modified Capabilities

- `mcp-server`: *Repository resolution* gains the optional home repository and per-call targeting
  in `--all-repos` mode. *Subcommand flag surface* gains `--all-repos`. *Documentation deliverable*
  no longer requires per-repo client entries. New requirements cover all-repos mode, `list_repos`,
  per-call `repo` targeting, the cross-repo access policy, and per-target configuration-error
  isolation.
- `core-init`: init registers the repository (added requirement).
- `core-configuration`: *MCP configuration section* gains `register`.

## Impact

- **Code:**
  - New `src/registry/`: `model.rs` (pure: `RepoId`, register/prune decisions), `wire.rs` (`v1`
    DTO and mapper), `store.rs` (path, lock, atomic write, `_in(dir)` variants).
  - `src/init.rs` (`run_init` registers after the `.gitignore` step).
  - `src/config/mod.rs` (`McpConfig::register`, init template).
  - `src/cli.rs` (`Mcp { all_repos }`, help text).
  - `src/mcp/mod.rs` (`ServeMode`, optional home, per-call `RepoContext`).
  - New `src/mcp/access.rs` (pure target and access decision).
  - New `src/mcp/tools/repos.rs` and `src/mcp/query/repos.rs` (`list_repos`).
  - Every `src/mcp/tools/*.rs` params struct gains `repo`.
  - `src/mcp/server.rs` (mode-dependent router: `list_repos` registration, `repo` schema
    stripping, instructions).
- **Tests:**
  - Unit tables: id derivation, register/prune, the access decision.
  - `tests/repo_registry.rs`: concurrency (threads and two `git paw init` processes),
    permissions, corrupt or newer-version files.
  - `tests/mcp_e2e.rs`: all-repos flows, single-repo schema unchanged, refusals, worktree
    downgrade.
  - Init tests.
  - All tests use the HOME/XDG override.
- **Docs:** `git paw mcp --help` and `git paw init --help`; README (MCP line and CLI table);
  `docs/src/user-guide/mcp.md`; `docs/src/cli-reference.md`; `docs/src/configuration/`;
  `docs/src/architecture.md`; the specifications index (new capability).
- **Enum-variant ripple:** none (`BrokerMessage` and `SpecBackendKind` are untouched).
- **Dependencies:** none new. Locking uses `std::fs::File::try_lock` (stable since 1.89, below the
  1.96 MSRV), plus `serde_json` and `chrono`, both already approved.
- **Cross-version dependencies:**
  - v0.17 `mcp-superpowers-support` modifies *Hard errors only for malformed configuration* and
    *Project knowledge tools* in `mcp-server`. This change does not modify either, but per-target
    validation reuses that change's accepted `[specs].type` set.
  - v0.17 `test-session-state-isolation`: the new tests must use its HOME/XDG override.
  - v0.17 `broker-repo-identity`: guarantees each target's broker serves that repository.
  - v0.18 `cmd-library-surface` provides root-session detection (`RepoContext::is_main_worktree`)
    and the root-only lifecycle and authority tools that this change confines to the home
    repository.
  - v0.18 `broker-verdict-authority`: no authority credential crosses repositories.
  - v0.21 wire-DTO/versioning, state-file locking and slug unification: the registry follows them
    from day one. If a shared lock helper or slug value object exists, use it.
  - v1.0.0 freeze: this change is additive. *Subcommand flag surface* and *Documentation
    deliverable* may also be touched by the freeze audit, so rebase over it.
  - v1.2.0 A2A builds on registry ids, and so would a future MCP HTTP transport. That transport
    is unscheduled: the A2A cycle dropped it.
- **Security / safety:**
  - `repo` is matched as an opaque string against registry ids and never used as a path.
  - Each target path is re-validated when it is used.
  - Cross-repo access is read-only, opt-in per client entry and opt-out per repo.
  - Worktree sessions are confined to their own worktree.
  - The registry file is `0600`, written atomically under a lock, and refused when group- or
    world-writable or from a newer schema version.
