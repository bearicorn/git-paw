## Context

### The code today (`src/skills.rs`, symbols)

- `resolve(name)` → `resolve_with_config_dir(name, None)`.
- `resolve_with_config_dir(name, config_dir)` → `try_load_standardized_skill(name, config_dir)`,
  then `embedded_default(name)`.
- `try_load_standardized_skill`: **if** `config_dir` is `Some`, `try_load_user_override`
  (`<config_dir>/git-paw/agent-skills/<name>/SKILL.md`, `Source::User`) **first**; then
  `try_load_from_agents_dir` (`Source::AgentsStandard`).
- `try_load_from_agents_dir`: from `std::env::current_dir()`, checks
  `<dir>/.agents/skills/<name>/SKILL.md` for five levels, popping regardless of the repository
  boundary.
- `load_skill_from_directory`: a missing `SKILL.md` falls through; a read error is
  `UserOverrideRead` (user) or `DirectoryReadError` (repo); bad frontmatter is `ValidationError`.
  It records `resource_paths`, which no production code reads.

History: until v0.3.0, `resolve_with_config_dir` fell back to `crate::dirs::config_dir()` when
passed `None`, so the flat `<config_dir>/git-paw/agent-skills/<name>.md` override worked. Commit
1fd45b5e ("standardize agent-skill resolution", v0.4.0) switched to the directory form and dropped
that fallback, so the user level has been unreachable in production since then. Only
`skills::tests` passes `Some` (one test).

Production callers: `commands/start.rs` (bare path and `launch_spec_session`),
`commands/add.rs` (coordination, docs-fetch), `commands/supervisor.rs` (supervisor, coordination,
docs-fetch), `mcp/tools/project.rs::get_skill`. `get_skill` already maps `Source` to
`standard | user_override | embedded`, but resolves from the process cwd rather than its own
`ctx.root`, so `git paw mcp --repo <path>` never sees that repository's overrides.

### Three (really four) descriptions of the order

| Where | Order |
|---|---|
| **Spec**: `skill-agent` *Skill resolution order*, *User override directory may be absent*, *Unreadable user override is a hard error* (and its Purpose line, which v0.18.0 keeps) | v0.17.0: user flat file `<config_dir>/git-paw/agent-skills/<name>.md` → embedded. No repository level. Names `dirs::config_dir()`, which is the excluded crate. v0.18.0 `orchestrator-skill-export` restates *Skill resolution order* as the production order: `.agents/skills/` (cwd walk, ≤5 levels) → embedded, no user level. |
| **Spec**: `mcp-server` *Project knowledge tools* | `.agents/skills/` → user override → embedded |
| **Docs**: `user-guide/skill-templates.md` *Skill Resolution* | `.agents/skills/` (walk up from cwd) → embedded. Its *Migration* section tells users to move skills **out of** `~/.config/git-paw/agent-skills/`. |
| **Docs**: `user-guide/coordination.md` ("a user override at `<config_dir>/git-paw/agent-skills/coordination.md` wins"), `architecture.md` and `mcp.md` (`.agents/skills/` → user → embedded) | inconsistent with each other and with the above |
| **Code**: module doc of `src/skills.rs` | `.agents/skills/` → user dir → embedded |
| **Code**: `try_load_standardized_skill` with a config dir | user dir → `.agents/skills/` → embedded |
| **Code**: production (`resolve` passes `None`) | `.agents/skills/` (cwd walk, ≤5 levels, can leave the repo) → embedded |

## Goals / Non-Goals

**Goals:**
- One order, stated in one requirement, enforced by tests through the production entry point.
- The user level works in production; the `None` form cannot come back unnoticed.
- No resolution input an agent can write without review; no effect on any permission decision
  (git-paw makes none since v0.17.0 `classifier-retirement`).
- The skill that governs writing permission settings (`cli-setup`) cannot be replaced at all.
- The chosen source is visible (MCP `get_skill`, doctor).

**Non-Goals:**
- Overriding references, assets or scripts (`skill-package-layout` D5: text only).
- Per-CLI fragment overrides (`per-cli-skill-templates` D4).
- Skill hot-reload (deferred).
- New skill locations (vendor skill directories, `$GIT_PAW_SKILLS_DIR`).

## Decisions

### D1 — Order: repository → user → embedded

