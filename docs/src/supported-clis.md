# Supported AI CLIs

git-paw auto-detects these AI coding CLIs on your PATH:

| CLI | Binary | Description | Install | Cold resume |
|-----|--------|-------------|---------|-------------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `claude` | Anthropic's AI coding assistant | `npm i -g @anthropic-ai/claude-code` | Yes (built-in) |
| [Codex](https://github.com/openai/codex) | `codex` | OpenAI's coding agent | `npm i -g @openai/codex` | No |
| Antigravity CLI | `agy` | Google's agentic coding CLI (replaces the retired Gemini CLI) | See project docs | No |
| [Aider](https://aider.chat) | `aider` | AI pair programming in the terminal | `pip install aider-chat` | No |
| [Vibe](https://docs.mistral.ai/capabilities/vibe/) | `vibe` | Mistral AI's coding CLI | See project docs | No |
| [Qwen](https://github.com/QwenLM) | `qwen` | Alibaba's Qwen coding CLI | See project docs | No |
| [Amp](https://ampcode.com) | `amp` | Sourcegraph's AI coding agent | See project docs | No |

"Cold resume" means git-paw can restore the CLI's prior conversation
when recovering a stopped session or relaunching a dead pane, rather
than starting it fresh — see [Per-CLI cold
resume](user-guide/pause.md#per-cli-cold-resume). A "No" here reflects
the CLI having no known session-assignment/resume flags, not a defect
in it; add `session_id_args` / `resume_args` to a `[clis.<name>]`
override (see [Custom CLIs](configuration/README.md#custom-clis)) to
enable resume for a CLI that supports it but isn't in git-paw's
built-in table yet.

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

## Prompt-shape detection

The list above is git-paw's *binary* detection — it says which CLIs it can
launch. Separately, the supervisor and auto-approve detectors recognise a
launched CLI's terminal surface (its ready banner, permission-prompt
wording, and so on) via a **prompt-shape profile**. git-paw ships only the
Claude Code profile, embedded in the binary; no other CLI in the table above
has a shipped profile, because a guessed one would misdetect confidently
rather than falling back visibly. Every CLI — listed or custom — resolves
the embedded Claude default for any field it has no profile override for.
See [CLI Prompt-Shape Profiles](user-guide/cli-prompt-profiles.md) for the
field reference and how to author one.
