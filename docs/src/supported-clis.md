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
| opencode | `opencode` | Open-source terminal coding agent | See project docs | No |
| Cline | `cline` | Cline's coding agent CLI | See project docs | No |
| Droid | `droid` | Factory's coding agent CLI | See project docs | No |
| Pi | `pi` | Terminal coding agent | See project docs | No |
| Junie | `junie` | JetBrains' coding agent CLI | See project docs | No |
| Cursor Agent | `cursor-agent` | Cursor's agent CLI | See project docs | No |
| GitHub Copilot CLI | `copilot` | GitHub Copilot in the terminal | See project docs | No |
| cn | `cn` | Continue's coding agent CLI | See project docs | No |
| Kilo Code | `kilo` | Kilo Code's coding agent CLI | See project docs | No |
| Kimi | `kimi` | Moonshot AI's coding agent CLI | See project docs | No |

"Cold resume" means git-paw can restore the CLI's prior conversation
when recovering a stopped session or relaunching a dead pane, rather
than starting it fresh — see [Per-CLI cold
resume](user-guide/pause.md#per-cli-cold-resume). A "No" here reflects
the CLI having no known session-assignment/resume flags, not a defect
in it; add `session_id_args` / `resume_args` to a `[clis.<name>]`
override (see [Custom CLIs](configuration/README.md#custom-clis)) to
enable resume for a CLI that supports it but isn't in git-paw's
built-in table yet.

The Cursor row names `cursor-agent`, Cursor's agent CLI. The bare `cursor`
binary is the Cursor *editor* launcher on most installations, so git-paw no
longer auto-detects it as an AI CLI. This affects auto-detection only — if
`cursor` is the command you want a pane to launch, any of these still
resolves it unchanged:

- `git paw start --cli cursor`,
- a `[clis.cursor]` entry (see [Custom CLIs](configuration/README.md#custom-clis)),
- a saved session whose recorded `cli` is `cursor`.

Spec-driven selection and `git paw add` validate availability against the
detected list, so there a `[clis.cursor]` entry is the route to take.
`default_cli` is **not** an escape hatch: it only pre-selects a row in the
interactive picker, and the picker is built from the detected list — an
undetected `cursor` has no row to pre-select, so the setting has no effect.

## Per-CLI Setup Support

[`git paw setup-cli`](user-guide/cli-setup.md) writes git-paw's setup items
into a CLI's own settings. An item is available only when git-paw's compiled
fact table declares it for that CLI **and** those facts have been verified
against an installed copy of it.

| CLI | `--allowlist` | `--trust` | `--hooks` |
|-----|---------------|-----------|-----------|
| `claude` | Supported, **verified** against Claude Code 2.1.220 (2026-10-15) | Not supported | Not supported |
| Every other CLI | Not supported | Not supported | Not supported |

Nothing is withheld here as a judgement about those CLIs — their settings
formats simply have not been checked yet. The per-CLI audit that adds them is
planned for v0.23.0. Until then `setup-cli` reports the item as not supported
and writes nothing, which is the honest answer rather than a guess at a file
format.

A Claude Code **variant** registered under another name (a wrapper binary in
a `[clis.<name>]` entry) has no row of its own, but is still covered by
`git paw setup-cli claude`: the target settings file belongs to the
repository, not to the binary.

`git paw doctor` reports the current status for your installation.


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
launch. Separately, the supervisor's prompt detectors recognise a
launched CLI's terminal surface (its ready banner, permission-prompt
wording, and so on) via a **prompt-shape profile**. git-paw ships only the
Claude Code profile, embedded in the binary; no other CLI in the table above
has a shipped profile, because a guessed one would misdetect confidently
rather than falling back visibly. Every CLI — listed or custom — resolves
the embedded Claude default for any field it has no profile override for.
See [CLI Prompt-Shape Profiles](user-guide/cli-prompt-profiles.md) for the
field reference and how to author one.
