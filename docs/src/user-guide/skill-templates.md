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
        └── supervisor/
            ├── SKILL.md
            └── scripts/
```

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

## Validation

Skills are validated against the agentskills.io specification:

- Required `name` and `description` fields must be present
- YAML frontmatter must be valid
- Skill directory must contain SKILL.md file
- Clear error messages for validation failures

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

The boot-prompt injection includes pre-expanded curl commands for all essential coordination operations (register, done, blocked, question) and is active in both supervisor and manual broker modes. See the [Coordination documentation](coordination.md#boot-prompt-injection) for details.
