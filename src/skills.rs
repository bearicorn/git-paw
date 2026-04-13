//! Agent skill template loading and rendering.
//!
//! Skills follow the agentskills.io specification: each skill is a directory containing
//! a SKILL.md file with YAML frontmatter and optional resource subdirectories
//! (scripts/, references/, assets/).
//!
//! ## Resolution order (agentskills.io compliant)
//!
//! When a skill is requested by name, the system searches in this order:
//!
//! 1. **Standard location** — `.agents/skills/<name>/SKILL.md` (walking up directory tree)
//! 2. **User override** — `<config_dir>/git-paw/agent-skills/<name>/SKILL.md`
//! 3. **Embedded default** — compiled into the binary via `include_str!`
//!
//! The first match wins. If none exist, resolution fails with [`SkillError::UnknownSkill`].
//!
//! ## Substitution rules
//!
//! During [`render`], the template content undergoes placeholder substitution:
//!
//! - `{{BRANCH_ID}}` is replaced with the slugified branch name (`feat/foo` → `feat-foo`)
//! - `{{PROJECT_NAME}}` is replaced with the project name (e.g. `"git-paw"`), used in the
//!   `paw-{{PROJECT_NAME}}` tmux session name
//! - `{{GIT_PAW_BROKER_URL}}` is substituted at render time with the actual broker URL
//! - `{{SKILL_NAME}}` is replaced with the skill name from metadata
//! - `{{SKILL_DESCRIPTION}}` is replaced with the skill description from metadata

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json;
use std::path::{Path, PathBuf};

/// The embedded coordination skill, compiled into the binary.
///
/// New embedded skills are added by adding a new `include_str!` constant
/// and a corresponding match arm in [`embedded_default`].
const COORDINATION_DEFAULT: &str = include_str!("../assets/agent-skills/coordination.md");

/// The embedded supervisor skill, compiled into the binary.
const SUPERVISOR_DEFAULT: &str = include_str!("../assets/agent-skills/supervisor.md");

/// Indicates where a resolved skill's content originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Content came from the binary's compiled-in default.
    Embedded,
    /// Content came from the agentskills.io standard location (.agents/skills/)
    AgentsStandard,
    /// Content came from the user's config directory override
    User,
}

/// Represents the format of a skill (standardized only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SkillFormat {
    /// Standardized format: directory with SKILL.md + optional subdirectories
    Standardized,
}

/// Standardized skill metadata following agentskills.io specification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StandardizedSkillMetadata {
    /// Skill name (max 64 chars, lowercase letters/numbers/hyphens only)
    pub name: String,
    /// Skill description (max 1024 chars)
    pub description: String,
    /// Optional license information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Optional compatibility information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<String>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// A loaded skill template ready for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTemplate {
    /// The skill name (e.g. `"coordination"`).
    pub name: String,
    /// The unrendered template content with placeholders.
    pub content: String,
    /// Where the content was loaded from.
    pub source: Source,
    /// The format of the skill (legacy or standardized).
    pub format: SkillFormat,
    /// Optional metadata for standardized skills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<StandardizedSkillMetadata>,
    /// Optional resource paths for standardized skills.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_paths: Option<Vec<PathBuf>>,
}

