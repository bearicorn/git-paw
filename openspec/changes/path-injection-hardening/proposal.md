## Why

git-paw interpolates raw, externally-controlled strings — the repository directory
name and the installed-binary path — directly into tmux targets and shell command
bodies. Worktree *directory* names are already sanitized (`git::worktree_dir_name`,
`git::branch_slug`), but three sibling sites were never hardened and fail on
well-formed real-world inputs:

1. The `paw-{project_name}` **tmux session name** is unsanitized
   (`src/tmux.rs:390,615,1119` via `git::project_name` `src/git.rs:164`). A repo
   directory like `my.app` or `My Project` yields a session name that tmux either
   refuses to create or whose `session:0.N` pane targets become ambiguous — tmux
   reserves `.` and `:` as window/pane separators in a target, so a `.` in the
   session name corrupts every scoped pane command.
2. `pipe_pane` interpolates an **unquoted path** into a `/bin/sh -c` body
   (`src/tmux.rs:230-237`, `format!("cat >> {}", log_path.display())`). A space in
   the repo path silently breaks per-pane logging.
3. The `__dashboard` launch command is sent **unquoted via `send-keys`** without
   `-l` at six sites (`src/main.rs:632,1397,2017,2188,2409,2481`, delivered through
   `PaneSpec.cli_command` and the pause/resume path at `:2226`). A spaced
   installed-binary path (e.g. `/Users/My User/bin/git-paw`) never launches the
   dashboard.

These are latent, user-facing edge-case bugs surfaced by the v0.13.0 code analysis
(`.git-paw/v0.13.0-wave3-code-analysis-principal-engineer.md` §6 CF1). Each **changes
observable behavior** (spaced/dotted names now work), so — per the code-standards
"a behavior change is not a refactor" rule — this ships as its own spec+test-gated
change with a reproducing test written FIRST for each of the three bugs.

## What Changes

- **Sanitize the tmux session name** to a tmux-safe slug at construction, matching the
  existing worktree-dir sanitization. `my.app` → `paw-my-app`, `My Project` →
  `paw-My-Project`. Well-formed names (`git-paw` → `paw-git-paw`) are unchanged.
- **Quote the path** interpolated into the `pipe_pane` `/bin/sh -c` body so spaces and
  shell metacharacters in the repo path are handled literally.
- **Send the `__dashboard` command literally** (`send-keys -l` + a separate `Enter`) or
  shell-quote its binary path so a spaced installed-binary path launches.
- **Introduce domain newtypes with smart constructors** (`SessionName`, plus a
  shell-quoting helper) per the `code-standards` skill: sanitize/quote **once at
  construction** so raw untrusted strings can no longer be interpolated downstream. This
  closes the whole session-name / path-quoting bug class, not just today's three sites.

Behavior-preserving for well-formed inputs; the only behavior change is that
spaced/dotted repo names and spaced binary paths now work.

## Capabilities

### New Capabilities
- `safe-process-invocation`: the contract that names/paths/commands crossing into tmux
  and shell contexts are sanitized, quoted, or sent literally at a single construction
  boundary (the `SessionName` newtype + shell-quoting helper), so untrusted directory
  and binary-path strings cannot corrupt a tmux target or shell command.

### Modified Capabilities
_None._ This is a hardening of currently-latent behavior; it adds a new capability and
changes no existing requirement's contract for well-formed inputs.

## Impact

- **Code:**
  - `src/tmux.rs` — session-name construction (`SessionBuilder::build` ~:390,
    `resolve_session_name` ~:615, the supervisor builder ~:1119) routes through a
    `SessionName` smart constructor; `pipe_pane` (~:230) shell-quotes `log_path`.
  - `src/git.rs` — `project_name` stays as-is (raw dir name); a new `SessionName`
    newtype (co-located with the existing `worktree_dir_name` / `branch_slug`
    sanitizers) owns the `paw-<slug>` sanitization.
  - `src/main.rs` — the six `__dashboard` command sites send the command with `-l`
    (plus a follow-up `Enter`) or via a shell-quoted binary path.
- **Not enum-variant ripple:** no `BrokerMessage` / `SpecBackendKind` variant is added
  or removed.
- **Frozen surfaces untouched:** no config key, no broker wire shape, no serde
  representation changes. The session name is a runtime tmux target, not a persisted
  contract; `Session.session_name` on disk simply carries the sanitized value from
  first boot.
- **Docs:** no new CLI surface or config field. A short note in the mdBook
  troubleshooting/limitations section that repo names with `.`/spaces are now
  supported; `mdbook build docs/` must pass.
- **Tests:** three reproducing tests (one per bug) written first, then unit tests over
  the `SessionName` constructor and the shell-quoting helper; a dry-run assertion that
  the built tmux commands carry a sanitized session name and a quoted pipe-pane path.
