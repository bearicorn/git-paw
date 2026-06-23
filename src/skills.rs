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
/// * `broker_url` - The fully-qualified broker URL. Retained for signature
///   stability and any future broker-URL placeholder; the boot block no
///   longer inlines the URL — each event calls `.git-paw/scripts/broker.sh`
///   (the `agent-broker-helper` capability), which discovers the URL itself.
///
/// # Returns
///
/// A string containing the complete boot instruction block with the
/// `{{BRANCH_ID}}` placeholder pre-expanded so each `broker.sh` invocation
/// carries the agent's literal id.
pub fn build_boot_block(branch_id: &str, broker_url: &str) -> String {
    let template = include_str!("../assets/boot-block-template.md");
    let slugified_branch = slugify_branch(branch_id);

    template
        .replace("{{BRANCH_ID}}", &slugified_branch)
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
}

/// Borrowed view of the seven gate-command templates substituted by
/// [`render`] into the supervisor skill.
///
/// Each field maps to a `{{...}}` placeholder in the skill template (see
/// [`render`] for the full list). `None` renders as the literal
/// `(not configured)` so the rendered prose stays readable and the
/// supervisor agent can machine-check the value to decide whether to skip
/// the tooling invocation for that gate.
///
/// `{{CHANGE_ID}}` is NOT a field here: it is a per-invocation placeholder
/// substituted by the supervisor agent at verification time (using the
/// change being audited), not by `render` at session boot.
///
/// Use [`SupervisorConfig::gate_commands`](crate::config::SupervisorConfig::gate_commands)
/// to build one from a config.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateCommands<'a> {
    /// Renders into `{{TEST_COMMAND}}`. Gate 1 test runner.
    pub test_command: Option<&'a str>,
    /// Renders into `{{LINT_COMMAND}}`. Gate 1 lint sub-step.
    pub lint_command: Option<&'a str>,
    /// Renders into `{{BUILD_COMMAND}}`. Gate 1 build sub-step.
    pub build_command: Option<&'a str>,
    /// Renders into `{{DOC_BUILD_COMMAND}}`. Gate 4 doc builder.
    pub doc_build_command: Option<&'a str>,
    /// Renders into `{{SPEC_VALIDATE_COMMAND}}`. Gate 3 spec validator.
    /// MAY contain a `{{CHANGE_ID}}` substring that the supervisor agent
    /// expands at verification time — `render` does NOT substitute it.
    pub spec_validate_command: Option<&'a str>,
    /// Renders into `{{FMT_CHECK_COMMAND}}`. Gate 1 format check.
    pub fmt_check_command: Option<&'a str>,
    /// Renders into `{{SECURITY_AUDIT_COMMAND}}`. Gate 5 security tooling.
    pub security_audit_command: Option<&'a str>,
    /// Renders into `{{DOC_TOOL_COMMAND}}`. Gate 4 API-doc generator,
    /// distinct from [`Self::doc_build_command`] which builds the human
    /// doc site. `None` renders as an empty string so the surrounding
    /// prose can read naturally without a stray `(not configured)` token
    /// — the supervisor template is authored to handle the empty case.
    pub doc_tool_command: Option<&'a str>,
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
/// - `{{LINT_COMMAND}}`, `{{BUILD_COMMAND}}`, `{{DOC_BUILD_COMMAND}}`,
///   `{{SPEC_VALIDATE_COMMAND}}`, `{{FMT_CHECK_COMMAND}}`,
///   `{{SECURITY_AUDIT_COMMAND}}` — the five additional gate commands
///   from `[supervisor]` config. `None` renders as `(not configured)`,
///   identical to `{{TEST_COMMAND}}` behaviour.
///
/// `{{CHANGE_ID}}` is **not** substituted here. The spec-validate command
/// typically embeds `{{CHANGE_ID}}` as a per-invocation placeholder that
/// the supervisor agent expands at verification time using the change name
/// being audited. Substituting it at render time would freeze the rendered
/// skill to a single change, which is wrong — the supervisor verifies
/// many changes over a session lifetime.
///
/// ## Language-agnostic supervisor placeholders
///
/// Three additional placeholders make the bundled supervisor skill render
/// correctly across language stacks without forking the template per
/// language family:
///
/// - `{{DOC_TOOL_COMMAND}}` — substitutes
///   `[supervisor].doc_tool_command` from config. Renders as the empty
///   string when unset (the template prose is authored to read
///   naturally without it; this avoids a stray `(not configured)` in
///   places where empty reads fine).
/// - `{{DEV_ALLOWLIST_PRESET}}` — substitutes a prose enumeration of
///   every entry in `DEV_ALLOWLIST_PRESET`, generated from the
///   constant so adding new entries does not require a skill-template
///   edit. See [`render_dev_allowlist_preset`].
/// - `{{SPEC_PATH_DOCTRINE}}` — substitutes a per-backend path doctrine
///   paragraph derived from the `backends` slice. Sessions resolving no
///   backend render a sentinel sentence; multi-backend sessions render
///   a paragraph listing each present backend's path conventions. See
///   [`render_spec_path_doctrine`].
///
/// The `backends` parameter is the session's resolved spec backends
/// (typically derived from `SpecEntry.backend` across the session's
/// `scan_specs(...)` result). For non-supervisor renders or sessions
/// without resolved specs, pass `&[]` and the doctrine placeholder
/// renders its sentinel.
///
/// Any remaining `{{...}}` placeholder after substitution is logged as a
/// warning to stderr but does not cause `render` to fail. The
/// `{{CHANGE_ID}}` form is whitelisted from this warning since the spec
/// expects it to survive intact (see the `agent-skills` spec delta).
///
/// For standardized skills, additional metadata placeholders may be available:
/// - `{{SKILL_NAME}}` — the skill name from metadata
/// - `{{SKILL_DESCRIPTION}}` — the skill description from metadata
pub fn render(
    template: &SkillTemplate,
    branch: &str,
    broker_url: &str,
    project: &str,
    gates: &GateCommands<'_>,
    backends: &[crate::specs::SpecBackendKind],
) -> String {
    const NOT_CONFIGURED: &str = "(not configured)";
    let branch_id = slugify_branch(branch);

    // Start with basic substitutions. Gate-command placeholders use the
    // literal `(not configured)` when the source value is `None` so the
    // rendered prose remains readable AND the supervisor agent can branch
    // on it to skip the tooling invocation.
    //
    // `{{DOC_TOOL_COMMAND}}` is the exception: it renders as an empty
    // string when unset because the supervisor template is authored so
    // the surrounding prose reads naturally without the value (per D5).
    let allowlist_prose = render_dev_allowlist_preset();
    let spec_doctrine = render_spec_path_doctrine(backends);
    let mut output = template
        .content
        .replace("{{BRANCH_ID}}", &branch_id)
        .replace("{{PROJECT_NAME}}", project)
        .replace("{{GIT_PAW_BROKER_URL}}", broker_url)
        .replace(
            "{{TEST_COMMAND}}",
            gates.test_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{LINT_COMMAND}}",
            gates.lint_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{BUILD_COMMAND}}",
            gates.build_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{DOC_BUILD_COMMAND}}",
            gates.doc_build_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{SPEC_VALIDATE_COMMAND}}",
            gates.spec_validate_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{FMT_CHECK_COMMAND}}",
            gates.fmt_check_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace(
            "{{SECURITY_AUDIT_COMMAND}}",
            gates.security_audit_command.unwrap_or(NOT_CONFIGURED),
        )
        .replace("{{DOC_TOOL_COMMAND}}", gates.doc_tool_command.unwrap_or(""))
        .replace("{{DEV_ALLOWLIST_PRESET}}", &allowlist_prose)
        .replace("{{SPEC_PATH_DOCTRINE}}", &spec_doctrine);

    // `{{CHANGE_ID}}` is intentionally NOT substituted: it is a
    // per-invocation placeholder owned by the supervisor agent at
    // verification time. It survives render verbatim and is expanded
    // when the supervisor runs spec-validate against a specific change.

    // Add metadata substitutions for standardized skills
    if let Some(metadata) = &template.metadata {
        output = output
            .replace("{{SKILL_NAME}}", &metadata.name)
            .replace("{{SKILL_DESCRIPTION}}", &metadata.description);
    }

    // Resolve the opsx role-gating regions. The forbidden-command sections are
    // scoped to the OpenSpec engine: kept when an OpenSpec backend is resolved
    // for the session, stripped entirely otherwise (speckit/markdown/none). The
    // region markers themselves are always removed. See the `opsx-role-gating`
    // spec, "Role-gating is scoped to the OpenSpec spec engine".
    let opsx_active = backends
        .iter()
        .any(|b| matches!(b, crate::specs::SpecBackendKind::OpenSpec));
    output = render_opsx_regions(&output, opsx_active);

    // Warn about any remaining {{...}} placeholders that were not consumed,
    // except `{{CHANGE_ID}}` which is whitelisted (see comment above).
    let mut start = 0;
    while let Some(open) = output[start..].find("{{") {
        let abs_open = start + open;
        if let Some(close) = output[abs_open..].find("}}") {
            let placeholder = &output[abs_open..abs_open + close + 2];
            if placeholder != "{{CHANGE_ID}}" {
                eprintln!(
                    "warning: unsubstituted placeholder {placeholder} in skill '{}'",
                    template.name
                );
            }
            start = abs_open + close + 2;
        } else {
            break;
        }
    }

    output
}

/// Marker opening an opsx role-gating region in a bundled skill template.
pub(crate) const OPSX_REGION_BEGIN: &str = "<!-- opsx-role-gating:begin -->";
/// Marker closing an opsx role-gating region in a bundled skill template.
pub(crate) const OPSX_REGION_END: &str = "<!-- opsx-role-gating:end -->";

/// Resolves the opsx role-gating regions delimited by [`OPSX_REGION_BEGIN`] /
/// [`OPSX_REGION_END`] in a rendered skill.
///
/// The marker lines are always dropped. When `keep` is `true` (the session's
/// resolved spec engine is `OpenSpec`) the region body is retained; when `false`
/// (speckit / markdown / no engine) the body is stripped along with the
/// markers, so the `/opsx:` forbidden-command sections never render under a
/// non-OpenSpec engine. Operates line-wise so a region that is left unclosed
/// degrades gracefully (the trailing lines are simply kept or dropped per the
/// current region state at end-of-input).
#[must_use]
pub(crate) fn render_opsx_regions(input: &str, keep: bool) -> String {
    let has_trailing_newline = input.ends_with('\n');
    let mut kept: Vec<&str> = Vec::new();
    let mut in_region = false;
    for line in input.split('\n') {
        let trimmed = line.trim();
        if trimmed == OPSX_REGION_BEGIN {
            in_region = true;
            continue;
        }
        if trimmed == OPSX_REGION_END {
            in_region = false;
            continue;
        }
        if in_region && !keep {
            continue;
        }
        kept.push(line);
    }
    let mut out = kept.join("\n");
    if has_trailing_newline {
        out.push('\n');
    }
    out
}

/// Sentinel rendered for `{{SPEC_PATH_DOCTRINE}}` when no spec backend
/// has been resolved for the session. Authored as a complete sentence so
/// the rendered output is grammatical even when no backend is present.
pub(crate) const SPEC_DOCTRINE_NO_BACKEND_SENTINEL: &str = "(no spec backend resolved for this session — see your project's documentation for where specs live.)";

/// Renders the bundled `DEV_ALLOWLIST_PRESET` constant into a
/// prose-friendly listing, grouped by first-word command family.
///
/// The output enumerates every entry from
/// [`crate::supervisor::dev_allowlist::DEV_ALLOWLIST_PRESET`] so adding
/// a new entry to the constant immediately changes the rendered prose
/// without requiring a skill-template edit. Entries that share a
/// prefix word (e.g. `cargo build`, `cargo test`) collapse into a
/// single `cargo (build, test, …)` group; single-word entries (`just`,
/// `find`, `grep`) appear bare. Multi-word entries with a unique first
/// word (`sed -n`) appear verbatim.
///
/// The result is a single semicolon-separated paragraph fragment that
/// callers can embed inline in skill prose.
#[must_use]
pub fn render_dev_allowlist_preset() -> String {
    use crate::supervisor::dev_allowlist::DEV_ALLOWLIST_PRESET;

    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for entry in DEV_ALLOWLIST_PRESET {
        let (head, tail) = match entry.split_once(' ') {
            Some((h, t)) => (h.to_string(), Some(t.to_string())),
            None => (entry.to_string(), None),
        };
        if let Some(existing) = groups.iter_mut().find(|(h, _)| h == &head) {
            if let Some(t) = tail {
                existing.1.push(t);
            }
        } else {
            groups.push((head, tail.into_iter().collect()));
        }
    }

    let parts: Vec<String> = groups
        .into_iter()
        .map(|(head, members)| {
            if members.is_empty() {
                head
            } else if members.len() == 1 {
                format!("{head} {}", members[0])
            } else {
                format!("{head} ({})", members.join(", "))
            }
        })
        .collect();
    parts.join("; ")
}

/// Renders the `{{SPEC_PATH_DOCTRINE}}` paragraph for the supervisor
/// skill based on the resolved session backends.
///
/// Each backend contributes a one-sentence path doctrine describing
/// where its specs live and the per-backend workflow. When `backends`
/// is empty the sentinel
/// [`SPEC_DOCTRINE_NO_BACKEND_SENTINEL`] is returned. When more than
/// one backend is present, every distinct backend's sentence is joined
/// into a single paragraph prefixed by an introductory clause so the
/// supervisor agent knows the session spans multiple formats.
#[must_use]
pub fn render_spec_path_doctrine(backends: &[crate::specs::SpecBackendKind]) -> String {
    use crate::specs::SpecBackendKind;

    let mut seen: Vec<SpecBackendKind> = Vec::new();
    for b in backends {
        if !seen.contains(b) {
            seen.push(*b);
        }
    }

    if seen.is_empty() {
        return SPEC_DOCTRINE_NO_BACKEND_SENTINEL.to_string();
    }

    let per_backend = |kind: SpecBackendKind| -> &'static str {
        match kind {
            SpecBackendKind::OpenSpec => {
                "OpenSpec specs live under `openspec/changes/<change-name>/{proposal,specs,tasks}.md` \
                 with archived deltas merged into `openspec/specs/`; run `openspec validate <change-name> --strict` \
                 to verify a change."
            }
            SpecBackendKind::SpecKit => {
                "Spec Kit specs live under `.specify/specs/<feature>/{spec,plan,tasks}.md` \
                 and use the Spec Kit checklist convention; mark `- [ ]` tasks complete as each one lands."
            }
            SpecBackendKind::Markdown => {
                "Markdown specs are flat `.md` files with `paw_status: pending` frontmatter; \
                 the format has no per-artifact workflow — the file itself is the contract."
            }
        }
    };

    if seen.len() == 1 {
        per_backend(seen[0]).to_string()
    } else {
        let intro =
            "This session spans multiple spec backends — apply the matching doctrine per spec:";
        let sentences: Vec<String> = seen
            .into_iter()
            .map(|b| format!("- {}", per_backend(b)))
            .collect();
        format!("{intro}\n{}", sentences.join("\n"))
    }
}

/// Canonical doc names for the `[governance]` paths, in the order they
/// appear in the supervisor boot prompt: `adr`, `test_strategy`, `security`,
/// `dod`, `constitution`. The canonical name is what shows up before the
/// path in each bullet (`- adr: docs/adr/`).
const GOVERNANCE_CANONICAL_NAMES: [&str; 5] =
    ["adr", "test_strategy", "security", "dod", "constitution"];

