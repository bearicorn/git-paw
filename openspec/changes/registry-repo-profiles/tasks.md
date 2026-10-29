## 0. Preconditions

- [ ] 0.1 Confirm `multi-repo-registry` is merged and archived. Its `core-repo-registry` capability and the `list_repos` requirement must be in `openspec/specs/`.
- [ ] 0.2 Confirm the v0.19.0 `cli-provider` capability (`cli-provider-record`) is archived, and that its resolved record's CLI id is the `[clis.<name>]` key or known binary name, as the profile reports it.

## 1. Profile resolution (pure)

- [ ] 1.1 Add `src/registry/profile.rs` with `RepoProfile { default_cli, default_spec_cli }` and `resolve_profile(&PawConfig) -> RepoProfile`. It takes no writer and no `CommandRunner`, and reads no CLI store.
- [ ] 1.2 Write a unit table for these scenarios:
  - Profile reflects the repository configuration
  - Configuration changes are reflected without re-registration
- [ ] 1.3 Write a side-effect test for the scenario Resolution writes nothing: the registry file and the repository config are byte-identical afterwards. Extend the existing "no agent CLI process is spawned" guardrail test to cover profile resolution, for the scenario No agent CLI is spawned.
- [ ] 1.4 Write an integration test for the scenario The registry stores no profile data: after `git paw init` with `default_cli` set, the registry entry has only `id`, `path` and `registered_at`.

## 2. `list_repos` profile

- [ ] 2.1 Add a `RepoProfileDto` with `profile: Option<RepoProfileDto>` (`skip_serializing_if = "Option::is_none"`) to the `list_repos` entry DTO in `src/mcp/tools/repos.rs`, and populate it for available entries in `src/mcp/query/repos.rs`, using the target repository's merged config.
- [ ] 2.2 Write e2e tests in `tests/mcp_e2e.rs` (HOME/XDG override) for these scenarios:
  - Available entry carries its profile
  - Unavailable entry carries no profile
  - Existing listing fields are unchanged
- [ ] 2.3 Update the closed tool-name allowlists if needed. The tool set is unchanged; only the `list_repos` output grows.

## 3. Documentation

- [ ] 3.1 In `docs/src/user-guide/mcp.md`, document the `list_repos` `profile` fields (the CLI ids as configured; not a statement that the CLI is installed), and that no folder-trust state is reported.
- [ ] 3.2 In the registry section of `docs/src/configuration/`, note that CLI preferences are resolved from the repository's config, never stored in the registry.

## 4. Quality gates

- [ ] 4.1 Run `cargo fmt`, `just check` (`cargo test --no-fail-fast`) and `just deny` (no new dependency). Check the real exit codes.
- [ ] 4.2 Run `mdbook build docs/`.
- [ ] 4.3 Walk the `security-and-safety-review` checklist: no config or registry write, no read of a CLI's own store, no CLI spawn, no new path input.
