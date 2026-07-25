## ADDED Requirements

### Requirement: Blocking subprocess calls run off the async worker threads

Blocking work reachable from an async HTTP handler SHALL execute off the tokio runtime's
async worker threads (e.g. via `tokio::task::spawn_blocking` or an equivalent
dedicated-thread mechanism) — in particular the `git` subprocess the role-gating guard runs
while processing a `committed` artifact publish. A burst of `committed`-artifact publishes
on a role-gating session SHALL NOT be able to saturate the async worker threads and stall
the broker's other HTTP endpoints. Relocating the blocking work SHALL NOT change what the
role-gating guard publishes, when it fires, or the broker's lock/await/spawn ordering.

#### Scenario: A publish burst does not stall other HTTP endpoints

- **GIVEN** a running broker on a multi-threaded runtime whose `BrokerState` carries an active role-gating context (`OpenSpec` engine, non-`off` mode)
- **WHEN** a burst of concurrent `agent.artifact { status: "committed" }` publishes arrives (each triggering the guard's blocking `git` read)
- **AND** a `GET /status` request is issued concurrently with the burst
- **THEN** the `GET /status` request SHALL complete successfully within a bounded time rather than blocking until every `git` invocation returns
- **AND** the blocking `git` invocation SHALL run off the async worker threads

#### Scenario: Guard behavior is unchanged by the offload

- **GIVEN** a role-gating session in which a coding agent commits `OpenSpec` archive activity
- **WHEN** the `committed` artifact is published and the guard runs off the worker thread
- **THEN** the guard SHALL still publish the same `agent.feedback` (and, in `block` mode, the same revert request) it published when the guard ran inline

### Requirement: A poisoned broker-state lock is recovered rather than fatal

Acquiring the `BrokerState` read or write lock SHALL recover from a poisoned lock (a lock
left poisoned because a thread panicked while holding the guard) and return a usable guard,
rather than propagating the poison as a panic. A single panic while a state-lock guard is
held SHALL NOT permanently disable the broker: subsequent HTTP handlers and background tasks
SHALL continue to acquire the lock and serve requests.

#### Scenario: A request after a lock-poisoning panic is still served

- **GIVEN** a running broker on a multi-threaded runtime
- **WHEN** a handler or task panics while holding the broker-state lock, poisoning it
- **AND** a later HTTP request that acquires the same lock arrives
- **THEN** the broker SHALL still respond successfully to the later request (the lock is recovered, not propagated as a panic)

#### Scenario: read and write both recover from poison

- **GIVEN** a `BrokerState` whose inner lock has been poisoned
- **WHEN** `BrokerState::read` or `BrokerState::write` is called
- **THEN** the call SHALL return a usable guard for the inner state instead of panicking
