## Why

`git paw replay` exists to recreate a session, and it cannot. It reads a `pipe-pane` byte capture of
a full-screen agent TUI and strips the ANSI codes (`replay::strip_ansi` → `replay_stripped`). The
TUI uses the alternate screen, cursor-addressed redraws, spinners and thinking-token counters, so
the stripped text is hundreds of repaint fragments in no readable order. `--color` pipes the same
bytes through `less -R`, which cannot interpret cursor addressing either. The peer-4-poker dogfood
(2026-08-31) confirmed it unusable.

What people want from "replay" is an audit log: what each agent did, and when. git-paw already holds
that data in structured form, and `session-record-capture` makes it durable per run: the broker
event stream, each agent's commits, and the agent CLI's own transcript (located through the recorded
`cli_session_id`). This change makes that **semantic timeline** the default output of
`git paw replay`. The faithful animated replay is the optional second layer,
`replay-terminal-cast`.

**Freeze tie-in.** `replay` is a shipped command that freezes into the stable contract at v1.0.0.
This cycle makes it work before the freeze. If this change misses the v0.24.0 feature freeze,
v1.0.0 hides `replay` (`#[command(hide = true)]`) and registers it deprecated rather than freeze its
current stripped-capture behaviour (v1.0.0 `cli-contract-freeze`, Open Questions). The animation
layer may slip without that consequence, because `--cast` is additive.

## What Changes

- **BREAKING** `git paw replay` prints a chronological timeline of the most recent session record:
  session start/end, agent attach/detach, broker events, commits (short SHA + subject) and CLI
  transcript turns (user prompt, assistant message first line, tool call name + target).
  `git paw replay <agent>` filters to one agent. Consecutive identical status heartbeats collapse to
  one counted line. Untrusted text (agent messages, commit subjects, transcript text) is sanitized of
  escape and control sequences before printing.
- `<agent>` becomes optional. Running with no argument replays the whole session.
- `--json` emits the same timeline as one JSON document under v0.21's `cli-machine-output`
  contract (`schema_version` 1), and `replay` joins that capability's JSON roster.
- `--raw` writes an agent's captured terminal bytes, unmodified, to stdout. This is the migration
  for `--color` (`git paw replay <agent> --raw | less -R`), and it replaces `--color` as the named
  escape-sequence exception in v0.21's `core-terminal-output`.
- **BREAKING** `--color` and the ANSI-stripped default display are removed.
- `--list` lists session records newest first, with their agents and which sources each holds.
- The CLI transcript is read in place through a per-CLI **transcript source** Strategy with a
  Null-Object default. The built-in Claude-format source finds `<cli_session_id>.jsonl` under the
  CLI's config directory. git-paw never copies transcripts.
- Legacy records (no manifest) cannot produce a timeline. Replay says so and points at `--raw`.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-logging`: replay's default output becomes the semantic timeline. These requirements
  change: `--list` output, most-recent selection, agent matching, empty and missing handling, and
  the ANSI sanitizer, now repurposed for untrusted timeline text. The stripped and `--color` display
  requirements are removed. New requirements cover timeline sources, ordering and collapse, the
  transcript strategy, `--json`, `--raw` and legacy records.
- `cli-parsing`: the `replay` subcommand's arguments (optional `<agent>`, `--json`, `--raw`; no
  `--color`).
- `cli-machine-output`: `replay` joins the JSON roster; its timeline and `--list` documents carry
  `schema_version` like every roster document.
- `core-terminal-output`: `replay <agent> --raw` replaces the removed `replay --color` as the one
  output allowed to carry escape sequences when piped or under `NO_COLOR`.

## Impact

- **Code:** `src/replay.rs` becomes `src/replay/`. The pure core `timeline.rs` builds, orders,
  collapses and sanitizes entries. The source adapters are the record reader, the commit history
  read through `src/git.rs` and `CommandRunner`, and the transcript sources. Text and JSON rendering
  happen in the entry layer. `src/commands/replay.rs` (the `cmd-library-surface` library function
  after v0.18) returns a typed outcome, and `src/cli.rs` `Replay` changes.
  `dashboard/broker_log.rs::{type_short, derive_summary}` and the heartbeat key move to
  `src/broker/summary.rs` so the dashboard and the timeline share one summary, with re-exports
  keeping the dashboard unchanged. `cmd_replay`'s non-test `expect()` disappears, because a missing
  agent is now valid.
- **Enum-variant ripple (`BrokerMessage`):** no variant is added. The exhaustive `type_short` /
  `derive_summary` matches move file, so AGENTS.md §7's compile-forcing list must name
  `src/broker/summary.rs` instead of `src/dashboard/broker_log.rs` for those two functions.
- **Tests:** replay unit tests are rewritten around the timeline core, and `tests/cli_tests.rs`
  replay parse tests change. New fixture-based transcript-parser tests and an end-to-end replay of a
  recorded session are added.
- **Docs:** `cli-reference.md` (`git paw replay`, and its two JSON documents with
  `schema_version`), `user-guide/session-logging.md` (Replaying →
  timeline, `--json`, `--raw`), README CLI table + feature bullets, `architecture.md` (Replay row),
  `specifications/README.md` (session-logging summary line), and `--help` / `long_about`.
- **Security / safety:** the change is read-only. Transcript paths are built only from a validated
  UUID-shaped `cli_session_id`. Untrusted text is sanitized before it reaches the terminal. Tool
  *results* are never rendered, which limits secret exposure. `--raw` output is unsanitized by
  design, and the help text says so.
- **Dependencies:** none (serde_json, chrono, which are approved).
- **Cross-version dependencies:**
  - `session-record-capture` (this release) must land first: the record layout, manifest and
    `events.jsonl`.
  - v0.18 `cmd-library-surface`: replay is a library function with a typed outcome and rendering
    in dispatch.
  - v0.18 `broker-host-separation`: the event record is written by the `__broker` host.
  - v0.19 `cli-provider-record` (capability `cli-provider`) and `cli-profile-threading`: the
    transcript source is resolved from the agent's resolved CLI profile, which also covers custom
    Claude variants (GP-29). If that slips, resolution falls back to the CLI name `claude`,
    mirroring `built_in_session_id_args`.
  - v0.21 `uniform-json-output` (`cli-machine-output`): the timeline and `--list` documents carry
    `schema_version` 1 and follow its contract. This change adds `replay` to the roster (MODIFIED
    *The JSON roster is fixed and guarded*, built on the roster as v0.21's `finish-command` leaves
    it). v0.21 wire-DTO split: the timeline JSON is a wire type.
  - v0.21 `terminal-output-robustness` (`core-terminal-output`): its `replay --color` exception
    becomes `replay <agent> --raw` (MODIFIED *Human-readable output carries no escape sequences when
    not on a terminal*). `replay-terminal-cast` adds `--cast` to the same exception.
  - v1.0.0 freeze: see the tie-in above.
  - No v0.17.0 or v0.18.0 change modified `session-logging` or the `Replay subcommand` requirement.
