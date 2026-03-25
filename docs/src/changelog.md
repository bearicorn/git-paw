# Changelog

All notable changes to this project will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Project scaffolding with clap v4 CLI entry point
- AI CLI auto-detection (claude, codex, gemini, aider, mistral, qwen, amp, copilot)
- Git worktree creation and management
- Tmux session orchestration with builder pattern
- Session state persistence with crash recovery
- TOML configuration (global and per-repo)
- Interactive selection prompts (mode, branch, CLI pickers)
- Error handling with actionable messages and exit codes
- Custom CLI registration (`add-cli` / `remove-cli`)
- Preset support for one-command launch
- Mouse mode for tmux sessions
- Pane titles showing branch and CLI names
- Dry-run mode (`--dry-run`)
- Non-interactive mode (`--cli` and `--branches` flags)
- mdBook documentation site

[Unreleased]: https://github.com/bearicorn/git-paw/compare/v0.1.0...HEAD
