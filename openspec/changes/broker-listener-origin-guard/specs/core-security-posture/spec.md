## ADDED Requirements

### Requirement: Security posture documents the broker's browser-exposure guard

The security-posture chapter SHALL state four things about the broker:

- the broker refuses requests from browser origins and from non-loopback `Host` names when it is bound to loopback, and why: DNS rebinding and prompt injection into agents that run with auto-approved execution;
- a non-loopback `bind` disables the Host check and exposes an unauthenticated broker to the network;
- the guard is not authentication, because any local process can still reach the broker;
- the migration for reverse proxies.

#### Scenario: Posture chapter covers the guard and its limits

- **WHEN** `docs/src/user-guide/security-posture.md` is read
- **THEN** it SHALL describe the Host and Origin checks, the DNS-rebinding threat they address, the non-loopback-bind exception, and the statement that local processes are not authenticated by this guard
