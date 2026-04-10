//! Agent skill template loading and rendering.
//!
//! Skills are markdown instruction files embedded into each worktree's `AGENTS.md`
//! to teach AI agents how to use git-paw capabilities (e.g. the coordination broker).
//!
//! ## Resolution order
//!
//! When a skill is requested by name, the system checks two locations in order:
//!
//! 1. **User override** — `<config_dir>/git-paw/agent-skills/<name>.md`
//! 2. **Embedded default** — compiled into the binary via `include_str!`
//!
//! The first match wins. If neither exists, resolution fails with
//! [`SkillError::UnknownSkill`].
//!
//! ## Substitution rules
//!
//! During [`render`], the template content undergoes placeholder substitution:
//!
//! - `{{BRANCH_ID}}` is replaced with the slugified branch name
//! - `${GIT_PAW_BROKER_URL}` is left untouched for shell-time expansion

use std::path::{Path, PathBuf};

/// The embedded coordination skill, compiled into the binary.
///
/// New embedded skills are added by adding a new `include_str!` constant
/// and a corresponding match arm in [`embedded_default`].
const COORDINATION_DEFAULT: &str = include_str!("../assets/agent-skills/coordination.md");

/// Indicates where a resolved skill's content originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Content came from the binary's compiled-in default.
    Embedded,
    /// Content came from a user override file in the config directory.
    User,
}

/// A loaded skill template ready for rendering.
#[derive(Debug, Clone)]
pub struct SkillTemplate {
    /// The skill name (e.g. `"coordination"`).
    pub name: String,
    /// The unrendered template content with placeholders.
    pub content: String,
    /// Where the content was loaded from.
    pub source: Source,
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

    /// A user override file exists but cannot be read.
    #[error("cannot read skill override at '{}' — check file permissions and encoding", path.display())]
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
        _ => None,
    }
}

/// Attempts to load a user override file for the given skill name.
///
/// The override path is `<config_dir>/git-paw/agent-skills/<skill_name>.md`.
///
/// ## Error handling contract
///
/// - Missing config directory → `Ok(None)` (normal, no override available)
/// - Missing `agent-skills/` subdirectory → `Ok(None)`
/// - Missing skill file → `Ok(None)`
/// - File exists but is unreadable (permissions, invalid UTF-8) →
///   `Err(SkillError::UserOverrideRead)` — this is a hard error to make
///   misconfiguration visible rather than silently falling back to defaults.
fn try_load_user_override(
    skill_name: &str,
    config_dir_override: Option<&Path>,
) -> Result<Option<String>, SkillError> {
    let config_dir = match config_dir_override {
        Some(dir) => dir.to_path_buf(),
        None => match crate::dirs::config_dir() {
            Some(dir) => dir,
            None => return Ok(None),
        },
    };

    let path = config_dir
        .join("git-paw")
        .join("agent-skills")
        .join(format!("{skill_name}.md"));

    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SkillError::UserOverrideRead { path, source }),
    }
}

/// Resolves a skill template by name.
///
/// Checks for a user override first, then falls back to the embedded default.
/// Returns [`SkillError::UnknownSkill`] if neither source has the skill.
pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError> {
    resolve_with_config_dir(skill_name, None)
}

