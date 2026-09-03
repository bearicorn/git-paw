## Context

Three small unattended-mode lifecycle fixes surfaced by the peer-4-poker dogfood: a
misreporting exit summary (GP-05), an orphaned drive loop after purge/stop (GP-06),
and an orchestrator pane missing from the session file. All are additive.

## Goals / Non-Goals

**Goals:** a finished wave reports as finished; a torn-down session's drive loop never
drives the next same-named session; the session file shows the full tiered roster.

**Non-Goals:** the broader session-state reconciliation / crash recovery and state-file
locking (v0.20); GP-05 does not add a new `WorkerPhase` or `SessionStatus` variant.

## Decisions

- **Resolve final state from terminal artifacts, not last status (GP-05).** Reuse the
  `WorkerPhase` lifecycle (v0.14) to read each agent's terminal state
  (verified/merged), falling back to a wave-level "N/M merged" line. Alternative
  rejected: continuing to print the last `agent.status`, which never advances past
  `working`.
- **Bind the loop to a session instance, verified each tick (GP-06).** The loop
  compares a durable instance token (the session's start receipt / PID / timestamp,
  already available in session state) against the live session before acting, and
  exits when it no longer matches. Alternative rejected: signalling the loop from
  `purge`/`stop`, which cannot reliably reach a detached (nohup) loop — the
  self-check is sufficient on its own and also covers the "replaced by a new
  same-named session" case.
- **Additive orchestrator entry (`#[serde(default)]`).** A new optional entry/field on
  `RepoSessionFile`; older files default it absent and `sweep.sh` ignores it.

## Risks / Trade-offs

- **Instance token unavailable/unstable** → use the durable start receipt already
  persisted in session state rather than a volatile handle.
- **sweep.sh reads the new field positionally** → the field is additive and named;
  `sweep.sh` enumerates by its documented keys and ignores unknown fields (existing
  back-compat scenario).

## Migration Plan

Additive; no migration. Older session files load; rollback is a straight revert.

## Open Questions

None.
