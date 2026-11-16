## 1. Delivery and broker state

- [ ] 1.1 `POST /publish` answers `202 {"seq": N}` (`publish_message` returns the seq)
- [ ] 1.2 Read cursor per agent from `GET /messages/<id>?since=N`; `/status` per-agent `read_through` and unread orchestrator messages (count, oldest seq, oldest age)
- [ ] 1.3 `[delivery]` question when a `committed` artifact arrives over unread orchestrator messages, once per unread set
- [ ] 1.4 Coordination chapter documents the read cursor and the publish response body

## 2. Wave activity

- [ ] 2.1 Broker host samples each watch target's surface every 30 s through the terminal backend, off the async workers; mid-response (prompt-profile check) plus a changed capture hash marks activity
- [ ] 2.2 `wave` gains `last_activity_unix_secs` and `busy_agents`; `stalled` also requires no activity within the threshold; `idle_seconds` unchanged

## 3. Helpers and skills

- [ ] 3.1 Root-side verbs print `seq <N>` from the publish response
- [ ] 3.2 `sweep.sh delivery <agent> [<seq>]` with exit `0` read / `1` unread
- [ ] 3.3 Coordination skill: poll the inbox before each commit
- [ ] 3.4 Orchestrator skill: `delivery` check before acting on silence, `[delivery]` questions explained

## 4. Tests (one or more per scenario)

- [ ] 4.1 Delivery: read cursor and unread; `[delivery]` once per unread set; the publish response seq matches `/log`
- [ ] 4.2 Wave activity with a fake backend: a changing mid-response pane keeps `stalled` false and lists the agent; a static pane stalls
- [ ] 4.3 `sweep.sh` tests: `seq` in output, `delivery` exit codes
- [ ] 4.4 Skill content test for the poll-before-commit rule

## 5. Gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.3 `shellcheck` on the changed helpers
- [ ] 5.4 `mdbook build docs/`
- [ ] 5.5 `openspec validate broker-delivery-and-activity --strict`