/// Internal resolver that accepts an optional config directory override for testing.
fn resolve_with_config_dir(
    skill_name: &str,
    config_dir: Option<&Path>,
) -> Result<SkillTemplate, SkillError> {
    if let Some(content) = try_load_user_override(skill_name, config_dir)? {
        return Ok(SkillTemplate {
            name: skill_name.to_string(),
            content,
            source: Source::User,
        });
    }

    if let Some(content) = embedded_default(skill_name) {
        return Ok(SkillTemplate {
            name: skill_name.to_string(),
            content: content.to_string(),
            source: Source::Embedded,
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

/// Renders a skill template for a specific worktree.
///
/// Substitutes `{{BRANCH_ID}}` with the slugified branch name. The
/// `${GIT_PAW_BROKER_URL}` placeholder is left untouched so the agent's
/// shell expands it at command-execution time.
///
/// The `broker_url` parameter is accepted for forward compatibility (e.g.
/// embedding the URL at render time as an alternative mode) but is **not**
/// substituted into the output in v0.3.0.
pub fn render(template: &SkillTemplate, branch: &str, _broker_url: &str) -> String {
    let branch_id = slugify_branch(branch);
    let output = template.content.replace("{{BRANCH_ID}}", &branch_id);

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
                .contains("${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}")
        );
    }

    // 9.4: User override is preferred
    #[test]
    fn user_override_is_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("git-paw").join("agent-skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("coordination.md"), "custom user content").unwrap();

        let tmpl =
            resolve_with_config_dir("coordination", Some(dir.path())).expect("should resolve");
        assert_eq!(tmpl.source, Source::User);
        assert_eq!(tmpl.content, "custom user content");
    }

    // 9.5: Missing user config directory falls through
    #[test]
    fn missing_config_dir_falls_through() {
        let nonexistent = PathBuf::from("/tmp/git-paw-test-nonexistent-dir-abc123");
        let result = try_load_user_override("coordination", Some(&nonexistent)).unwrap();
        assert!(result.is_none());
    }

    // 9.6: Missing agent-skills subdirectory falls through
    #[test]
    fn missing_agent_skills_subdir_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        // Create git-paw/ but not git-paw/agent-skills/
        std::fs::create_dir_all(dir.path().join("git-paw")).unwrap();
        let result = try_load_user_override("coordination", Some(dir.path())).unwrap();
        assert!(result.is_none());
    }

    // 9.7: Missing skill file falls through
    #[test]
    fn missing_skill_file_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("git-paw").join("agent-skills")).unwrap();
        let result = try_load_user_override("coordination", Some(dir.path())).unwrap();
        assert!(result.is_none());
    }

    // 9.8: Unreadable user override returns hard error
    #[cfg(unix)]
    #[test]
    fn unreadable_override_returns_hard_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("git-paw").join("agent-skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        let file_path = skills_dir.join("coordination.md");
        std::fs::write(&file_path, "secret").unwrap();
        std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000)).unwrap();

        let result = try_load_user_override("coordination", Some(dir.path()));
        assert!(
            matches!(result, Err(SkillError::UserOverrideRead { .. })),
            "expected UserOverrideRead error, got {result:?}"
        );
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
        };
        let output = render(&tmpl, "feat/http-broker", "http://127.0.0.1:9119");
        assert!(output.contains("feat-http-broker"));
        assert!(!output.contains("{{BRANCH_ID}}"));
    }

    // 9.11: ${GIT_PAW_BROKER_URL} is preserved verbatim
    #[test]
    fn broker_url_placeholder_preserved() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "curl ${GIT_PAW_BROKER_URL}/status".into(),
            source: Source::Embedded,
        };
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        assert!(output.contains("${GIT_PAW_BROKER_URL}"));
    }

    // 9.12: Slug substitution matches slugify_branch
    #[test]
    fn slug_substitution_matches_slugify_branch() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "id={{BRANCH_ID}}".into(),
            source: Source::Embedded,
        };
        let output = render(&tmpl, "Feature/HTTP_Broker", "http://127.0.0.1:9119");
        let expected = slugify_branch("Feature/HTTP_Broker");
        assert_eq!(output, format!("id={expected}"));
    }

    // 9.13: Render is deterministic
    #[test]
    fn render_is_deterministic() {
        let tmpl = resolve("coordination").unwrap();
        let a = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        let b = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        assert_eq!(a, b);
    }

    // 9.14: Render performs no I/O (resolve then render after "deletion")
    #[test]
    fn render_performs_no_io() {
        let dir = tempfile::tempdir().unwrap();
        let skills_dir = dir.path().join("git-paw").join("agent-skills");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("coordination.md"), "user {{BRANCH_ID}}").unwrap();

        let tmpl = resolve_with_config_dir("coordination", Some(dir.path())).unwrap();
        assert_eq!(tmpl.source, Source::User);

        // Delete the override file — render must still succeed from in-memory content
        std::fs::remove_file(skills_dir.join("coordination.md")).unwrap();
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        assert!(output.contains("feat-x"));
    }

    // 9.15: Unknown placeholder survives in output (warning is emitted to stderr)
    #[test]
    fn unknown_placeholder_survives() {
        let tmpl = SkillTemplate {
            name: "test".into(),
            content: "url={{UNKNOWN_THING}}".into(),
            source: Source::Embedded,
        };
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        assert!(
            output.contains("{{UNKNOWN_THING}}"),
            "unknown placeholder should survive in output"
        );
    }

    // 9.16: No {{...}} remains after rendering the embedded coordination template
    #[test]
    fn no_unknown_placeholders_after_render() {
        let tmpl = resolve("coordination").unwrap();
        let output = render(&tmpl, "feat/x", "http://127.0.0.1:9119");
        assert!(
            !output.contains("{{"),
            "no double-curly placeholders should remain: {output}"
        );
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
}