/// Renders the supervisor boot-prompt's `## Governance documents` section
/// from the five governance path fields, in canonical order.
///
/// Returns an empty `String` when every path is `None`. When at least one
/// path is set, the result is a self-contained block:
///
/// ```text
/// ## Governance documents
///
/// The supervisor consults these documents during spec audit.
///
/// - adr: docs/adr/
/// - dod: docs/dod.md
/// ```
///
/// The bullet list is built from the configured paths only — fields whose
/// value is `None` are skipped entirely (no placeholder line). The section
/// does not include any "gates" sub-line or per-doc enforcement metadata;
/// the `governance-config` capability dropped per-doc gate flags so there
/// is nothing to convey here beyond the paths themselves.
///
/// The caller is responsible for the blank line separating the section
/// from preceding boot-prompt content. When this function returns the
/// empty string, the boot prompt remains byte-identical to its v0.4
/// shape.
pub fn governance_section_paths(
    adr: Option<&Path>,
    test_strategy: Option<&Path>,
    security: Option<&Path>,
    dod: Option<&Path>,
    constitution: Option<&Path>,
) -> String {
    let bullets: [Option<&Path>; 5] = [adr, test_strategy, security, dod, constitution];
    if bullets.iter().all(Option::is_none) {
        return String::new();
    }

    let mut out = String::with_capacity(192);
    out.push_str("## Governance documents\n");
    out.push('\n');
    out.push_str("The supervisor consults these documents during spec audit.\n");
    out.push('\n');
    for (name, path) in GOVERNANCE_CANONICAL_NAMES.iter().zip(bullets.iter()) {
        if let Some(p) = path {
            use std::fmt::Write as _;
            // `writeln!` into a `String` never fails — formatting to a
            // growable buffer cannot run out of capacity. The `let _ =`
            // discards the `fmt::Result` without panicking.
            let _ = writeln!(out, "- {name}: {}", p.display());
        }
    }
    out
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

    // === forward-coordination: existing-scenario coverage gaps ===

    #[test]
    fn coordination_skill_documents_automatic_status_publishing() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("publishes your status automatically")
                || lowered.contains("status publishing is automatic")
                || lowered.contains("publishes status automatically"),
            "coordination skill should indicate that agent.status publishing is automatic"
        );
        assert!(
            !tmpl.content.contains("MUST publish agent.status"),
            "coordination skill must not contain the legacy 'MUST publish agent.status' instruction"
        );
    }

    #[test]
    fn coordination_skill_contains_cherry_pick_instructions() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("git cherry-pick"),
            "coordination skill should contain the literal 'git cherry-pick' command"
        );
        assert!(
            tmpl.content.contains("Cherry-pick peer commits"),
            "coordination skill should contain a 'Cherry-pick peer commits' heading"
        );
    }

    /// `advanced-main-event` §5: the coordination skill SHALL include a "When
    /// main advances" subsection teaching the four-step polling discipline —
    /// polling source, the no-auto-rebase rule, the fetch+inspect+decide flow,
    /// and the commit-or-stash-first safety rule — plus a concrete
    /// uncommitted-edits example.
    #[test]
    fn coordination_skill_teaches_main_advances_discipline() {
        let tmpl = resolve("coordination").unwrap();
        let content = &tmpl.content;

        let idx = content
            .find("When main advances")
            .expect("coordination skill has a 'When main advances' subsection");
        let section = &content[idx..];
        let lowered = section.to_lowercase();

        // (1) Polling source: arrives on the normal /messages poll.
        assert!(
            section.contains("agent.advanced-main") && section.contains("/messages/{{BRANCH_ID}}"),
            "subsection must name the event and its delivery on the normal /messages poll"
        );
        // (2) No-auto-rebase rule with a safety rationale.
        assert!(
            lowered.contains("not auto-rebase")
                || lowered.contains("not trigger an automatic rebase"),
            "subsection must contain an explicit do-not-auto-rebase rule"
        );
        // (3) Fetch + inspect + decide flow.
        assert!(
            section.contains("git fetch origin")
                && section.contains("git log HEAD..origin/")
                && lowered.contains("decide"),
            "subsection must document the fetch + inspect + decide flow"
        );
        // (4) Commit-or-stash-first safety before any rebase.
        assert!(
            (lowered.contains("commit") || lowered.contains("stash"))
                && lowered.contains("before")
                && lowered.contains("rebase"),
            "subsection must require a commit or stash before any rebase"
        );
        // Concrete uncommitted-edits example.
        assert!(
            lowered.contains("uncommitted"),
            "subsection must include the concrete uncommitted-edits example"
        );
    }

    // === forward-coordination: agent.intent skill content ===

    #[test]
    fn coordination_skill_contains_before_you_start_editing_heading() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("Before you start editing"),
            "coordination skill should contain 'Before you start editing' heading"
        );
    }

    #[test]
    fn coordination_skill_contains_agent_intent_curl_example() {
        let tmpl = resolve("coordination").unwrap();
        let curl_pos = tmpl
            .content
            .find("agent.intent")
            .expect("coordination skill should mention agent.intent");
        // Look at a window around the intent example and assert all required
        // payload fields appear there.
        let window_start = curl_pos.saturating_sub(200);
        let window_end = (curl_pos + 800).min(tmpl.content.len());
        let window = &tmpl.content[window_start..window_end];
        assert!(
            window.contains("curl"),
            "agent.intent example should be a curl invocation"
        );
        assert!(
            window.contains("\"files\""),
            "agent.intent example should include the files field"
        );
        assert!(
            window.contains("\"summary\""),
            "agent.intent example should include the summary field"
        );
        assert!(
            window.contains("\"valid_for_seconds\""),
            "agent.intent example should include valid_for_seconds"
        );
    }

    #[test]
    fn coordination_skill_contains_while_youre_editing_heading() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("While you're editing"),
            "coordination skill should contain 'While you're editing' heading"
        );
    }

    #[test]
    fn coordination_skill_instructs_republish_on_scope_growth() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("scope grows") || lowered.contains("scope grow"),
            "coordination skill should instruct re-publishing when scope grows"
        );
        assert!(
            lowered.contains("re-publish"),
            "coordination skill should mention re-publishing the intent"
        );
    }

    #[test]
    fn coordination_skill_instructs_question_on_peer_intent_overlap() {
        let tmpl = resolve("coordination").unwrap();
        // The skill should tell agents to send agent.question on overlap, not
        // race the peer.
        assert!(
            tmpl.content.contains("agent.question"),
            "coordination skill should reference agent.question"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("overlap") || lowered.contains("overlapping"),
            "coordination skill should call out overlap as the trigger for agent.question"
        );
    }

    #[test]
    fn coordination_skill_contains_must_not_anti_pattern_statements() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("must not"),
            "coordination skill should contain explicit MUST NOT statements"
        );
        assert!(
            lowered.contains("pairwise"),
            "coordination skill should reject pairwise check-ins"
        );
        assert!(
            lowered.contains("go-ahead") || lowered.contains("go ahead"),
            "coordination skill should reject waiting for go-ahead"
        );
        assert!(
            lowered.contains("broker silence") || lowered.contains("silence"),
            "coordination skill should reject blocking on broker silence"
        );
    }

    #[test]
    fn supervisor_skill_contains_watch_peer_intents_section() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Watch peer intents"),
            "supervisor skill should contain 'Watch peer intents' heading"
        );
        assert!(
            tmpl.content.contains("agent.intent"),
            "supervisor skill should mention agent.intent"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("not part of this release") || lowered.contains("conflict-detection"),
            "supervisor skill should note that automatic conflict-warning logic is not part of this release"
        );
    }

    /// `supervisor-bugfixes-v0-5-x` §3.10: the rendered supervisor skill SHALL
    /// invoke `.git-paw/scripts/sweep.sh` for snapshot / capture / approve /
    /// verified / feedback-gate, and SHALL NOT include legacy multi-pane
    /// `for p in …; do tmux capture-pane` loops.
    #[test]
    fn supervisor_skill_references_bundled_sweep_helper() {
        let tmpl = resolve("supervisor").unwrap();
        let required = [
            ".git-paw/scripts/sweep.sh snapshot",
            ".git-paw/scripts/sweep.sh capture",
            ".git-paw/scripts/sweep.sh approve",
            ".git-paw/scripts/sweep.sh verified",
            ".git-paw/scripts/sweep.sh feedback-gate",
        ];
        for needle in required {
            assert!(
                tmpl.content.contains(needle),
                "supervisor skill should reference {needle:?}; content does not"
            );
        }
        assert!(
            !tmpl.content.contains("for p in 2 3 4 5"),
            "supervisor skill should not contain legacy `for p in 2 3 4 5` capture-pane loops"
        );
    }

    // --- supervisor-verify-scratch-dir: skill content ---

    /// `supervisor-verify-scratch-dir` §"Isolated verification worktrees use a
    /// repo-local gitignored scratch dir": the skill SHALL instruct creating the
    /// verify worktree under `.git-paw/tmp/` and SHALL NOT teach `/tmp` for
    /// verification scratch.
    #[test]
    fn supervisor_skill_uses_repo_local_verify_scratch_dir() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains(".git-paw/tmp/verify-"),
            "supervisor skill should name the repo-local verify scratch path .git-paw/tmp/verify-"
        );
        assert!(
            tmpl.content.contains("git worktree add --detach"),
            "supervisor skill should teach the `git worktree add --detach` verify recipe"
        );
        assert!(
            !tmpl.content.contains("/tmp/paw-verify"),
            "supervisor skill must not teach an OS-temp (/tmp/paw-verify) path for verify scratch"
        );
    }

    // --- supervisor-introspection: skill content (task 2.6) ---

    /// `supervisor-introspection` §"Supervisor phase taxonomy": the skill SHALL
    /// document an introspection section with a taxonomy table listing at least
    /// the seven v0.6.0 phase values.
    #[test]
    fn supervisor_skill_has_introspection_section_with_phase_taxonomy() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content
                .contains("### Introspection: what to publish and when"),
            "supervisor skill must include the introspection section"
        );
        for phase in [
            "sweep",
            "audit",
            "merge",
            "feedback",
            "intent_watch",
            "learnings",
            "idle",
        ] {
            assert!(
                tmpl.content.contains(phase),
                "the phase taxonomy must document the {phase:?} phase value"
            );
        }
        // The table documents detail field names, not just phase labels.
        for field in ["agents_checked", "audit_step", "intended_targets"] {
            assert!(
                tmpl.content.contains(field),
                "the taxonomy must document the {field:?} detail field"
            );
        }
    }

    /// `supervisor-introspection` scenario "Audit phase detail names the five
    /// gates": the audit detail's `audit_step` SHALL enumerate the v0.5.0 five
    /// gates (tests, spec, docs, security, regression).
    #[test]
    fn supervisor_skill_audit_step_enumerates_five_gates() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("audit_step"),
            "the audit phase must document the audit_step field"
        );
        for gate in ["tests", "regression", "spec", "docs", "security"] {
            assert!(
                tmpl.content.contains(gate),
                "audit_step must enumerate the {gate:?} gate"
            );
        }
    }

    /// `supervisor-introspection` scenario "Cadence rules documented in skill
    /// prose": emit on phase transition, rate-limit to ~30s within a phase,
    /// single-emit on idle.
    #[test]
    fn supervisor_skill_documents_emission_cadence() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("phase transition"),
            "cadence rules must require a status on every phase transition"
        );
        assert!(
            lowered.contains("30 second") || tmpl.content.contains("~30 seconds"),
            "cadence rules must document the ~30s rate-limit within a phase"
        );
        assert!(
            lowered.contains("idle"),
            "cadence rules must document the single-emit-on-idle rule"
        );
    }

    /// `supervisor-introspection` scenario "Checkpoint emission uses phase =
    /// checkpoint": the skill SHALL acknowledge `checkpoint` as a valid phase
    /// value and the checkpoint emission SHALL set `phase: "checkpoint"`.
    #[test]
    fn supervisor_skill_documents_checkpoint_phase() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("checkpoint"),
            "the skill must document the checkpoint phase value"
        );
        assert!(
            tmpl.content.contains("\"phase\":\"checkpoint\""),
            "the checkpoint emission example must set phase = checkpoint"
        );
    }

    /// `advanced-main-event` §4: the Merge orchestration section SHALL teach
    /// the supervisor to publish an `agent.advanced-main` event after a
    /// successful merge to main, with a concrete curl-to-`/publish` example,
    /// and SHALL document `base` as the resolved default-branch value rather
    /// than a hardcoded literal.
    #[test]
    fn supervisor_skill_publishes_advanced_main_after_merge() {
        let tmpl = resolve("supervisor").unwrap();
        let content = &tmpl.content;

        // The publish step lives inside the merge-orchestration procedure.
        let merge_idx = content
            .find("Merge orchestration")
            .expect("supervisor skill has a Merge orchestration section");
        let merge_section = &content[merge_idx..];

        assert!(
            merge_section.contains("agent.advanced-main"),
            "the merge section must teach publishing an agent.advanced-main event"
        );
        // A concrete curl-to-/publish example for the variant.
        assert!(
            merge_section.contains("/publish") && merge_section.contains("new_main_sha"),
            "the merge section must include a concrete curl /publish example carrying new_main_sha"
        );
        // The publish fires after a successful merge + passing tests.
        let lowered = merge_section.to_lowercase();
        assert!(
            lowered.contains("test command passes") || lowered.contains("after the merge succeeds"),
            "the publish step must fire after a successful merge"
        );
        // `base` is the resolved default-branch value, not hardcoded "main".
        assert!(
            merge_section.contains("$MAIN_BRANCH")
                && merge_section.contains("resolved default-branch"),
            "the example must source `base` from the resolved default branch, not a hardcoded literal"
        );
        assert!(
            !merge_section.contains("\"base\":\"main\"")
                && !merge_section.contains("\"base\": \"main\""),
            "the example must not hardcode base as the literal \"main\""
        );
    }

    // === supervisor-skill-discipline-v0-6-x: pane/git/commit disciplines ===

    /// Spec "Mandate sweep.sh; forbid inline pane loops": a section directs
    /// all pane work through sweep.sh and explicitly forbids `for p in ...;
    /// do tmux ...; done` loops with the `simple_expansion` rationale.
    #[test]
    fn supervisor_skill_mandates_helper_and_forbids_inline_pane_loops() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Driving agent panes"),
            "supervisor skill should contain a 'Driving agent panes' section"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("for p in") && lowered.contains("do tmux"),
            "the section should name the forbidden `for p in ...; do tmux ...` loop shape"
        );
        assert!(
            lowered.contains("simple_expansion"),
            "the section should cite the simple_expansion permission gate as the reason"
        );
    }

    /// Spec "Never send-keys to the supervisor's own pane": the section states
    /// the supervisor must not send-keys to pane 0, with the self-interrupt
    /// rationale.
    #[test]
    fn supervisor_skill_states_never_own_pane_rule() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("never") && lowered.contains("pane 0"),
            "supervisor skill should state it must never send-keys to its own pane (pane 0)"
        );
        assert!(
            lowered.contains("interrupt"),
            "the never-own-pane rule should give the self-interrupt rationale"
        );
    }

    /// Spec "Cross-worktree git uses git -C, never cd": the rule mandates
    /// `git -C <path>`, forbids `cd <path> && git`, and states both the
    /// untrusted-hooks and wrong-branch (cwd-leak) rationales.
    #[test]
    fn supervisor_skill_mandates_git_dash_c_and_forbids_cd() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("git -C"),
            "supervisor skill should mandate `git -C <path>` for cross-worktree git"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("cd ") && lowered.contains("&& git"),
            "the rule should name and forbid the `cd <path> && git` shape"
        );
        assert!(
            lowered.contains("untrusted-hooks") || lowered.contains("untrusted hooks"),
            "the rule should cite the untrusted-hooks warning"
        );
        assert!(
            lowered.contains("wrong branch") || lowered.contains("wrong-branch"),
            "the rule should cite the wrong-branch (cwd-leak) risk"
        );
    }

    /// Spec "Reliable commit-cadence nudge": the coordination section states
    /// the ~10-uncommitted-file threshold and includes a sample
    /// `agent.feedback` nudge.
    #[test]
    fn supervisor_skill_states_commit_cadence_nudge() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("uncommitted") && lowered.contains("10"),
            "supervisor skill should state the ~10-uncommitted-file commit-nudge threshold"
        );
        assert!(
            lowered.contains("commit-cadence") || lowered.contains("commit cadence"),
            "supervisor skill should label the commit-cadence nudge"
        );
        assert!(
            tmpl.content.contains("feedback-gate"),
            "the nudge should be a published agent.feedback (via the feedback-gate helper)"
        );
    }

    /// Spec "Testing gate runs the full suite without fail-fast"
    /// (verification-no-fail-fast-v0-6-x, W2-7): the testing gate mandates
    /// `--no-fail-fast` + guard neutralization and states the truncated-run
    /// caveat.
    #[test]
    fn supervisor_skill_mandates_no_fail_fast_verification() {
        // Stack-agnostic: the skill states the discipline generically via
        // {{TEST_COMMAND}} — no repo-specific runner/flag literals (those
        // would trip the no-language-leak audit).
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("never fail-fast") || lowered.contains("no-fail-fast"),
            "testing gate must mandate running the whole suite (no fail-fast)"
        );
        assert!(
            lowered.contains("guard test"),
            "testing gate must name the environment guard-test failure mode"
        );
        assert!(
            lowered.contains("incomplete, not a pass")
                || lowered.contains("not a pass unless every later suite"),
            "testing gate must state that an early-aborted (guard-only) run is not a PASS"
        );
    }

    // === per-commit-verification-v0-6-x: "Verify on each event" subsection ===

    /// `per-commit-verification` spec, scenario "Skill contains the per-event
    /// rule": the subsection exists with MUST/MUST-NOT language and a worked
    /// example of the batching anti-pattern.
    #[test]
    fn supervisor_skill_mandates_per_event_verification() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content
                .contains("### Verify on each event, never batch"),
            "supervisor skill must contain the 'Verify on each event, never batch' subsection"
        );
        assert!(
            tmpl.content
                .contains("MUST NOT** defer a ready verification"),
            "subsection must state the no-batch rule in MUST-NOT terms"
        );
        assert!(
            tmpl.content
                .contains("MUST** start a branch's five-gate sweep"),
            "subsection must state the per-event trigger in MUST terms"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("batching anti-pattern"),
            "subsection must include a worked example of the batching anti-pattern"
        );
        assert!(
            lowered.contains("still mid-task"),
            "the worked example must name the wave-1 failure: waiting for a second agent to finish"
        );
    }

    /// `per-commit-verification` spec, scenario "Dependency-driven deferral
    /// remains permitted".
    #[test]
    fn supervisor_skill_permits_dependency_driven_deferral() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("only acceptable reason to defer is a genuine dependency"),
            "subsection must state the genuine-dependency deferral exception"
        );
        assert!(
            lowered.contains("state that dependency explicitly"),
            "subsection must require stating the dependency explicitly when deferring"
        );
    }

    /// `per-commit-verification` spec, scenario "Concurrency permission
    /// documented".
    #[test]
    fn supervisor_skill_permits_concurrent_verification() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("per-branch verifications may run concurrently"),
            "subsection must state per-branch verifications may run concurrently"
        );
        assert!(
            lowered.contains("does **not** block starting agent b's verification"),
            "subsection must state verifying agent A does not block verifying agent B"
        );
    }

    /// The subsection references the broker `supervisor.verify-now` nudge as
    /// the explicit trigger event.
    #[test]
    fn supervisor_skill_references_verify_now_nudge() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("supervisor.verify-now"),
            "subsection must reference the broker's supervisor.verify-now nudge"
        );
        assert!(
            tmpl.content.contains("verify_on_commit_nudge"),
            "subsection must reference the [supervisor] verify_on_commit_nudge config gate"
        );
    }

    /// Bug 4 (auto-approve-scope-v0-6-x): the supervisor skill names the
    /// bundled helper as the canonical stuck-agent detector, documents the
    /// detection + dedup behaviour, and forbids inline-bash signature-dedup
    /// monitors.
    #[test]
    fn supervisor_skill_has_detecting_stuck_agents_section() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("### Detecting stuck agents"),
            "supervisor skill must include a 'Detecting stuck agents' section"
        );
        assert!(
            tmpl.content
                .contains(".git-paw/scripts/sweep.sh detect-stuck"),
            "the section must name the bundled detect-stuck helper command"
        );
        assert!(
            tmpl.content.contains("stuck-on-prompt"),
            "the section must document the stuck-on-prompt phase value"
        );
        assert!(
            tmpl.content.contains("Pasted text #N"),
            "the section must document the paste-buffer marker"
        );
        // Dedup behaviour is documented.
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("dedup") && lowered.contains("prompt-shape"),
            "the section must document the (agent_id, prompt-shape) dedup"
        );
        // Inline-bash reinvention is explicitly forbidden, with rationale.
        assert!(
            tmpl.content
                .contains("Do NOT hand-roll an inline-bash monitor"),
            "the section must forbid inline-bash signature-dedup monitors"
        );
        assert!(
            lowered.contains("eats repeat-pattern prompts"),
            "the section must give the bug-9 rationale (signature dedup eats repeat-pattern prompts)"
        );
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
            &GateCommands::default(),
            &[],
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
        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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
            &GateCommands::default(),
            &[],
        );
        let expected = slugify_branch("Feature/HTTP_Broker");
        assert_eq!(output, format!("id={expected}"));
    }

    // 9.13: Render is deterministic
    #[test]
    fn render_is_deterministic() {
        let tmpl = resolve("coordination").unwrap();
        let a = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
        let b = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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
        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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
        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
        assert!(
            output.contains("{{UNKNOWN_THING}}"),
            "unknown placeholder should survive in output"
        );
    }

    // 9.16: No {{...}} remains after rendering the embedded coordination template
    #[test]
    fn no_unknown_placeholders_after_render() {
        let tmpl = resolve("coordination").unwrap();
        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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

    /// Returns the substring containing the supervisor skill's `agent.verified`
    /// curl example body (the JSON payload region), used to scope wire-format
    /// assertions to the verified example without picking up other prose.
    fn verified_curl_example_body(content: &str) -> &str {
        let start = content
            .find("\"type\":\"agent.verified\"")
            .expect("supervisor skill should contain an agent.verified curl example");
        let rest = &content[start..];
        let end = rest
            .find("}}'")
            .expect("agent.verified curl example should terminate with the closing payload `}}'`");
        &rest[..end + 3]
    }

    /// Returns the substring containing the supervisor skill's `agent.feedback`
    /// curl example body (the JSON payload region).
    fn feedback_curl_example_body(content: &str) -> &str {
        let start = content
            .find("\"type\":\"agent.feedback\"")
            .expect("supervisor skill should contain an agent.feedback curl example");
        let rest = &content[start..];
        let end = rest
            .find("}}'")
            .expect("agent.feedback curl example should terminate with the closing payload `}}'`");
        &rest[..end + 3]
    }

    #[test]
    fn supervisor_verified_example_uses_correct_payload_fields() {
        let tmpl = resolve("supervisor").unwrap();
        let example = verified_curl_example_body(&tmpl.content);
        assert!(
            example.contains("verified_by"),
            "agent.verified example must use the `verified_by` payload field: {example}"
        );
        assert!(
            example.contains("message"),
            "agent.verified example must use the `message` payload field: {example}"
        );
        for wrong in ["\"target\"", "\"result\"", "\"notes\""] {
            assert!(
                !example.contains(wrong),
                "agent.verified example must not contain the stale field key {wrong}: {example}"
            );
        }
    }

    #[test]
    fn supervisor_feedback_example_uses_correct_payload_fields() {
        let tmpl = resolve("supervisor").unwrap();
        let example = feedback_curl_example_body(&tmpl.content);
        assert!(
            example.contains("\"from\""),
            "agent.feedback example must use the `from` payload field: {example}"
        );
        assert!(
            example.contains("\"errors\""),
            "agent.feedback example must use the `errors` payload field: {example}"
        );
        assert!(
            example.contains('['),
            "agent.feedback example's errors field must be a JSON array (contains `[`): {example}"
        );
        assert!(
            example.contains(']'),
            "agent.feedback example's errors field must be a JSON array (contains `]`): {example}"
        );
        for wrong in ["\"target\"", "\"message\""] {
            assert!(
                !example.contains(wrong),
                "agent.feedback example must not contain the stale field key {wrong}: {example}"
            );
        }
    }

    #[test]
    fn supervisor_examples_clarify_recipient_vs_sender() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();

        // Verified-section clarification (between the verified heading and the
        // feedback heading).
        let verified_start = tmpl
            .content
            .find("### Publish verification outcome")
            .expect("verified heading should be present");
        let feedback_start = tmpl
            .content
            .find("### Publish feedback to a peer agent")
            .expect("feedback heading should be present");
        let verified_section = tmpl.content[verified_start..feedback_start].to_lowercase();
        assert!(
            verified_section.contains("recipient") && verified_section.contains("sender"),
            "verified section should clarify recipient-vs-sender semantics, got: {verified_section}"
        );

        // Feedback-section clarification (between the feedback heading and the
        // next `### ` heading).
        let after_feedback =
            &tmpl.content[feedback_start + "### Publish feedback to a peer agent".len()..];
        let feedback_end_rel = after_feedback
            .find("\n### ")
            .unwrap_or(after_feedback.len());
        let feedback_section = after_feedback[..feedback_end_rel].to_lowercase();
        assert!(
            feedback_section.contains("recipient") && feedback_section.contains("sender"),
            "feedback section should clarify recipient-vs-sender semantics, got: {feedback_section}"
        );

        // Defensive sanity: the words exist somewhere in the document.
        assert!(lowered.contains("recipient"));
        assert!(lowered.contains("sender"));
    }

    #[test]
    fn supervisor_workflow_prose_drops_legacy_verified_fields() {
        let tmpl = resolve("supervisor").unwrap();
        // Strip whitespace inside the matches so a stray space doesn't hide a
        // regression like `result : "pass"` or `notes : ""`.
        let condensed: String = tmpl
            .content
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            !condensed.contains("result:\"pass\""),
            "workflow prose must not reference `result:\"pass\"` as the verified payload"
        );
        assert!(
            !condensed.contains("notes:\"\""),
            "workflow prose must not reference `notes:\"\"` as the verified payload"
        );
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
            tmpl.content.contains("{{SPEC_PATH_DOCTRINE}}"),
            "v0.6.0+ supervisor template should embed the SPEC_PATH_DOCTRINE placeholder so spec layout is rendered per backend, not hardcoded"
        );
        assert!(
            tmpl.content.contains("grep"),
            "should instruct to grep for matching tests"
        );
        // When rendered against the OpenSpec backend, the rendered output
        // SHALL still reference the openspec/changes/ path doctrine.
        let rendered = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[crate::specs::SpecBackendKind::OpenSpec],
        );
        assert!(
            rendered.contains("openspec/changes/"),
            "OpenSpec-rendered supervisor skill should reference openspec/changes/ via the SPEC_PATH_DOCTRINE substitution"
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

    // Paste-buffer recovery sub-case under stall detection (prompt-submit-fix).

    #[test]
    fn supervisor_skill_mentions_paste_buffer_recovery() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("paste-buffer") || lowered.contains("paste buffer"),
            "supervisor skill should contain paste-buffer recovery sub-case"
        );
    }

    #[test]
    fn supervisor_skill_mentions_pasted_text_indicator() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Pasted text"),
            "supervisor skill should mention the Claude Code 'Pasted text' indicator"
        );
    }

    #[test]
    fn supervisor_skill_paste_buffer_recovery_uses_tmux() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .to_lowercase()
            .find("paste-buffer recovery")
            .or_else(|| tmpl.content.to_lowercase().find("paste buffer recovery"))
            .expect("paste-buffer recovery sub-case heading should be present");
        // Take a window around the heading large enough to cover the
        // recovery example (a couple thousand chars now that the sub-case
        // also references the proactive launch-time sweep).
        let window_end = (start + 2200).min(tmpl.content.len());
        let window = &tmpl.content[start..window_end];
        // The inspect step now goes through `sweep.sh capture <pane>`; the
        // earlier shape `tmux capture-pane …` is still acceptable for
        // historical content. Either form satisfies the inspect contract.
        assert!(
            window.contains(".git-paw/scripts/sweep.sh capture")
                || window.contains("tmux capture-pane"),
            "paste-buffer recovery should reference a pane-capture command (sweep.sh capture or tmux capture-pane)"
        );
        assert!(
            window.contains("tmux send-keys"),
            "paste-buffer recovery should reference tmux send-keys for the Enter recovery"
        );
        assert!(
            window.contains("Enter"),
            "paste-buffer recovery should specify Enter as the recovery keystroke"
        );
    }

    #[test]
    fn supervisor_skill_mentions_launch_time_sweep() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("launch-time pane sweep")
                || lowered.contains("launch time pane sweep")
                || lowered.contains("launch sweep"),
            "supervisor skill should contain a launch-time pane sweep heading"
        );
    }

    #[test]
    fn supervisor_skill_launch_sweep_lists_four_pane_categories() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("launch-time pane sweep")
            .or_else(|| lowered.find("launch sweep"))
            .expect("launch-time pane sweep heading should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];
        assert!(
            window.contains("paste-buffer") || window.contains("paste buffer"),
            "launch sweep should enumerate paste-buffer category"
        );
        assert!(
            window.contains("permission prompt"),
            "launch sweep should enumerate permission-prompt category"
        );
        assert!(
            window.contains("working"),
            "launch sweep should enumerate working category"
        );
        assert!(
            window.contains("idle"),
            "launch sweep should enumerate idle category"
        );
    }

    #[test]
    fn supervisor_skill_launch_sweep_references_down_enter_keystroke() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("launch-time pane sweep")
            .or_else(|| lowered.find("launch sweep"))
            .expect("launch-time pane sweep heading should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];
        // Safe-command auto-approval uses Down to move to "Yes, don't ask
        // again", then Enter to select it. Both keystrokes must be in the
        // section so the supervisor agent knows the pattern.
        assert!(
            window.contains("down"),
            "launch sweep should reference the Down keystroke for selecting 'don't ask again'"
        );
        assert!(
            window.contains("enter"),
            "launch sweep should reference the Enter keystroke for confirming approval"
        );
        // Confirm the "don't ask again" phrasing is present so future
        // pattern allowlist behavior is documented in the skill.
        assert!(
            window.contains("don't ask again") || window.contains("don't ask"),
            "launch sweep should mention the 'don't ask again' approval option"
        );
    }

    #[test]
    fn supervisor_skill_paste_buffer_recovery_is_safe_by_default() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("paste-buffer recovery")
            .or_else(|| lowered.find("paste buffer recovery"))
            .expect("paste-buffer recovery sub-case heading should be present");
        let window_end = (start + 2200).min(lowered.len());
        let window = &lowered[start..window_end];
        let safe_phrasing = window.contains("safe-by-default")
            || window.contains("safe by default")
            || window.contains("no-op")
            || window.contains("no harm");
        assert!(
            safe_phrasing,
            "paste-buffer recovery should explicitly note the Enter is safe-by-default / no-op / no harm"
        );
    }

    // Governance verification sub-step in the supervisor skill (governance-context §5).

    #[test]
    fn supervisor_skill_contains_governance_verification() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Governance verification"),
            "supervisor skill should contain 'Governance verification' heading"
        );
    }

    #[test]
    fn supervisor_skill_governance_is_substep_of_spec_audit() {
        let tmpl = resolve("supervisor").unwrap();
        let audit_pos = tmpl
            .content
            .find("### Spec Audit Procedure")
            .expect("Spec Audit Procedure heading must exist");
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification must exist");
        let conflict_pos = tmpl
            .content
            .find("### Conflict detection")
            .unwrap_or(tmpl.content.len());
        assert!(
            gov_pos > audit_pos,
            "Governance verification should appear inside Spec Audit Procedure (after its heading)"
        );
        assert!(
            gov_pos < conflict_pos,
            "Governance verification should appear before the next top-level subsection (Conflict detection), keeping it inside Spec Audit Procedure"
        );
        assert!(
            !tmpl.content.contains("step 7.5"),
            "Governance verification must not be framed as a separate 'step 7.5' flow step"
        );
    }

    #[test]
    fn supervisor_skill_governance_examples_cover_all_five_docs() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        // Confine the search to the governance subsection (everything between
        // the heading and the next `### ` top-level subsection or EOF).
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        for needle in &["DoD", "ADR", "Security", "Test strategy", "Constitution"] {
            assert!(
                section.contains(needle),
                "governance section should mention `{needle}` as a per-doc example, got:\n{section}"
            );
        }
    }

    #[test]
    fn supervisor_skill_governance_findings_via_agent_feedback() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        assert!(
            section.contains("agent.feedback"),
            "governance section must state that findings flow through `agent.feedback`"
        );
    }

    #[test]
    fn supervisor_skill_no_governance_gate_tag() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            !tmpl.content.contains("[governance-gate:"),
            "supervisor skill must not contain the dropped `[governance-gate:<doc>]` tag prefix"
        );
    }

    #[test]
    fn supervisor_skill_no_governance_gates_table() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            !tmpl.content.contains("[governance.gates]"),
            "supervisor skill must not reference the dropped `[governance.gates]` table"
        );
    }

    #[test]
    fn supervisor_skill_no_gating_language() {
        let tmpl = resolve("supervisor").unwrap();
        // The opsx-role-gating capability legitimately uses the tokens
        // `role-gating` / `role_gating` (a feature name, not the dropped
        // governance-"gating" terminology this test guards against). Strip
        // those tokens before checking so the original intent — no governance
        // "gating"/"blocking" language — still holds.
        let lowered = tmpl
            .content
            .to_lowercase()
            .replace("opsx-role-gating", "")
            .replace("role-gating", "")
            .replace("role_gating", "");
        assert!(
            !lowered.contains("gating"),
            "supervisor skill must not use the language of 'gating' (outside the opsx role-gating feature name)"
        );
        assert!(
            !lowered.contains("blocking on governance failures"),
            "supervisor skill must not use the language of 'blocking on governance failures'"
        );
    }

    #[test]
    fn supervisor_skill_governance_missing_doc_handling() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("missing"),
            "governance section should describe missing-doc handling"
        );
        assert!(
            section.contains("agent.feedback"),
            "missing-doc handling should reference `agent.feedback` errors list"
        );
    }

    #[test]
    fn supervisor_skill_governance_missing_doc_is_not_distinct_failure_type() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("not a distinct failure")
                || lowered.contains("not a separate failure")
                || lowered.contains("treat it as a finding"),
            "governance section must state that missing files are findings, not a distinct failure type; got:\n{section}"
        );
    }

    #[test]
    fn supervisor_skill_governance_states_activation_condition() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("skip"),
            "governance section must instruct the supervisor to skip the sub-step when the boot prompt has no `## Governance documents` section; got:\n{section}"
        );
        assert!(
            section.contains("## Governance documents"),
            "governance section must reference the boot-prompt heading explicitly as its activation condition; got:\n{section}"
        );
    }

    #[test]
    fn supervisor_skill_governance_examples_state_they_are_illustrative() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("illustrative") || lowered.contains("not exhaustive"),
            "governance section must state per-doc examples are illustrative / not exhaustive rubrics; got:\n{section}"
        );
    }

    #[test]
    fn supervisor_skill_governance_states_judgment_per_project_conventions() {
        let tmpl = resolve("supervisor").unwrap();
        let gov_pos = tmpl
            .content
            .find("Governance verification")
            .expect("Governance verification section must exist");
        let after = &tmpl.content[gov_pos..];
        let end = after.find("\n### ").unwrap_or(after.len());
        let section = &after[..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("judgment"),
            "governance section must state the supervisor applies judgment; got:\n{section}"
        );
        assert!(
            lowered.contains("convention") || lowered.contains("project"),
            "governance section must reference the project's conventions / process when describing judgment; got:\n{section}"
        );
    }

    // === supervisor-stream-timeout-recovery: "Stream-timeout recovery" ===

    /// Returns the body of the "Stream-timeout recovery" section — from
    /// its `### ` heading to the next `### ` top-level subsection or EOF.
    fn stream_timeout_section(content: &str) -> &str {
        let start = content
            .find("### Stream-timeout recovery")
            .expect("supervisor skill must contain the Stream-timeout recovery section");
        let after = &content[start..];
        // skip past this section's own heading before searching for the
        // next top-level `### ` boundary
        let body_offset = "### Stream-timeout recovery".len();
        let end = after[body_offset..]
            .find("\n### ")
            .map_or(after.len(), |i| body_offset + i);
        &after[..end]
    }

    /// `supervisor-stream-timeout-recovery` spec, scenario "Section exists
    /// with the four pieces in recovery order": the heading is present and
    /// the four subsections appear in the documented order.
    #[test]
    fn supervisor_skill_stream_timeout_section_has_four_ordered_pieces() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);

        let error_shape = section
            .find("error-shape recognition")
            .expect("subsection 1 must name error-shape recognition");
        let checkpoint = section
            .find("pre-action checkpoint")
            .expect("subsection 2 must name the pre-action checkpoint");
        let replay = section
            .find("replay-missing-publishes")
            .expect("subsection 3 must name replay-missing-publishes");
        let confirmation = section
            .find("Confirmation rule")
            .expect("subsection 4 must name the Confirmation rule");

        assert!(
            error_shape < checkpoint && checkpoint < replay && replay < confirmation,
            "the four pieces must appear in recovery order: error-shape recognition, \
             pre-action checkpoint, replay-missing-publishes, confirmation rule"
        );
    }

    /// `Requirement: Error-shape recognition`, scenario "Symptoms are named
    /// generically across CLIs": at least two visible symptom patterns and
    /// no specific CLI's exact error string.
    #[test]
    fn supervisor_skill_stream_timeout_names_two_generic_symptoms() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("mid-stream cutoff"),
            "error-shape subsection must name the mid-stream cutoff symptom"
        );
        assert!(
            lowered.contains("transport error") || lowered.contains("stream error"),
            "error-shape subsection must name a transport-error / stream-error symptom"
        );
    }

    /// `Requirement: Pre-action checkpoint via agent.status`, scenario
    /// "Checkpoint shape is documented": a concrete `agent.status` shape
    /// with `status: "checkpoint"` and a `summary` enumerating targets.
    #[test]
    fn supervisor_skill_stream_timeout_documents_checkpoint_shape() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        assert!(
            section.contains("agent.status"),
            "checkpoint subsection must show an agent.status publish"
        );
        assert!(
            section.contains("\"status\":\"checkpoint\"")
                || section.contains("status: \"checkpoint\""),
            "checkpoint subsection must show status: \"checkpoint\""
        );
        assert!(
            section.contains("summary"),
            "checkpoint subsection must show a summary enumerating intended targets"
        );
    }

    /// `Requirement: Pre-action checkpoint`, scenario "Checkpoint required
    /// only for multi-publish iterations".
    #[test]
    fn supervisor_skill_stream_timeout_checkpoint_only_for_multi_publish() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("more than one"),
            "checkpoint subsection must state it applies only to iterations with \
             more than one intended downstream publish"
        );
        assert!(
            lowered.contains("not to every sweep") || lowered.contains("not every sweep"),
            "checkpoint subsection must clarify it does not apply to every sweep"
        );
    }

    /// `Requirement: Replay-missing-publishes recovery`, scenario
    /// "Per-target poll-then-replay pattern documented".
    #[test]
    fn supervisor_skill_stream_timeout_documents_replay_loop() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        assert!(
            section.contains("/messages/"),
            "replay subsection must show polling the target's /messages/ stream"
        );
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("since=") || lowered.contains("checkpoint timestamp"),
            "replay subsection must poll since the checkpoint timestamp"
        );
        assert!(
            lowered.contains("re-publish"),
            "replay subsection must re-publish the missing record"
        );
        assert!(
            lowered.contains("idempotent"),
            "replay subsection must state the replay is idempotent so duplicates are safe"
        );
        assert!(
            lowered.contains("for each"),
            "replay subsection must show a per-target loop"
        );
    }

    /// `Requirement: Confirmation rule`, scenario "Confirmation rule
    /// appears prominently": bold `**` markers around the key sentence
    /// plus a stream-timeout rationale.
    #[test]
    fn supervisor_skill_stream_timeout_confirmation_rule_is_prominent() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        assert!(
            section.contains("**Never advance to the next sub-action"),
            "confirmation rule must be marked prominently with bold (`**`) formatting"
        );
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("timed out mid-write") || lowered.contains("may have timed out"),
            "confirmation rule must pair with a one-sentence rationale referencing stream-timeout risk"
        );
    }

    /// `Requirement: Recovery learning record`, scenario "Skill prose names
    /// the recovery learning trigger": each recovery emits a
    /// `recovery_cycles` `agent.learning` record with a structured body.
    #[test]
    fn supervisor_skill_stream_timeout_names_recovery_learning_record() {
        let tmpl = resolve("supervisor").unwrap();
        let section = stream_timeout_section(&tmpl.content);
        assert!(
            section.contains("recovery_cycles"),
            "replay subsection must name the recovery_cycles learning category"
        );
        assert!(
            section.contains("agent.learning"),
            "replay subsection must state the recovery emits an agent.learning record"
        );
        for field in [
            "checkpoint_id",
            "intended_targets",
            "replayed_targets",
            "skipped_targets",
        ] {
            assert!(
                section.contains(field),
                "recovery learning body must document the `{field}` field"
            );
        }
    }

    // -----------------------------------------------------------------
    // render_dev_allowlist_preset (lang-agnostic-skills)
    // -----------------------------------------------------------------

    #[test]
    fn dev_allowlist_preset_renders_every_constant_entry() {
        // Spec contract: every entry from the constant contributes to the
        // rendered output such that adding a new entry to the constant
        // would change the output without a skill-template edit. The prose
        // groups entries by first word — `cargo build` shows as `cargo
        // (build, …)`. We verify each entry's head word AND tail (if any)
        // both appear, which would break the moment a new entry is added
        // without re-rendering.
        use crate::supervisor::dev_allowlist::DEV_ALLOWLIST_PRESET;
        let prose = render_dev_allowlist_preset();
        for entry in DEV_ALLOWLIST_PRESET {
            let (head, tail) = match entry.split_once(' ') {
                Some((h, t)) => (h, Some(t)),
                None => (*entry, None),
            };
            assert!(
                prose.contains(head),
                "rendered preset must contain head word `{head}` from entry `{entry}`; got:\n{prose}"
            );
            if let Some(t) = tail {
                assert!(
                    prose.contains(t),
                    "rendered preset must contain tail `{t}` from entry `{entry}`; got:\n{prose}"
                );
            }
        }
    }

    #[test]
    fn dev_allowlist_preset_groups_by_first_word() {
        // `git status` and `git log` share `git`; the rendered prose
        // must collapse them into a single `git (...)` group so the
        // listing reads as families, not as a flat array.
        let prose = render_dev_allowlist_preset();
        let git_groups = prose.matches("git (").count();
        assert_eq!(
            git_groups, 1,
            "multi-entry git prefix must collapse into a single grouped clause; got {git_groups} occurrences of `git (` in:\n{prose}"
        );
    }

    #[test]
    fn dev_allowlist_preset_preserves_single_word_entries() {
        let prose = render_dev_allowlist_preset();
        for bare in ["find", "grep"] {
            assert!(
                prose.contains(bare),
                "bare single-word entry `{bare}` should appear verbatim in:\n{prose}"
            );
        }
    }

    // -----------------------------------------------------------------
    // render_spec_path_doctrine (lang-agnostic-skills)
    // -----------------------------------------------------------------

    #[test]
    fn spec_doctrine_empty_backends_renders_sentinel() {
        let out = render_spec_path_doctrine(&[]);
        assert!(
            out.contains("no spec backend"),
            "empty backend slice should render the sentinel; got: {out}"
        );
    }

    #[test]
    fn spec_doctrine_openspec_references_openspec_paths_and_workflow() {
        use crate::specs::SpecBackendKind;
        let out = render_spec_path_doctrine(&[SpecBackendKind::OpenSpec]);
        assert!(
            out.contains("openspec/changes/"),
            "OpenSpec doctrine should name the openspec/changes/ path; got: {out}"
        );
        assert!(
            out.contains("openspec validate"),
            "OpenSpec doctrine should reference the openspec validate workflow; got: {out}"
        );
    }

    #[test]
    fn spec_doctrine_speckit_references_specify_paths_and_checklist() {
        use crate::specs::SpecBackendKind;
        let out = render_spec_path_doctrine(&[SpecBackendKind::SpecKit]);
        assert!(
            out.contains(".specify/specs/"),
            "Spec Kit doctrine should name the .specify/specs/ path; got: {out}"
        );
        assert!(
            out.to_lowercase().contains("checklist"),
            "Spec Kit doctrine should reference the checklist convention; got: {out}"
        );
    }

    #[test]
    fn spec_doctrine_markdown_references_paw_status_frontmatter() {
        use crate::specs::SpecBackendKind;
        let out = render_spec_path_doctrine(&[SpecBackendKind::Markdown]);
        assert!(
            out.contains("paw_status: pending"),
            "Markdown doctrine should reference paw_status: pending; got: {out}"
        );
    }

    #[test]
    fn spec_doctrine_multi_backend_lists_each_present_backend() {
        use crate::specs::SpecBackendKind;
        let out = render_spec_path_doctrine(&[
            SpecBackendKind::OpenSpec,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Markdown,
        ]);
        assert!(
            out.contains("openspec/changes/"),
            "multi-backend doctrine should mention OpenSpec; got:\n{out}"
        );
        assert!(
            out.contains(".specify/specs/"),
            "multi-backend doctrine should mention Spec Kit; got:\n{out}"
        );
        assert!(
            out.contains("paw_status: pending"),
            "multi-backend doctrine should mention Markdown; got:\n{out}"
        );
        assert!(
            out.contains("spans multiple"),
            "multi-backend doctrine should introduce the multi-backend session shape; got:\n{out}"
        );
    }

    #[test]
    fn spec_doctrine_dedupes_repeated_backends() {
        use crate::specs::SpecBackendKind;
        let out = render_spec_path_doctrine(&[
            SpecBackendKind::OpenSpec,
            SpecBackendKind::OpenSpec,
            SpecBackendKind::OpenSpec,
        ]);
        // A single backend (even repeated) renders the single-backend
        // sentence shape, not the multi-backend intro.
        assert!(
            !out.contains("spans multiple"),
            "duplicate backends must collapse to the single-backend shape; got:\n{out}"
        );
    }

    // -----------------------------------------------------------------
    // render() new placeholder substitutions (lang-agnostic-skills)
    // -----------------------------------------------------------------

    #[test]
    fn render_doc_tool_command_substitutes_from_gates() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{DOC_TOOL_COMMAND}} for API docs.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            doc_tool_command: Some("sphinx-build -W docs docs/_build"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert_eq!(output, "Run sphinx-build -W docs docs/_build for API docs.");
        assert!(!output.contains("{{DOC_TOOL_COMMAND}}"));
    }

    #[test]
    fn render_doc_tool_command_empty_when_unset() {
        // Unlike the other gate placeholders, DOC_TOOL_COMMAND renders as
        // an empty string when None — the supervisor template is authored
        // to surround the placeholder with prose that reads naturally
        // even when empty (per D5 of the design).
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "API doc tool: `{{DOC_TOOL_COMMAND}}`".into(),
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
            &GateCommands::default(),
            &[],
        );
        assert_eq!(output, "API doc tool: ``");
        assert!(!output.contains("(not configured)"));
    }

    #[test]
    fn render_dev_allowlist_preset_placeholder_substitutes() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Allowed: {{DEV_ALLOWLIST_PRESET}}".into(),
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
            &GateCommands::default(),
            &[],
        );
        assert!(
            output.contains("git (status"),
            "rendered placeholder should embed the grouped preset prose; got:\n{output}"
        );
        assert!(!output.contains("{{DEV_ALLOWLIST_PRESET}}"));
    }

    #[test]
    fn render_spec_path_doctrine_placeholder_substitutes_per_backend() {
        use crate::specs::SpecBackendKind;
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Spec layout: {{SPEC_PATH_DOCTRINE}}".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let openspec_output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[SpecBackendKind::OpenSpec],
        );
        assert!(openspec_output.contains("openspec/changes/"));
        assert!(!openspec_output.contains("{{SPEC_PATH_DOCTRINE}}"));

        let speckit_output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[SpecBackendKind::SpecKit],
        );
        assert!(speckit_output.contains(".specify/specs/"));
    }

    #[test]
    fn render_spec_path_doctrine_empty_renders_sentinel() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "{{SPEC_PATH_DOCTRINE}}".into(),
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
            &GateCommands::default(),
            &[],
        );
        assert!(output.contains("no spec backend"));
    }

    // governance_section_paths renderer (governance-context §1, §3).

    #[test]
    fn governance_section_empty_when_all_paths_none() {
        let out = governance_section_paths(None, None, None, None, None);
        assert!(
            out.is_empty(),
            "governance_section_paths should return empty string when all paths are None, got: {out:?}"
        );
    }

    #[test]
    fn governance_section_one_path_only_dod() {
        let dod = Path::new("docs/dod.md");
        let out = governance_section_paths(None, None, None, Some(dod), None);
        assert!(
            out.contains("## Governance documents"),
            "section should include the canonical heading, got:\n{out}"
        );
        assert!(
            out.contains("- dod: docs/dod.md"),
            "section should include the dod bullet, got:\n{out}"
        );
        for unset in [
            "- adr:",
            "- test_strategy:",
            "- security:",
            "- constitution:",
        ] {
            assert!(
                !out.contains(unset),
                "section should not mention `{unset}` when its path is None, got:\n{out}"
            );
        }
    }

    #[test]
    fn governance_section_lists_all_five_in_canonical_order() {
        let adr = Path::new("docs/adr/");
        let test_strategy = Path::new("docs/test-strategy.md");
        let security = Path::new("docs/security.md");
        let dod = Path::new("docs/dod.md");
        let constitution = Path::new("docs/constitution.md");
        let out = governance_section_paths(
            Some(adr),
            Some(test_strategy),
            Some(security),
            Some(dod),
            Some(constitution),
        );

        let order = [
            "- adr: docs/adr/",
            "- test_strategy: docs/test-strategy.md",
            "- security: docs/security.md",
            "- dod: docs/dod.md",
            "- constitution: docs/constitution.md",
        ];
        let mut last_pos = 0usize;
        for bullet in order {
            let idx = out
                .find(bullet)
                .unwrap_or_else(|| panic!("bullet `{bullet}` not found in:\n{out}"));
            assert!(
                idx >= last_pos,
                "bullets must appear in canonical adr -> test_strategy -> security -> dod -> constitution order; `{bullet}` came before a previous bullet in:\n{out}"
            );
            last_pos = idx;
        }
    }

    #[test]
    fn governance_section_has_no_gates_text() {
        let out = governance_section_paths(
            Some(Path::new("docs/adr/")),
            Some(Path::new("docs/test-strategy.md")),
            Some(Path::new("docs/security.md")),
            Some(Path::new("docs/dod.md")),
            Some(Path::new("docs/constitution.md")),
        );
        let lowered = out.to_lowercase();
        assert!(
            !lowered.contains("gated docs"),
            "section should not contain a 'Gated docs' line, got:\n{out}"
        );
        assert!(
            !lowered.contains("governance gates"),
            "section should not contain a 'Governance gates' sub-section, got:\n{out}"
        );
        assert!(
            !out.contains("[governance.gates]"),
            "section should not reference the dropped [governance.gates] table, got:\n{out}"
        );
        assert!(
            !out.contains("[governance-gate:"),
            "section should not introduce the dropped [governance-gate:<doc>] tag, got:\n{out}"
        );
    }

    #[test]
    fn governance_section_has_preamble_line() {
        let out = governance_section_paths(None, None, None, Some(Path::new("docs/dod.md")), None);
        let preamble = "The supervisor consults these documents during spec audit.";
        assert!(
            out.contains(preamble),
            "section should include the preamble line; got:\n{out}"
        );
        // Preamble must come before bullets and after the heading.
        let heading_pos = out.find("## Governance documents").unwrap();
        let preamble_pos = out.find(preamble).unwrap();
        let bullet_pos = out.find("- dod:").unwrap();
        assert!(
            heading_pos < preamble_pos && preamble_pos < bullet_pos,
            "section layout should be heading -> preamble -> bullets; got:\n{out}"
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
        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "my-app",
            &GateCommands::default(),
            &[],
        );
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
        let output = render(
            &tmpl,
            "feat/http-broker",
            "url",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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

        let output = render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );
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
            &GateCommands {
                test_command: Some("just check"),
                ..Default::default()
            },
            &[],
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
            &GateCommands::default(),
            &[],
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
        //
        // `{{CHANGE_ID}}` is a per-invocation placeholder (substituted by the
        // supervisor agent, not by render) and is therefore expected to
        // survive a render pass.
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands {
                test_command: Some("just check"),
                ..Default::default()
            },
            &[],
        );
        assert!(
            !output.contains("{{TEST_COMMAND}}"),
            "supervisor template still contains a literal {{TEST_COMMAND}} after render"
        );
        let remaining: String = output.replace("{{CHANGE_ID}}", "").chars().collect();
        assert!(
            !remaining.contains("{{"),
            "supervisor template has unsubstituted {{...}} placeholder (other than {{CHANGE_ID}}) after render"
        );
    }

    // --- Gate-command placeholder substitution (supervisor-gate-templating-v0-5-x) ---

    /// Helper: render `template` with all gate placeholders set to the same
    /// `Some(value)` or all `None`.
    fn render_with_gates_uniform(template: &str, value: Option<&str>) -> String {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: template.into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            test_command: value,
            lint_command: value,
            build_command: value,
            doc_build_command: value,
            spec_validate_command: value,
            fmt_check_command: value,
            security_audit_command: value,
            doc_tool_command: value,
        };
        render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        )
    }

    #[test]
    fn render_test_command_placeholder_substitutes_from_config() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{TEST_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            test_command: Some("just check"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(
            output.contains("Run just check."),
            "expected 'Run just check.' in: {output}"
        );
    }

    #[test]
    fn render_test_command_placeholder_none_renders_not_configured() {
        let output = render_with_gates_uniform("Run {{TEST_COMMAND}}.", None);
        assert!(
            output.contains("Run (not configured)."),
            "expected 'Run (not configured).' in: {output}"
        );
    }

    #[test]
    fn render_lint_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{LINT_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            lint_command: Some("cargo clippy -- -D warnings"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(
            output.contains("Run cargo clippy -- -D warnings."),
            "expected substitution in: {output}"
        );

        let none_output = render_with_gates_uniform("Run {{LINT_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "expected '(not configured)' fallback in: {none_output}"
        );
    }

    #[test]
    fn render_build_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{BUILD_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            build_command: Some("cargo build"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(output.contains("Run cargo build."), "got: {output}");

        let none_output = render_with_gates_uniform("Run {{BUILD_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "got: {none_output}"
        );
    }

    #[test]
    fn render_doc_build_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{DOC_BUILD_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            doc_build_command: Some("mdbook build docs/"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(output.contains("Run mdbook build docs/."), "got: {output}");

        let none_output = render_with_gates_uniform("Run {{DOC_BUILD_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "got: {none_output}"
        );
    }

    #[test]
    fn render_spec_validate_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{SPEC_VALIDATE_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            spec_validate_command: Some("openspec validate {{CHANGE_ID}} --strict"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(
            output.contains("Run openspec validate {{CHANGE_ID}} --strict."),
            "got: {output}"
        );

        let none_output = render_with_gates_uniform("Run {{SPEC_VALIDATE_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "got: {none_output}"
        );
    }

    #[test]
    fn render_fmt_check_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{FMT_CHECK_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            fmt_check_command: Some("cargo fmt --check"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(output.contains("Run cargo fmt --check."), "got: {output}");

        let none_output = render_with_gates_uniform("Run {{FMT_CHECK_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "got: {none_output}"
        );
    }

    #[test]
    fn render_security_audit_command_placeholder_substitutes_and_none_fallback() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{SECURITY_AUDIT_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            security_audit_command: Some("cargo audit"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(output.contains("Run cargo audit."), "got: {output}");

        let none_output = render_with_gates_uniform("Run {{SECURITY_AUDIT_COMMAND}}.", None);
        assert!(
            none_output.contains("Run (not configured)."),
            "got: {none_output}"
        );
    }

    #[test]
    fn supervisor_skill_renders_with_all_six_gate_placeholders_set() {
        // With distinct Some("CMD-N") values, the rendered supervisor skill
        // contains each CMD-N value (proving the gate prose references the
        // placeholders, not hardcoded git-paw commands).
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        let gates = GateCommands {
            test_command: Some("CMD-TEST"),
            lint_command: Some("CMD-LINT"),
            build_command: Some("CMD-BUILD"),
            doc_build_command: Some("CMD-DOC"),
            spec_validate_command: Some("CMD-SPEC"),
            fmt_check_command: Some("CMD-FMT"),
            security_audit_command: Some("CMD-SEC"),
            doc_tool_command: Some("CMD-DOCTOOL"),
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        for needle in [
            "CMD-TEST",
            "CMD-LINT",
            "CMD-BUILD",
            "CMD-DOC",
            "CMD-SPEC",
            "CMD-FMT",
            "CMD-SEC",
        ] {
            assert!(
                output.contains(needle),
                "rendered supervisor skill should contain '{needle}'; not found"
            );
        }
    }

    #[test]
    fn supervisor_skill_renders_not_configured_in_each_gate_when_none() {
        // With all placeholders None, every gate section in the rendered
        // skill must show '(not configured)' so the supervisor agent can
        // recognise the gate as having no tooling-aided phase.
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        );

        // Gate 1 (Testing) section.
        let testing_start = output.find("**Testing**").expect("Testing gate present");
        let testing_end = output[testing_start..]
            .find("**Regression analysis**")
            .map(|p| testing_start + p)
            .expect("Regression follows Testing");
        let testing_section = &output[testing_start..testing_end];
        assert!(
            testing_section.contains("(not configured)"),
            "Testing gate should render '(not configured)' when gate fields are None; got:\n{testing_section}"
        );

        // Gate 3 (Spec audit).
        let spec_start = output.find("**Spec audit**").expect("Spec audit present");
        let spec_end = output[spec_start..]
            .find("**Doc audit**")
            .map(|p| spec_start + p)
            .expect("Doc audit follows Spec audit");
        let spec_section = &output[spec_start..spec_end];
        assert!(
            spec_section.contains("(not configured)"),
            "Spec audit gate should render '(not configured)' when None; got:\n{spec_section}"
        );

        // Gate 4 (Doc audit).
        let doc_start = output.find("**Doc audit**").expect("Doc audit present");
        let doc_end = output[doc_start..]
            .find("**Security audit**")
            .map(|p| doc_start + p)
            .expect("Security audit follows Doc audit");
        let doc_section = &output[doc_start..doc_end];
        assert!(
            doc_section.contains("(not configured)"),
            "Doc audit gate should render '(not configured)' when None; got:\n{doc_section}"
        );

        // Gate 5 (Security audit).
        let security_start = output
            .find("**Security audit**")
            .expect("Security audit present");
        let security_end = output[security_start..]
            .find("**Verify or feedback**")
            .map(|p| security_start + p)
            .expect("Verify-or-feedback follows Security audit");
        let security_section = &output[security_start..security_end];
        assert!(
            security_section.contains("(not configured)"),
            "Security audit gate should render '(not configured)' when None; got:\n{security_section}"
        );
    }

    /// Pre-render audit: the embedded supervisor template must not hardcode
    /// `just check`, `cargo test`, `cargo clippy`, `cargo audit`,
    /// `cargo fmt --check`, `mdbook build`, or `openspec validate` in its
    /// gate prose. Matches inside fenced code blocks demonstrating example
    /// config values (e.g. `# test_command = "just check"`) are tolerated:
    /// the audit windows are the §4-§7 gate-prose paragraphs only.
    #[test]
    fn supervisor_template_gate_prose_has_no_hardcoded_git_paw_commands() {
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        let content = &tmpl.content;
        let start = content
            .find("Steps 4-7 below are the **five first-class verification gates**")
            .expect("five-gate intro present");
        let end = content
            .find("### Spec Audit Procedure")
            .expect("Spec Audit Procedure heading present");
        let gate_prose = &content[start..end];
        for needle in [
            "just check",
            "cargo test",
            "cargo clippy",
            "cargo audit",
            "cargo fmt --check",
            "mdbook build",
        ] {
            // The §7 agent.feedback example body intentionally contains the
            // string `cargo test failed: ...` as an illustration of error
            // reporting. The example may be written either with brackets
            // (`[testing] cargo test failed`, the historical wire-format
            // shape) or via the helper invocation
            // (`feedback-gate ... testing "cargo test failed`, the v0.5.0
            // helper-call shape). We allow both.
            if needle == "cargo test"
                && (gate_prose.contains("[testing] cargo test failed")
                    || gate_prose.contains("testing \"cargo test failed"))
            {
                let cleaned = gate_prose.replace("cargo test failed", "<failure>");
                assert!(
                    !cleaned.contains("cargo test"),
                    "gate prose must not contain hardcoded 'cargo test' outside the §7 example"
                );
                continue;
            }
            assert!(
                !gate_prose.contains(needle),
                "gate prose must not contain hardcoded '{needle}'; replace with the matching placeholder"
            );
        }
    }

    #[test]
    fn render_change_id_placeholder_passes_through() {
        let tmpl = SkillTemplate {
            name: "supervisor".into(),
            content: "Run {{SPEC_VALIDATE_COMMAND}}.".into(),
            source: Source::Embedded,
            format: SkillFormat::Standardized,
            metadata: None,
            resource_paths: None,
        };
        let gates = GateCommands {
            spec_validate_command: Some("openspec validate {{CHANGE_ID}} --strict"),
            ..Default::default()
        };
        let output = render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &gates,
            &[],
        );
        assert!(
            output.contains("Run openspec validate {{CHANGE_ID}} --strict."),
            "outer placeholder substituted but inner {{CHANGE_ID}} preserved; got: {output}"
        );
        assert!(
            output.contains("{{CHANGE_ID}}"),
            "{{CHANGE_ID}} must survive verbatim (not substituted at render time); got: {output}"
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

    /// Task 5.3 — the rendered boot block calls `.git-paw/scripts/broker.sh`
    /// for all four events, contains no raw broker `curl`, and the DONE
    /// fallback still publishes `agent.artifact { status: "done" }`.
    #[test]
    fn boot_block_all_four_events_call_helper_no_raw_curl() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");

        // No raw broker curl anywhere.
        assert!(
            !block.contains("curl -s -X POST"),
            "boot block must not inline a raw broker curl for any event"
        );
        assert!(
            !block.contains("{{GIT_PAW_BROKER_URL}}"),
            "boot block must not leak the broker-URL placeholder"
        );

        // Each event calls the helper with the pre-expanded agent id.
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test status booting"),
            "REGISTER event should call broker.sh status"
        );
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test artifact"),
            "DONE-fallback event should call broker.sh artifact"
        );
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test blocked"),
            "BLOCKED event should call broker.sh blocked"
        );
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test question"),
            "QUESTION event should call broker.sh question"
        );

        // The DONE fallback still describes publishing agent.artifact status done.
        assert!(
            block.contains("agent.artifact") && block.contains("status: \"done\""),
            "DONE fallback should still publish agent.artifact status: done"
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
    fn boot_block_uses_helper_not_raw_broker_url() {
        let block = build_boot_block("feat/x", "http://127.0.0.1:9119");
        // The broker URL and JSON shaping now live inside the helper, so the
        // boot block must not inline a raw broker `curl` for any event.
        assert!(
            !block.contains("curl -s -X POST http://127.0.0.1:9119/publish"),
            "boot block must not inline a raw broker curl"
        );
        assert!(
            !block.contains("{{GIT_PAW_BROKER_URL}}"),
            "GIT_PAW_BROKER_URL placeholder must not leak into the rendered block"
        );
        assert!(
            block.contains(".git-paw/scripts/broker.sh"),
            "boot block must invoke the bundled broker.sh helper"
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
    fn boot_block_contains_pre_expanded_helper_invocations() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");

        // Each event calls the helper with the pre-expanded branch id.
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test status booting"),
            "REGISTER should call broker.sh with the pre-expanded agent id"
        );
        assert!(
            block.contains(".git-paw/scripts/broker.sh --agent feat-test"),
            "Agent ID not substituted in broker.sh invocations"
        );
        // No raw broker curl should remain anywhere in the block.
        assert!(
            !block.contains("curl -s -X POST"),
            "boot block must not contain a raw broker curl"
        );
    }

    fn done_section_body(block: &str) -> String {
        let start = block
            .find("### 2. DONE")
            .expect("rendered boot block should contain the DONE section heading");
        let end = block
            .find("### 3. BLOCKED")
            .expect("rendered boot block should contain the BLOCKED section heading");
        block[start..end].to_string()
    }

    #[test]
    fn boot_block_done_section_leads_with_commit_instruction() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");
        let done_body = done_section_body(&block);

        let commit_idx = done_body
            .find("commit your work")
            .or_else(|| done_body.find("git commit"))
            .expect("DONE section should lead with a commit-first instruction");

        let manual_done_idx = done_body
            .find(".git-paw/scripts/broker.sh --agent feat-test artifact")
            .expect("DONE section should still contain the manual artifact helper as a fallback");

        assert!(
            commit_idx < manual_done_idx,
            "commit-first instruction (byte {commit_idx}) must appear before the manual artifact helper (byte {manual_done_idx})"
        );
    }

    #[test]
    fn boot_block_done_section_names_committed_status_published_by_hook() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");
        let done_body = done_section_body(&block);

        assert!(
            done_body.contains("status: \"committed\"")
                || done_body.contains("status:\"committed\""),
            "DONE section should name the `status: \"committed\"` event published by the hook"
        );
        assert!(
            done_body.contains("post-commit hook"),
            "DONE section should mention the post-commit hook that publishes on the agent's behalf"
        );
    }

    #[test]
    fn boot_block_done_section_scopes_manual_done_to_code_less_tasks() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");
        let done_body = done_section_body(&block);

        let hits = ["docs-only", "planning", "exploration"]
            .iter()
            .filter(|needle| done_body.contains(*needle))
            .count();
        assert!(
            hits >= 2,
            "DONE section should enumerate at least two code-less task examples \
             (docs-only / planning / exploration); only {hits} present"
        );
    }

    #[test]
    fn boot_block_done_section_warns_against_manual_done_with_uncommitted_changes() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");
        let done_body = done_section_body(&block);

        assert!(
            done_body.contains("uncommitted"),
            "DONE section should warn about uncommitted changes"
        );
        assert!(
            done_body.contains("manual `done`") || done_body.contains("manual done"),
            "DONE section warning should reference manual `done`"
        );
        assert!(
            done_body.contains("**WARNING") || done_body.contains("**DO NOT"),
            "DONE section warning should be emphasised with bold markers (**...**)"
        );
    }

    #[test]
    fn boot_block_done_section_retains_manual_done_helper() {
        let block = build_boot_block("feat/test", "http://127.0.0.1:9119");
        let done_body = done_section_body(&block);

        // The manual fallback is now a copy-pasteable broker.sh artifact
        // invocation, not a raw curl.
        assert!(
            done_body.contains(".git-paw/scripts/broker.sh --agent feat-test artifact"),
            "DONE section should retain the manual artifact helper invocation"
        );
        assert!(
            !done_body.contains("curl -s -X POST"),
            "DONE section must not retain a raw broker curl"
        );
        // The helper publishes the same agent.artifact { status: "done" }
        // shape; the prose names the event and the exports/files fields the
        // helper maps onto the payload.
        assert!(
            done_body.contains("agent.artifact"),
            "DONE section should name the agent.artifact event the helper publishes"
        );
        assert!(
            done_body.contains("status: \"done\"") || done_body.contains("status:\"done\""),
            "DONE section should describe the status: done manual fallback"
        );
        assert!(
            done_body.contains("--exports"),
            "DONE section should show the --exports flag mapping onto the exports field"
        );
        assert!(
            done_body.contains("--files"),
            "DONE section should show the --files flag mapping onto modified_files"
        );
    }

    // -----------------------------------------------------------------
    // conflict-detection skill content (v0.5.0)
    // -----------------------------------------------------------------

    #[test]
    fn supervisor_skill_contains_conflict_detector_tag() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("[conflict-detector]"),
            "supervisor skill should reference the [conflict-detector] tag"
        );
    }

    #[test]
    fn supervisor_skill_documents_broker_side_detection() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("auto-detect") || lowered.contains("auto-emit"),
            "skill should mention auto-detection/auto-emission by the broker"
        );
        assert!(
            lowered.contains("forward conflict"),
            "skill should mention forward conflict"
        );
        assert!(
            lowered.contains("in-flight conflict"),
            "skill should mention in-flight conflict"
        );
        assert!(
            lowered.contains("ownership violation"),
            "skill should mention ownership violation"
        );
    }

    #[test]
    fn supervisor_skill_removes_v04_manual_conflict_detection() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            !tmpl
                .content
                .contains("Compare the `modified_files` arrays from every `agent.artifact` event"),
            "supervisor skill should no longer contain the v0.4 manual conflict-comparison instructions"
        );
    }

    #[test]
    fn supervisor_skill_mentions_agent_intent() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(tmpl.content.contains("agent.intent"));
        assert!(
            tmpl.content.contains("Watch peer intents")
                || tmpl
                    .content
                    .contains("Watch peer intents and broker-side conflict detection"),
            "skill should contain a 'Watch peer intents' heading"
        );
    }

    #[test]
    fn supervisor_skill_focuses_on_question_escalations() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        // The supervisor agent's role on detector output is to react to
        // agent.question escalations and follow up on repeat offenders.
        assert!(
            lowered.contains("agent.question")
                && (lowered.contains("escalation") || lowered.contains("escalat")),
            "skill should direct the supervisor agent at agent.question escalations"
        );
        assert!(
            lowered.contains("do not") && lowered.contains("manually"),
            "skill should tell the supervisor not to duplicate by manual comparison"
        );
    }

    // --- Spec Kit consolidated worktree section (`spec-kit-format` change) ---

    #[test]
    fn embedded_coordination_mentions_spec_kit_consolidated_worktrees() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("Spec Kit")
                && (tmpl.content.contains("consolidated") || tmpl.content.contains("phase/")),
            "coordination skill should mention Spec Kit consolidated worktrees"
        );
    }

    #[test]
    fn embedded_coordination_instructs_sequential_work_and_writeback() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("sequential") || tmpl.content.contains("Sequential"),
            "should instruct sequential execution"
        );
        assert!(
            tmpl.content.contains("`- [x]`") || tmpl.content.contains("- [x]"),
            "should mention - [x] writeback"
        );
        assert!(
            tmpl.content.contains("tasks.md"),
            "should reference tasks.md as writeback target"
        );
    }

    #[test]
    fn embedded_coordination_states_agent_done_timing_for_consolidated() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("agent.done"),
            "should mention agent.done"
        );
        let lower = tmpl.content.to_lowercase();
        assert!(
            lower.contains("every task")
                || lower.contains("all listed tasks")
                || lower.contains("all tasks"),
            "should tie agent.done to completion of all listed tasks"
        );
    }

    #[test]
    fn embedded_coordination_clarifies_p_worktrees_follow_standard_pattern() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("[P]") || tmpl.content.contains("task/"),
            "should distinguish [P] / task/ worktrees from consolidated ones"
        );
        assert!(
            tmpl.content.contains("standard"),
            "should reference the standard before/while-editing pattern"
        );
    }

    // -----------------------------------------------------------------------
    // supervisor-as-pane (v0.5.0) — interactive user input + merge orchestration
    // -----------------------------------------------------------------------

    /// section heading.
    #[test]
    fn supervisor_skill_has_user_input_section() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("When the user types in your pane"),
            "supervisor skill should include the 'When the user types in your pane' section"
        );
    }

    /// 8.2 — user-input section maps directives to `agent.feedback`.
    #[test]
    fn supervisor_skill_user_input_uses_agent_feedback_for_directives() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("When the user types in your pane")
            .expect("user-input section heading present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("agent.feedback"),
            "user-input directives section should reference agent.feedback"
        );
    }

    /// 8.3 — user-input section maps judgment-call asks to `agent.question`.
    #[test]
    fn supervisor_skill_user_input_uses_agent_question_for_judgment_calls() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("When the user types in your pane")
            .expect("user-input section heading present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("agent.question"),
            "user-input judgment-call section should reference agent.question"
        );
    }

    /// 8.4 — user-input section states the autonomous loop continues.
    #[test]
    fn supervisor_skill_user_input_states_loop_continues() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("When the user types in your pane")
            .expect("user-input section heading present");
        let window = &tmpl.content[start..];
        assert!(
            window.to_lowercase().contains("autonomous"),
            "user-input section should state the autonomous loop continues alongside user input"
        );
    }

    /// 8.5 — supervisor skill contains the "Merge orchestration" section.
    #[test]
    fn supervisor_skill_has_merge_orchestration_section() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("Merge orchestration"),
            "supervisor skill should include the 'Merge orchestration' section"
        );
    }

    /// 8.6 — merge orchestration uses `git merge --ff-only`.
    #[test]
    fn supervisor_skill_merge_uses_ff_only() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Merge orchestration")
            .expect("merge orchestration section present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("git merge --ff-only"),
            "merge orchestration should specify git merge --ff-only"
        );
    }

    /// revert.
    #[test]
    fn supervisor_skill_merge_reverts_via_reset_hard() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Merge orchestration")
            .expect("merge orchestration section present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("git reset --hard"),
            "merge orchestration should describe regression revert via git reset --hard"
        );
    }

    /// `agent.question`.
    #[test]
    fn supervisor_skill_merge_cycle_uses_agent_question() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Merge orchestration")
            .expect("merge orchestration section present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("agent.question") && window.to_lowercase().contains("cycle"),
            "merge orchestration cycle handling should publish agent.question"
        );
    }

    /// 8.9 — merge orchestration ends with a final `agent.status` summary.
    #[test]
    fn supervisor_skill_merge_publishes_final_status_summary() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Merge orchestration")
            .expect("merge orchestration section present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("agent.status") && window.to_lowercase().contains("summary"),
            "merge orchestration should end with a final agent.status summary"
        );
    }

    // === coordination-skill-followups: drift 34, 37, 54, 55, 56, 57 ===

    /// drift 54 — coordination skill names both `agent_id` and `slugify_branch` in a
    /// references/terminology section.
    #[test]
    fn coordination_skill_documents_slugify_terminology() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("agent_id"),
            "coordination skill should mention the agent_id identifier form"
        );
        assert!(
            tmpl.content.contains("slugify_branch"),
            "coordination skill should name slugify_branch as the canonical conversion"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("references & terminology")
                || lowered.contains("references and terminology")
                || lowered.contains("terminology"),
            "coordination skill should contain a references/terminology heading"
        );
    }

    /// drift 57 — coordination skill documents stash-hygiene rules.
    #[test]
    fn coordination_skill_documents_stash_hygiene() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("git stash list"),
            "stash-hygiene section should reference `git stash list`"
        );
        assert!(
            tmpl.content.contains("git stash show -p"),
            "stash-hygiene section should reference `git stash show -p`"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("stash hygiene") || lowered.contains("stash safety"),
            "coordination skill should contain a stash-hygiene heading"
        );
        assert!(
            lowered.contains("pop only") || lowered.contains("only pop"),
            "coordination skill should instruct agents to pop only their own stashes"
        );
    }

    /// drift 55 — supervisor skill documents publishing agent.intent for main-side
    /// work with `agent_id` = "supervisor".
    #[test]
    fn supervisor_skill_documents_main_side_intent() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("supervisor publishes agent.intent")
                || lowered.contains("publish intent")
                || lowered.contains("main-side work"),
            "supervisor skill should contain a heading naming supervisor-side intent publishing"
        );
        let start = tmpl
            .content
            .find("Supervisor publishes agent.intent")
            .expect("supervisor-publishes-intent heading present");
        let window = &tmpl.content[start..];
        assert!(
            window.contains("agent.intent"),
            "section should mention agent.intent"
        );
        assert!(
            window.contains("\"supervisor\""),
            "section should show agent_id = \"supervisor\" in the example"
        );
        assert!(
            window.contains("\"files\"")
                && window.contains("\"summary\"")
                && window.contains("\"valid_for_seconds\""),
            "section should include a curl example with files, summary, valid_for_seconds"
        );
    }

    /// drift 34 — supervisor skill instructs `tmux send-keys` alongside
    /// `agent.feedback` answers, with the "agents do not poll" rationale.
    #[test]
    fn supervisor_skill_documents_tmux_send_keys_alongside_feedback() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Send the answer to the agent pane too")
            .expect("drift-34 subsection should be present");
        let next_heading = tmpl.content[start + 1..]
            .find("\n### ")
            .map_or(tmpl.content.len(), |off| start + 1 + off);
        let section = &tmpl.content[start..next_heading];
        assert!(
            section.contains("tmux send-keys"),
            "section should contain `tmux send-keys`"
        );
        assert!(
            section.contains("agent.feedback"),
            "section should reference agent.feedback in the same section"
        );
        let lowered_section = section.to_lowercase();
        assert!(
            lowered_section.contains("do not poll") || lowered_section.contains("don't poll"),
            "section should state the rationale (agents do not poll their inbox)"
        );
    }

    /// drift 37 — coordination skill documents the working-heartbeat cadence and
    /// the filesystem-watcher rationale.
    #[test]
    fn coordination_skill_documents_working_heartbeat() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("working heartbeat") || lowered.contains("heartbeat"),
            "coordination skill should contain a working-heartbeat heading"
        );
        assert!(
            tmpl.content.contains("every 5 tool uses"),
            "coordination skill should state the cadence as 'every 5 tool uses'"
        );
        assert!(
            tmpl.content.contains("agent.status"),
            "heartbeat reuses the agent.status shape — substring should be present"
        );
        let start = tmpl
            .content
            .find("Working heartbeat")
            .expect("Working heartbeat heading present");
        let next_heading = tmpl.content[start + 1..]
            .find("\n### ")
            .map_or(tmpl.content.len(), |off| start + 1 + off);
        let section = &tmpl.content[start..next_heading].to_lowercase();
        assert!(
            section.contains("filesystem watcher") || section.contains("watcher"),
            "heartbeat section should explain why the filesystem watcher is insufficient"
        );
    }

    /// drift 56 — supervisor skill documents the accept-edits `modified_files` audit
    /// step with explicit non-silent-approval guidance.
    #[test]
    fn supervisor_skill_documents_accept_edits_audit() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("accept-edits commits") || lowered.contains("accept edits"),
            "supervisor skill should contain an accept-edits audit heading"
        );
        assert!(
            tmpl.content.contains("modified_files"),
            "audit section should reference the modified_files payload field"
        );
        let start = tmpl
            .content
            .find("Verify accept-edits commits before merge")
            .expect("accept-edits audit heading present");
        let next_heading = tmpl.content[start + 1..]
            .find("\n### ")
            .map_or(tmpl.content.len(), |off| start + 1 + off);
        let section_lower = tmpl.content[start..next_heading].to_lowercase();
        assert!(
            section_lower.contains("out-of-scope"),
            "audit section should call out 'out-of-scope' edits"
        );
        assert!(
            section_lower.contains("shall not be silently")
                || section_lower.contains("not be silently auto-approved")
                || section_lower.contains("silently auto-approved"),
            "audit section should forbid silent auto-approval"
        );
    }

    /// drift 54 (optional 3.5) — coordination skill describes the slugify rule's
    /// effect: lowercase, non-allowed-char replacement, and `agent` fallback.
    #[test]
    fn coordination_skill_describes_slugify_rule() {
        let tmpl = resolve("coordination").unwrap();
        let start = tmpl
            .content
            .find("slugify_branch")
            .expect("slugify_branch should be named in the references section");
        let next_heading = tmpl.content[start + 1..]
            .find("\n### ")
            .map_or(tmpl.content.len(), |off| start + 1 + off);
        let section_lower = tmpl.content[start..next_heading].to_lowercase();
        assert!(
            section_lower.contains("lowercase"),
            "slugify rule should mention lowercase step"
        );
        assert!(
            tmpl.content[start..next_heading].contains("[a-z0-9_]"),
            "slugify rule should describe the allowed char class"
        );
        assert!(
            (section_lower.contains("fallback") || section_lower.contains("fall back"))
                && section_lower.contains("agent"),
            "slugify rule should describe the empty-fallback to `agent`"
        );
    }

    // --- test-coverage-v0-5-0 -------------------------------------------------
    //
    // The following tests close per-scenario coverage gaps from the v0.5.0
    // archived spec set. See `openspec/changes/test-coverage-v0-5-0/tasks.md`.

    // Renders the supervisor skill with a representative set of substitutions.
    // Tests assert against the rendered output so any post-render
    // transformation regressions are caught.
    fn rendered_supervisor() -> String {
        let tmpl = resolve("supervisor").expect("supervisor skill resolves");
        render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        )
    }

    fn rendered_coordination() -> String {
        let tmpl = resolve("coordination").expect("coordination skill resolves");
        render(
            &tmpl,
            "feat/x",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
        )
    }

    // Maps to scenario `Supervisor skill — lenient indicator framing` from
    // prompt-submit-fix. (task 3.3)
    #[test]
    fn supervisor_skill_paste_buffer_framing_is_lenient() {
        let content = rendered_supervisor();
        let lowered = content.to_lowercase();
        assert!(
            lowered.contains("even if"),
            "supervisor skill should frame recovery as attempted even when indicator absent; got:\n{content}"
        );
        assert!(
            lowered.contains("judgment"),
            "supervisor skill should describe applying judgment; got:\n{content}"
        );
        assert!(
            lowered.contains("long buffered text"),
            "supervisor skill should mention the long-buffered-text heuristic; got:\n{content}"
        );
    }

    // Maps to scenario `Coordination skill rejects pairwise over-coordination
    // patterns` from forward-coordination. (task 4.1)
    #[test]
    fn coordination_skill_rejects_pairwise_overcoordination() {
        let content = rendered_coordination();
        assert!(
            content.contains("pairwise"),
            "coordination skill should name `pairwise` under a MUST-NOT clause; got:\n{content}"
        );
        let lowered = content.to_lowercase();
        assert!(
            lowered.contains("explicit go-ahead"),
            "coordination skill should reject waiting for an explicit go-ahead; got:\n{content}"
        );
        assert!(
            lowered.contains("broker silence") || lowered.contains("block on broker silence"),
            "coordination skill should reject blocking on broker silence; got:\n{content}"
        );
    }

    // Maps to scenario `Verification/feedback wording separability` from
    // forward-coordination. (task 4.3)
    //
    // The two message types must be separately reachable — i.e. each lives in
    // its own bullet or heading. We assert their distinct anchor lines:
    // `- **agent.verified**` and `- **agent.feedback**`.
    #[test]
    fn coordination_skill_verified_and_feedback_substrings_independent() {
        let content = rendered_coordination();
        let verified_anchor = "- **`agent.verified`**";
        let feedback_anchor = "- **`agent.feedback`**";
        assert!(
            content.contains(verified_anchor),
            "coordination skill should anchor `agent.verified` in its own bullet; got:\n{content}"
        );
        assert!(
            content.contains(feedback_anchor),
            "coordination skill should anchor `agent.feedback` in its own bullet; got:\n{content}"
        );
        // The two anchors must not be on the same line.
        let v = content.find(verified_anchor).unwrap();
        let f = content.find(feedback_anchor).unwrap();
        let between = if v < f {
            &content[v..f]
        } else {
            &content[f..v]
        };
        assert!(
            between.contains('\n'),
            "the verified and feedback bullets must be on separate lines; got slice:\n{between}"
        );
    }

    // Maps to scenario `Supervisor skill specifies the ordering` from
    // governance-context. (task 10.1)
    //
    // Ordering invariant: Spec Audit Procedure < Governance verification <
    // the publish step that emits `agent.verified`.
    #[test]
    fn supervisor_skill_governance_after_spec_audit_before_verified() {
        let content = rendered_supervisor();
        let spec_audit = content
            .find("Spec Audit Procedure")
            .expect("Spec Audit Procedure heading present in supervisor skill");
        let governance = content
            .find("Governance verification")
            .expect("Governance verification heading present in supervisor skill");
        // The closest publish step emitting `agent.verified` after the
        // governance heading is the next occurrence of `agent.verified`.
        let verified_after = content[governance..]
            .find("agent.verified")
            .map(|o| governance + o)
            .expect("agent.verified mention after Governance verification");

        assert!(
            spec_audit < governance,
            "Spec Audit Procedure should appear before Governance verification \
             (spec_audit={spec_audit}, governance={governance})"
        );
        assert!(
            governance < verified_after,
            "Governance verification should appear before the next agent.verified \
             publish step (governance={governance}, verified_after={verified_after})"
        );
    }

    // Maps to scenario `Coordination skill states agent.done timing for
    // consolidated worktrees` from spec-kit-format. (task 11.6)
    #[test]
    fn coordination_skill_consolidated_agent_done_timing() {
        let content = rendered_coordination();
        let start = content
            .find("consolidated worktree")
            .or_else(|| content.find("Consolidated worktree"))
            .expect("coordination skill should have a consolidated-worktree section");
        let section = &content[start..];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("agent.done") || lowered.contains("agent.artifact"),
            "consolidated-worktree section should describe agent.done timing; got:\n{section}"
        );
        assert!(
            section.contains("- [x]"),
            "consolidated-worktree section should require every task to show - [x]; got:\n{section}"
        );
        assert!(
            lowered.contains("every task") || lowered.contains("every"),
            "consolidated-worktree section should make the rule cover every task; got:\n{section}"
        );
    }

    /// drift 55 (optional 3.6) — supervisor-publishes-intent section cross-references
    /// the agent-side `Before you start editing` flow in `coordination.md`.
    #[test]
    fn supervisor_skill_cross_references_agent_intent_flow() {
        let tmpl = resolve("supervisor").unwrap();
        let start = tmpl
            .content
            .find("Supervisor publishes agent.intent")
            .expect("supervisor-publishes-intent heading present");
        let next_heading = tmpl.content[start + 1..]
            .find("\n### ")
            .map_or(tmpl.content.len(), |off| start + 1 + off);
        let section = &tmpl.content[start..next_heading];
        assert!(
            section.contains("Before you start editing"),
            "supervisor-publishes-intent section should cross-reference the agent-side \
             `Before you start editing` heading"
        );
        assert!(
            section.contains("coordination.md"),
            "cross-reference should name the coordination skill file"
        );
    }

    // ---------------------------------------------------------------------------
    // supervisor-as-pane-followups: skill-content tests
    // (tasks 8.3, 8.4, 8a.4-8a.7, 8b.7-8b.12)
    // ---------------------------------------------------------------------------

    fn render_supervisor() -> String {
        let tmpl = resolve("supervisor").expect("resolve supervisor template");
        render(
            &tmpl,
            "supervisor",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands {
                test_command: Some("just check"),
                ..Default::default()
            },
            &[],
        )
    }

    /// 8.3 — resolved supervisor skill contains a curl publishing an
    /// `agent.status` for `agent_id = "supervisor"`, and that payload does
    /// NOT self-report a `cli` (git-paw pre-fills the CLI authoritatively at
    /// launch — a self-reported guess once clobbered the seed).
    #[test]
    fn supervisor_skill_self_register_curl_omits_cli_field() {
        let rendered = render_supervisor();
        let start = rendered
            .find("Bootstrap")
            .expect("Bootstrap section heading present");
        let next = rendered[start..]
            .find("### Poll session status and messages")
            .map_or(rendered.len(), |p| start + p);
        let section = &rendered[start..next];
        assert!(
            section.contains("agent.status"),
            "bootstrap section must publish agent.status; got:\n{section}"
        );
        assert!(
            section.contains("\"agent_id\":\"supervisor\""),
            "bootstrap curl must use agent_id=\"supervisor\"; got:\n{section}"
        );
        assert!(
            !section.contains("\"cli\""),
            "bootstrap payload must NOT self-report a cli field (git-paw pre-fills it); got:\n{section}"
        );
    }

    /// 8.4 — bootstrap section names this as the FIRST action after
    /// reading the skill / AGENTS.md, not a "you may" suggestion.
    #[test]
    fn supervisor_skill_self_register_is_first_action() {
        let rendered = render_supervisor();
        let pos_bootstrap = rendered
            .find("Bootstrap")
            .expect("Bootstrap heading present");
        let section_end = rendered[pos_bootstrap..]
            .find("### Poll session status and messages")
            .map_or(rendered.len(), |p| pos_bootstrap + p);
        let section = &rendered[pos_bootstrap..section_end];
        let lower = section.to_lowercase();
        assert!(
            lower.contains("first action") || lower.contains("very first"),
            "bootstrap section must state this is the agent's first action; got:\n{section}"
        );
    }

    /// 8a.4 — Watch section explicitly mentions per-iteration sweeping.
    #[test]
    fn supervisor_skill_watch_mentions_per_iteration_sweep() {
        let rendered = render_supervisor();
        let start = rendered
            .find("**Watch**")
            .expect("Watch step heading present");
        let end = rendered[start..]
            .find("Stall detection")
            .map_or(rendered.len(), |p| start + p);
        let section = &rendered[start..end];
        let lower = section.to_lowercase();
        assert!(
            lower.contains("every iteration")
                || lower.contains("every monitoring")
                || lower.contains("each monitoring")
                || lower.contains("each iteration"),
            "Watch section must mention per-iteration sweeping; got:\n{section}"
        );
    }

    /// 8a.5 — Rules section bullet mentions absorbing routine approvals
    /// AND at least three routine command families (now sourced from the
    /// rendered `{{DEV_ALLOWLIST_PRESET}}` prose).
    #[test]
    fn supervisor_skill_rules_bullet_mentions_routine_absorption() {
        let rendered = render_supervisor();
        let start = rendered.find("### Rules").expect("Rules section present");
        let end = rendered[start..]
            .find("### Auto-approve permission prompts")
            .map_or(rendered.len(), |p| start + p);
        let section = &rendered[start..end];
        let lower = section.to_lowercase();
        assert!(
            lower.contains("absorb routine approval") || lower.contains("rubber-stamp"),
            "Rules must include the routine-approval absorption framing; got:\n{section}"
        );
        // The rules bullet embeds {{DEV_ALLOWLIST_PRESET}}, which now
        // renders the stack-neutral universal preset grouped by first
        // word: `git (status, log, diff, ...)`, `find`, `grep`,
        // `sed -n`. Match against the universal families.
        let mut family_hits = 0;
        for family in ["git (", "find", "grep", "sed"] {
            if section.contains(family) {
                family_hits += 1;
            }
        }
        assert!(
            family_hits >= 3,
            "Rules bullet must enumerate at least 3 routine families; only {family_hits} found in:\n{section}",
        );
    }

    /// 8a.6 — Rules bullet also enumerates at least two non-routine
    /// escalation cases.
    #[test]
    fn supervisor_skill_rules_bullet_enumerates_escalation_cases() {
        let rendered = render_supervisor();
        let start = rendered.find("### Rules").expect("Rules section present");
        let end = rendered[start..]
            .find("### Auto-approve permission prompts")
            .map_or(rendered.len(), |p| start + p);
        let section = &rendered[start..end];
        let lower = section.to_lowercase();
        let mut hits = 0;
        for case in [
            "cross-agent conflict",
            "destructive",
            "scope",
            "spec decisions",
            "novel",
        ] {
            if lower.contains(case) {
                hits += 1;
            }
        }
        assert!(
            hits >= 2,
            "Rules bullet must enumerate at least 2 escalation cases; only {hits} found in:\n{section}",
        );
    }

    /// 8a.7 — Watch section contains the phrase "every iteration" or
    /// "every monitoring" (verbatim).
    #[test]
    fn supervisor_skill_contains_every_iteration_phrase() {
        let rendered = render_supervisor();
        let lower = rendered.to_lowercase();
        assert!(
            lower.contains("every iteration") || lower.contains("every monitoring"),
            "skill must contain 'every iteration' or 'every monitoring' phrasing somewhere",
        );
    }

    /// 8b.7 — supervisor skill contains the five gate names in order.
    #[test]
    fn supervisor_skill_enumerates_five_gates_in_order() {
        let rendered = render_supervisor();
        let pos = |needle: &str| {
            rendered
                .find(needle)
                .unwrap_or_else(|| panic!("gate '{needle}' not found in supervisor skill"))
        };
        let pos_testing = pos("**Testing**");
        let pos_regression = pos("**Regression analysis**");
        let pos_spec = pos("**Spec audit**");
        let pos_doc = pos("**Doc audit**");
        let pos_security = pos("**Security audit**");
        assert!(
            pos_testing < pos_regression
                && pos_regression < pos_spec
                && pos_spec < pos_doc
                && pos_doc < pos_security,
            "five gates must appear in order Testing < Regression < Spec < Doc < Security; \
             got positions Testing={pos_testing} Regression={pos_regression} \
             Spec={pos_spec} Doc={pos_doc} Security={pos_security}",
        );
    }

    /// 8b.8 — §7 Verify-or-feedback's `agent.verified` example body
    /// mentions all five gate names.
    #[test]
    fn supervisor_skill_verified_message_enumerates_five_gates() {
        let rendered = render_supervisor();
        // Anchor on §7 specifically — the supervisor skill has an earlier
        // `agent.verified` example near the top of the file that pre-dates
        // the five-gate restructure.
        let verify_start = rendered
            .find("**Verify or feedback**")
            .expect("Verify or feedback step present");
        let window = &rendered[verify_start..];
        let lower = window.to_lowercase();
        for needle in [
            "testing",
            "regression",
            "spec audit",
            "doc audit",
            "security audit",
        ] {
            assert!(
                lower.contains(needle),
                "§7 Verify-or-feedback must mention '{needle}'; got window:\n{window}",
            );
        }
    }

    /// 8b.9 — §7's `agent.feedback` examples mention the gate-name
    /// convention with at least three of the five gates shown. The
    /// supervisor skill now wraps feedback through
    /// `.git-paw/scripts/sweep.sh feedback-gate <agent> <gate> <msg>`,
    /// so a gate name passed as the second argument satisfies the
    /// convention equivalently to a bracketed `[gate]` prefix.
    #[test]
    fn supervisor_skill_feedback_example_uses_gate_name_prefixes() {
        let rendered = render_supervisor();
        let verify_start = rendered
            .find("**Verify or feedback**")
            .expect("Verify or feedback step present");
        // Cap the window at the next top-level section so we don't bleed
        // into "Spec Audit Procedure".
        let end = rendered[verify_start..]
            .find("\n### ")
            .map_or(rendered.len(), |p| verify_start + p);
        let window = &rendered[verify_start..end];
        let mut hits = 0;
        for (bracketed, helper_arg) in [
            ("[testing]", " testing "),
            ("[regression]", " regression "),
            ("[spec audit]", " \"spec audit\" "),
            ("[doc audit]", " \"doc audit\" "),
            ("[security audit]", " \"security audit\" "),
        ] {
            if window.contains(bracketed)
                || window.contains(&format!("feedback-gate __FILL_IN_AGENT_ID__{helper_arg}"))
            {
                hits += 1;
            }
        }
        assert!(
            hits >= 3,
            "§7 agent.feedback example must show at least 3 gates (bracketed or helper-arg); \
             only {hits} found in:\n{window}",
        );
    }

    /// 8b.10 — Doc audit gate enumerates the doc-surface categories any
    /// project might carry. v0.6.0+ uses language-neutral wording instead
    /// of Rust-specific surfaces (was `docs/src/`, `rustdoc`); the
    /// equivalents now are "user-guide pages" + the configured
    /// `{{DOC_TOOL_COMMAND}}` placeholder for the API-doc generator.
    #[test]
    fn supervisor_skill_doc_audit_enumerates_surfaces() {
        let rendered = render_supervisor();
        let start = rendered
            .find("**Doc audit**")
            .expect("Doc audit gate present");
        let end = rendered[start..]
            .find("**Security audit**")
            .map(|p| start + p)
            .expect("Security audit follows Doc audit");
        let section = &rendered[start..end];
        let mut hits = 0;
        for surface in [
            "user-guide",
            "README.md",
            "AGENTS.md",
            "--help",
            "doc_tool_command",
        ] {
            if section.contains(surface) {
                hits += 1;
            }
        }
        assert!(
            hits >= 4,
            "Doc audit must enumerate at least 4 of 5 doc-surface categories; only {hits} found in:\n{section}",
        );
    }

    /// 8b.11 — Security audit gate enumerates at least 4 of 6 OWASP
    /// categories AND mentions the `unwrap()`/`expect()` rule.
    #[test]
    fn supervisor_skill_security_audit_enumerates_owasp_categories() {
        let rendered = render_supervisor();
        let start = rendered
            .find("**Security audit**")
            .expect("Security audit gate present");
        let end = rendered[start..]
            .find("**Verify or feedback**")
            .map_or(rendered.len(), |p| start + p);
        let section = &rendered[start..end];
        let lower = section.to_lowercase();
        let mut hits = 0;
        for cat in [
            "command injection",
            "xss",
            "sql injection",
            "path traversal",
            "unvalidated external input",
            "secret leakage",
        ] {
            if lower.contains(cat) {
                hits += 1;
            }
        }
        assert!(
            hits >= 4,
            "Security audit must enumerate at least 4 of 6 OWASP categories; only {hits} found in:\n{section}",
        );
        assert!(
            section.contains("unwrap()") || section.contains("expect()"),
            "Security audit must mention the unwrap()/expect() rule; got:\n{section}",
        );
    }

    /// 8b.12 — Governance verification sub-step is preserved (`DoD`,
    /// ADRs, `security.md`, `test-strategy.md`, `constitution.md` still present).
    #[test]
    fn supervisor_skill_governance_verification_substep_preserved() {
        let rendered = render_supervisor();
        let start = rendered
            .find("Governance verification")
            .expect("Governance verification sub-step still present");
        let end = (start + 2000).min(rendered.len());
        let section = &rendered[start..end];
        for needle in [
            "DoD",
            "ADR",
            "security.md",
            "test-strategy.md",
            "constitution.md",
        ] {
            assert!(
                section.contains(needle),
                "governance sub-step must still reference '{needle}'; got:\n{section}",
            );
        }
    }

    // ---------------------------------------------------------------------------
    // coordination-skill-followups-2: skill-content tests
    // (tasks 1.3, 2.3, 2.4, 3.3)
    // ---------------------------------------------------------------------------

    /// Spec `agent-skills` / "Coordination skill SHALL teach per-group commit
    /// cadence": the coordination skill names the per-group cadence and defers
    /// commit-message FORMAT entirely to the host project's `AGENTS.md`. It does
    /// NOT present a Conventional-Commits prefix as git-paw's example, default,
    /// or recommendation — Conventional Commits is git-paw's OWN repo convention
    /// (it lives only in git-paw's injected `AGENTS.md`, never in the asset the
    /// binary exports to every consumer).
    #[test]
    fn coordination_skill_documents_commit_cadence() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("commit cadence") || lowered.contains("per-group commit cadence"),
            "coordination skill should have a heading naming the commit-cadence concept; \
             got:\n{}",
            tmpl.content
        );
        assert!(
            lowered.contains("group") || lowered.contains("section"),
            "commit-cadence section should mention the GROUP/section grain"
        );
        // De-opinionated: the section defers commit-message format to the host
        // project's `AGENTS.md` instead of mandating, defaulting to, or
        // recommending one.
        assert!(
            lowered.contains("agents.md"),
            "commit-cadence section should reference the project's AGENTS.md as the \
             source of commit-message conventions"
        );
        // The exported asset SHALL NOT present a Conventional-Commits prefix as
        // git-paw's example, default, or recommendation. Any commit example the
        // section needs (e.g. the `(part N of M)` split) uses a format-neutral
        // subject with no convention-specific prefix.
        let has_conventional_prefix = ["feat(", "fix(", "docs(", "test(", "chore("]
            .iter()
            .any(|p| tmpl.content.contains(p));
        assert!(
            !has_conventional_prefix,
            "commit-cadence section must NOT show a Conventional-Commits prefix \
             (feat(/fix(/…) — defer message format to the project's AGENTS.md and \
             use only format-neutral commit examples"
        );
    }

    /// 2.3 — coordination skill explicitly forbids the coding agent from
    /// invoking `/opsx:verify` and `/opsx:archive`.
    #[test]
    fn coordination_skill_forbids_opsx_verify_and_archive() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("/opsx:verify"),
            "coordination skill should name `/opsx:verify` literally"
        );
        assert!(
            tmpl.content.contains("/opsx:archive"),
            "coordination skill should name `/opsx:archive` literally"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("off-limits")
                || lowered.contains("do not invoke")
                || lowered.contains("shall not")
                || lowered.contains("supervisor's job"),
            "coordination skill should state both are not the coding agent's responsibility"
        );
    }

    /// 2.4 — coordination skill names `agent.artifact` as the terminal action
    /// with status "done" or "committed".
    #[test]
    fn coordination_skill_names_terminal_action() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content.contains("agent.artifact"),
            "coordination skill should name `agent.artifact` as the terminal publish"
        );
        assert!(
            tmpl.content.contains("\"done\"") || tmpl.content.contains("\"committed\""),
            "coordination skill should reference status: \"done\" or \"committed\""
        );
    }

    /// 3.3 — supervisor skill teaches `pane_current_path` as the canonical
    /// pane→agent resolution mechanism.
    #[test]
    fn supervisor_skill_documents_pane_current_path_resolution() {
        let tmpl = resolve("supervisor").unwrap();
        assert!(
            tmpl.content.contains("tmux display-message"),
            "supervisor skill should show the tmux display-message command"
        );
        assert!(
            tmpl.content.contains("pane_current_path"),
            "supervisor skill should name pane_current_path literally"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("not alphabetical")
                || lowered.contains("not sorted alphabetically")
                || lowered.contains("are not alphabetical"),
            "supervisor skill should warn against alphabetical pane-index assumptions"
        );
        assert!(
            lowered.contains("cli-argument order")
                || lowered.contains("cli argument order")
                || lowered.contains("argument order"),
            "supervisor skill should warn against CLI-argument-order pane-index assumptions"
        );
    }

    // prompt-submit-fix coverage: ensure the supervisor skill's launch-time
    // pane sweep section continues to teach the three timing/escalation/
    // cross-reference contracts that the prompt-submit-fix change locked in.

    #[test]
    fn supervisor_skill_documents_proactive_launch_sweep() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("launch-time pane sweep")
            .or_else(|| lowered.find("launch sweep"))
            .expect("launch-time pane sweep heading should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];
        assert!(
            window.contains("immediately after attaching")
                || window.contains("before the poll thread")
                || window.contains("first-few-seconds")
                || window.contains("first few seconds"),
            "launch sweep should link the sweep to the first-few-seconds-after-attach window",
        );
    }

    #[test]
    fn supervisor_skill_launch_sweep_escalates_unknown_via_agent_question() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("launch-time pane sweep")
            .or_else(|| lowered.find("launch sweep"))
            .expect("launch-time pane sweep heading should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];
        assert!(
            window.contains("unknown") || window.contains("wider scope"),
            "launch sweep should classify a third category for unknown/wider-scope prompts",
        );
        assert!(
            window.contains("agent.question"),
            "launch sweep should instruct agent.question escalation for unknown prompts",
        );
        assert!(
            window.contains("escalate"),
            "launch sweep should use the word 'escalate' alongside the agent.question instruction",
        );
    }

    #[test]
    fn supervisor_skill_launch_sweep_complements_auto_approve_thread() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("launch-time pane sweep")
            .or_else(|| lowered.find("launch sweep"))
            .expect("launch-time pane sweep heading should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];
        assert!(
            window.contains("complements"),
            "launch sweep should describe itself as complementing the auto-approve thread",
        );
        assert!(
            window.contains("does not replace")
                || window.contains("not replace")
                || window.contains("does **not** replace"),
            "launch sweep should explicitly say it does NOT replace the auto-approve thread",
        );
        assert!(
            window.contains("[supervisor.auto_approve]") || window.contains("auto_approve"),
            "launch sweep should cross-reference the [supervisor.auto_approve] poll thread",
        );
    }

    // coordination-skill-followups: when the supervisor sends an
    // `agent.feedback` answer to a peer's `agent.question`, it must
    // dual-write via `tmux send-keys` AND cross-reference the
    // paste-buffer recovery sub-case for long answers. The test below
    // asserts that cross-reference is present in the send-keys section.
    // v0-5-0-audit-cleanup task 8.1.

    #[test]
    fn supervisor_skill_paste_buffer_cross_ref_in_send_keys_section() {
        let tmpl = resolve("supervisor").unwrap();
        let lowered = tmpl.content.to_lowercase();
        // Anchor on the "send the answer to the agent pane too" heading
        // — that's the section drift-34 owns. Fall back to a substring
        // unique to the section if the heading wording shifts.
        let start = lowered
            .find("send the answer to the agent pane")
            .or_else(|| lowered.find("agents do not poll their inbox"))
            .expect("send-keys-alongside-agent.feedback section should be present");
        let window_end = (start + 2200).min(lowered.len());
        let window = &lowered[start..window_end];

        assert!(
            window.contains("paste-buffer")
                || window.contains("paste buffer")
                || window.contains("follow-up enter")
                || window.contains("follow-up `enter`"),
            "send-keys-alongside-feedback section must cross-reference paste-buffer recovery / follow-up Enter for long answers",
        );
    }

    // coordination-skill-followups-2: the `pane_current_path` resolution
    // section must contain a warning against using `git paw status`
    // output order as a pane→agent mapping source. The dashboard and
    // status output are alphabetically sorted by the broker and have no
    // relationship to the launcher's pane assignment.
    // v0-5-0-audit-cleanup task 8.2.

    #[test]
    fn supervisor_skill_warns_against_git_paw_status_ordering() {
        let tmpl = resolve("supervisor").unwrap();
        // Case-sensitive search first for the literal `git paw status`
        // substring, then case-insensitive for the surrounding warning.
        assert!(
            tmpl.content.contains("git paw status"),
            "supervisor skill should reference `git paw status` by name when warning against using its ordering as a mapping source",
        );

        let lowered = tmpl.content.to_lowercase();
        let start = lowered
            .find("pane_current_path")
            .expect("pane_current_path resolution section should be present");
        let window_end = (start + 2500).min(lowered.len());
        let window = &lowered[start..window_end];

        assert!(
            window.contains("git paw status"),
            "the warning against `git paw status` ordering must appear within the pane_current_path resolution section",
        );
        assert!(
            window.contains("shall not be inferred")
                || window.contains("must not")
                || window.contains("not be inferred")
                || window.contains("not used as a mapping")
                || window.contains("no relationship"),
            "section must forbid using `git paw status` order as a mapping source",
        );
    }

    // === coordination-context-budget: context-budget skill content ===

    /// Spec "Context budget section in coordination skill" /
    /// "Section placement after 'While you're editing'": the coordination
    /// skill SHALL contain a "Context budget" heading and it SHALL appear
    /// after the v0.5.0 "While you're editing" heading.
    #[test]
    fn coordination_skill_contains_context_budget_after_while_editing() {
        let tmpl = resolve("coordination").unwrap();
        let editing = tmpl
            .content
            .find("While you're editing")
            .expect("coordination skill should contain 'While you're editing' heading");
        let budget = tmpl
            .content
            .find("### Context budget")
            .expect("coordination skill should contain a 'Context budget' heading");
        assert!(
            budget > editing,
            "the 'Context budget' section must appear after the 'While you're editing' section"
        );
    }

    /// Spec "Context budget section in coordination skill" /
    /// "Section exists with the three topics": the section covers the
    /// residual-budget heuristic, the named moments, and the
    /// commit-before-compact discipline.
    #[test]
    fn coordination_skill_context_budget_covers_three_topics() {
        let tmpl = resolve("coordination").unwrap();
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("residual-budget heuristic"),
            "context-budget section should cover the residual-budget heuristic"
        );
        assert!(
            lowered.contains("when to compact, clear, or summarise"),
            "context-budget section should cover the named compact/clear/summarise moments"
        );
        assert!(
            lowered.contains("commit before you compact"),
            "context-budget section should cover the commit-before-compact discipline"
        );
    }

    /// Spec "Residual-budget heuristic" / "Heuristic stated in prose": the
    /// "at least 60% free post-boot" target is phrased as prose, and no new
    /// config field is introduced in the section.
    #[test]
    fn coordination_skill_residual_budget_heuristic_in_prose() {
        let tmpl = resolve("coordination").unwrap();
        let start = tmpl
            .content
            .find("### Context budget")
            .expect("context-budget section present");
        let end = tmpl.content[start..]
            .find("### Check for messages")
            .map_or(tmpl.content.len(), |o| start + o);
        let section = &tmpl.content[start..end];
        let lowered = section.to_lowercase();
        assert!(
            lowered.contains("60%") && lowered.contains("free"),
            "residual-budget heuristic should reference keeping ~60% of the window free"
        );
        assert!(
            lowered.contains("heuristic"),
            "residual-budget guidance should be framed as a heuristic"
        );
        assert!(
            lowered.contains("no config field")
                || lowered.contains("there is no\nconfig field")
                || lowered.contains("there is no config field"),
            "the section should state there is no config field for the ratio"
        );
    }

    /// Spec "Three named moments to compact / clear / summarise" /
    /// "Three moments documented in priority order": the three moments
    /// appear in the documented order, each with its action labelled.
    #[test]
    fn coordination_skill_three_moments_in_priority_order() {
        let tmpl = resolve("coordination").unwrap();
        let content = &tmpl.content;
        let scenario = content
            .find("After each spec scenario completes")
            .expect("first moment present");
        let working_set = content
            .find("working set grows past")
            .expect("second moment present");
        let switching = content
            .find("switching between sub-tasks")
            .expect("third moment present");
        assert!(
            scenario < working_set && working_set < switching,
            "the three named moments must appear in the documented priority order"
        );

        // Each moment labels its associated action (compact for 1 & 2,
        // clear for 3). Check the action label sits near its moment.
        let first = &content[scenario..working_set];
        let second = &content[working_set..switching];
        let third = &content[switching..(switching + 300).min(content.len())];
        assert!(
            first.to_lowercase().contains("compact"),
            "moment 1 should be labelled with the compact action"
        );
        assert!(
            second.to_lowercase().contains("compact"),
            "moment 2 should be labelled with the compact action"
        );
        assert!(
            third.to_lowercase().contains("clear"),
            "moment 3 should be labelled with the clear action"
        );
    }

    /// Spec "Commit-before-compact discipline" /
    /// "Discipline stated explicitly with safety rationale": the rule is a
    /// clearly-marked statement paired with a rationale about why ordering
    /// matters.
    #[test]
    fn coordination_skill_states_commit_before_compact_discipline() {
        let tmpl = resolve("coordination").unwrap();
        assert!(
            tmpl.content
                .contains("**Never compact, clear, or summarise without first committing"),
            "commit-before-compact discipline should be a bold, explicit statement"
        );
        let lowered = tmpl.content.to_lowercase();
        assert!(
            lowered.contains("agent.artifact"),
            "the discipline should mention publishing an agent.artifact as the alternative to committing"
        );
        assert!(
            lowered.contains("can't recover") || lowered.contains("cannot recover"),
            "the discipline should pair the rule with a safety rationale about recoverability"
        );
    }

    /// Spec "Per-CLI compact mechanism table" /
    /// "Table includes claude and claude-oss explicitly" + "Generic 'other'
    /// row points users at their CLI's equivalent".
    #[test]
    fn coordination_skill_per_cli_mechanism_table() {
        let tmpl = resolve("coordination").unwrap();
        let start = tmpl
            .content
            .find("#### Per-CLI mechanism")
            .expect("per-CLI mechanism subsection present");
        let section = &tmpl.content[start..];
        // claude and claude-oss rows, each naming /compact and /clear.
        assert!(
            section.contains("| `claude` | `/compact` | `/clear` |"),
            "table should contain a claude row naming /compact and /clear"
        );
        assert!(
            section.contains("| `claude-oss` | `/compact` | `/clear` |"),
            "table should contain a claude-oss row naming /compact and /clear"
        );
        // Generic "other" fallback row directing to the CLI's equivalent.
        let other = section
            .find("| other |")
            .map(|o| &section[o..(o + 200).min(section.len())])
            .expect("table should contain an 'other' fallback row");
        assert!(
            other.contains("/compact") && other.contains("/save") && other.contains("/reset"),
            "the 'other' row should point users at the CLI's /compact, /save, or /reset equivalent"
        );
    }

    // --- opsx role-gating skill sections (opsx-role-gating 2.3, 7.3, 1a.4) ---

    use crate::specs::SpecBackendKind;

    fn render_skill(name: &str, backends: &[SpecBackendKind]) -> String {
        let tmpl = resolve(name).unwrap_or_else(|_| panic!("resolve {name}"));
        render(
            &tmpl,
            if name == "supervisor" {
                "supervisor"
            } else {
                "feat/x"
            },
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            backends,
        )
    }

    #[test]
    fn coordination_lists_forbidden_commands_under_openspec() {
        let out = render_skill("coordination", &[SpecBackendKind::OpenSpec]);
        assert!(
            out.contains("Commands you must not run"),
            "coordination must carry the forbidden-command section"
        );
        assert!(out.contains("/opsx:verify"), "lists /opsx:verify");
        assert!(out.contains("/opsx:archive"), "lists /opsx:archive");
        assert!(
            out.contains("supervisor-only"),
            "names the commands supervisor-only"
        );
        assert!(
            out.contains("role-gating guard"),
            "references the role-gating guard"
        );
    }

    #[test]
    fn supervisor_has_must_must_not_section_under_openspec() {
        let out = render_skill("supervisor", &[SpecBackendKind::OpenSpec]);
        assert!(
            out.contains("Commands you must run (not coding agents)"),
            "supervisor must carry the supervisor-only section"
        );
        assert!(out.contains("/opsx:verify") && out.contains("/opsx:archive"));
        // MUST / MUST NOT framing.
        assert!(out.contains("MUST") && out.contains("MUST NOT"));
        // Instruction to call out violations via agent.feedback.
        let idx = out
            .find("Commands you must run (not coding agents)")
            .expect("section present");
        let section = &out[idx..];
        assert!(
            section.contains("agent.feedback"),
            "section instructs calling out violations via agent.feedback"
        );
    }

    #[test]
    fn supervisor_has_revert_flow_under_openspec() {
        let out = render_skill("supervisor", &[SpecBackendKind::OpenSpec]);
        assert!(
            out.contains("Handling an opsx-role-gating revert request"),
            "merge-orchestration carries the revert-request flow"
        );
        assert!(out.contains("git revert"), "teaches git revert");
        assert!(
            out.contains("auto_revert"),
            "references the [supervisor] auto_revert opt-out"
        );
    }

    #[test]
    fn opsx_sections_omitted_under_non_openspec_engines() {
        for backends in [
            vec![SpecBackendKind::Markdown],
            vec![SpecBackendKind::SpecKit],
            vec![],
        ] {
            let coord = render_skill("coordination", &backends);
            assert!(
                !coord.contains("Commands you must not run"),
                "coordination forbidden section must be omitted for {backends:?}"
            );
            let sup = render_skill("supervisor", &backends);
            assert!(
                !sup.contains("Commands you must run (not coding agents)"),
                "supervisor-only section must be omitted for {backends:?}"
            );
            assert!(
                !sup.contains("Handling an opsx-role-gating revert request"),
                "revert flow must be omitted for {backends:?}"
            );
        }
    }

    #[test]
    fn opsx_region_markers_never_survive_rendering() {
        for name in ["coordination", "supervisor"] {
            for backends in [
                vec![SpecBackendKind::OpenSpec],
                vec![SpecBackendKind::Markdown],
                vec![],
            ] {
                let out = render_skill(name, &backends);
                assert!(
                    !out.contains(OPSX_REGION_BEGIN) && !out.contains(OPSX_REGION_END),
                    "{name} under {backends:?} must not leak region markers"
                );
            }
        }
    }

    #[test]
    fn opsx_multi_backend_session_keeps_sections_when_openspec_present() {
        // A session spanning OpenSpec + another engine still renders the
        // sections (OpenSpec is present).
        let out = render_skill(
            "supervisor",
            &[SpecBackendKind::Markdown, SpecBackendKind::OpenSpec],
        );
        assert!(out.contains("Commands you must run (not coding agents)"));
    }

    #[test]
    fn render_opsx_regions_strips_body_when_not_kept() {
        let input = "before\n<!-- opsx-role-gating:begin -->\nSECRET\n<!-- opsx-role-gating:end -->\nafter\n";
        let kept = render_opsx_regions(input, true);
        assert!(kept.contains("SECRET"));
        assert!(!kept.contains("opsx-role-gating:begin"));
        let stripped = render_opsx_regions(input, false);
        assert!(!stripped.contains("SECRET"));
        assert!(stripped.contains("before") && stripped.contains("after"));
    }

    #[test]
    fn raw_coordination_template_carries_the_forbidden_section() {
        // The bundled template (pre-render) contains the section; rendering is
        // what gates it per engine. Satisfies the spec's "bundled coordination.md
        // is inspected" scenario.
        let tmpl = resolve("coordination").unwrap();
        assert!(tmpl.content.contains("Commands you must not run"));
        assert!(tmpl.content.contains(OPSX_REGION_BEGIN));
    }
}
