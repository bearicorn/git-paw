## Context

The reattach path exists for a sensible reason: if git-paw's own broker for this repository
is already up, starting a second one on the same port would fail, so attach instead. The
probe's only test is "is this a git-paw broker", which was enough while one machine ran one
repository. It is not enough once two repositories default to the same port.

The reattached handle is also thinner than it looks — it wraps the caller's *new* state and
starts nothing — so even a same-repository reattach yields a dashboard that does not see the
running broker's state. That is a separate, older question and not this change's target;
this change only stops a *different* repository from being treated as ours.

## Goals / Non-Goals

**Goals:**

- Never route one repository's agents into another repository's broker.
- Fail loudly, with a fix the user can apply in one line of config.
- One broker log per session, so purge cannot delete a neighbour's log.

**Non-Goals:**

- Automatic free-port selection.
- Making a same-repository reattach share the running broker's state.
- Multi-repo brokers (that is the v1.1.0 registry work).

## Decisions

**Identify by canonical repository root.** It is already what `find_session_for_repo`
matches on, it is stable for the life of a session, and it is cheap to add to `/status`.
Compare canonicalized paths on both sides so `/var` vs `/private/var` style aliases match.

**Unknown owner is refused, not trusted.** A broker that does not report `repo_root` is an
older git-paw. Trusting it would keep the bug for exactly the users who have not upgraded
both sides; refusing costs a one-line `[broker] port` change. The error text should say an
older git-paw broker may be holding the port.

**A dedicated error variant.** Reusing `PortInUse` would make "a foreign program holds the
port" and "another repository's git-paw holds the port" indistinguishable to the user and
to tests. They have different fixes.

**Per-session log name.** Session names are already unique per repository on a machine, so
`<session_name>.broker.log` needs no new identity scheme, and purge already knows the
session name it is purging.

## Risks / Trade-offs

- **Users with two repositories must now configure a port.** Today they silently get a broken
  second session; after this they get an error telling them what to set. That is strictly
  better, but it is a visible change and belongs in the changelog.
- **Pause/resume across a binary upgrade.** A session paused under an older git-paw and
  resumed under this one finds its own old broker, which reports no `repo_root`, and is
  refused. Rare; the error names the port, and stopping the old broker resolves it.
- **`BrokerState` needs the repository root.** Plumb it from the launch path; do not
  re-derive it inside the broker from the current directory.
