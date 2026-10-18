## 1. Credential persistence

- [ ] 1.1 Add `Session.broker_authority: Option<String>` (`serde(default, skip_serializing_if)`); `save_session` writes `0600` on Unix; tests: round-trip, earlier file → `None`, mode `0600`, discovery file carries no `broker_authority`
- [ ] 1.2 `src/random.rs`: hex token from ≥32 bytes of `/dev/urandom` (handled error on an unreadable source, as `generate_uuid_v4`); unit tests
- [ ] 1.3 Mint in `cmd_start`, `launch_spec_session`, `cmd_supervisor`; write it to the `__broker` host's stdin when spawning it (beside `broker-host-separation`'s argv session name + log path); persist the same value in the `Session` saved after the panes exist (the receipt's unchanged position); `recover_session` and `restart_from_pause` hand the host the saved credential when present, and `restart_from_pause`'s post-host save re-reads the credential so a host back-fill is kept; never on argv or in `set-environment`; tests: saved value shape, receipt credential accepted by the host (202), failed fresh start saves nothing, restart keeps a back-fill, emitted commands/env contain no credential

## 2. Broker enforcement

- [ ] 2.1 `BrokerState` carries `authority: Option<String>`; `/publish` refuses `agent.verified` and `agent.feedback` with `payload.from == "supervisor"` without a matching constant-time `X-Paw-Authority` (403 `{"error":"authority required"}`, no state mutation); `None` fails closed
- [ ] 2.2 Server tests for every `broker-protocol` scenario: verified without header → 403; supervisor feedback wrong header → 403; verified with header → 202 + unchanged broadcast; branch-guard feedback without header → 202; five non-verdict verbs → 202; forged gate failure → 403 and no correction cycle; in-process detector feedback delivered
- [ ] 2.3 `git paw __broker` takes the credential from stdin when handed one, otherwise reads it from the saved `Session` it was named; back-fills (mint, re-save `0600`, before serving) only when neither has one; never logs it; tests for stdin, load and back-fill
- [ ] 2.4 Confinement tests: `/status`, `/log`, `git paw status --json`, MCP `get_session_status`, the broker log contain no credential

## 3. Root-side clients

- [ ] 3.1 `BrokerClient` publish accepts an optional authority header; the drive loop attaches it to its verdict publishes (`GitBranchRefresher::notify`); test "The drive loop's branch-refresh notification is accepted"
- [ ] 3.2 Hidden `git paw __authority`: prints the credential for the repository's session (exit 0), non-zero + `git paw start` hint when none, non-zero + empty stdout from a linked worktree, decided with the shipped `cli_setup::shell::resolve_layout` + `RepoLayout::is_linked_worktree` (`src/cli_setup/shell.rs:41`; git dir vs common dir, handles `--separate-git-dir`); hidden from `--help` (`cli.rs::root_help_hides_internal_subcommands` gains it); CLI tests
- [ ] 3.3 `assets/scripts/sweep.sh`: `verified`, `feedback-gate` and `conflict-ack` (`agent.feedback` from `supervisor`, `sweep.sh:826`; exits 1 on a rejected publish) fetch via `git paw __authority` and send `X-Paw-Authority`; `bash -n`; `tests/sweep_sh_conventions.rs` + live-broker header tests for `verified` and `conflict-ack`; `broker.sh` unchanged (test: no credential verb); `.git-paw/scripts/sweep.sh` is gitignored (written by `git paw init`) — refresh the local copy for dogfooding, nothing to commit
- [ ] 3.4 E2E: a worktree-side publisher (broker.sh, worktree MCP session, curl) posting `agent.verified` gets 403 and the branch is not verified; `sweep.sh verified` at the root gets 202; the post-commit branch-guard hook still gets 202

## 4. Skill + docs

- [ ] 4.1 `assets/agent-skills/supervisor.md`: already uses `sweep.sh verified` / `sweep.sh feedback-gate` for the `agent.verified` / `agent.feedback` examples since v0.17 (`supervisor.md:162/182`); check the recipient semantics and wire-field prose against the `skill-agent` delta and add the skill-content tests for its scenarios; export-agnosticism audit green
- [ ] 4.2 `docs/src/user-guide/security-posture.md` (rule, storage, surfaces, threat model, what stays ungated: `agent.status` incl. from `supervisor`, one agent publishing as another), `coordination.md` (403 shape; which feedback is a verdict), `supervisor.md` (verdict verbs via the helper), `architecture.md` broker section; `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base
- [ ] 5.2 Security review recorded: no secret on argv/env, constant-time compare, `0600`, no log leakage, wire unchanged, branch-guard hook unaffected
