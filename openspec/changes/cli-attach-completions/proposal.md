## Why

`git paw` lacks a first-class reattach — users run raw `tmux attach -t paw-<project>`
— and has no shell completions, and the docs reference a phantom `git paw resume`
command that was never implemented (the consistency sweep flagged it). Small DX gaps
worth closing before the freeze.

## What Changes

- Add **`git paw attach`** — reattach the current terminal to the running session for
  the current repository (resolving the session by repo path, as `status` does, and
  invoking the existing tmux attach path).
- Add **`git paw completions <shell>`** — print a bash / zsh / fish completion script
  to stdout (via `clap_complete`), for installation through the shell's standard
  mechanism.
- **Strike the phantom `git paw resume`** from the docs; add **no** `resume` verb.
  Reattaching a running session is `git paw attach`; reviving a paused/stopped session
  remains `git paw start`.

Additive subcommands; existing behaviour unchanged. Not breaking.

## Capabilities

### New Capabilities

<!-- None — extends the existing CLI surface. -->

### Modified Capabilities

- `cli-parsing`: adds the `attach` and `completions` subcommands and pins that no
  `resume` subcommand exists (reattach is `attach`, revival is `start`).

## Impact

- **Code:** `src/cli.rs` (two additive `Commands` variants + help/`long_about`);
  dispatch in `src/commands/…` reusing session-by-repository resolution
  (`session-state`) and the existing `tmux::attach()`; completions generated from the
  clap command via `clap_complete`.
- **Dependencies:** adds **`clap_complete`** (first-party clap companion crate,
  permissive licence) — approved as part of the v0.16 DX scope. Add to the approved
  dependency table in `AGENTS.md`.
- **Enum-variant ripple:** none of the hazard enums — the new `Commands` variants are
  matched at the single dispatch site, not `BrokerMessage` or `SpecBackendKind`.
- **Backward compatibility:** additive subcommands; no config or state change;
  `git paw resume` remains (correctly) an unknown subcommand.
- **Docs:** CLI reference, README, and mdBook gain `attach` and `completions`; every
  `git paw resume` reference is removed.
