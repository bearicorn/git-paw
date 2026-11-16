## Context

**Absent CLIs (DF-03).** `detect::resolve_custom_clis_in` (`src/detect.rs:199-226`) drops a
`[clis.*]` entry whose command does not resolve and prints
`warning: custom CLI '<name>' not found at '<command>', skipping` (`src/detect.rs:218-221`). Doctor's
`probe_clis` (`src/doctor.rs:1977-1995`) calls `detect_clis`, so that line lands on stderr above the
report, while `check_clis` (`src/doctor.rs:764-790`) only reports "N available" (✓) or "none" (⚠). A
`default_cli` naming an absent *known* CLI is not reported at all. At launch, a `default_cli` that is
not in the detected list just does not pre-select (`src/interactive/resolver.rs:1260-1275`), and the
non-prompt tiers (`default_spec_cli`, presets, profiles; `src/worker.rs:97`) hand the name to the
pane, which fails with "command not found". Doctor already enumerates the configured worker CLIs in
`configured_worker_clis` (`src/doctor.rs:2446-2455`) and `probe_sandbox_mode`
(`src/doctor.rs:1870-1938`).

**Broker port (DV-01).** `BrokerConfig.port` is a plain `u16` with serde default 9119
(`src/config/broker.rs:63-65`, `:82-92`, `:120-122`), so the model cannot tell "unset" from "9119".
`broker-runtime`'s stale-broker probe refuses a port held by another repository's broker and tells
the user to set a distinct `[broker] port`; doctor's `probe_broker` (`src/doctor.rs:2420-2444`)
reports the same clash as ⚠ with that remedy (`src/doctor.rs:1360-1365`). The root skills render the
broker URL from `config.broker.url()` (`src/init.rs:261`), and the helpers read `[broker] port` from
the config with 9119 as their fallback (`assets/agent-skills/coordination/scripts/broker.sh:12-13`;
`config-schema-versioning` design D7 lists `[broker].port` / `bind` as script-read keys). The
template documents `# port = 9119` (`src/config/mod.rs:1239-1243`). `broker-runtime` already injects
`GIT_PAW_BROKER_URL` into the tmux session.

**Relative command (PK-20).** Detection accepts an absolute path that exists, otherwise runs `which`
with a reduced `PATH` (`src/detect.rs:130-165`); `which` resolves a `./x` or `scripts/x` against the
git-paw process's current directory, so it works from the repository root and fails from a
subdirectory or another worktree. `add-cli` already stores an absolute path (`src/config/cli.rs:205-212`).
The load only rejects an empty command (`src/config/mod.rs:808-819`). Doctor already resolves the
main checkout for a repository-relative path (`src/doctor.rs:2109-2110`).

**Spec Kit directory (PK-02).** `SpecKitBackend::default_dir` returns `.specify/specs`
(`src/specs/speckit.rs:94-96`); init's interactive choice writes each backend's `default_dir`
(`src/init.rs:901-903`, `:937-940`) and the non-interactive comment says
`speckit: ".specify/specs"` (`src/init.rs:908-911`). Current Spec Kit keeps features in
`specs/<NNN-feature>/` at the repository root; `.specify/` holds its memory, scripts and templates.

## Goals / Non-Goals

**Goals:** a committed config that is portable across machines and repositories; doctor says what
will not work on *this* machine, with a remedy; old configs load and behave the same where they
pinned a value.

