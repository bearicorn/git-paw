# Tiered-Model Workflow

A **smart orchestrator running a fleet of cheaper workers** is the workflow git-paw
is built for: one capable model supervises, and several lower-cost models do the
implementation in parallel worktrees, with little-to-no human babysitting. This
chapter is the *process* — how to set a run up so the cheap fleet actually
succeeds. For the mechanics of how the orchestrator drives the wave, see
[Supervisor → the loop is a pump, the supervisor is the brain](supervisor.md#the-loop-is-a-pump-the-supervisor-is-the-brain).

## The two roles

git-paw already gives you both halves:

- **Orchestrator** — the supervisor pane (`[supervisor].cli` + `--supervisor`). Its
  job is judgment: spawn order, answering cross-agent questions, sequencing merges,
  and deciding when a worker is unrecoverable. Give it your most capable model.
- **Worker** — an ordinary CLI in a worktree (`default_spec_cli` / `--cli`). Its job
  is execution: implement one change against a concrete task list. This is where a
  cheaper model earns its keep — *if* the task list is concrete enough (see below).

Two things make the cheap fleet viable rather than a prompt-storm: the orchestrator
[handles the judgment calls the loop can't](supervisor.md#the-loop-is-a-pump-the-supervisor-is-the-brain)
instead of parking them in an inbox nobody reads, and the
[self-healing correction loop](supervisor.md#unattended-correction-loop) turns a
gate failure into an automatic re-engagement rather than a human hop. Both ship in
v0.14.0. What *you* bring is the two pre-steps this chapter is about: a plan, and a
definition of "verified."

## Step 1 — Plan first; planning is not a git-paw command

git-paw does not author your change set — it *executes* one. Author the plan
**before** `git paw start`, in any [supported spec format](spec-driven-launch.md):
OpenSpec (`openspec/changes/<name>/`), Spec Kit, Superpowers plan documents, or
plain Markdown-with-frontmatter. git-paw starts once those artifacts exist and turns
each into a worker session.

Because planning is the one step that most rewards a capable model, do it with your
**smart** model — a planning pass, or an `/opsx` authoring session — and only then
hand the result to the cheap fleet.

## Step 2 — Make the tasks task-level concrete (the rule that decides success)

This is the central rule of the whole workflow: **a cheaper worker executes reliably
only when the tasks artifact is concrete.** Vague tasks are where cheap models
diverge; concrete tasks are where they succeed — and an under-specified plan costs
*more* in orchestrator correction cycles than a smart worker would have cost in the
first place.

Concrete means, per task: the exact files to touch, the per-step verification
command, and an explicit acceptance criterion.

```
# Too vague — a cheap worker will guess, and guess differently each run:
- [ ] Add validation for the config

# Concrete — a cheap worker can execute it deterministically:
- [ ] In src/config/supervisor.rs, reject max_cycles = 0 at load with an
      actionable error (mirror the invalid on_exhausted handling); add a unit
      test asserting 0 fails and 1 is accepted. Verify: cargo test --lib
      config::supervisor.
```

The OpenSpec/Superpowers task shape (a checkbox list where each item names its file,
its verification, and maps back to a spec scenario) is the exemplar. If you find the
orchestrator burning correction cycles on a worker, the fix is almost always a
sharper task, not a smarter worker.

## Step 3 — Define "verified" in your `AGENTS.md`, not in git-paw

What "verified" *means* — the exact test/lint/build commands, the acceptance
criteria, the definition of done — is a **per-project convention**, so it lives in
your repo's injected instructions (`AGENTS.md` / `CLAUDE.md`), never baked into
git-paw. This is the [export-agnosticism](../architecture.md) principle: git-paw
ships only the generic five-gate *scaffold* — testing → regression → spec audit →
doc audit → security — and the supervisor follows *your* concrete gates when it
verifies a worker's branch. Spell them out, or the orchestrator falls back to the
scaffold and guesses at your commands.

Wire the gate commands into `[supervisor]` (`test_command`, `lint_command`,
`build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`,
`security_audit_command`) and write the definition of done into `AGENTS.md`.
git-paw's own `AGENTS.md` — its "Change Checklist" and "Supervisor verification is a
five-gate framework" sections — is the exemplar to model yours on.

## Step 4 — Launch, and let the orchestrator drive

With the plan authored and "verified" defined:

```bash
# Attended: you watch the panes; the dashboard auto-approves the safe set.
git paw start --specs my-change --supervisor

# Unattended: the in-process loop drives to completion with no human in the seat.
git paw start --specs my-change --supervisor --unattended
```

Turn on the self-healing loop so a gate failure re-engages the worker automatically
instead of waiting for you:

```toml
[supervisor.correction]
auto_loopback = true      # re-engage a gate-failed worker's pane with the feedback
max_cycles = 5            # give up on a branch after this many verify-fix rounds
on_exhausted = "escalate" # escalate | abandon
```

From here the orchestrator answers questions, sequences merges, and re-engages
failing workers on its own; see [the correction loop](supervisor.md#unattended-correction-loop)
and [the pump-to-brain hand-off](supervisor.md#the-loop-is-a-pump-the-supervisor-is-the-brain)
for exactly what it handles.

## Where the human still comes in

You are the **exception handler**, not the operator. A human is re-engaged only for
what the orchestrator genuinely cannot decide, plus two structural cases: the
~25-minute heartbeat exit, and a run with **no supervisor pane at all** (a pure
`--unattended` run with no orchestrator escalates to the broker inbox exactly as it
did before). Everything the fleet can resolve — safe approvals, cross-agent
questions, verify-fix cycles, merge sequencing — resolves without you.
