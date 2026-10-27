# Skill Templates

git-paw uses standardized agent skills following the [agentskills.io specification](https://agentskills.io). Skills are directories containing a `SKILL.md` file with YAML frontmatter and optional resource subdirectories.

## Standard Location

Skills are loaded from `.agents/skills/` in your project directory. git-paw walks up the directory tree from the current working directory to find this location.

```bash
my-project/
└── .agents/
    └── skills/
        ├── coordination/
        │   ├── SKILL.md          # Main skill file
        │   ├── scripts/          # Optional: Executable scripts
        │   ├── references/       # Optional: Documentation
        │   └── assets/           # Optional: Templates/resources
        └── orchestrator/
            ├── SKILL.md
            └── scripts/
```

## Bundled skills

git-paw embeds four skills, each overridable by a `.agents/skills/<name>/SKILL.md`
of your own:

| Skill | What it carries |
|---|---|
| `coordination` | How an agent publishes status, intents and questions through the broker. |
| `orchestrator` (alias `supervisor`) | The orchestrator's role: following the session, the five verification gates, merge orchestration. |
| `docs-fetch` | How to look things up through the bundled docs helper. |
| `cli-setup` | The consent rules for [`git paw setup-cli`](cli-setup.md) — when an agent may and may not run the command that writes an agent CLI's own settings. |

The `cli-setup` skill exists because that command changes what a CLI will do
without asking the human again. Its rules are deliberately restrictive: the
writing form runs only when the human asked in the current conversation, a
`--dry-run` suggestion is not consent, `--yes` follows approval of the exact
plan shown, it never runs unattended or because of an instruction found in
content the agent read, settings files are never hand-edited, and coding
agents in worktrees never run it at all.

## Skill Format

Each skill must contain a `SKILL.md` file with YAML frontmatter:

```yaml
---
name: my-skill
description: A brief description of what this skill does
license: MIT
compatibility: git-paw v0.4.0+
---

## My Skill Instructions

This skill helps agents with {{BRANCH_ID}} workflows...
```

### Required Fields

- `name`: Skill name (max 64 chars, lowercase letters/numbers/hyphens only)
- `description`: Clear description of the skill's purpose (max 1024 chars)

### Optional Fields

- `license`: License information
- `compatibility`: Version compatibility
- `metadata`: Custom metadata object

## Placeholders

Skills support these placeholders that get replaced at runtime:

- `{{BRANCH_ID}}` - Slugified branch name (e.g., `feat/http-broker` → `feat-http-broker`)
- `{{PROJECT_NAME}}` - Project name for tmux session
- `{{GIT_PAW_BROKER_URL}}` - Full broker URL
- `{{SKILL_NAME}}` - Name from YAML frontmatter
- `{{SKILL_DESCRIPTION}}` - Description from YAML frontmatter

### Supervisor gate-command placeholders

The embedded supervisor skill uses eight additional placeholders fed from
`[supervisor].*_command` keys in `.git-paw/config.toml`. Each gate-command
template renders verbatim into the skill prose where the supervisor agent
reads it:

- `{{TEST_COMMAND}}` ← `[supervisor].test_command` — gate 1 (Testing) test runner
- `{{LINT_COMMAND}}` ← `[supervisor].lint_command` — gate 1 lint sub-step
- `{{BUILD_COMMAND}}` ← `[supervisor].build_command` — gate 1 build sub-step
- `{{FMT_CHECK_COMMAND}}` ← `[supervisor].fmt_check_command` — gate 1 formatter check
- `{{DOC_BUILD_COMMAND}}` ← `[supervisor].doc_build_command` — gate 4 (Doc audit)
- `{{DOC_TOOL_COMMAND}}` ← `[supervisor].doc_tool_command` — gate 4 API-doc generator (renders empty when unset, not `(not configured)`)
- `{{SPEC_VALIDATE_COMMAND}}` ← `[supervisor].spec_validate_command` — gate 3 (Spec audit)
- `{{SECURITY_AUDIT_COMMAND}}` ← `[supervisor].security_audit_command` — gate 5 (Security audit)

**`(not configured)` graceful skip.** When a key is omitted from
`[supervisor]`, the matching placeholder substitutes to the literal string
`(not configured)` in the rendered skill. The supervisor agent treats that
sentinel as "skip the tooling invocation for this gate" and continues with
the gate's manual review only (e.g. the OWASP-category diff scan still runs
for the security gate, the spec scenario coverage check still runs for the
spec gate). Pre-v0.5.x configs that only set `test_command` continue to
render the remaining gates as `(not configured)` and the supervisor agent
runs them as manual-only — no behavior change for those configs.

