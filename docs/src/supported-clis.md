# Supported AI CLIs

git-paw auto-detects these AI coding CLIs on your PATH:

| CLI | Binary | Description | Install |
|-----|--------|-------------|---------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `claude` | Anthropic's AI coding assistant | `npm i -g @anthropic-ai/claude-code` |
| [Codex](https://github.com/openai/codex) | `codex` | OpenAI's coding agent | `npm i -g @openai/codex` |
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | `gemini` | Google's Gemini in the terminal | `npm i -g @anthropic-ai/gemini-cli` |
| [Aider](https://aider.chat) | `aider` | AI pair programming in the terminal | `pip install aider-chat` |
| [Mistral](https://github.com/mistralai) | `mistral` | Mistral AI's coding CLI | See project docs |
| [Qwen](https://github.com/QwenLM) | `qwen` | Alibaba's Qwen coding CLI | See project docs |
| [Amp](https://ampcode.com) | `amp` | Sourcegraph's AI coding agent | See project docs |
| [GitHub Copilot](https://github.com/features/copilot) | `copilot` | GitHub Copilot CLI | `gh extension install github/gh-copilot` |

## How Detection Works

git-paw scans your `PATH` for each known binary name. If found, it records the full path and makes the CLI available for selection.

Detection runs every time you start a session, so newly installed CLIs are picked up automatically.

## Adding Custom CLIs

Any AI CLI not in the list above can be registered as a custom CLI:

```bash
# Register by path
git paw add-cli my-agent /usr/local/bin/my-agent

# Register by binary name (resolved via PATH)
git paw add-cli my-agent my-agent --display-name "My Agent"
```

Custom CLIs appear alongside detected ones in the selection prompt. See [Configuration](configuration/README.md) for more details.

## Deduplication

If a custom CLI has the same binary name as a detected one, the custom definition takes precedence. This lets you override the path or display name of a detected CLI.

## Missing CLIs

If a custom CLI's command cannot be found (the binary doesn't exist at the specified path and isn't on PATH), it is excluded from the selection list with a warning. This prevents launching sessions that would immediately fail.
