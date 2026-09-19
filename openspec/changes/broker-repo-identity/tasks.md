## 1. Failing tests first

- [ ] 1.1 With a broker for repository A on a port, `start_broker` for repository B on the same port returns an error naming A and the port, and no handle (fails before the fix — today it reattaches)
- [ ] 1.2 A broker whose `/status` has no `repo_root` is refused rather than reattached
- [ ] 1.3 A broker for the same repository is still reattached
- [ ] 1.4 `/status` includes `repo_root` equal to the broker's canonical repository path
- [ ] 1.5 Two sessions write to different `<session_name>.broker.log` files, and purging one leaves the other's log in place

## 2. Repository identity

- [ ] 2.1 Give `BrokerState` the canonical repository root, plumbed from the launch path
- [ ] 2.2 Add `repo_root` to the `/status` JSON body
- [ ] 2.3 In `probe_existing_broker`, parse `repo_root` and compare canonical paths; add a `ProbeResult` variant for a git-paw broker of a different or unknown repository
- [ ] 2.4 Add a `BrokerError` variant for the refusal, distinct from `PortInUse`, whose message names the port, the other repository when known, and the `[broker] port` fix; mention that an older git-paw broker may be the holder when `repo_root` is absent

## 3. Per-session log

- [ ] 3.1 Replace every `session_state_dir().join("broker.log")` with `<session_name>.broker.log` (start.rs twice, supervisor.rs, main.rs)
- [ ] 3.2 Make purge delete only the purged session's log
- [ ] 3.3 Grep for any other reader of `broker.log` (helpers, docs, tests) and update it

## 4. Docs and gates

- [ ] 4.1 Document in the broker / configuration chapter that two repositories need distinct `[broker] port` values, and what the new error means
- [ ] 4.2 Note the per-session log name wherever `broker.log` is documented
- [ ] 4.3 Verify every scenario in `specs/broker-runtime/spec.md` maps to a test
- [ ] 4.4 `cargo fmt`, then `just lint`
- [ ] 4.5 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 4.6 `openspec validate broker-repo-identity --strict`
