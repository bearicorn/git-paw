## 1. Coordination skill — worktree environment orientation

- [x] 1.1 Add a stack-agnostic worktree-environment orientation section to `assets/agent-skills/coordination.md`: install artifacts (gitignored; maybe provisioned by an `on_create` hook; run your stack's install if missing; don't touch the repo-root copy), FS-confinement is policy (don't probe `Operation not permitted` outside the worktree — adapt), setuid `ps` fails under a sandbox (expected)
- [x] 1.2 Skill-content test (`*_skill_content.rs`) asserts the section: `Operation not permitted` framed as policy + "don't probe" guidance; install-if-genuinely-missing; `ps`-under-sandbox note
- [x] 1.3 Mirror the additions in `docs/src/user-guide/coordination.md` (the coordination-mirror requirement)

## 2. Supervisor skill — tiered permission model

- [x] 2.1 Add the tiered permission model + safe-command policy section to `assets/agent-skills/supervisor.md`: CLI-native check → `git paw __classify` (authoritative, single-source) → escalate; summarise safe/danger classes as orientation
- [x] 2.2 Skill-content test asserts the ladder order and that it directs the supervisor to `git paw __classify` as the authority
- [x] 2.3 Mirror the additions in `docs/src/user-guide/supervisor.md`

## 3. Export-agnosticism + single-source

- [x] 3.1 Run the export-agnosticism conformance test (`tests/agent_skills_conform.rs`); the new content contains no hard-coded stack package-manager command, artifact name, or toolchain verb
- [x] 3.2 Confirm the supervisor safe-command section carries no parallel authoritative allowlist (defers entirely to `git paw __classify`)

## 4. Gates

- [x] 4.1 `just check` green (skill-content + conformance tests pass)
- [x] 4.2 `mdbook build docs/` succeeds
- [x] 4.3 Every spec scenario maps to a test; Gate-5 note: the skill guidance defers to the classifier boundary rather than weakening it
