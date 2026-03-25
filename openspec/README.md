# git-paw OpenSpec Specifications

Formal capability specifications for git-paw using [OpenSpec](https://github.com/Fission-AI/OpenSpec).

## Structure

```
openspec/
├── README.md              # This file
├── changes/               # Active change proposals (empty when stable)
│   └── archive/           # Archived completed changes
└── specs/                 # Main specifications (source of truth)
    ├── cli-detection/     # AI CLI binary detection and PATH scanning
    ├── cli-parsing/       # Command-line argument parsing and subcommands
    ├── configuration/     # TOML config parsing, merging, and CLI management
    ├── error-handling/    # Central error type with actionable messages
    ├── git-operations/    # Repository validation, branches, and worktrees
    ├── interactive-selection/ # Branch and CLI picker prompts
    ├── session-state/     # Session persistence and recovery
    └── tmux-orchestration/# Tmux session and pane management
```

## Conventions

- **RFC 2119 keywords**: `SHALL`, `MUST`, `SHOULD`, `MAY` indicate requirement levels
- **GIVEN/WHEN/THEN**: Each scenario follows this format for clarity
- **Test traceability**: Every scenario includes a `Test:` annotation linking to the Rust test function (e.g., `Test: detect::tests::all_known_clis_detected_when_present`)
- **One spec per capability**: Each `spec.md` covers a single module in `src/`

## Spec ↔ Source Mapping

| Spec | Source Module |
|------|---------------|
| `cli-detection` | `src/detect.rs` |
| `cli-parsing` | `src/cli.rs` |
| `git-operations` | `src/git.rs` |
| `tmux-orchestration` | `src/tmux.rs` |
| `session-state` | `src/session.rs` |
| `configuration` | `src/config.rs` |
| `interactive-selection` | `src/interactive.rs` |
| `error-handling` | `src/error.rs` |

## Usage

```bash
# List all specs
openspec list --specs

# Validate a spec
openspec validate --specs cli-detection

# View all specs interactively
openspec view
```
