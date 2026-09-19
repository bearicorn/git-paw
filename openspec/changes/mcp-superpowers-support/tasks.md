## 1. Failing tests first

- [ ] 1.1 `git paw mcp` started in a repository with `[specs].type = "superpowers"` starts and serves rather than exiting non-zero (fails before the fix)
- [ ] 1.2 `get_specs()` in a Superpowers repository lists its plans with `backend` `superpowers`
- [ ] 1.3 `get_skill` in a Superpowers repository renders the Superpowers spec-path doctrine, not the no-backend sentinel (fails before the fix)
- [ ] 1.4 A test asserting every `SpecBackendKind` variant's type string is accepted by startup validation, so a future variant cannot be rejected

## 2. Fix

- [ ] 2.1 Replace `validate_startup_config`'s `VALID` list with a check derived from the spec-discovery mapping; build the invalid-type message's list of valid values from the same source
- [ ] 2.2 In `get_skill`, parse the type into `SpecBackendKind` and `match` exhaustively, so a new variant fails to compile here
- [ ] 2.3 Correct the three-backend doc comment in `src/mcp/query/specs.rs`

## 3. Docs

- [ ] 3.1 Add the remaining non-compile-forcing spec-type string dispatches to AGENTS.md §7's `SpecBackendKind` list
- [ ] 3.2 Check the MCP chapter of the user guide for a three-backend list and update it

## 4. Gates

- [ ] 4.1 Verify every scenario in `specs/mcp-server/spec.md` maps to a test
- [ ] 4.2 `cargo fmt`, then `just lint`
- [ ] 4.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 4.4 `openspec validate mcp-superpowers-support --strict`
