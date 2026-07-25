# Non-Functional Requirements (git-paw)

## Why this file exists

These are the quality attributes that **drive** git-paw's code standards — the rationale
behind every rule in `SKILL.md`. NFRs always conflict; the point of writing them down is the
stated **precedence** for resolving those conflicts, so the implementing agent and the
supervisor's review gate reason from the same priorities. When a change trades one NFR for
another, this file says which one wins.

## The NFR set

### Top tier — git-paw's defining attributes

git-paw **auto-approves commands** and **runs unattended in real repositories**, approaching a
v1.0.0 freeze. That makes three attributes existential (a failure here causes real harm, not
mere annoyance):

- **Security & safety** — drives: least-privilege, path-scoped allowlists · worktree /
  blast-radius confinement · injection newtypes (sanitize/quote at construction) · no secrets in
  flags or env · export-agnosticism · agent-memory-isolation.
- **Reliability (unattended)** — drives: idempotency · graceful failure + recoverability
  (`pause`/`recover`) · no orphaned processes or worktrees · broker lock discipline · network
  timeouts.
- **Stability / backward-compatibility** — drives: frozen serde/wire/CLI surfaces ·
  `#[serde(default)]` on new optional fields · semver · `#[doc(hidden)]` internals · additive-only
  change.

### Must-haves

- **Maintainability** — domain modules, no god files, findings-first refactor, `PawError`
  constructors, no drive-by edits.
- **Testability** — the `CommandRunner` seam / ports-and-adapters, behavioral tests, the
  `test-strategy` skill.
- **Documentation** — the 4 doc layers (`--help`, README, mdBook, rustdoc), rustdoc on public
  items, `--json`/agent-friendly surfaces, the doc-audit gate.

### Second tier (tracked, subordinate to the above)

- **Observability / debuggability** — logging/replay, `doctor` + `selftest`, actionable errors.
- **Portability** — no locale-dependent output parsing, single-binary, XDG, macOS/Linux/WSL.
- **Composability / interoperability** — stdout/stderr discipline, `--json` as the stable
  contract, exit codes, the MCP server.
- **Supply-chain & license integrity** — `cargo deny`/`audit`, minimal approved deps, permissive
  licenses, pinned MSRV.
- **Privacy** — no telemetry / phone-home; consent-first.
- **Developer experience** — sensible defaults, discoverability, frictionless `init`/dogfood.
- **Extensibility** — pluggable `SpecBackend`, `[clis.*]`, presets, skills — additive, not core
  surgery.
- **Contributability** — CONTRIBUTING, architecture docs, the spec-driven workflow, these skills.

## Conflicts & resolutions

| Conflict | Tension | Resolution |
|---|---|---|
| **Security ↔ Automation/DX** | Auto-approval is frictionless but expands blast radius. *The central tension.* | Security wins ties: deny-by-default, least-privilege path-scoped allowlists, conservative classifier, tiered approval (drive loop = safe set only; human/supervisor = the rest). The approval architecture *is* this resolution. |
| **Stability/freeze ↔ Maintainability** | The freeze locks ugly serde/wire/CLI/`pub` surfaces you can't then clean up. | Freeze the observable surface; refactor freely *behind* it (behavior-preserving). Shrink the semver surface (`#[doc(hidden)]`, MSRV) *before* the freeze — now-or-never. The do-not-touch list is this encoded. |
| **Security ↔ Back-compat** | A security fix changes observable behavior (e.g. sanitizing session names). | Security overrides compat — but as an explicit, *versioned*, migrated change, never silently; prefer additive/relax-only. |
| **Testability ↔ Simplicity** | The `CommandRunner` seam adds indirection purely to enable mocking — vs "no abstractions for single-use code." | Add the seam only where behavior is otherwise untestable (the blind tmux/git runtime); it earns its keep by unlocking a whole e2e-only surface. |
| **Reliability ↔ Simplicity** | Handling timeouts/poison/orphans/locale/spaces vs "no error handling for impossible scenarios." | On unattended + security paths, robustness wins (those cases happen unsupervised); on one-shot interactive paths, simplicity wins. |
| **Observability ↔ Privacy** | Pane logging + learnings capture potentially sensitive terminal content. | Local-only, no telemetry. Privacy wins absolutely — nothing leaves the machine. |
| **Machine output ↔ Human output** | Rich human output vs stable machine output. | stdout = machine-clean / stderr = human; `--json`/`--plain` are the frozen contract, human output may evolve. |

## Precedence spine

When two NFRs collide and no specific rule above applies, resolve top-down:

> **Safety → Correctness/Reliability → Contract stability → Internal quality (testability,
> maintainability) → DX/convenience.**

Two caveats flip the middle:

- **Inside a freeze cycle, contract-stability rises above internal quality** — defer the cleanup
  rather than break the surface (why W4 is behavior-preserving).
- **Security can override contract-stability** — but only via an explicit, migrated, versioned
  change, never a silent one.

## Using this at the review gate

The supervisor asks: **does this change trade a higher NFR for a lower one?** If so, it must be
justified (e.g. security overriding compat, with a migration) or rejected. Concretely:

- No broadening of auto-approval without least-privilege, path-scoped scoping.
- No change to the observable serde/wire/CLI surface during the freeze unless the change's spec
  declares it (a versioned, migrated break).
- No telemetry or network egress of local content.
- Robustness (timeouts, poison, orphan cleanup) is required on the unattended and security paths,
  not optional.
