# CLI & Dev-Tool Design (git-paw reference)

Condensed from the [Command Line Interface Guidelines](https://clig.dev/). git-paw is a CLI
dev tool (`git paw …`), so these bind the CLI surface directly. Each rule notes git-paw's
current status where relevant.

## First principles
- **Human-first, but composable.** Primary output is usable by both humans and machines.
- **Consistency & discoverability.** Follow terminal conventions; lean on help, examples, and
  actionable errors to offset the learning curve.

## Arguments & flags
- **Prefer flags over positional args**; reserve positionals for one primary action.
- Provide short + long forms; use standard names: `-h/--help`, `--version`, `-v/--verbose`,
  `-q/--quiet`, `-f/--force`, `--json`, `-n/--dry-run`, `--no-input`, `-o/--output`.
- **Sensible defaults** — most users won't hunt for flags.
- **Order-independent** flags/args where possible.
- **Never take secrets via flags** (leak into `ps`/history) — files, stdin, or IPC only.
- Support `-` for stdin/stdout where piping makes sense.

## Output — stdout vs stderr (hard rule)
- **Primary/machine output → stdout; logs, errors, progress, messaging → stderr.** Never mix.
- **`--json`** for structured output (git-paw: `doctor --json`; extend to other read commands
  where an agent would consume it). `--plain` to drop human formatting when piped.
- **Respect the terminal:** detect TTY per-stream; disable color when not a TTY, when `NO_COLOR`
  is set, `TERM=dumb`, or `--no-color`; disable animations off-TTY (no CI "christmas trees").
- **Quiet by default, brief on success**; state what changed. Provide `-q/--quiet` for scripts.

## Errors
- **Actionable messages** — say what failed *and how to fix it* (git-paw: `doctor` remedy lines;
  `PawError` hint tests). Most important info last; red sparingly; group similar errors.
- Unexpected errors → debug info + how to report a bug; verbose logs to a file, not the terminal.

## Exit codes
- **0 on success, non-zero on failure**; map distinct codes to important failure modes (git-paw:
  `PawError` → exit-code mapping). Scripts depend on this.

## Interactivity
- **Only prompt when stdin is a TTY.** Off-TTY, fail with the flag to pass instead of hanging.
- `--no-input` disables all prompts.
- **Destructive actions confirm**, scaled to severity (mild = optional, moderate = prompt + offer
  dry-run, severe = type-the-name); `-f/--force` bypasses; non-TTY without `--force` errors with
  guidance. git-paw: `stop`/`purge` (note: `stop` currently doesn't prompt — reconciled in the
  spec-audit change).
- Don't echo passwords; keep Ctrl-C working.

## Configuration
- **Precedence: flags > env vars > project config > user config > system.** git-paw: repo config
  overrides user config; flags override both.
- **Follow XDG** for config/state (git-paw: session state in the XDG data dir).
- Don't store secrets in env vars; ask permission before modifying config you don't own; prefer
  creating new config over appending.

## Robustness & idempotency
- Validate input early, before any state change.
- Print something within ~100ms (avoid the appearance of hanging); show progress for long ops.
- Sensible network timeouts; be idempotent/recoverable; anticipate misuse (spaces in paths,
  case-insensitive FS, scripted wrapping — see the injection-hardening seam).

## Future-proofing
- **Additive changes**; warn before breaking, show the new usage, stop warning once switched
  (git-paw: the `--from-specs` → `--from-all-specs` hint pattern).
- Human output may evolve; **machine output (`--json`/`--plain`) is the stable contract** — script
  authors should use it.
- No catch-all/abbreviated subcommands that foreclose future names.

## Dev-tool ergonomics (git-paw specifics)
- **Self-diagnostics**: `doctor` (static preflight) + `selftest` (live smoke) — the "why won't it
  run?" answer in one command.
- **Single-binary distribution** (cargo-dist), easy install/uninstall, `--version` truthful.
- **Minimal, license-clean deps** (the AGENTS.md approved set + `cargo deny`).
- **Deterministic, composable output** so agents and scripts can chain commands.
- **No telemetry without consent** — git-paw's learnings mode ships a no-telemetry guarantee.
- **Cross-platform care**: macOS + Linux first-class; Windows via WSL (tmux is Unix-only).
