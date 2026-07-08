## Context

Follow-up to `dashboard-orphan-exit` (v0.10.0). The v0.10.0 dogfood proved the shipped fix handles the common reparent-to-init case but leaves two leak paths open (see proposal). Both keep a dashboard busy-looping at high CPU after its session is gone.

## Goals / Non-Goals

**Goals:** no dashboard can busy-loop after its session ends, regardless of how it was orphaned (init OR lingering shell) or whether its broker bind succeeded.
**Non-Goals:** no change to rendering, layout, or the broker protocol; not a rework of where the broker is hosted (it stays in the dashboard process for now).

## Decisions

- **D1 — Exit on broker-bind failure.** If the in-process broker fails to bind its port, the dashboard SHALL log a diagnostic and exit non-zero rather than loop. Rationale: a bind failure almost always means a stale dashboard already owns the port; a second busy-looping process helps nobody and is exactly the leak. *Alternative:* render a "broker unavailable" banner and keep polling — rejected: still risks the orphan busy-loop and masks the real problem.
- **D2 — Lifecycle check on ALL loop paths.** The `shutdown || orphaned() || tty_gone()` check SHALL guard every iteration/branch, not only the normal `event::poll` arm, so an error/degraded path cannot skip it.
- **D3 — Broaden orphan detection with a tty/stdout-gone check.** In addition to `getppid() == 1`, exit when writing to / polling the terminal indicates the controlling tty is gone (poll `Err`, or a write error to stdout). This catches reparent-to-shell, where `getppid()` is a live but unrelated shell. *Alternative:* walk the process tree to find whether the spawning tmux server is alive — rejected as heavier and racy; tty-gone is the reliable signal that the pane is gone.

## Risks / Trade-offs

- A transient tty hiccup could trigger an unnecessary exit → acceptable: the dashboard is cheap to relaunch (`git paw start` re-attaches), and a false-negative (leak) is worse than a false-positive (early exit).
- Non-Unix platforms lack `getppid`==1 semantics → retain prior SIGHUP behavior; the tty-gone check is portable and adds coverage there too.
