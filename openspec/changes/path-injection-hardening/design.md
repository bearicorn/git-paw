# Design — path-injection-hardening

## Context

Three sibling sites interpolate externally-controlled strings into tmux targets and
shell command bodies without sanitizing or quoting them. Worktree *directory* names
already go through `git::worktree_dir_name` / `git::branch_slug`; the session name, the
`pipe_pane` log path, and the `__dashboard` `send-keys` command are the un-hardened gap
(`.git-paw/v0.13.0-wave3-code-analysis-principal-engineer.md` §6 CF1). Fixing them
changes observable behavior for spaced/dotted inputs, so this is a bug-fix change with a
reproducing test per bug — not a refactor wave.

## Decisions

### D1 — A `SessionName` newtype with a smart constructor (the code-standards pattern)

Per the `code-standards` skill ("Domain newtypes with smart constructors for
injection-prone strings — `SessionName`, `BranchSlug`, `WorktreePath`; sanitize/quote
once at construction so downstream code can't interpolate a raw untrusted string"), the
session name becomes a `SessionName` newtype. Its only constructor,
`SessionName::from_project(project_name: &str)`, produces `paw-<slug>` where `<slug>`
keeps only tmux-target-safe characters and replaces the rest with `-`. A `SessionName`
value therefore *cannot* hold a tmux-unsafe string. It exposes `AsRef<str>` / `Display`
so it drops into the existing `TmuxCommand::new(&[...])` argv sites unchanged.

This is deliberately the same shape as the existing `worktree_dir_name` sanitizer, so
the three consumers (`SessionBuilder::build`, `resolve_session_name`, the supervisor
builder) share one sanitization boundary instead of three raw `format!("paw-{}", …)`
call sites.

### D2 — The tmux-safe character set

tmux uses `.` and `:` as separators inside a target (`session:window.pane`) and rejects
whitespace in a session name. The slug keeps ASCII letters, digits, `_`, and `-`, and
maps every other character (including `.`, `:`, and whitespace) to `-`. Case is
preserved (`My Project` → `My-Project`) to keep the name recognizable; tmux session
names are case-sensitive and case is not a separator. Consecutive separators are
collapsed and leading/trailing `-` trimmed so `my..app` → `my-app`, not `my--app`.
Empty or all-unsafe input falls back to the existing `project_name` default so a name
is always produced.

### D3 — `pipe_pane` shell-quotes the log path

`pipe_pane` builds a `/bin/sh -c` body `cat >> <path>`. The path is wrapped by a
single-quoting shell-quote helper (single-quote the string, escaping embedded single
quotes as `'\''`) so any space or metacharacter is literal. For a path with no special
characters the quoted form is byte-for-byte behavior-equivalent to today's output (the
shell strips the quotes), preserving existing dry-run expectations except for the added
quotes in the emitted command string.

### D4 — `__dashboard` command sent literally

The `__dashboard` command embeds `std::env::current_exe()`, whose path can contain a
space. The fix sends the command with `send-keys -l` (literal) followed by a separate
`Enter` key — the established pattern already used for boot-prompt injection
(`main.rs:1808/1814`, `tmux::build_send_keys_args`) — OR shell-quotes the binary path
inside the command string. Sending literally is preferred because it matches the
existing prompt-injection seam and needs no per-argument quoting. The follow-up `Enter`
must be a separate `send-keys` invocation (a buffered first Enter does not submit — see
the local learning on send-keys nudges).

### D5 — No frozen surface touched

The session name is a runtime tmux target, not a persisted wire/config contract.
`Session.session_name` on disk simply records the sanitized value from first boot; no
serde shape, config key, or broker message changes. This keeps the change clear of the
v1.0.0-frozen danger zones listed in `code-standards`.

### D6 — Test-first per bug (behavior change, not refactor)

Because each site changes observable behavior, a reproducing test is written FIRST for
each — a `My Project` / `my.app` session name that today produces a corrupt target, a
spaced repo path whose `pipe_pane` logging breaks, and a spaced binary path whose
dashboard command never launches — then the fix makes each pass. This follows the
code-standards rule that a bug fix ships with a reproducing test, never folded into a
refactor.

## Non-goals

- No change to how worktree directory names are sanitized (already correct).
- No new CLI flag, config field, or broker wire shape.
- No broader shell-escaping audit of unrelated `Command` sites — scoped to the three
  CF1 sites and the newtype/helper that generalize their fix.

## Risks

- **Low.** Additive newtype + quoting; behavior-preserving for well-formed inputs. The
  one residual risk is a dry-run test pinned to the exact unquoted `pipe_pane` string;
  those expectations are updated in the same change to reflect the quoted (correct)
  form. Session-name collision resolution (`resolve_session_name`'s `-2`/`-3` suffixes)
  composes with the newtype: the suffix is appended to the already-sanitized base.
