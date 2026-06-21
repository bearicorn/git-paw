## ADDED Requirements

### Requirement: Tests SHALL select the broker port via an OS-assigned ephemeral port

Every integration test and the `selftest` harness that needs a free TCP port for the broker (or any helper named `pick_broker_port`, `broker_port`, `pick_port`, or equivalent) SHALL obtain that port by binding `127.0.0.1:0`, reading back the OS-assigned local port, and releasing the listener so the broker can claim the port. The helper SHALL NOT derive the port from the process id via a `BASE + (std::process::id() % N)` scheme.

The former scheme `24_000 + (std::process::id() % 200)` (and its siblings such as `BASE + (process::id() % 100)`, `% 1000`, `% 5000`) keyed the port on the process id modulo a small constant, yielding at most `N` distinct ports. `N` concurrent `cargo test` runs collided modulo that constant, the broker failed to bind with "address already in use", and the verify run reported a false-negative failure. This was the real cause of the in-session verify flakes — not a live-session collision. An OS-assigned ephemeral port is collision-proof at any concurrency because the kernel guarantees each `bind 127.0.0.1:0` returns a port not currently in use.

The canonical implementation SHALL be the helper already present at `tests/e2e_supervisor_stop.rs::pick_broker_port`:

```rust
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}
```

Tests that previously used a PID-derived port base SHALL be migrated to this helper (or an equivalent ephemeral-bind call). There is an inherent, accepted race window between releasing the listener and the broker binding the port; because the window is microseconds and the port is OS-assigned (not contended by other test workers), this is dramatically less collision-prone than the PID-mod scheme and is the same trade-off the broker's own free-port discovery already makes.

#### Scenario: The broker-port helper returns a free, OS-assigned port

- **GIVEN** a test (or the `selftest` harness) needs a broker port
- **WHEN** it calls the ephemeral-port helper
- **THEN** the helper SHALL bind `127.0.0.1:0`, read back the kernel-assigned port, and release the listener
- **AND** the returned port SHALL be one the broker can immediately bind

#### Scenario: Concurrent test runs do not collide on the broker port

- **GIVEN** two or more `cargo test` invocations run concurrently on the same host (e.g. under `cargo llvm-cov` or parallel CI shards)
- **WHEN** each invocation allocates a broker port via the ephemeral-port helper
- **THEN** each invocation SHALL receive a distinct OS-assigned port
- **AND** no invocation SHALL fail the broker bind with "address already in use" caused by a PID-modulo collision

#### Scenario: No test relies on the PID-modulo port scheme

- **GIVEN** the source tree after this change is applied
- **WHEN** the broker-port helpers across `tests/` are audited
- **THEN** no broker-port helper SHALL compute its port as `BASE + (std::process::id() % N)`
- **AND** every broker-port helper SHALL obtain its port by binding `127.0.0.1:0`
