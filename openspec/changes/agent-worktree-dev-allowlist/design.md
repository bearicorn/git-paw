## Context

Confirmed by code audit (2026-07-14): no per-worktree settings seeding exists. `seed_supervisor_session` writes `<repo>/.claude/settings.json` + configured `settings_path` files (parent-must-exist); `setup_curl_allowlist` likewise. Claude-format CLIs resolve project settings from the working directory — an agent whose cwd is its worktree never sees the repo-root file, and home-level files are optional config. v0.10.0's `worktree-helper-provisioning` established the pattern (and the seeding point: `attach_agent`, `src/main.rs:991-995`) for helper scripts; the allowlists were left behind. Result: 6× per-wave manual approvals on preset-safe commands, and CLI don't-ask-again grants that pin full command strings and never match again.

## Goals / Non-Goals

**Goals:** every agent worktree carries the same grants the repo root does, from the moment the pane boots; nothing seeded can leak into a commit; recovery refreshes.
**Non-Goals:** other CLIs' config formats (Codex/Gemini/opencode config files stay v1.0.0 hook-provider territory — the existing per-CLI placement clause is unchanged); changing what the patterns contain (that's `classifier-stack-de-opinionation`); supervisor-pane seeding (already covered by the repo-root file).

## Decisions

- **D1 — Seed the worktree-local project settings file.** `<worktree>/.claude/settings.json` is the one location a claude-format CLI deterministically reads for that agent regardless of home-dir config. Reuses `setup_dev_allowlist`/`setup_curl_allowlist` unchanged (both already take an arbitrary settings path).
- **D2 — Seed at `attach_agent` + recovery.** `attach_agent` is the single per-worktree setup point shared by `start` and `add` (same place helper scripts are provisioned — grants land next to the scripts they authorize). Recovery loops restored worktrees so preset updates propagate, mirroring the repo-root re-seed.
- **D3 — Worktree-local `info/exclude`, not `.gitignore`.** An untracked `.claude/` in the worktree would be staged by an agent's `git add .` and pollute PRs. The worktree's own exclude file (`<repo>/.git/worktrees/<name>/info/exclude`) is invisible to the diff, per-worktree, and requires no tracked-file edits in consumer repos. Fail-open with a warning if unwritable (grants still work; hygiene degrades gracefully).
- **D4 — Both pattern families ride together.** Helper prefixes without dev patterns (or vice versa) reproduces the half-seeded state the dogfood hit; one seeding event, two gated content sources.

## Risks / Trade-offs

- **Untracked-file noise in consumer worktrees** if `info/exclude` writing fails — mitigated by the warning; the remove-dirty-check porcelain filter already ignores `.git-paw/`, and `.claude/` joins the excluded set in the normal path.
- **Grant breadth**: worktree seeding grants preset verbs inside the agent's own sandbox — least-privilege is preserved (path-scoped helper grants, no `curl *`, destructive git verbs excluded from the preset).
- Recovery ordering: seed before prompt injection so the first boot command never prompts (same ordering the repo-root seeding uses).

## Migration Plan

None: additive writes into git-paw-created worktrees; disabling `[supervisor.common_dev_allowlist]` (or the broker) gates the respective content exactly as at repo root.

## Open Questions

None.
