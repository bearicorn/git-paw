## 1. Widen the sweep.sh status-publish surface

- [ ] 1.1 In `assets/scripts/sweep.sh`, extend `cmd_status_publish` to parse
      optional `--phase <phase>` and `--detail '<json-object>'` flags before
      the positional `<message…>`, preserving the plain `status-publish
      <message…>` form unchanged
- [ ] 1.2 Shape the `agent.status` payload internally (via a `python3 -c
      "$(cat <<'EOF' … EOF)"` block consistent with `cmd_feedback_gate` /
      `cmd_verified`): include `phase` only when `--phase` is supplied and
      `detail` only when `--detail` is supplied; omit both keys otherwise
- [ ] 1.3 Validate that a supplied `--detail` argument parses to a JSON
      object; on failure exit non-zero with a diagnostic on stderr and do
      NOT publish
- [ ] 1.4 Update the `status-publish` usage line in `sweep.sh`'s `usage()`
      to document the `--phase` / `--detail` flags

## 2. Route the supervisor skill through the helper

- [ ] 2.1 In `assets/agent-skills/supervisor.md`, replace the boot
      self-register raw `curl …/publish` (agent.status) with a `sweep.sh
      status-publish --phase baseline "supervisor online"` example
- [ ] 2.2 Replace the introspection phase-taxonomy example(s) (the audit
      example with `--phase audit --detail '{"branch":…,"audit_step":…}'`)
      with `sweep.sh status-publish` forms
- [ ] 2.3 Replace the `checkpoint` emission raw curl with `sweep.sh
      status-publish --phase checkpoint --detail '{"intended_targets":[…]}'`
- [ ] 2.4 Audit `supervisor.md` for any remaining `curl …/publish` whose body
      is an `agent.status` and convert it to `sweep.sh status-publish`
- [ ] 2.5 Confirm `coordination.md` has no raw `agent.status` publish example
      (it states the watcher publishes status automatically; leave the
      heartbeat guidance consistent — route any explicit agent.status example
      through the helper if present)

## 3. Allowlist verification (no broadening)

- [ ] 3.1 In `src/supervisor/curl_allowlist.rs`, confirm the existing by-path
      grant for `.git-paw/scripts/sweep.sh` covers the widened verb; add no
      broad `curl *` grant

## 4. Tests

- [ ] 4.1 `tests/sweep_sh_*`: assert `status-publish <msg>` (no flags)
      produces an `agent.status` with `agent_id="supervisor"`,
      `status="working"`, the message, and NO `phase`/`detail` keys
- [ ] 4.2 `tests/sweep_sh_*`: assert `status-publish --phase audit --detail
      '{"branch":"feat/auth","audit_step":"tests"}' "auditing feat/auth"`
      produces an `agent.status` with `phase="audit"` and a `detail` object
      carrying `branch` and `audit_step`
- [ ] 4.3 `tests/sweep_sh_*`: assert `status-publish --detail 'not-json'`
      exits non-zero, writes a stderr diagnostic, and publishes nothing
- [ ] 4.4 Supervisor-skill-content test: assert `supervisor.md` contains NO
      `/publish` example whose body is an `agent.status`
      (`"type":"agent.status"`), and that the boot/audit/checkpoint/summary
      emissions use `sweep.sh status-publish`
- [ ] 4.5 Curl-allowlist test: assert the seeded grant authorises
      `.git-paw/scripts/sweep.sh` by path and contains no broad `curl *` grant
- [ ] 4.6 `tests/sweep_sh_conventions.rs`: confirm the widened
      `cmd_status_publish` keeps the `-c "$(cat <<'EOF' … EOF)"` shape (no
      stdin-claiming `interpreter - <<` heredoc reintroduced)

## 5. Docs + quality gates

- [ ] 5.1 Update mdBook supervisor/CLI reference if it documents
      `sweep.sh status-publish`, to mention the `--phase` / `--detail` flags;
      `mdbook build docs/` must succeed
- [ ] 5.2 `just check` (fmt + clippy + tests) passes
- [ ] 5.3 `just deny` passes
- [ ] 5.4 `openspec validate "broker-helper-full-surface" --strict` passes