/// Errors that can occur during skill loading.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// No embedded or user override found for the requested skill name.
    #[error("unknown skill '{name}' — no embedded default or user override exists")]
    UnknownSkill {
        /// The skill name that was requested.
        name: String,
    },

    /// Standardized skill validation failed.
    #[error("skill '{name}' validation failed: {reason}")]
    ValidationError {
        /// The skill name that failed validation.
        name: String,
        /// The validation error reason.
        reason: String,
    },

    /// Standardized skill directory cannot be read.
    #[error("cannot read skill directory at '{}' — check directory permissions", path.display())]
    DirectoryReadError {
        /// The path that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// User override skill file cannot be read.
    #[error("cannot read user override skill file at '{}' — check file permissions", path.display())]
    UserOverrideRead {
        /// The path that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

/// Looks up the embedded default for a skill by name.
///
/// Returns `Some(content)` if an embedded skill exists with that name,
/// or `None` otherwise. New embedded skills are added by introducing a
/// new `include_str!` constant and a new match arm here.
fn embedded_default(skill_name: &str) -> Option<&'static str> {
    match skill_name {
        "coordination" => Some(COORDINATION_DEFAULT),
        "supervisor" => Some(SUPERVISOR_DEFAULT),
        _ => None,
    }
}

/// Resolves a skill template by name.
///
/// Checks for a user override first, then falls back to the embedded default.
/// Returns [`SkillError::UnknownSkill`] if neither source has the skill.
pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError> {
    resolve_with_config_dir(skill_name, None)
}

/// Attempts to load a standardized skill from .agents/skills/ directory.
///
/// Walks up the directory tree from current directory looking for .agents/skills/<name>/SKILL.md
/// Also checks user override location if `config_dir_override` is provided
fn try_load_standardized_skill(
    skill_name: &str,
    config_dir_override: Option<&Path>,
) -> Result<Option<SkillTemplate>, SkillError> {
    // First try user override if config directory is provided
    if let Some(config_dir) = config_dir_override
        && let Some(skill) = try_load_user_override(skill_name, config_dir)?
    {
        return Ok(Some(skill));
    }

    // Then try standardized agents directory
    try_load_from_agents_dir(skill_name)
}

/// Try loading from user override location in config directory
fn try_load_user_override(
    skill_name: &str,
    config_dir: &Path,
) -> Result<Option<SkillTemplate>, SkillError> {
    let skill_dir = config_dir
        .join("git-paw")
        .join("agent-skills")
        .join(skill_name);

    if skill_dir.is_dir() {
        let skill_md_path = skill_dir.join("SKILL.md");
        if skill_md_path.exists() {
            return load_skill_from_directory(&skill_dir, skill_name, Source::User);
        }
    }

    Ok(None)
}

/// Try loading from .agents/skills/ by walking up directory tree
fn try_load_from_agents_dir(skill_name: &str) -> Result<Option<SkillTemplate>, SkillError> {
    let Ok(mut current_dir) = std::env::current_dir() else {
        return Ok(None);
    };

    for _ in 0..5 {
        // Limit to 5 levels up to prevent infinite loops
        let agents_dir = current_dir.join(".agents").join("skills").join(skill_name);

        if agents_dir.is_dir() {
            let skill_md_path = agents_dir.join("SKILL.md");
            if skill_md_path.exists() {
                return load_skill_from_directory(&agents_dir, skill_name, Source::AgentsStandard);
            }
        }

        if !current_dir.pop() {
            break;
        }
    }

    Ok(None)
}

/// Common loading logic for both locations
fn load_skill_from_directory(
    skill_dir: &Path,
    skill_name: &str,
    source: Source,
) -> Result<Option<SkillTemplate>, SkillError> {
    let skill_md_path = skill_dir.join("SKILL.md");

    let content = match std::fs::read_to_string(&skill_md_path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source_err) => {
            let error = match source {
                Source::User => SkillError::UserOverrideRead {
                    path: skill_md_path.clone(),
                    source: source_err,
                },
                _ => SkillError::DirectoryReadError {
                    path: skill_dir.to_path_buf(),
                    source: source_err,
                },
            };
            return Err(error);
        }
    };

    // Parse metadata from frontmatter if present
    let (metadata, content_without_frontmatter) = parse_standardized_metadata(&content)?;

    // Collect resource paths
    let mut resource_paths = Vec::new();
    for subdir in ["scripts", "references", "assets"] {
        let subdir_path = skill_dir.join(subdir);
        if subdir_path.exists() && subdir_path.is_dir() {
            resource_paths.push(subdir_path);
        }
    }

    Ok(Some(SkillTemplate {
        name: skill_name.to_string(),
        content: content_without_frontmatter,
        source,
        format: SkillFormat::Standardized,
        metadata,
        resource_paths: if resource_paths.is_empty() {
            None
        } else {
            Some(resource_paths)
        },
    }))
}

