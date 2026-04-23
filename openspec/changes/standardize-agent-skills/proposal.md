## Why

The current agent skills system uses a custom SKILL.md format that is not compatible with the emerging agentskills.io standardization effort. This creates interoperability issues and makes it difficult to share skills across different AI systems. Standardizing to the agentskills.io format will improve compatibility, enable skill sharing, and future-proof our agent ecosystem.

## What Changes

- Migrate all existing skills from custom SKILL.md format to agentskills.io standardized structure
- Create directory structure with SKILL.md + optional subdirectories for each skill
- Maintain backward compatibility with existing skill invocation mechanisms
- Add validation to ensure new skills conform to the standardized format
- Update documentation and examples to reflect the new format

## Capabilities

### New Capabilities
- `skill-standardization`: Standardized skill format with directory structure and SKILL.md files
- `skill-validation`: Validation mechanism to ensure skills conform to agentskills.io format
- `backward-compatibility`: Maintain existing skill invocation while supporting new format

### Modified Capabilities
- None (this is a new capability, not modifying existing requirements)

## Impact

- All existing skills in `.opencode/skills/` directory will need migration
- Skill loading and parsing logic will need updates
- Documentation and examples will need revision
- No breaking changes to existing skill invocation APIs
- Configuration files may need updates to support new skill discovery