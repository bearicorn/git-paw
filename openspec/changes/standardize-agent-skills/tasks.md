## 1. Skill Format Implementation

- [ ] 1.1 Add SkillFormat enum to src/skills.rs with Legacy and Standardized variants
- [ ] 1.2 Extend SkillTemplate struct to include format field and optional resource paths
- [ ] 1.3 Implement format detection logic in resolve() function
- [ ] 1.4 Add standardized format parsing for directory-based skills with SKILL.md

## 2. Standardized Format Support

- [ ] 2.1 Create agentskills.io schema validation module
- [ ] 2.2 Implement JSON Schema validation for new format skills
- [ ] 2.3 Add resource loading for optional subdirectories (scripts/, references/, assets/)
- [ ] 2.4 Implement progressive disclosure loading (discovery → activation → execution)

## 3. Backward Compatibility

- [ ] 3.1 Ensure legacy SKILL.md format continues to work unchanged
- [ ] 3.2 Add format auto-detection that handles both formats seamlessly
- [ ] 3.3 Create migration utilities with dry-run capability
- [ ] 3.4 Add deprecation warnings for legacy format with migration guidance

## 4. Validation and Error Handling

- [ ] 4.1 Implement comprehensive validation error reporting
- [ ] 4.2 Add specific error messages for missing required fields
- [ ] 4.3 Create validation warnings for deprecated fields
- [ ] 4.4 Add PawError::SkillValidationError variant for validation failures

## 5. Testing and Verification

- [ ] 5.1 Add unit tests for format detection logic
- [ ] 5.2 Create integration tests for mixed format scenarios
- [ ] 5.3 Add validation test cases for both valid and invalid skills
- [ ] 5.4 Test progressive disclosure loading behavior

## 6. Documentation and Migration

- [ ] 6.1 Update AGENTS.md with new skill format documentation
- [ ] 6.2 Create migration guide with step-by-step instructions
- [ ] 6.3 Add examples of standardized format skills
- [ ] 6.4 Update README with format compatibility information