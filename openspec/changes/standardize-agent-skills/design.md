## Context

Currently, git-paw uses a custom SKILL.md format for agent skills located in `.opencode/skills/`. The agentskills.io standardization effort defines a new format that includes a directory structure with SKILL.md as the main file plus optional subdirectories for resources. This design addresses migrating from the current format to the standardized one while maintaining backward compatibility.

## Goals / Non-Goals

**Goals:**
- Migrate all existing skills to agentskills.io standardized format
- Add validation to ensure new skills conform to the standard
- Update documentation and examples

**Non-Goals:**
- Changing the core skill invocation API
- Modifying skill discovery mechanisms beyond format support

## Decisions

**Standardized Format Adoption**: 
- Decision: Adopt agentskills.io format with directory structure (SKILL.md + optional subdirectories)
- Rationale: Improves interoperability, enables skill sharing across AI systems, future-proofs the ecosystem
- Alternatives considered: Custom format extension (rejected due to lack of standardization benefits)

**Validation Mechanism**:
- Decision: Add schema validation for standardized format skills
- Rationale: Ensures quality and compliance with standardization
- Implementation: JSON Schema validation during skill loading

## Risks / Trade-offs

**Performance Impact**:
- Risk: Validation may slow down skill loading
- Mitigation: Caching mechanism, optimize validation process