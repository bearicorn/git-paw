# Specifications

git-paw uses [OpenSpec](https://github.com/openspec) for formal, testable
specifications: each capability has a dedicated `spec.md` under
[`openspec/specs/`](https://github.com/bearicorn/git-paw/tree/main/openspec/specs)
using RFC 2119 keywords (`SHALL`, `MUST`, `SHOULD`) and GIVEN/WHEN/THEN
scenarios, and every scenario maps to at least one test.

The eight **foundational capabilities** (the original v0.1–v0.2 surface) are
reproduced in full below. The project has since grown to many more capabilities
(broker, supervisor, dashboard, MCP, learnings, conflict detection, unattended
operation, agent-friendly docs, …); the **complete index** at the end of this
page links to every capability spec in the repository.

## Foundational capabilities

| Capability | Description |
|------------|-------------|
| [CLI Parsing](#cli-parsing) | Command-line argument parsing and subcommands |
| [CLI Detection](#cli-detection) | Auto-detect AI CLIs on PATH, load custom CLIs |
| [Git Operations](#git-operations) | Validate repos, list branches, manage worktrees |
| [Tmux Orchestration](#tmux-orchestration) | Create sessions, manage panes, apply layout |
| [Session State](#session-state) | Persist and recover session state |
| [Configuration](#configuration) | Parse and merge TOML config files |
| [Interactive Selection](#interactive-selection) | User prompts for mode, branch, and CLI selection |
| [Error Handling](#error-handling) | Unified error types with exit codes |

---

## CLI Parsing

{{#include ../../../openspec/specs/cli-parsing/spec.md}}

---

## CLI Detection

{{#include ../../../openspec/specs/cli-detection/spec.md}}

---

## Git Operations

{{#include ../../../openspec/specs/git-operations/spec.md}}

---

## Tmux Orchestration

{{#include ../../../openspec/specs/tmux-orchestration/spec.md}}

---

## Session State

{{#include ../../../openspec/specs/session-state/spec.md}}

---

## Configuration

{{#include ../../../openspec/specs/configuration/spec.md}}

---

## Interactive Selection

{{#include ../../../openspec/specs/interactive-selection/spec.md}}

---

## Error Handling

{{#include ../../../openspec/specs/error-handling/spec.md}}

---

## Complete capability index

Every capability spec in the repository (links go to the canonical
`openspec/specs/` source on GitHub):

- [`add-branch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/add-branch/spec.md)
- [`advanced-main-event`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/advanced-main-event/spec.md)
- [`agent-broker-helper`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/agent-broker-helper/spec.md)
- [`agent-friendly-docs-site`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/agent-friendly-docs-site/spec.md)
- [`agent-learning-variant`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/agent-learning-variant/spec.md)
- [`agent-skills`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/agent-skills/spec.md)
- [`agents-md-injection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/agents-md-injection/spec.md)
- [`approval-configuration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/approval-configuration/spec.md)
- [`approval-pattern-surfacing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/approval-pattern-surfacing/spec.md)
- [`auto-approve-file-edits`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/auto-approve-file-edits/spec.md)
- [`automatic-approval`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/automatic-approval/spec.md)
- [`boot-block-format`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/boot-block-format/spec.md)
- [`broker-endpoints`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-endpoints/spec.md)
- [`broker-lifecycle`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-lifecycle/spec.md)
- [`broker-mediated-approvals`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-mediated-approvals/spec.md)
- [`broker-messages`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-messages/spec.md)
- [`broker-roster-hygiene`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-roster-hygiene/spec.md)
- [`broker-server`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-server/spec.md)
- [`cli-detection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-detection/spec.md)
- [`cli-parsing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-parsing/spec.md)
- [`cli-selection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-selection/spec.md)
- [`cli-specs-supervisor-filter`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-specs-supervisor-filter/spec.md)
- [`cli-submit-profile`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-submit-profile/spec.md)
- [`cold-start-ci-parity`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cold-start-ci-parity/spec.md)
- [`configuration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/configuration/spec.md)
- [`conflict-detection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/conflict-detection/spec.md)
- [`conflict-detector-fn-granularity`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/conflict-detector-fn-granularity/spec.md)
- [`coordination-context-budget`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/coordination-context-budget/spec.md)
- [`curl-allowlist`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/curl-allowlist/spec.md)
- [`custom-cli-curl-seeding`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/custom-cli-curl-seeding/spec.md)
- [`dashboard`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/dashboard/spec.md)
- [`dashboard-broker-log`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/dashboard-broker-log/spec.md)
- [`dev-command-allowlist`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/dev-command-allowlist/spec.md)
- [`docs-fetch-skill`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/docs-fetch-skill/spec.md)
- [`error-handling`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/error-handling/spec.md)
- [`filesystem-watcher`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/filesystem-watcher/spec.md)
- [`from-specs-launch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/from-specs-launch/spec.md)
- [`git-hook-injection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/git-hook-injection/spec.md)
- [`git-operations`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/git-operations/spec.md)
- [`governance-config`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/governance-config/spec.md)
- [`interactive-selection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/interactive-selection/spec.md)
- [`lang-agnostic-skills`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/lang-agnostic-skills/spec.md)
- [`learnings-mode`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/learnings-mode/spec.md)
- [`manual-injection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/manual-injection/spec.md)
- [`markdown-integration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/markdown-integration/spec.md)
- [`mcp-read-tools`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/mcp-read-tools/spec.md)
- [`mcp-server`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/mcp-server/spec.md)
- [`message-delivery`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/message-delivery/spec.md)
- [`no-fail-fast-verification`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/no-fail-fast-verification/spec.md)
- [`openspec-integration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/openspec-integration/spec.md)
- [`opsx-role-gating`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/opsx-role-gating/spec.md)
- [`per-commit-verification`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/per-commit-verification/spec.md)
- [`permission-detection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/permission-detection/spec.md)
- [`project-initialization`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/project-initialization/spec.md)
- [`qualitative-learnings`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/qualitative-learnings/spec.md)
- [`remove-branch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/remove-branch/spec.md)
- [`replay-command`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/replay-command/spec.md)
- [`robust-cli-launch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/robust-cli-launch/spec.md)
- [`safe-command-classification`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/safe-command-classification/spec.md)
- [`selftest`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/selftest/spec.md)
- [`session-json-location`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-json-location/spec.md)
- [`session-logging`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-logging/spec.md)
- [`session-receipt-hygiene`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-receipt-hygiene/spec.md)
- [`session-recovery-integrity`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-recovery-integrity/spec.md)
- [`session-state`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-state/spec.md)
- [`session-summary`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-summary/spec.md)
- [`shared-helper`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/shared-helper/spec.md)
- [`skill-standardization`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/skill-standardization/spec.md)
- [`skill-validation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/skill-validation/spec.md)
- [`spec-kit-integration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/spec-kit-integration/spec.md)
- [`spec-scanning`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/spec-scanning/spec.md)
- [`start-force-flag`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/start-force-flag/spec.md)
- [`status-republish-on-write`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/status-republish-on-write/spec.md)
- [`stuck-prompt-detection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/stuck-prompt-detection/spec.md)
- [`supervisor-agent-inventory`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-agent-inventory/spec.md)
- [`supervisor-cli`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-cli/spec.md)
- [`supervisor-config`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-config/spec.md)
- [`supervisor-first-agent-cwd`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-first-agent-cwd/spec.md)
- [`supervisor-injection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-injection/spec.md)
- [`supervisor-introspection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-introspection/spec.md)
- [`supervisor-launch`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-launch/spec.md)
- [`supervisor-pane-affordances`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-pane-affordances/spec.md)
- [`supervisor-skill-discipline`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-skill-discipline/spec.md)
- [`supervisor-stream-timeout-recovery`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-stream-timeout-recovery/spec.md)
- [`supervisor-tell`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/supervisor-tell/spec.md)
- [`template-substitution`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/template-substitution/spec.md)
- [`terminal-status-protection`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/terminal-status-protection/spec.md)
- [`test-coverage-v0-5-0`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/test-coverage-v0-5-0/spec.md)
- [`test-isolation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/test-isolation/spec.md)
- [`tmux-orchestration`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/tmux-orchestration/spec.md)
- [`unattended-operation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/unattended-operation/spec.md)
- [`user-documentation`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/user-documentation/spec.md)
- [`worktree-agents-md`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/worktree-agents-md/spec.md)
- [`worktree-branch-guard`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/worktree-branch-guard/spec.md)
- [`worktree-embedded-placement`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/worktree-embedded-placement/spec.md)
