//! Agent profiles: `[profiles.<name>]`.
//!
//! A profile states what an agent is for — a CLI pinned to its launch
//! arguments, an optional role skill, an approval level and a one-line
//! purpose. A [`ProfileKind::Worker`] profile launches a normal coding agent;
//! a [`ProfileKind::Delegate`] profile runs headless and one-shot through
//! `git paw delegate`.
//!
//! Every entry is validated as it is deserialized, so a bad entry fails the
//! configuration load with an error naming `profiles.<name>` and the key.
//! Whether `cli` and `skill` resolve is checked on use, not at load.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::ApprovalLevel;

/// Default delegate run time limit, in seconds.
pub const DEFAULT_DELEGATE_TIMEOUT_SECONDS: u64 = 600;

/// Skill names a profile may never use as its role: the orchestrator package
/// never reaches a worktree, and a headless orchestrator is the escalation
/// one-shot's job.
const REFUSED_ROLE_SKILLS: [&str; 2] = ["orchestrator", "supervisor"];

/// A validated profile name: `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`.
///
/// Profile names reach log paths and pane titles, so they are validated once
/// here and can never carry a path separator or shell metacharacter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ProfileName(String);

impl ProfileName {
    /// Validates `name` as a profile name.
    ///
    /// # Errors
    ///
    /// Returns a message naming the value when it does not match
    /// `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`.
    pub fn new(name: &str) -> Result<Self, String> {
        let mut chars = name.chars();
        let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
        let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if first_ok && rest_ok && name.len() <= 64 {
            Ok(Self(name.to_string()))
        } else {
            Err(format!(
                "invalid profile name '{name}' (expected [A-Za-z0-9][A-Za-z0-9._-]{{0,63}})"
            ))
        }
    }

    /// The name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a profile launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileKind {
    /// A normal coding agent: pane, branch, worktree.
    Worker,
    /// A headless one-shot agent run at the repository root for the
    /// orchestrator.
    Delegate,
}

impl ProfileKind {
    /// The lowercase wire name (`"worker"` or `"delegate"`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Delegate => "delegate",
        }
    }
}

/// One `[profiles.<name>]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileEntry {
    /// Worker or delegate.
    pub kind: ProfileKind,
    /// One-line purpose, shown in the picker and `git paw delegate --list`.
    pub description: String,
    /// A detected CLI id or a `[clis.<name>]` key.
    pub cli: String,
    /// Launch arguments appended after the CLI's own `args`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Role skill name, resolved through the skill resolver on use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    /// Approval level; unset means the worker default (workers) or `manual`
    /// (delegates).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalLevel>,
    /// Delegate run time limit in seconds (delegates only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

impl ProfileEntry {
    /// The level this profile runs at: its `approval` when set, otherwise
    /// `worker_default` for a worker and `manual` for a delegate.
    #[must_use]
    pub fn effective_approval(&self, worker_default: ApprovalLevel) -> ApprovalLevel {
        self.approval.unwrap_or(match self.kind {
            ProfileKind::Worker => worker_default,
            ProfileKind::Delegate => ApprovalLevel::Manual,
        })
    }

    /// The delegate run time limit: `timeout_seconds`, else 600 seconds.
    #[must_use]
    pub fn effective_timeout_seconds(&self) -> u64 {
        self.timeout_seconds
            .unwrap_or(DEFAULT_DELEGATE_TIMEOUT_SECONDS)
    }

    /// The role skill a worker profile adds after the coordination skill:
    /// `None` when `skill` is unset or names `coordination`.
    #[must_use]
    pub fn worker_role_skill(&self) -> Option<&str> {
        self.skill.as_deref().filter(|s| *s != "coordination")
    }