/// Parses standardized skill metadata from YAML frontmatter.
///
/// Extracts YAML frontmatter (between --- lines) and parses it into `StandardizedSkillMetadata`.
fn parse_standardized_metadata(
    content: &str,
) -> Result<(Option<StandardizedSkillMetadata>, String), SkillError> {
    // Check if content starts with YAML frontmatter
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 || !lines[0].trim().starts_with("---") {
        // No frontmatter, return None for metadata and original content
        return Ok((None, content.to_string()));
    }

    // Find the end of frontmatter
    let mut frontmatter_end = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim().starts_with("---") {
            frontmatter_end = Some(i);
            break;
        }
    }

    let Some(frontmatter_end) = frontmatter_end else {
        return Ok((None, content.to_string())); // No closing ---, treat as no frontmatter
    };

    // Extract frontmatter YAML
    let frontmatter_lines = &lines[1..frontmatter_end];
    let frontmatter_yaml = frontmatter_lines.join("\n");

    // Parse YAML into metadata
    let metadata: StandardizedSkillMetadata = match serde_yaml::from_str(&frontmatter_yaml) {
        Ok(meta) => meta,
        Err(e) => {
            return Err(SkillError::ValidationError {
                name: "unknown".to_string(),
                reason: format!("invalid YAML frontmatter: {e}"),
            });
        }
    };

    // Validate required fields
    if metadata.name.is_empty() {
        return Err(SkillError::ValidationError {
            name: "unknown".to_string(),
            reason: "missing required 'name' field in frontmatter".to_string(),
        });
    }

    if metadata.description.is_empty() {
        return Err(SkillError::ValidationError {
            name: metadata.name.clone(),
            reason: "missing required 'description' field in frontmatter".to_string(),
        });
    }

    // Extract content after frontmatter
    let content_without_frontmatter = lines[frontmatter_end + 1..].join("\n");

    Ok((Some(metadata), content_without_frontmatter))
}

/// Internal resolver that accepts an optional config directory override for testing.
fn resolve_with_config_dir(
    skill_name: &str,
    config_dir: Option<&Path>,
) -> Result<SkillTemplate, SkillError> {
    // Try standardized format
    if let Some(skill) = try_load_standardized_skill(skill_name, config_dir)? {
        return Ok(skill);
    }

    // Try embedded default (now also uses standardized format)
    if let Some(content) = embedded_default(skill_name) {
        // Parse embedded content as standardized format
        let (metadata, content_without_frontmatter) = parse_standardized_metadata(content)?;

        return Ok(SkillTemplate {
            name: skill_name.to_string(),
            content: content_without_frontmatter,
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata,
            resource_paths: None,
        });
    }

    Err(SkillError::UnknownSkill {
        name: skill_name.to_string(),
    })
}

/// Re-export of [`crate::broker::messages::slugify_branch`] to ensure skill
/// template rendering uses the exact same slug algorithm as the broker.
fn slugify_branch(branch: &str) -> String {
    crate::broker::messages::slugify_branch(branch)
}

/// Builds the standardized boot instruction block for agent initialization.
///
/// The boot block contains instructions for four essential runtime events:
/// 1. REGISTER - Initial status publication
/// 2. DONE - Task completion reporting
/// 3. BLOCKED - Dependency waiting notification
/// 4. QUESTION - Uncertainty escalation with explicit wait instruction
///
/// # Arguments
///
/// * `branch_id` - The branch name (will be slugified)
/// * `broker_url` - The fully-qualified broker URL for curl commands
///
/// # Returns
///
/// A string containing the complete boot instruction block with all placeholders
/// substituted and curl commands pre-expanded.
pub fn build_boot_block(branch_id: &str, broker_url: &str) -> String {
    let template = include_str!("../assets/boot-block-template.md");
    let slugified_branch = slugify_branch(branch_id);

    template
        .replace("{{BRANCH_ID}}", &slugified_branch)
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
}