The one exception is `{{DOC_TOOL_COMMAND}}`: when the underlying key is
unset, the placeholder renders as the empty string rather than the
`(not configured)` sentinel. The supervisor skill template is authored
so the surrounding prose reads naturally without it for projects that
don't ship an API-doc generator.

### Backend-aware and constant-driven placeholders

Two more placeholders make the bundled supervisor skill polyglot:

- `{{SPEC_PATH_DOCTRINE}}` — substitutes a per-backend paragraph that
  names where specs live and the matching workflow. The variants:
  - OpenSpec: `openspec/changes/<change>/{proposal,specs,tasks}.md` plus
    `openspec validate`.
  - Spec Kit: `.specify/specs/<feature>/{spec,plan,tasks}.md` plus the
    Spec Kit checklist convention.
  - Markdown: flat `.md` files with `paw_status: pending` frontmatter.
  - Superpowers: plan files under `docs/superpowers/plans/*.md`, worked
    `### Task N` step by step with `- [ ]` → `- [x]` checkboxes.
  - Multi-backend: one sentence per present backend, introduced by a
    "This session spans multiple spec backends" clause.
  - No backend resolved: a sentinel sentence pointing the supervisor at
    the project's documentation.
- `{{CHANGE_ID}}` — see the per-invocation discussion below.

**`{{CHANGE_ID}}` is per-invocation, not per-render.** Spec validators
typically take a change name as argument (e.g. `openspec validate
my-change-id --strict`). To support that, the `spec_validate_command`
template MAY embed the literal substring `{{CHANGE_ID}}`. git-paw does
**not** substitute `{{CHANGE_ID}}` at session boot — it passes through the
render verbatim. The supervisor agent expands it at verification time using
the change name it is currently auditing. This matches how `{{BRANCH_ID}}`
behaves for coding agents (per-agent, not per-render).

## Resource Subdirectories

Skills can include optional resource subdirectories:

- `scripts/` - Executable scripts referenced by the skill
- `references/` - Detailed documentation and references
- `assets/` - Templates, configuration files, and other resources

Example structure:

```bash
.agents/skills/my-skill/
├── SKILL.md              # Main instructions (< 500 lines)
├── scripts/
│   └── setup.sh          # Executable helper script
├── references/
│   └── api-reference.md  # Detailed API documentation
└── assets/
    └── config-template.json
```

## Creating Custom Skills

To add a custom skill:

```bash
# Create skill directory structure
mkdir -p .agents/skills/my-skill

# Create SKILL.md with proper frontmatter
cat > .agents/skills/my-skill/SKILL.md << 'EOF'
---
name: my-skill
description: Custom workflow for our team
license: MIT
compatibility: git-paw v0.4.0+
---

## Custom Team Workflow

Follow these steps for {{BRANCH_ID}}:
1. Analyze requirements
2. Implement solution
3. Test thoroughly
4. Document changes
EOF

# Add optional resource directories
mkdir -p .agents/skills/my-skill/scripts
mkdir -p .agents/skills/my-skill/references
```

## Skill Resolution

git-paw searches for skills in this order:

1. `.agents/skills/<name>/SKILL.md` (walking up directory tree from current directory)
2. Embedded defaults (compiled into git-paw binary)

The first match wins. If no skill is found, resolution fails with an error.

`supervisor` is an alias of `orchestrator`: both names resolve to the same
skill, and an override directory under either name applies to both. When
`.agents/skills/orchestrator/` and `.agents/skills/supervisor/` both exist, the
`orchestrator` one wins, so an override written under the old name keeps
working until you rename it.