**Non-Goals:** a per-machine override file (`.git-paw/config.local.toml`) — open question 1; scanning
for free ports outside the derived range; changing which file wins a merge (repo still overrides
global); validating CLI names against configuration (that is `config-check-command`'s D3).

## Decisions

### D1 — Configured CLIs are checked rows, detection never prints under doctor
Detection returns the unresolved custom entries as data (`Vec<UnresolvedCli { name, command }>`)
next to the resolved list; the stderr line moves to the callers that want it (picker, `start`).
Doctor adds one CLIs-group row per configured CLI id (deduplicated, sorted, each naming the key that
set it): ✓ "resolves to <path>", ⚠ "<id> (from <key> in <file>) does not resolve on this machine"
with the remedy "install it, set `[clis.<id>] command` to where it lives here, or pass `--cli`". ⚠,
not ✗: a manual launch can still pick another CLI.
*Alternative:* ✗ when the CLI is the only way an unattended launch can run — rejected for now; the
launch itself refuses (D2), and doctor stays a diagnosis.

### D2 — Launch: picker notices, non-prompt tiers refuse
`default_cli` / `default_profile` only pre-select, so an absent one is dropped with a one-line
notice and the picker opens with no pre-selection. `default_spec_cli`, a preset's `cli` and a profile's
`cli` launch without a prompt, so an absent one refuses the launch before any worktree is created
(no partial session), naming the CLI, the key and the remedies. A CLI passed explicitly with `--cli`
keeps its pass-through behaviour (`cli-resolution` "An explicit `cursor` CLI still passes through").

### D3 — Derived port: FNV-1a of the main working tree path into 9120–9919
When neither file sets `[broker] port`, `port = 9120 + fnv1a64(canonical main working tree path) % 800`.
The main working tree (not the current worktree) keeps root and agent worktrees on one port. FNV-1a
is fixed by this spec so the value never changes between releases (Rust's `DefaultHasher` may).
9119 stays outside the range, so a derived port never collides with a pinned or pre-0.21 default.
The internal `BrokerConfig.port` becomes `Option<u16>` with `effective_port(repo_root)`; the wire
type keeps serializing only a pinned value (fits `config-schema-versioning` D4).

### D4 — Collision-aware at start only, then recorded
`git paw start` probes the derived port with the existing stale-broker probe: this repository's
broker → reattach; free → bind; another repository's broker or a foreign service → try the next port
in the range (wrapping), at most 8 attempts, then fail naming `[broker] port`. The chosen URL is
written to the session receipt (`broker_url`), and `status`, `add`, the dashboard, doctor and the
`GIT_PAW_BROKER_URL` injection read it from there while the session exists. A pinned port never
moves. Readers that find a receipt without `broker_url` (written by an older binary) use 9119 when
the config pins nothing, which is what that binary used.

### D5 — Helpers: environment first, then the binary
`discover_broker_url` in `_paw_common.sh` uses, in order: `GIT_PAW_BROKER_URL` when set; a pinned
`[broker] port`/`bind` from the config (today's read); `git paw __broker-url` (prints the effective
URL for the current repository, following the `__prompt-profile` precedent at
`src/main.rs:739-752`); 9119 last. The shell never re-implements the hash.

### D6 — Relative command against the main working tree
A `command` with a `/` that is not absolute is joined to the main working tree (the directory that
holds `.git-paw/config.toml`'s repository) and the launch line uses the absolute result, so every
worktree runs the same wrapper the config names. A bare name keeps the `PATH` lookup. The same rule
applies to a relative command in the global file (documented: it is relative to whichever repository
runs it).

### D7 — Spec Kit default `specs`, legacy fallback
`default_dir` becomes `specs`. When `[specs] type = "speckit"` (or `--specs-format speckit`) has no
`dir`, the scanner uses `specs`, unless `specs/` does not exist and `.specify/specs/` does — then the
legacy directory, so a 0.20 config that relied on the old default keeps scanning. An explicit `dir`
is used verbatim. This picks a directory for a format the config already chose; it never infers the
format.

## Risks / Trade-offs

- [A repository moved on disk gets a new derived port] → acceptable: the port only matters while a
  session runs, and the receipt pins the live one; doctor shows the derived value.
- [Two repositories hash to the same port] → D4 steps past it at start; the cost is one probe.
- [Old helper copies in a worktree read 9119] → helpers are refreshed on every launch; a stale copy
  with no `GIT_PAW_BROKER_URL` is already flagged by doctor's bundled-scripts check.
- [Users who relied on the implicit 9119 from outside git-paw, e.g. a hand-written curl] → CHANGELOG
  note; pin `[broker] port = 9119` to keep it.
- [A relative `command` points into the repository, so a branch can change what runs] → same trust
  model as `[worktree.hooks]` and a committed `command` today; the main working tree is used, not the
  agent's worktree, so an agent's edit to its own copy does not change its launcher.

## Migration Plan

1. Detection returns unresolved entries; doctor rows; launch rule (D1, D2).
2. Relative command (D6) and Spec Kit default (D7) — independent, small.
3. `port: Option<u16>` behind `effective_port`, all readers switched (no behaviour change while the
   derivation returns 9119); then the derivation, start-time stepping and `broker_url` in the
   receipt; then the helper and `__broker-url`.
Rollback per step. A 0.20 binary ignores `broker_url` in a receipt (unknown field).

## Open Questions

1. Add a gitignored per-machine override (`.git-paw/config.local.toml`, merged last) so a machine
   can replace a committed `default_cli` without `--cli` on every launch? Recommended as a follow-up;
   out of scope here.
2. Does the session receipt already carry the broker URL anywhere (task 3.1 checks)? If so, reuse it
   instead of adding `broker_url`.
3. Range 9120–9919 and 8 attempts are proposals; confirm they avoid common local services.