    /// The profile's extra launch arguments, split on whitespace.
    #[must_use]
    pub fn arg_parts(&self) -> Vec<String> {
        self.args
            .as_deref()
            .map(|a| a.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

/// The on-disk shape of an entry before validation.
#[derive(Deserialize)]
struct RawProfileEntry {
    kind: Option<String>,
    description: Option<String>,
    cli: Option<String>,
    args: Option<String>,
    skill: Option<String>,
    approval: Option<String>,
    timeout_seconds: Option<u64>,
}

impl RawProfileEntry {
    /// Validates the raw entry, returning an error naming `profiles.<name>`
    /// and the offending key.
    fn validate(self, name: &str) -> Result<ProfileEntry, String> {
        let at = |key: &str, why: &str| format!("profiles.{name}: {key} {why}");
        let kind = match self.kind.as_deref() {
            Some("worker") => ProfileKind::Worker,
            Some("delegate") => ProfileKind::Delegate,
            Some(other) => {
                return Err(at(
                    "kind",
                    &format!("has unknown value '{other}' (expected one of: worker, delegate)"),
                ));
            }
            None => {
                return Err(at("kind", "is missing (expected one of: worker, delegate)"));
            }
        };
        let description = match self.description {
            Some(d) if d.trim().is_empty() => return Err(at("description", "is empty")),
            Some(d) if d.contains('\n') || d.contains('\r') => {
                return Err(at("description", "must be a single line"));
            }
            Some(d) => d,
            None => return Err(at("description", "is missing")),
        };
        let cli = match self.cli {
            Some(c) if !c.trim().is_empty() => c,
            _ => return Err(at("cli", "is missing")),
        };
        let approval = match self.approval.as_deref() {
            None => None,
            Some("manual") => Some(ApprovalLevel::Manual),
            Some("auto") => Some(ApprovalLevel::Auto),
            Some("full-auto") => Some(ApprovalLevel::FullAuto),
            Some(other) => {
                return Err(at(
                    "approval",
                    &format!(
                        "has unknown value '{other}' (expected one of: manual, auto, full-auto)"
                    ),
                ));
            }
        };
        if let Some(skill) = self.skill.as_deref()
            && REFUSED_ROLE_SKILLS.contains(&skill)
        {
            return Err(at(
                "skill",
                &format!("cannot be '{skill}': the orchestrator skill is not a profile role"),
            ));
        }
        match kind {
            ProfileKind::Delegate => {
                if self.skill.as_deref().is_none_or(|s| s.trim().is_empty()) {
                    return Err(at("skill", "is required on a delegate profile"));
                }
                if approval == Some(ApprovalLevel::FullAuto) {
                    return Err(at(
                        "approval",
                        "cannot be \"full-auto\" on a delegate profile (use \"manual\" or \"auto\")",
                    ));
                }
                if self.timeout_seconds == Some(0) {
                    return Err(at("timeout_seconds", "must be greater than 0"));
                }
            }
            ProfileKind::Worker => {
                if self.timeout_seconds.is_some() {
                    return Err(at(
                        "timeout_seconds",
                        "applies to delegate profiles only; remove it from this worker profile",
                    ));
                }
            }
        }
        Ok(ProfileEntry {
            kind,
            description,
            cli,
            args: self.args,
            skill: self.skill,
            approval,
            timeout_seconds: self.timeout_seconds,
        })
    }
}

/// Deserializes and validates the `[profiles]` table.
pub(crate) fn deserialize_profiles<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<ProfileName, ProfileEntry>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = BTreeMap::<String, RawProfileEntry>::deserialize(deserializer)?;
    let mut out = BTreeMap::new();
    for (name, entry) in raw {
        let key = ProfileName::new(&name).map_err(serde::de::Error::custom)?;
        let entry = entry.validate(&name).map_err(serde::de::Error::custom)?;
        out.insert(key, entry);
    }
    Ok(out)
}

/// Deserializes and validates `default_profile`.
pub(crate) fn deserialize_default_profile<'de, D>(
    deserializer: D,
) -> Result<Option<ProfileName>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(|n| {
            ProfileName::new(&n)
                .map_err(|e| serde::de::Error::custom(format!("default_profile: {e}")))
        })
        .transpose()
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
