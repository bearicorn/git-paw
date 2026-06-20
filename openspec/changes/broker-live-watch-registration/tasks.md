# Tasks

> Target release: v0.7.0 (follow-up to v0.6.0 `git-paw-add`).

## 1. Broker live target set

- [x] 1.1 Make the broker's watch-target list mutable at runtime
      (behind the existing lock) instead of fixed at start
- [x] 1.2 Idempotent insert: registering an already-watched path is a
      no-op

## 2. Endpoint

- [x] 2.1 Add `POST /watch` route (agent id + worktree path + cli),
      loopback-only, validating the body like other endpoints
- [x] 2.2 Wire the registered path into the running watcher so it
      surfaces activity without a restart
- [x] 2.3 Unit/integration tests: register → dirty worktree → appears
      in `/status`; duplicate register is a no-op

## 3. git paw add / remove integration

- [x] 3.1 `git paw add` POSTs the new worktree to `/watch` after
      creation
- [x] 3.2 `git paw remove` deregisters (or the broker prunes a target
      whose worktree disappears)
- [x] 3.3 E2E: hot-added agent surfaces via the watcher (not only via
      CLI self-registration) — closes git-paw-add's deferred 6.1/6.2

## 4. Quality gates

- [x] 4.1 `just check` + `just deny` pass
- [x] 4.2 Docs: broker-endpoints reference gains `/watch`