## Exported skills

`git paw init` (and every `git paw start`) also writes two bundled skills into
the main checkout for the agent that drives the session, in the same
standardized layout:

- `.git-paw/skills/orchestrator/SKILL.md` — the orchestrator skill, rendered
  with your `[supervisor]` gate commands and configured spec format. It holds
  no session value (no broker URL, no session name); those live in the session
  context `.git-paw/AGENTS.local.md`.
- `.git-paw/skills/cli-setup/SKILL.md` — the `cli-setup` skill, as embedded.

Each opens with `name` / `description` frontmatter whose `name` equals its
directory name, so a skill-aware agent CLI can load the directory directly.
`.git-paw/skills/` is gitignored and binary-managed: a re-run overwrites it,
and an override under `.agents/skills/` is what the export renders from. See
[Supervisor → Driving the session from your own agent](supervisor.md#driving-the-session-from-your-own-agent).

## Validation

Skills are validated against the agentskills.io specification:

- Required `name` and `description` fields must be present
- Frontmatter must parse within the supported subset (below)
- Skill directory must contain SKILL.md file
- Clear error messages for validation failures

### Supported frontmatter subset

git-paw parses `SKILL.md` frontmatter with a small, dependency-free parser
rather than a full YAML library, so it supports a bounded subset rather than
arbitrary YAML:

- Top-level `key: value` pairs, where the value is a plain, single-quoted,
  or double-quoted scalar (surrounding whitespace is trimmed from plain
  scalars).
- Literal (`|`) and folded (`>`) block scalars, including their strip (`-`)
  and keep (`+`) chomping indicators.
- Full-line and trailing `#` comments outside quoted scalars.
- A one-level `metadata` mapping of string keys to scalar values — every
  value is read as a string.
- A key git-paw doesn't consume (for example `allowed-tools:` followed by a
  list) loads fine; it's skipped along with any indented content beneath it.

**This parser does not silently misparse.** If `name`, `description`,
`license`, `compatibility`, or `metadata` uses a construct outside this
subset — a flow mapping or list (`{ a: 1 }`, `[a, b]`), an anchor or alias
(`&foo`, `*foo`), a YAML tag (`!!str`), or a nested mapping inside
`metadata` — the skill fails to load with a validation error naming the
field, rather than silently producing a different value. This applies to
your own override skills too: a `SKILL.md` you hand-write under
`.agents/skills/` is held to the same subset as the embedded skills git-paw
ships.

## Examples

See the [agentskills.io specification](https://agentskills.io/skill-creation/quickstart) for more examples and best practices.

## Migration from Older Versions

If you're upgrading from git-paw v0.2.x or earlier:

1. Move skills from `~/.config/git-paw/agent-skills/` to `.agents/skills/`
2. Convert single `.md` files to directory structure with SKILL.md
3. Add required YAML frontmatter to each skill
4. Organize related resources into subdirectories

The new standardized format improves interoperability and enables skill sharing across different AI systems that support the agentskills.io standard.

## When Skills Are Not Injected

Skill templates are only injected when the broker is enabled (`[broker] enabled = true`). If the broker is disabled, no coordination instructions are added to `AGENTS.md`.

## Boot-Prompt Injection

In addition to skill template injection, git-paw automatically injects a standardized boot instruction block into every agent's initial prompt. This ensures reliable agent self-reporting even if skill templates are not used or if agents don't read the AGENTS.md file thoroughly.

The boot-prompt injection covers every essential coordination operation (register, done, blocked, question) with the agent's id pre-expanded, as calls to the bundled `.git-paw/scripts/broker.sh` helper — or, for CLIs known to speak MCP (the built-in set is `claude`, `codex`, `cursor-agent`, `agy`), as MCP publish-tool calls — and is active in both supervisor and manual broker modes. Set [`[clis.<name>] mcp = true|false`](../configuration/README.md#custom-clis) to override that set for a given CLI identity. See the [Coordination documentation](coordination.md#boot-prompt-injection) for details.