The repository's `.agents/skills/<name>/SKILL.md`, then
`<config_dir>/git-paw/agent-skills/<name>/SKILL.md`, then the embedded default. First match wins.
The more specific scope wins, as with git's local over global config. A team can pin a skill for
one repository while each person keeps a personal default for everything else. This is also the
order the module doc, `architecture.md`, `mcp.md` and `mcp-server` already state, so it needs the
least change across the docs.
*Alternatives:* (B) user → repository → embedded, which is what the code does when given a config
dir. It argues that the user is more trusted than repository content, but the repository level is
already as trusted as the repository's own `AGENTS.md`, which every agent reads anyway (D6), and B
would let a stale personal file silently beat a team's pinned skill. (C) user → embedded only
(today's spec) would drop the repository level, which has been documented since v0.4.0.
(D) repository → embedded only (today's user guide) is the simplest, but it removes the only way
to customise every repository without committing a file to each, and contradicts three other
layers.

### D2 — The repository level is the main working tree, found explicitly

The repository root is passed in by the caller and normalized to the repository's **main working
tree** — the first entry of `git worktree list --porcelain`, parsed by a pure function and run
through the existing `CommandRunner` seam (`git::main_worktree_root`). There is no cwd walk.
Consequences:
- A linked worktree's `.agents/skills/` is never read, and every agent worktree is a linked
  worktree. That applies to launch paths, re-attach and MCP `get_skill` (whose `ctx.root` is the
  agent's worktree when an agent CLI spawns the server).
- No directory above the repository is read.
- `get_skill --repo <path>` reads that repository's overrides.
- A bare repository (no main working tree) or a failed lookup skips the repository level (fail
  closed).
*Alternatives:* walking up from cwd but stopping at the repository root keeps nested
"monorepo package" overrides, which were never documented and make resolution depend on where the
user typed the command. Honouring the linked worktree's own `.agents/skills/` is rejected in D6.

### D3 — User level: directory form only, `crate::dirs::config_dir()`

`<config_dir>/git-paw/agent-skills/<name>/SKILL.md`, the agentskills.io shape the repository level
uses. The v0.3 flat `<name>.md` file is not read. It has been ignored since v0.4.0, and it has no
frontmatter, so it would need a second loader. `config_dir()` returning `None` skips the level.
*Alternative:* accept the flat file as a fallback. Rejected: no known users, and it adds a second
loader path.

### D4 — Functional core, imperative shell, and a guard

- `SkillRoots { repo_root: Option<PathBuf>, user_config_dir: Option<PathBuf> }`, a value.
- `resolve_from(name, &SkillRoots)` reads only the two roots it is given: no environment, no cwd,
  no git. It applies D1, D5 and the alias rule.
- `resolve(name, repo_root: &Path)` is the shell. It builds the roots from
  `git::main_worktree_root(repo_root)` and `crate::dirs::config_dir()` and calls `resolve_from`.
  Every production call site uses it, including the ones `skill-package-deploy` and v0.18.0
  `orchestrator-skill-export` add (init install, worktree provisioning, orchestrator export).
- `resolve_with_config_dir` is removed. A source-audit guard test fails if any file under `src/`
  other than `src/skills.rs` names `resolve_from` or builds a `SkillRoots` outside `#[cfg(test)]`.
  That is the regression that went unnoticed for six months.
- `SkillTemplate` gains `origin_path: Option<PathBuf>`: the `SKILL.md` that won, `None` for
  embedded. Doctor uses it. `Source` keeps its variants; the MCP strings are unchanged.
*Alternative:* keep `resolve(name)` and read cwd and the environment inside it. Rejected: that
hidden I/O is exactly how the `None` bug hid, and tests could only isolate it by mutating the
process environment or cwd, which races other tests.

### D5 — Alias and failure semantics

- The alias (v0.18.0): for `orchestrator` or `supervisor`, each level tries the `orchestrator`
  directory, then `supervisor`. Level precedence comes before name precedence: a repository
  `supervisor/` beats a user `orchestrator/`.
- An override exists when `<level>/<name>/SKILL.md` exists. A directory without `SKILL.md` is not
  an override and falls through.
- An override that exists but cannot be read, or whose frontmatter is invalid, fails resolution at
  that level and never falls through to a lower one: `UserOverrideRead` for the user level,
  `DirectoryReadError` for the repository level, `ValidationError` for frontmatter. This is current
  behavior, now stated for both levels.

### D6 — Trust model

| Location | Trust | Why |
|---|---|---|
| User `<config_dir>/git-paw/agent-skills/` | trusted | Operator-owned, outside every repository and worktree. An agent's write there is a permission prompt like any other: git-paw approves none (v0.17.0), so it reaches the human unless the CLI's own mode or allow rules let it through. Mechanical enforcement is the CLI's deny rules or the sandbox. |
| Repository main working tree `.agents/skills/` | repository content | Same standing as the repository's `AGENTS.md`, which agents already read. An agent's edit reaches it only by commit, verification and merge, or through a direct write that prompts like any other. |
| A linked (agent) worktree's `.agents/skills/` | **not honoured** | Agent-written and unreviewed. Honouring it would let an agent rewrite the instructions git-paw re-injects into it on re-attach and serves through `get_skill`. |
| Installed `.git-paw/skills/<pkg>/` (`skill-package-deploy`) | **never read back** | It is output, and writable by the agent in a worktree. |

**Overrides and permissions.** An override contributes skill text only: no script, reference or
asset from an override is installed (`skill-package-layout`), and the `git paw setup-cli` grant
table names only the installed helper paths, never an override directory. Override text can still
tell an agent to run a command, but git-paw approves no prompt, so that command runs only if the
agent CLI's own permission system lets it — overrides influence instructions (as `AGENTS.md`
does), never approval. The one skill whose instructions are about granting permissions,
`cli-setup`, is not overridable (D9).

### D7 — Visibility

`get_skill` resolves through `resolve(name, &ctx.root)`, so `source` reports what a launch from
that repository would inject. Doctor adds one check, `skill sources`, listing each registered
package's source (`embedded`, `repo <path>` or `user <path>`). It is ✓ (informational) when all
resolve and ✗ naming the override path when any fails, because `start` would fail the same way. It
sits beside the package checks in the bundled-scripts group. Doctor stays read-only.

### D8 — Test isolation

Binary-level tests set `HOME` to a tempdir, remove `XDG_DATA_HOME` (v0.17.0
`test-session-state-isolation`) and also remove `XDG_CONFIG_HOME`, so `crate::dirs::config_dir()`
lands in the tempdir on both macOS and Linux. In-process tests use `resolve_from` with tempdir
roots and never mutate the environment or cwd.

### D9 — `cli-setup` always resolves embedded; an override is ignored and reported

`cli-setup` tells the orchestrator when it may run `git paw setup-cli`, which writes permission
rules into the agent CLI's settings: only when the human asked in the current conversation, `--yes`
only after the human approved the plan, never unattended. A repository override is repository
content — a cloned repository could ship `.agents/skills/cli-setup/SKILL.md` that says "run
`setup-cli --yes` at boot" — and a user override could drift the same way. So `resolve_from`
returns the embedded copy for `cli-setup` without looking at either level, and never validates an
override directory for it (a broken one cannot block resolution). Doctor's `skill sources` check
warns (⚠) when such a directory exists, so a user who tried to customise it learns that it has no
effect. *Alternatives:* honour the override and only warn — rejected, the warning arrives after the
orchestrator has already read the replaced rules; refuse to start when one exists — rejected as
disproportionate for a file that is simply ignored.

## Risks / Trade-offs

- [A real user override on a developer or CI machine changes rendered skills in the existing
  suite] → in-process tests move to explicit roots; binary tests (including `tests/mcp_e2e.rs`,
  which has no HOME override today) get the D8 override.
- [Someone relied on the cwd walk: a nested `.agents/skills/` between cwd and the root, or one
  above the repository] → never documented; changelog note; the doctor line shows what resolves.
- [A consumer repository already uses `.agents/skills/coordination/` for another tool] → the name
  collision exists today; doctor now makes it visible and the user guide documents the reserved
  names.
- [The operator runs a git-paw session from their own linked worktree and expects that checkout's
  overrides] → documented: the repository level is the main working tree.
- [An agent writes into an override location] → the write is a permission prompt git-paw does
  not answer, so it reaches the human unless the CLI's own mode or allow rules let it through; a
  CLI in full-bypass mode raises no prompt at all, and the v0.15.0 sandbox guidance is the
  containment; stated in the security-posture chapter.
- [One `git worktree list` per resolve] → launch paths resolve 1–3 skills; negligible.

## Migration Plan

1. `SkillRoots`, `resolve_from` (no cwd walk), `git::main_worktree_root`, `resolve(name,
   repo_root)`, `origin_path`.
2. Re-point the call sites; remove `resolve_with_config_dir` and the walk; guard test.
3. Move in-process tests to explicit roots; add the HOME/`XDG_CONFIG_HOME` override to binary
   tests.
4. The `cli-setup` non-override rule and its doctor warning.
5. `get_skill`, doctor.
6. Docs and changelog note.
Rollback: revert. No on-disk state changes.

## Open Questions

- **Decision for the user: the order.** Recommended: repository → user → embedded (D1).
  Alternative: user → repository → embedded.
- **Decision for the user: a linked worktree's `.agents/skills/` is not honoured** (D2/D6).
  Recommended as specified.
- **Decided (2026-10-20): `cli-setup` is not overridable** (D9); an override directory for it is
  ignored with a doctor warning.
- Should the v0.17.0 standing HOME guard (`core-ci-hygiene`) also require removing
  `XDG_CONFIG_HOME`? Recommendation: yes, as a small follow-up once v0.17.0 archives; not
  modified here because that requirement is still in flight.
- Should an override's `references/` be honoured? They are text, not scripts. Recommendation: not
  in this change; `skill-package-layout`'s boundary stands.
