//! Bundled per-backend task-prompt templates and the single exhaustive
//! [`SpecBackendKind`] selector over them.
//!
//! A spec format's task prompt is a pair of bundled assets: the **prose**
//! template every backend has, and an optional **entry-point** template — one
//! command line the launch submits as its own composer message once the
//! agent's pane goes idle (`boot-block`, "A declared workflow entry point is
//! submitted as its own composer line").
//!
//! The selector lives here, in one place, because three consumers need the
//! same answer and a second hand-maintained match would drift: the launch
//! flow's `build_task_prompt`, which renders both templates; the doctor's
//! spec-workflow check, which probes whether the workflow command a format's
//! prompt names is reachable from a worktree; and the compiler, which rejects
//! a new [`SpecBackendKind`] variant until it is given both templates here.
//!
//! No workflow command is written in Rust. The command text exists only in
//! the backend's bundled entry-point template, so a format's apply invocation
//! can change without touching compiled code — and a format that has no
//! apply workflow simply declares no template.

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

const TASK_PROMPT_NO_SPEC: &str = include_str!("../assets/boot-task-prompts/no-spec.md");
const TASK_PROMPT_OPENSPEC: &str = include_str!("../assets/boot-task-prompts/openspec.md");
const TASK_PROMPT_OPENSPEC_ENTRY: &str =
    include_str!("../assets/boot-task-prompts/openspec.entry.md");
const TASK_PROMPT_MARKDOWN: &str = include_str!("../assets/boot-task-prompts/markdown.md");
const TASK_PROMPT_SPECKIT: &str = include_str!("../assets/boot-task-prompts/speckit.md");
const TASK_PROMPT_SUPERPOWERS: &str = include_str!("../assets/boot-task-prompts/superpowers.md");

/// The templates for `backend`, or — for `None` — the no-spec fallback used
/// when no spec is associated with the agent's branch.
///
/// The `Some` arm is the one exhaustive match over [`SpecBackendKind`] in the
/// codebase for task-prompt purposes: adding a variant fails to compile here
/// until both of its templates are named.
#[must_use]
pub fn templates_for(backend: Option<SpecBackendKind>) -> TaskPromptTemplates {
    let Some(backend) = backend else {
        return TaskPromptTemplates {
            prose: TASK_PROMPT_NO_SPEC,
            entry_point: None,
        };
    };
    match backend {
        SpecBackendKind::OpenSpec => TaskPromptTemplates {
            prose: TASK_PROMPT_OPENSPEC,
            entry_point: Some(TASK_PROMPT_OPENSPEC_ENTRY),
        },
        SpecBackendKind::Markdown => TaskPromptTemplates {
            prose: TASK_PROMPT_MARKDOWN,
            entry_point: None,
        },
        SpecBackendKind::SpecKit => TaskPromptTemplates {
            prose: TASK_PROMPT_SPECKIT,
            entry_point: None,
        },
        SpecBackendKind::Superpowers => TaskPromptTemplates {
            prose: TASK_PROMPT_SUPERPOWERS,
            entry_point: None,
        },
    }
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
