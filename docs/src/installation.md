# Installation

## Prerequisites

Before installing git-paw, ensure you have:

- **Git** 2.20 or later
- **tmux** — any recent version

### Installing tmux

**macOS:**
```bash
brew install tmux
```

**Ubuntu / Debian:**
```bash
sudo apt install tmux
```

**Fedora:**
```bash
sudo dnf install tmux
```

**Arch Linux:**
```bash
sudo pacman -S tmux
```

## Install git-paw

### From crates.io (recommended)

```bash
cargo install git-paw
```

### Via Homebrew

```bash
brew install bearicorn/tap/git-paw
```

### Shell installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/bearicorn/git-paw/releases/latest/download/git-paw-installer.sh | sh
```

### From source

```bash
git clone https://github.com/bearicorn/git-paw.git
cd git-paw
cargo install --path .
```

## Verify installation

```bash
git-paw --version
```

You should see output like:

```
git-paw 0.1.0
```

Since git-paw is named with the `git-` prefix, git recognizes it as a subcommand. Both of these work:

```bash
git-paw --help
git paw --help
```

## Platform Support

| Platform | Support |
|----------|---------|
| macOS (ARM / Apple Silicon) | Full support |
| macOS (x86_64 / Intel) | Full support |
| Linux (x86_64) | Full support |
| Linux (ARM64 / aarch64) | Full support |
| Windows | WSL only |

### Windows (WSL)

git-paw requires tmux, which is not natively available on Windows. Use [Windows Subsystem for Linux (WSL)](https://learn.microsoft.com/en-us/windows/wsl/install):

```powershell
# Install WSL (PowerShell as admin)
wsl --install

# Then inside WSL:
sudo apt install tmux
cargo install git-paw
```

All git-paw features work inside WSL. Your AI CLIs must also be installed within the WSL environment.

## Install an AI CLI

git-paw needs at least one AI coding CLI installed. See [Supported AI CLIs](supported-clis.md) for the full list. Some popular options:

```bash
# Claude Code
npm install -g @anthropic-ai/claude-code

# OpenAI Codex
npm install -g @openai/codex

# Aider
pip install aider-chat
```

## Next Steps

- [Quick Start: Same CLI Mode](quick-start-same-cli.md)
- [Quick Start: Per-Branch CLI Mode](quick-start-per-branch.md)