/// Renders a skill template for a specific worktree.
///
/// Substitutes the following placeholders at render time:
///
/// - `{{BRANCH_ID}}` — the slugified branch name (`feat/foo` → `feat-foo`)
/// - `{{PROJECT_NAME}}` — the project name (e.g. `"git-paw"`), used in the
///   `paw-{{PROJECT_NAME}}` tmux session name
/// - `{{GIT_PAW_BROKER_URL}}` — the fully-qualified broker URL, pre-expanded
///   here so the agent's curl commands contain a literal URL and no shell
///   expansion is needed at execution time. Pre-expanding at render time is
///   important: some CLI tools gate shell-variable expansion behind extra
///   permission prompts, which breaks the "don't ask again for `curl:*`"
///   allowlist flow.
/// - `{{TEST_COMMAND}}` — the supervisor's configured `test_command` (e.g.
///   `"just check"`). When `test_command` is `None`, the placeholder
///   substitutes to the literal `"(not configured)"` so the rendered prose
///   stays readable.
///
/// Any remaining `{{...}}` placeholder after substitution is logged as a
/// warning to stderr but does not cause `render` to fail.
///
/// For standardized skills, additional metadata placeholders may be available:
/// - `{{SKILL_NAME}}` — the skill name from metadata
/// - `{{SKILL_DESCRIPTION}}` — the skill description from metadata
pub fn render(
    template: &SkillTemplate,
    branch: &str,
    broker_url: &str,
    project: &str,
    test_command: Option<&str>,
) -> String {
    let branch_id = slugify_branch(branch);
    let test_command_value = test_command.unwrap_or("(not configured)");

    // Start with basic substitutions
    let mut output = template
        .content
        .replace("{{BRANCH_ID}}", &branch_id)
        .replace("{{PROJECT_NAME}}", project)
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
        .replace("{{TEST_COMMAND}}", test_command_value);

    // Add metadata substitutions for standardized skills
    if let Some(metadata) = &template.metadata {
        output = output
            .replace("{{SKILL_NAME}}", &metadata.name)
            .replace("{{SKILL_DESCRIPTION}}", &metadata.description);
    }

    // Warn about any remaining {{...}} placeholders that were not consumed.
    let mut start = 0;
    while let Some(open) = output[start..].find("{{") {
        let abs_open = start + open;
        if let Some(close) = output[abs_open..].find("}}") {
            let placeholder = &output[abs_open..abs_open + close + 2];
            eprintln!(
                "warning: unsubstituted placeholder {placeholder} in skill '{}'",
                template.name
            );
            start = abs_open + close + 2;
        } else {
            break;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // 9.2: Embedded coordination skill is reachable without any user files
    #[test]
    fn embedded_coordination_is_reachable() {
        let tmpl = resolve("coordination").expect("should resolve coordination");
        assert_eq!(tmpl.source, Source::Embedded);
        assert!(!tmpl.content.is_empty());
    }

    // 9.3: Embedded coordination skill contains all four operations
    #[test]
    fn embedded_coordination_contains_all_operations() {
        let tmpl = resolve("coordination").unwrap();
        assert!(tmpl.content.contains("agent.status"));
        assert!(tmpl.content.contains("agent.artifact"));
        assert!(tmpl.content.contains("agent.blocked"));
        assert!(
            tmpl.content
                .contains("{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}")
        );
    }

    #[test]
    fn embedded_coordination_documents_supervisor_messages() {
        let tmpl = resolve("coordination").unwrap();
        assert!(tmpl.content.contains("agent.verified"));
        assert!(tmpl.content.contains("agent.feedback"));
        assert!(tmpl.content.contains("re-publish"));
    }

    // 9.4: Standard location skill loading
    #[test]
    #[serial(directory_changes)]
    fn standard_location_skill_loading() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create skill in standard location
        let skill_dir = project_dir
            .join(".agents")
            .join("skills")
            .join("coordination");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let skill_md_content = "---\nname: coordination\ndescription: Custom coordination skill\n---\n\ncustom skill content";
        std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        // Change to project directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let tmpl = resolve("coordination").expect("should resolve");
        assert_eq!(tmpl.source, Source::AgentsStandard);
        assert!(tmpl.content.contains("custom skill content"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    // 9.9: Unknown skill name returns error
    #[test]
    fn unknown_skill_returns_error() {
        let result = resolve("nonexistent");
        assert!(
            matches!(result, Err(SkillError::UnknownSkill { ref name }) if name == "nonexistent"),
            "expected UnknownSkill error, got {result:?}"
        );
    }

    // 9.10: {{BRANCH_ID}} is substituted
    #[test]
    fn branch_id_is_substituted() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "agent_id:\"{{BRANCH_ID}}\"".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(
            &tmpl,
            "feat/http-broker",
            "http://127.0.0.1:9119",
            "git-paw",
            None,
        );
        assert!(output.contains("feat-http-broker"));
        assert!(!output.contains("{{BRANCH_ID}}"));
    }

    // 9.11: {{GIT_PAW_BROKER_URL}} is substituted at render time
    #[test]
    fn broker_url_placeholder_substituted() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "curl {{GIT_PAW_BROKER_URL}}/status".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert!(output.contains("http://127.0.0.1:9119/status"));
        assert!(!output.contains("{{GIT_PAW_BROKER_URL}}"));
    }

    // 9.12: Slug substitution matches slugify_branch
    #[test]
    fn slug_substitution_matches_slugify_branch() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "id={{BRANCH_ID}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(
            &tmpl,
            "Feature/HTTP_Broker",
            "http://127.0.0.1:9119",
            "git-paw",
            None,
        );
        let expected = slugify_branch("Feature/HTTP_Broker");
        assert_eq!(output, format!("id={expected}"));
    }

    // 9.13: Render is deterministic
    #[test]
    fn render_is_deterministic() {
        let tmpl = resolve("coordination").unwrap();
        let a = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        let b = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert_eq!(a, b);
    }

    // 9.14: Render performs no I/O (resolve then render after "deletion")
    #[test]
    #[serial(directory_changes)]
    fn render_performs_no_io() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let skill_dir = project_dir
            .join(".agents")
            .join("skills")
            .join("coordination");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let skill_md_content = "---\nname: coordination\ndescription: Test coordination skill\n---\n\nuser {{BRANCH_ID}}";
        std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        // Change to project directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let tmpl = resolve("coordination").unwrap();
        assert_eq!(tmpl.source, Source::AgentsStandard);

        // Delete the skill directory — render must still succeed from in-memory content
        std::fs::remove_dir_all(skill_dir).unwrap();
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert!(output.contains("feat-x"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    // 9.15: Unknown placeholder survives in output (warning is emitted to stderr)
    #[test]
    fn unknown_placeholder_survives() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "url={{UNKNOWN_THING}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert!(
            output.contains("{{UNKNOWN_THING}}"),
            "unknown placeholder should survive in output"
        );
    }

    // 9.16: No {{...}} remains after rendering the embedded coordination template
    #[test]
    fn no_unknown_placeholders_after_render() {
        let tmpl = resolve("coordination").unwrap();
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert!(
            !output.contains("{{"),
            "no double-curly placeholders should remain: {output}"
        );
    }

    // Supervisor skill is reachable as an embedded default
    #[test]
    fn embedded_supervisor_is_reachable() {
        let tmpl = resolve("supervisor").expect("should resolve supervisor");
        assert_eq!(tmpl.source, Source::Embedded);
        assert!(!tmpl.content.is_empty());
    }

    // Supervisor skill contains role definition
    #[test]
    fn supervisor_skill_contains_role_definition() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(tmpl.content.contains("do NOT write code"));
    }

    // Supervisor skill contains broker status endpoint
    #[test]
    fn supervisor_skill_contains_broker_status() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(tmpl.content.contains("{{GIT_PAW_BROKER_URL}}/status"));
    }

    // Supervisor skill contains verified and feedback message types
    #[test]
    fn supervisor_skill_contains_verified_and_feedback() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(tmpl.content.contains("agent.verified"));
        assert!(tmpl.content.contains("agent.feedback"));
    }

    // Supervisor skill contains tmux commands targeting the session name
    #[test]
    fn supervisor_skill_contains_tmux_commands() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(tmpl.content.contains("tmux capture-pane"));
        assert!(tmpl.content.contains("tmux send-keys"));
        assert!(tmpl.content.contains("paw-{{PROJECT_NAME}}"));
    }

    #[test]
    fn supervisor_skill_contains_spec_audit_procedure() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Spec Audit"),
            "supervisor skill should contain Spec Audit section"
        );
        assert!(
            tmpl.content.contains("openspec/changes/"),
            "should reference openspec/changes/ for spec file discovery"
        );
        assert!(
            tmpl.content.contains("grep"),
            "should instruct to grep for matching tests"
        );
    }

    #[test]
    fn supervisor_skill_spec_audit_after_test_before_verified() {
        let tmpl = resolve("supervisor").unwrap();
        let test_pos = tmpl.content.find("Regression check").unwrap_or(0);
        let audit_pos = tmpl.content.find("Spec Audit").unwrap_or(0);
        let verify_pos = tmpl.content.find("Verify or feedback").unwrap_or(0);
        assert!(
            audit_pos > test_pos,
            "spec audit should appear after test/regression check"
        );
        assert!(
            audit_pos < verify_pos,
            "spec audit should appear before verify/feedback"
        );
    }

    // {{PROJECT_NAME}} is substituted by render
    #[test]
    fn project_name_is_substituted() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "session=paw-{{PROJECT_NAME}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "my-app", None);
        assert!(output.contains("paw-my-app"));
        assert!(!output.contains("{{PROJECT_NAME}}"));
    }

    // Both BRANCH_ID and PROJECT_NAME substituted in the same template
    #[test]
    fn branch_id_and_project_name_both_substituted() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "agent={{BRANCH_ID}} session=paw-{{PROJECT_NAME}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(&tmpl, "feat/http-broker", "url", "git-paw", None);
        assert!(output.contains("feat-http-broker"));
        assert!(output.contains("paw-git-paw"));
        assert!(!output.contains("{{BRANCH_ID}}"));
        assert!(!output.contains("{{PROJECT_NAME}}"));
    }

    // Standardized skill format is detected and loaded
    #[test]
    #[serial(directory_changes)]
    fn standardized_skill_format_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let skill_dir = project_dir
            .join(".agents")
            .join("skills")
            .join("test-standardized");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let skill_md_content = "---\nname: test-standardized\ndescription: A test standardized skill\n---\n\nThis is the skill content with {{BRANCH_ID}} placeholder.";
        std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        // Change to project directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let tmpl = resolve("test-standardized").expect("should resolve");
        assert_eq!(tmpl.format, SkillFormat::Standardized);
        assert!(tmpl.content.contains("This is the skill content"));
        assert!(tmpl.content.contains("{{BRANCH_ID}}"));
        assert!(tmpl.metadata.is_some());
        let metadata = tmpl.metadata.as_ref().unwrap();
        assert_eq!(metadata.name, "test-standardized");
        assert_eq!(metadata.description, "A test standardized skill");

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    // Standardized skill with resources loads resource paths
    #[test]
    fn standardized_skill_with_resources_loads_paths() {
        let dir = tempfile::tempdir().unwrap();
        let skills_parent_dir = dir.path().join("git-paw").join("agent-skills");
        let specific_skill_dir = skills_parent_dir.join("test-with-resources");
        std::fs::create_dir_all(&specific_skill_dir).unwrap();

        // Create skill directory structure
        std::fs::create_dir_all(specific_skill_dir.join("scripts")).unwrap();
        std::fs::create_dir_all(specific_skill_dir.join("references")).unwrap();
        std::fs::create_dir_all(specific_skill_dir.join("assets")).unwrap();

        let skill_md_content = "---\nname: test-with-resources\ndescription: Skill with resources\n---\n\nMain content here.";
        std::fs::write(specific_skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        let tmpl = resolve_with_config_dir("test-with-resources", Some(dir.path()))
            .expect("should resolve");
        assert_eq!(tmpl.format, SkillFormat::Standardized);
        assert!(tmpl.resource_paths.is_some());
        let resource_paths = tmpl.resource_paths.as_ref().unwrap();
        assert_eq!(resource_paths.len(), 3);
        assert!(resource_paths.iter().any(|p| p.ends_with("scripts")));
        assert!(resource_paths.iter().any(|p| p.ends_with("references")));
        assert!(resource_paths.iter().any(|p| p.ends_with("assets")));
    }

    // Standard location (.agents/skills/) loading
    #[test]
    #[serial(directory_changes)]
    fn standard_location_loading() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        // Create skill in standard location
        let standard_skill_dir = project_dir
            .join(".agents")
            .join("skills")
            .join("test-skill");
        std::fs::create_dir_all(&standard_skill_dir).unwrap();
        let standard_content = "---\nname: test-skill\ndescription: Standard location skill\n---\n\nContent from .agents/skills/";
        std::fs::write(standard_skill_dir.join("SKILL.md"), standard_content).unwrap();

        // Change to project directory so .agents/skills/ can be found
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let tmpl = resolve("test-skill").expect("should resolve");

        // Should load from standard location
        assert_eq!(tmpl.source, Source::AgentsStandard);
        assert!(tmpl.content.contains("Content from .agents/skills/"));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    // Standardized skill metadata placeholders are substituted
    #[test]
    fn standardized_skill_metadata_placeholders_are_substituted() {
        let metadata = StandardizedSkillMetadata {
            name: "test-skill".to_string(),
            description: "Test description".to_string(),
            license: None,
            compatibility: None,
            metadata: None,
        };

        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "Name: {{SKILL_NAME}}, Desc: {{SKILL_DESCRIPTION}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: Some(metadata),
            resource_paths: None,
        };

        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119", "git-paw", None);
        assert!(output.contains("Name: test-skill, Desc: Test description"));
        assert!(!output.contains("{{SKILL_NAME}}"));
        assert!(!output.contains("{{SKILL_DESCRIPTION}}"));
    }

    #[test]
    fn test_command_placeholder_substitutes_when_set() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run `{{TEST_COMMAND}}` after each merge.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            Some("just check"),
        );
        assert_eq!(output, "Run `just check` after each merge.");
        assert!(!output.contains("{{TEST_COMMAND}}"));
    }

    #[test]
    fn test_command_placeholder_falls_back_when_unset() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Baseline: {{TEST_COMMAND}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            None,
        );
        assert_eq!(output, "Baseline: (not configured)");
        assert!(!output.contains("{{TEST_COMMAND}}"));
    }

    #[test]
    fn supervisor_template_no_unsubstituted_placeholders_when_test_command_set() {
        // Regression: rendering the embedded supervisor skill with a configured
        // test_command must NOT leave {{TEST_COMMAND}} in the output. Captured
        // during a live dogfood run that produced the warning
        // "unsubstituted placeholder {{TEST_COMMAND}} in skill 'supervisor'".
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            Some("just check"),
        );
        assert!(
            !output.contains("{{TEST_COMMAND}}"),
            "supervisor template still contains a literal {{TEST_COMMAND}} after render"
        );
        assert!(
            !output.contains("{{"),
            "supervisor template has unsubstituted {{...}} placeholder after render"
        );
    }

    // Invalid standardized skill frontmatter returns validation error
    #[test]
    fn invalid_standardized_skill_frontmatter_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let skill_dir = project_dir
            .join(".agents")
            .join("skills")
            .join("invalid-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        // Missing required 'description' field
        let skill_md_content = "---\nname: invalid-skill\n---\n\nContent here.";
        std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        // Change to project directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let result = resolve("invalid-skill");
        assert!(matches!(result, Err(SkillError::ValidationError { .. })));

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    // 9.17: SkillTemplate is cloneable
    #[test]
    fn skill_template_is_cloneable() {
        let tmpl = resolve("coordination").unwrap();
        let cloned = tmpl.clone();
        assert_eq!(tmpl.name, cloned.name);
        assert_eq!(tmpl.content, cloned.content);
        assert_eq!(tmpl.source, cloned.source);
    }

    // Boot block function tests
    #[test]
    fn boot_block_contains_all_four_essential_events() {
        let block = build_boot_block("feat/errors", "http://localhost:9119");
        assert!(
            block.contains("### 1. REGISTER"),
            "Missing REGISTER section"
        );
        assert!(block.contains("### 2. DONE"), "Missing DONE section");
        assert!(block.contains("### 3. BLOCKED"), "Missing BLOCKED section");
        assert!(
            block.contains("### 4. QUESTION"),
            "Missing QUESTION section"
        );
    }

    #[test]
    fn boot_block_substitutes_branch_id_placeholder() {
        let block = build_boot_block("Feature/HTTP_Broker", "http://localhost:9119");
        assert!(
            block.contains("feature-http_broker"),
            "Branch ID not properly slugified"
        );
        assert!(
            !block.contains("{{BRANCH_ID}}"),
            "BRANCH_ID placeholder not substituted"
        );
    }

    #[test]
    fn boot_block_substitutes_broker_url_placeholder() {
        let block = build_boot_block("feat/x", "http://127.0.0.1:9119");
        assert!(
            block.contains("http://127.0.0.1:9119/publish"),
            "Broker URL not substituted"
        );
        assert!(
            !block.contains("{{GIT_PAW_BROKER_URL}}"),
            "GIT_PAW_BROKER_URL placeholder not substituted"
        );
    }

    #[test]
    fn boot_block_contains_paste_handling_instructions() {
        let block = build_boot_block("feat/x", "http://localhost:9119");
        assert!(
            block.contains("PASTE HANDLING"),
            "Missing paste handling section"
        );
        assert!(
            block.contains("additional Enter key"),
            "Missing Enter key instruction"
        );
        assert!(
            block.contains("[Pasted text #N]"),
            "Missing paste text reference"
        );
    }

    #[test]
    fn boot_block_question_section_emphasizes_waiting() {
        let block = build_boot_block("feat/x", "http://localhost:9119");
        assert!(
            block.contains("DO NOT CONTINUE UNTIL YOU RECEIVE AN ANSWER!"),
            "Missing wait emphasis"
        );
        assert!(
            block.contains("WAIT for the answer before continuing"),
            "Missing wait instruction"
        );
    }

    #[test]
    fn boot_block_is_deterministic() {
        let a = build_boot_block("feat/x", "http://localhost:9119");
        let b = build_boot_block("feat/x", "http://localhost:9119");
        assert_eq!(a, b, "Boot block generation should be deterministic");
    }

    #[test]
    fn boot_block_handles_complex_branch_names() {
        let block = build_boot_block("fix/topological-cycle-fallback", "http://localhost:9119");
        assert!(
            block.contains("fix-topological-cycle-fallback"),
            "Complex branch name not properly slugified"
        );
    }

    #[test]
    fn boot_block_contains_pre_expanded_curl_commands() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");

        // Check that all curl commands have the actual URL substituted
        assert!(
            block.contains("curl -s -X POST http://127.0.0.1:9119/publish"),
            "Curl commands not pre-expanded"
        );

        // Check that all curl commands have the actual branch ID substituted
        assert!(
            block.contains("\"agent_id\":\"feat-test\""),
            "Agent ID not substituted in curl commands"
        );
    }
}
