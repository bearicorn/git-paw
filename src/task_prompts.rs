//! Task-prompt templates: the shape every spec backend's templates share,
//! their rendering, and the no-spec fallback.
//!
//! A spec format's task prompt is a pair of bundled assets: the **prose**
//! template every backend has, and an optional **entry-point** template — one
//! command line the launch submits as its own composer message once the
//! agent's pane goes idle (`boot-block`, "A declared workflow entry point is
//! submitted as its own composer line").
//!
//! Each format's templates are declared by its
//! [`SpecBackend`](crate::specs::SpecBackend). Two consumers read them through
//! [`templates_for`] so they cannot disagree: the launch flow's
//! `build_task_prompt`, which renders both templates, and the doctor's
//! spec-workflow check, which probes whether the workflow command a format's
//! prompt names is reachable from a worktree.
//!
//! No workflow command is written in Rust. The command text exists only in
//! the backend's bundled entry-point template, so a format's apply invocation
//! can change without touching compiled code — and a format that has no
//! apply workflow simply declares no template.

use crate::agents::SIDECAR_REL_PATH;
use crate::specs::SpecBackendKind;

/// The prose template every backend's task prompt carries, and the optional
/// entry-point template it declares. Both still hold their placeholders
/// (`{{SIDECAR_REL_PATH}}`, `{{SPEC_ID}}`); substitution is the caller's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskPromptTemplates {
    /// The prose naming the agent's assignment and pointing at the sidecar.
    pub prose: &'static str,
    /// The single command line to submit after the prose, when the backend
    /// declares a workflow entry point.
    pub entry_point: Option<&'static str>,
}

impl TaskPromptTemplates {
    /// Fills `{{SIDECAR_REL_PATH}}` and `{{SPEC_ID}}` into both templates.
    #[must_use]
    pub fn render(self, spec_id: &str) -> TaskPrompt {
        let fill = |template: &str| {
            template
                .replace("{{SIDECAR_REL_PATH}}", SIDECAR_REL_PATH)
                .replace("{{SPEC_ID}}", spec_id)
        };
        TaskPrompt {
            prose: fill(self.prose),
            entry_point: self.entry_point.map(fill),
        }
    }
}

/// An agent's rendered task prompt: the prose its first message carries, and
/// — when the spec format declares one — the workflow entry point submitted
/// as its own composer line once the pane goes idle.
///
/// The entry point is deliberately NOT part of `prose`. A command embedded in
/// a prose paragraph is a command a weaker model may narrate instead of
/// invoke (GP-54); submitted alone it is what a paste-aware CLI parses as a
/// slash-command invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPrompt {
    /// The assignment prose appended to the boot block.
    pub prose: String,
    /// The single command line to submit after `prose`, when declared.
    pub entry_point: Option<String>,
}

const TASK_PROMPT_NO_SPEC: &str = include_str!("../assets/boot-task-prompts/no-spec.md");

/// The templates for `backend`, or — for `None` — the no-spec fallback used
/// when no spec is associated with the agent's branch.
#[must_use]
pub fn templates_for(backend: Option<SpecBackendKind>) -> TaskPromptTemplates {
    backend.map_or(
        TaskPromptTemplates {
            prose: TASK_PROMPT_NO_SPEC,
            entry_point: None,
        },
        |kind| kind.backend().task_prompt_templates(),
    )
}

/// The bare command word of `backend`'s declared entry point — the leading
/// token before its placeholders — or `None` when the backend declares none.
///
/// This is what the doctor's spec-workflow check probes for reachability: it
/// asks whether the *command* the launch will submit has a definition the
/// worktrees can see, which does not depend on the spec id substituted after
/// it. Reading it from the same template the launch submits is what keeps the
/// check honest — a format whose entry point changes cannot leave the doctor
/// probing a command nothing types.
#[must_use]
pub fn workflow_command(backend: SpecBackendKind) -> Option<&'static str> {
    templates_for(Some(backend))
        .entry_point
        .and_then(|template| template.split_whitespace().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openspec_declares_an_entry_point_and_keeps_it_out_of_the_prose() {
        let templates = templates_for(Some(SpecBackendKind::OpenSpec));
        assert_eq!(templates.entry_point, Some("/opsx:apply {{SPEC_ID}}"));
        assert!(
            !templates.prose.contains("/opsx:apply"),
            "the OpenSpec prose must not embed the command the entry point submits; got: {}",
            templates.prose
        );
    }

    #[test]
    fn every_other_backend_declares_no_entry_point() {
        for backend in SpecBackendKind::ALL {
            if backend == SpecBackendKind::OpenSpec {
                continue;
            }
            assert_eq!(
                templates_for(Some(backend)).entry_point,
                None,
                "{backend:?} has no apply workflow and must declare no entry point"
            );
        }
        assert_eq!(templates_for(None).entry_point, None);
    }

    #[test]
    fn workflow_command_is_the_entry_points_leading_token() {
        assert_eq!(
            workflow_command(SpecBackendKind::OpenSpec),
            Some("/opsx:apply")
        );
        assert_eq!(workflow_command(SpecBackendKind::Markdown), None);
        assert_eq!(workflow_command(SpecBackendKind::SpecKit), None);
        assert_eq!(workflow_command(SpecBackendKind::Superpowers), None);
    }

    /// No template may arrive with surrounding whitespace: the entry point is
    /// typed literally into a composer, where a leading space or a trailing
    /// newline changes what the CLI parses.
    #[test]
    fn templates_carry_no_surrounding_whitespace() {
        for backend in std::iter::once(None).chain(SpecBackendKind::ALL.map(Some)) {
            let templates = templates_for(backend);
            assert_eq!(templates.prose, templates.prose.trim());
            if let Some(entry) = templates.entry_point {
                assert_eq!(entry, entry.trim());
            }
        }
    }
}
