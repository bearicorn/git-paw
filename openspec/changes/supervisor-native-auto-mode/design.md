## Context

The supervisor pane's launch flags are currently derived from the same `agent_approval` knob as every coding agent (`src/main.rs:1162` start, `src/main.rs:2323` recovery, via `config::approval_flags` at `src/config.rs:958`). The v0.9.0 unattended dogfood showed the supervisor's own compound verify commands are the dominant un-prefix-matchable approval cost — but relaxing `agent_approval` to `full-auto` today drops every coding agent's guardrails at once. `approval_flags` is a static two-CLI table (`claude`, `codex`); `CustomCli` (`src/config.rs:16-45`) has no flag override.

## Goals / Non-Goals

**Goals:** a supervisor-only approval level; native full-auto for the major CLIs known to run unattended; a config seam so variant CLIs work without code changes; strict backward compatibility (absent key = inherit `agent_approval`).
**Non-Goals:** the per-CLI trait-object provider registry (v1.0.0); changing coding-agent flag behavior; auto-approve classifier changes (`#5`); forcing `full-auto` under `--unattended` (operator choice stays explicit).

## Decisions

- **D1 — Reuse `ApprovalLevel`, add `approval: Option<ApprovalLevel>`.** No new enum ("prompt" from the roadmap sketch maps to the existing `Manual`). `Option` rather than a default value so "unset" is distinguishable and inherits `agent_approval` — the only fully backward-compatible default. `auto` ships as a named tier for free since the enum already has it (it names today's seeded-allowlist + classifier behavior; no new mechanics).
- **D2 — Flag resolution becomes config-aware: override → table → empty.** `approval_flags(cli, level)` (returns `&'static str`) grows into a resolution that first consults `[clis.<name>].approval_args` (new `HashMap<String, String>` on `CustomCli`, keys = kebab-case levels, validated at load). Signature changes to return `String` (or `Cow`) and take the clis map; the existing fn can remain as the built-in-table step. Rationale: the override is what makes "etc." CLIs (opencode, droid, claude-oss variants) usable at full-auto without waiting for v1.0.0.
- **D3 — Built-in rows: claude, codex (existing), gemini + qwen (`--yolo`).** opencode and the remaining detected CLIs (`src/detect.rs:31`: aider, vibe, amp, cline, droid, pi, junie, cursor, copilot, cn, kilo, kimi) get NO built-in row this release — their skip-permissions story is config-file- or subcommand-shaped rather than a stable flag, so hardcoding would rot. They are served by D2's override. Implementation task verifies the gemini/qwen/codex flags against upstream docs before landing; a changed upstream flag is a spec amendment, not a silent divergence.
- **D4 — Warn-and-degrade on unmapped full-auto.** `full-auto` + no resolution → stderr warning naming the CLI and the `[clis.<name>].approval_args` remedy; pane launches flagless (auto behavior). Rationale: failing the launch would brick sessions for typo'd CLI names; silent degradation (today's behavior for agents) hides the misconfiguration the operator explicitly requested.
- **D5 — Supervisor-only scope.** Agent panes keep `agent_approval`. The unattended drive loop needs no change: with a full-auto supervisor its pane simply stops presenting prompts to sweep (pane-0 sweep at `src/supervisor/drive.rs` stays as a no-op fallback for auto/manual levels).

## Risks / Trade-offs

- **A full-auto supervisor is a real blast-radius increase** — it runs in `repo_root`, not a worktree. Mitigated by: opt-in config, `agent-memory-isolation` (#2) landing in the same release, and the supervisor skill's own discipline rules. Called out in docs as "trusted-pane" semantics.
- **`approval_args` validation** (unknown level keys rejected) means a config typo fails the load — chosen over silent ignore because a typo here silently downgrades security posture otherwise.
- Dry-run output change is additive (extra line when levels differ) — no consumer parses it.

## Migration Plan

None required: absent `approval` inherits `agent_approval`; absent `approval_args` resolves through the built-in table exactly as v0.10.0.

## Open Questions

None — CLI breadth (major-CLIs + seam), `auto` tier inclusion, and supervisor-only scope were resolved with the operator 2026-07-14.
