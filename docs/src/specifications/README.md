# Specifications

git-paw uses [OpenSpec](https://github.com/openspec) for formal, testable specifications. Each capability has a dedicated spec file using RFC 2119 keywords (`SHALL`, `MUST`, `SHOULD`) and GIVEN/WHEN/THEN scenarios.

## Specification Index

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
