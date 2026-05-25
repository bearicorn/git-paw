//! Spec Kit format backend.
//!
//! Spec Kit projects place each feature in its own directory under
//! `.specify/specs/`, containing `spec.md`, `plan.md`, `tasks.md`, and an
//! optional `checklists/` subdirectory. Unlike the `OpenSpec` and `Markdown`
//! backends (one input unit → one [`SpecEntry`]), Spec Kit decomposes a
//! feature's current phase into one entry per `[P]`-marked task plus one
//! consolidated entry for the non-`[P]` remainder.
//!
//! See `openspec/changes/spec-kit-format/` for the full specification.
//!
//! [`SpecBackend`]: crate::specs::SpecBackend
//! [`SpecEntry`]: crate::specs::SpecEntry

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::broker::messages::slugify_branch;
use crate::error::PawError;
use crate::specs::{SpecBackend, SpecBackendKind, SpecEntry};

/// Phase number assigned to tasks that appear before any `## Phase N` heading.
pub(crate) const IMPLICIT_PHASE_NUMBER: u32 = 0;

/// A single task line parsed from `tasks.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Identifier such as `T009`.
    pub id: String,
    /// `true` if the task line carried the `[P]` parallel marker.
    pub p_marker: bool,
    /// `true` if the task is checked (`- [x]` / `- [X]`).
    pub complete: bool,
    /// Description text following the marker.
    pub description: String,
    /// Phase number this task belongs to.
    pub phase: u32,
}

/// A phase within a feature's `tasks.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    /// Phase number (1-based; `0` for the implicit phase used when the file
    /// contains no `## Phase N` headings, or for tasks that appear before any
    /// heading).
    pub number: u32,
    /// Phase name extracted from the heading (empty string for the implicit
    /// phase).
    pub name: String,
    /// Tasks attached to this phase, in source order.
    pub tasks: Vec<Task>,
}

/// A Spec Kit feature directory and its parsed contents.
#[derive(Debug, Clone)]
pub struct Feature {
    /// Path to the feature directory.
    pub dir: PathBuf,
    /// Phases parsed from `tasks.md`.
    pub phases: Vec<Phase>,
    /// Contents of `spec.md`, if present.
    pub spec_md: Option<String>,
    /// Contents of `plan.md`, if present.
    pub plan_md: Option<String>,
    /// Checklist files as `(filename, content)` pairs, sorted by filename.
    pub checklists: Vec<(String, String)>,
}

/// Backend for the Spec Kit (`.specify/`) artefact format.
#[derive(Debug)]
pub struct SpecKitBackend;

impl SpecBackend for SpecKitBackend {
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError> {
        let read = fs::read_dir(dir).map_err(|e| {
            PawError::SpecError(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        let mut features: Vec<Feature> = Vec::new();
        for raw in read {
            let raw = raw
                .map_err(|e| PawError::SpecError(format!("error reading directory entry: {e}")))?;
            let path = raw.path();
            if !path.is_dir() {
                continue;
            }

            let Some(feature) = read_feature(&path)? else {
                continue;
            };

            features.push(feature);
        }

        // Stable ordering: by feature directory name.
        features.sort_by(|a, b| a.dir.file_name().cmp(&b.dir.file_name()));

        let mut entries: Vec<SpecEntry> = Vec::new();
        for feature in &features {
            entries.extend(decompose_feature(feature));
        }
        Ok(entries)
    }
}

/// Reads a Spec Kit feature directory into a [`Feature`].
///
/// Returns `Ok(None)` (with a stderr warning) if the directory has no
/// `tasks.md`. Errors only on filesystem failures.
pub(crate) fn read_feature(dir: &Path) -> Result<Option<Feature>, PawError> {
    let tasks_path = dir.join("tasks.md");
    if !tasks_path.exists() {
        eprintln!(
            "warning: skipping feature {}: no tasks.md found",
            dir.display()
        );
        return Ok(None);
    }

    let tasks_content = fs::read_to_string(&tasks_path)
        .map_err(|e| PawError::SpecError(format!("cannot read {}: {e}", tasks_path.display())))?;
    let phases = parse_tasks_md(&tasks_content);

    let spec_md = read_optional(&dir.join("spec.md"))?;
    let plan_md = read_optional(&dir.join("plan.md"))?;
    let checklists = read_checklists(&dir.join("checklists"))?;

    Ok(Some(Feature {
        dir: dir.to_path_buf(),
        phases,
        spec_md,
        plan_md,
        checklists,
    }))
}

/// Reads a file into a `String`, returning `Ok(None)` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>, PawError> {
    if !path.exists() {
        return Ok(None);
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|e| PawError::SpecError(format!("cannot read {}: {e}", path.display())))
}

/// Reads every regular file in `dir` as a `(filename, content)` pair, sorted
/// by filename. Returns an empty vector if `dir` is missing or not a directory.
fn read_checklists(dir: &Path) -> Result<Vec<(String, String)>, PawError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let read = fs::read_dir(dir)
        .map_err(|e| PawError::SpecError(format!("read dir {}: {e}", dir.display())))?;

    let mut items: Vec<(String, String)> = Vec::new();
    for raw in read {
        let raw = raw.map_err(|e| PawError::SpecError(format!("read entry: {e}")))?;
        let path = raw.path();
        if !path.is_file() {
            continue;
        }
        let name = raw.file_name().to_string_lossy().to_string();
        let content = fs::read_to_string(&path)
            .map_err(|e| PawError::SpecError(format!("cannot read {}: {e}", path.display())))?;
        items.push((name, content));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(items)
}

// --- tasks.md parser ---

fn phase_heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `## Phase <N> <separator> <Name>` where separator is `:`, `—`, or `-`.
        Regex::new(r"^##\s+Phase\s+(\d+)\s*[:\-\u{2014}]\s*(.+?)\s*$")
            .expect("phase heading regex must compile")
    })
}

fn incomplete_task_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^-\s+\[\s\]\s+(T\d+)(\s+\[P\])?\s+(.+?)\s*$")
            .expect("incomplete task regex must compile")
    })
}

fn complete_task_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^-\s+\[[xX]\]\s+(T\d+)(\s+\[P\])?\s+(.+?)\s*$")
            .expect("complete task regex must compile")
    })
}

/// Parses Spec Kit `tasks.md` content into a list of [`Phase`] values.
///
/// Tasks attach to the most recently seen `## Phase N` heading. Tasks that
/// appear before any heading (or in a file with no headings at all) live in
/// the implicit phase numbered [`IMPLICIT_PHASE_NUMBER`].
pub(crate) fn parse_tasks_md(content: &str) -> Vec<Phase> {
    let mut phases: Vec<Phase> = Vec::new();
    let mut current_phase_idx: Option<usize> = None;

    let push_phase = |phases: &mut Vec<Phase>, number: u32, name: String| -> usize {
        phases.push(Phase {
            number,
            name,
            tasks: Vec::new(),
        });
        phases.len() - 1
    };

    let ensure_implicit_phase = |phases: &mut Vec<Phase>, current_idx: &mut Option<usize>| {
        if current_idx.is_none() {
            let idx = push_phase(phases, IMPLICIT_PHASE_NUMBER, String::new());
            *current_idx = Some(idx);
        }
    };

    for line in content.lines() {
        if let Some(caps) = phase_heading_re().captures(line) {
            let number: u32 = caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let name = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let idx = push_phase(&mut phases, number, name);
            current_phase_idx = Some(idx);
            continue;
        }

        if let Some(caps) = incomplete_task_re().captures(line) {
            ensure_implicit_phase(&mut phases, &mut current_phase_idx);
            let idx = current_phase_idx.expect("ensure_implicit_phase set Some");
            let phase_number = phases[idx].number;
            let task = Task {
                id: caps[1].to_string(),
                p_marker: caps.get(2).is_some(),
                complete: false,
                description: caps[3].to_string(),
                phase: phase_number,
            };
            phases[idx].tasks.push(task);
            continue;
        }

        if let Some(caps) = complete_task_re().captures(line) {
            ensure_implicit_phase(&mut phases, &mut current_phase_idx);
            let idx = current_phase_idx.expect("ensure_implicit_phase set Some");
            let phase_number = phases[idx].number;
            let task = Task {
                id: caps[1].to_string(),
                p_marker: caps.get(2).is_some(),
                complete: true,
                description: caps[3].to_string(),
                phase: phase_number,
            };
            phases[idx].tasks.push(task);
        }
        // Unrecognised lines are ignored — preserves intra-phase commentary.
    }

    phases
}

/// Returns the lowest-numbered phase with at least one incomplete task, or
/// `None` if every task is complete (or no tasks exist).
pub(crate) fn current_phase(phases: &[Phase]) -> Option<&Phase> {
    phases
        .iter()
        .filter(|p| p.tasks.iter().any(|t| !t.complete))
        .min_by_key(|p| p.number)
}

/// Kind of `SpecEntry` produced by Spec Kit decomposition.
pub(crate) enum EntryKind<'a> {
    /// A single `[P]` task — its own worktree.
    Single { task: &'a Task },
    /// All incomplete non-`[P]` tasks in the current phase — one worktree.
    Consolidated {
        tasks: Vec<&'a Task>,
        phase_number: u32,
        phase_name: &'a str,
    },
}

/// Decomposes a feature's current phase into the canonical worktree layout
/// (one entry per `[P]` task plus one consolidated entry for the non-`[P]`
/// remainder).
///
/// Returns an empty vector when the feature has no incomplete tasks. Emits a
/// stderr warning for fully completed features and parse-empty `tasks.md`
/// files.
pub(crate) fn decompose_feature(feature: &Feature) -> Vec<SpecEntry> {
    let feature_dir = feature
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let total_tasks: usize = feature.phases.iter().map(|p| p.tasks.len()).sum();
    let Some(phase) = current_phase(&feature.phases) else {
        if total_tasks > 0 {
            eprintln!(
                "warning: feature {} has no incomplete tasks — skipping",
                feature.dir.display()
            );
        }
        // Empty / parse-empty: skip silently per spec.
        return Vec::new();
    };

    let mut entries: Vec<SpecEntry> = Vec::new();

    // One entry per incomplete [P] task, in source order.
    for task in phase.tasks.iter().filter(|t| !t.complete && t.p_marker) {
        let id = format!("{feature_dir}-{}", task.id);
        let branch_input = format!("{}-{}", task.id, task.description);
        let branch = format!("task/{}", slugify_branch(&branch_input));
        let prompt = build_prompt(feature, &EntryKind::Single { task });
        entries.push(SpecEntry {
            id,
            backend: SpecBackendKind::SpecKit,
            branch,
            cli: None,
            prompt,
            owned_files: None,
        });
    }

    // One consolidated entry for the union of incomplete non-[P] tasks.
    let non_p: Vec<&Task> = phase
        .tasks
        .iter()
        .filter(|t| !t.complete && !t.p_marker)
        .collect();
    if !non_p.is_empty() {
        let id = format!("{feature_dir}-phase-{}", phase.number);
        let branch_input = format!("{feature_dir}-{}", phase.name);
        let branch = format!("phase/{}", slugify_branch(&branch_input));
        let kind = EntryKind::Consolidated {
            tasks: non_p,
            phase_number: phase.number,
            phase_name: &phase.name,
        };
        let prompt = build_prompt(feature, &kind);
        entries.push(SpecEntry {
            id,
            backend: SpecBackendKind::SpecKit,
            branch,
            cli: None,
            prompt,
            owned_files: None,
        });
    }

    entries
}

/// Boot-prompt delimiter between sections.
const SECTION_DELIM: &str = "\n\n---\n\n";

/// Builds the boot-prompt content for a Spec Kit `SpecEntry`.
///
/// Sections in order, separated by `\n\n---\n\n`:
///
/// 1. `## Feature Context` — full `spec.md` content (omitted if missing).
/// 2. `## Implementation Plan` — full `plan.md` content (omitted if missing).
/// 3. `## Validation Criteria (advisory)` — checklists content (omitted if
///    none present).
/// 4. `## Your Task` — single-task description, or ordered list + sequential
///    instructions for consolidated entries.
pub(crate) fn build_prompt(feature: &Feature, kind: &EntryKind<'_>) -> String {
    let mut sections: Vec<String> = Vec::new();

    if let Some(spec) = feature.spec_md.as_deref() {
        let trimmed = spec.trim();
        if !trimmed.is_empty() {
            sections.push(format!("## Feature Context\n\n{trimmed}"));
        }
    }

    if let Some(plan) = feature.plan_md.as_deref() {
        let trimmed = plan.trim();
        if !trimmed.is_empty() {
            sections.push(format!("## Implementation Plan\n\n{trimmed}"));
        }
    }

    if !feature.checklists.is_empty() {
        let mut section = String::from(
            "## Validation Criteria (advisory)\n\n\
             The following checklists are advisory context for this release \
             (full enforcement is planned for v1.0.0).",
        );
        for (name, content) in &feature.checklists {
            let _ = write!(section, "\n\n### {name}\n\n{}", content.trim());
        }
        sections.push(section);
    }

    sections.push(your_task_section(kind));

    sections.join(SECTION_DELIM)
}

fn your_task_section(kind: &EntryKind<'_>) -> String {
    let mut out = String::from("## Your Task\n\n");
    match kind {
        EntryKind::Single { task } => {
            let id = &task.id;
            let desc = &task.description;
            let _ = write!(out, "{id} — {desc}");
        }
        EntryKind::Consolidated {
            tasks,
            phase_number,
            phase_name,
        } => {
            let _ = writeln!(
                out,
                "Phase {phase_number} ({phase_name}). Complete the following tasks in order:"
            );
            for task in tasks {
                let id = &task.id;
                let desc = &task.description;
                let _ = write!(out, "\n- {id} — {desc}");
            }
            out.push_str(
                "\n\nWork through these tasks sequentially in the order listed. \
                 After completing each task, flip its `- [ ]` checkbox to \
                 `- [x]` in this worktree's `tasks.md`. You may commit the \
                 writeback alongside the task's code change or as a separate \
                 commit. Publish `agent.done` only when every task above \
                 shows `- [x]` in `tasks.md`.",
            );
        }
    }
    out
}

/// Returns the path to a Spec Kit project's `constitution.md` if one exists.
///
/// The probe examines `<specs_dir>/../memory/constitution.md` — the canonical
/// location relative to a `.specify/specs/` configuration. Returns `None`
/// when the file does not exist or `specs_dir` has no parent.
pub fn detect_constitution(specs_dir: &Path) -> Option<PathBuf> {
    let parent = specs_dir.parent()?;
    let candidate = parent.join("memory").join("constitution.md");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- backend / scan ---

    #[test]
    fn backend_constructs() {
        let backend = SpecKitBackend;
        let dbg = format!("{backend:?}");
        assert!(dbg.contains("SpecKitBackend"));
    }

    #[test]
    fn scan_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let backend = SpecKitBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_skips_non_directory_children() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("loose-file.md"), "hello").unwrap();
        let backend = SpecKitBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_skips_feature_without_tasks_md() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("001-no-tasks")).unwrap();
        let backend = SpecKitBackend;
        let result = backend.scan(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    // --- read_feature ---

    #[test]
    fn read_feature_loads_optional_files() {
        let tmp = tempfile::tempdir().unwrap();
        let feat = tmp.path().join("002-onboarding");
        fs::create_dir(&feat).unwrap();
        fs::write(
            feat.join("tasks.md"),
            "## Phase 1: Setup\n- [ ] T001 do thing\n",
        )
        .unwrap();
        fs::write(feat.join("spec.md"), "the spec").unwrap();
        fs::write(feat.join("plan.md"), "the plan").unwrap();
        fs::create_dir(feat.join("checklists")).unwrap();
        fs::write(feat.join("checklists/security.md"), "sec criteria").unwrap();
        fs::write(feat.join("checklists/perf.md"), "perf criteria").unwrap();

        let feature = read_feature(&feat).unwrap().expect("feature should load");
        assert_eq!(feature.dir, feat);
        assert_eq!(feature.spec_md.as_deref(), Some("the spec"));
        assert_eq!(feature.plan_md.as_deref(), Some("the plan"));
        assert_eq!(feature.checklists.len(), 2);
        assert_eq!(feature.checklists[0].0, "perf.md");
        assert_eq!(feature.checklists[1].0, "security.md");
        assert_eq!(feature.phases.len(), 1);
        assert_eq!(feature.phases[0].number, 1);
        assert_eq!(feature.phases[0].name, "Setup");
        assert_eq!(feature.phases[0].tasks.len(), 1);
    }

    #[test]
    fn read_feature_optional_files_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let feat = tmp.path().join("004-bare");
        fs::create_dir(&feat).unwrap();
        fs::write(feat.join("tasks.md"), "## Phase 1: Setup\n").unwrap();

        let feature = read_feature(&feat).unwrap().expect("feature should load");
        assert!(feature.spec_md.is_none());
        assert!(feature.plan_md.is_none());
        assert!(feature.checklists.is_empty());
    }

    #[test]
    fn read_feature_returns_none_when_tasks_md_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let feat = tmp.path().join("005-empty");
        fs::create_dir(&feat).unwrap();

        let result = read_feature(&feat).unwrap();
        assert!(result.is_none());
    }

    // --- parse_tasks_md ---

    #[test]
    fn parses_standard_task_line() {
        let phases = parse_tasks_md("## Phase 1: Setup\n- [ ] T001 Create project structure\n");
        assert_eq!(phases.len(), 1);
        let t = &phases[0].tasks[0];
        assert_eq!(t.id, "T001");
        assert!(!t.p_marker);
        assert!(!t.complete);
        assert_eq!(t.description, "Create project structure");
    }

    #[test]
    fn parses_p_marker() {
        let phases = parse_tasks_md(
            "## Phase 2: Build\n- [ ] T009 [P] Contract test POST /api/v1/auth/otp/request\n",
        );
        let t = &phases[0].tasks[0];
        assert_eq!(t.id, "T009");
        assert!(t.p_marker);
        assert_eq!(t.description, "Contract test POST /api/v1/auth/otp/request");
    }

    #[test]
    fn parses_complete_task_lowercase_and_uppercase_x() {
        let phases = parse_tasks_md("## Phase 1: Setup\n- [x] T001 lower\n- [X] T002 upper\n");
        assert_eq!(phases[0].tasks.len(), 2);
        assert!(phases[0].tasks[0].complete);
        assert!(phases[0].tasks[1].complete);
    }

    #[test]
    fn parses_phase_heading_separator_variants() {
        let phases = parse_tasks_md(
            "## Phase 1: Setup\n\
             - [ ] T001 a\n\
             ## Phase 2 — Foundational\n\
             - [ ] T002 b\n\
             ## Phase 3 - User Story 1\n\
             - [ ] T003 c\n",
        );
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0].number, 1);
        assert_eq!(phases[0].name, "Setup");
        assert_eq!(phases[1].number, 2);
        assert_eq!(phases[1].name, "Foundational");
        assert_eq!(phases[2].number, 3);
        assert_eq!(phases[2].name, "User Story 1");
    }

    #[test]
    fn tasks_attach_to_preceding_phase() {
        let phases = parse_tasks_md(
            "## Phase 1: Setup\n\
             - [ ] T001 a\n\
             - [ ] T002 b\n\
             ## Phase 2: Foundational\n\
             - [ ] T003 c\n\
             - [ ] T004 d\n\
             - [ ] T005 e\n",
        );
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].tasks.len(), 2);
        assert_eq!(phases[1].tasks.len(), 3);
    }

    #[test]
    fn unrecognised_lines_are_ignored() {
        let phases = parse_tasks_md(
            "## Phase 1: Setup\n\
             Some prose paragraph.\n\
             - [ ] T001 real task\n\
             Another commentary line.\n\
             > a quote\n",
        );
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].tasks.len(), 1);
    }

    #[test]
    fn phase_less_file_uses_implicit_phase() {
        let phases = parse_tasks_md("- [ ] T001 first\n- [ ] T002 [P] second\n");
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].number, IMPLICIT_PHASE_NUMBER);
        assert!(phases[0].name.is_empty());
        assert_eq!(phases[0].tasks.len(), 2);
    }

    #[test]
    fn duplicate_task_ids_are_kept_as_separate_records() {
        // The parser does not deduplicate — that is a Spec-Kit-author concern.
        let phases = parse_tasks_md("## Phase 1: Setup\n- [ ] T001 first\n- [ ] T001 dup\n");
        assert_eq!(phases[0].tasks.len(), 2);
    }

    // --- current_phase ---

    #[test]
    fn current_phase_skips_fully_complete_phases() {
        let phases = parse_tasks_md(
            "## Phase 1: Setup\n\
             - [x] T001 done\n\
             ## Phase 2: Build\n\
             - [x] T002 done\n\
             - [ ] T003 todo\n\
             ## Phase 3: Polish\n\
             - [ ] T004 future\n",
        );
        let cp = current_phase(&phases).unwrap();
        assert_eq!(cp.number, 2);
    }

    #[test]
    fn current_phase_returns_none_when_all_complete() {
        let phases = parse_tasks_md(
            "## Phase 1: Setup\n- [x] T001 done\n## Phase 2: Build\n- [x] T002 done\n",
        );
        assert!(current_phase(&phases).is_none());
    }

    #[test]
    fn current_phase_handles_implicit_phase() {
        let phases = parse_tasks_md("- [ ] T001 only\n");
        let cp = current_phase(&phases).unwrap();
        assert_eq!(cp.number, IMPLICIT_PHASE_NUMBER);
    }

    // --- decompose_feature ---

    fn feature_fixture(dir_name: &str, tasks_md: &str) -> Feature {
        Feature {
            dir: PathBuf::from(dir_name),
            phases: parse_tasks_md(tasks_md),
            spec_md: Some("SPEC".to_string()),
            plan_md: Some("PLAN".to_string()),
            checklists: vec![],
        }
    }

    #[test]
    fn decompose_mixed_phase_produces_n_plus_one() {
        let feat = feature_fixture(
            "003-user-list",
            "## Phase 2: Build\n\
             - [ ] T009 [P] do A\n\
             - [ ] T010 [P] do B\n\
             - [ ] T011 do C\n\
             - [ ] T012 do D\n\
             - [ ] T013 do E\n",
        );
        let entries = decompose_feature(&feat);
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.id == "003-user-list-T009"));
        assert!(entries.iter().any(|e| e.id == "003-user-list-T010"));
        assert!(entries.iter().any(|e| e.id == "003-user-list-phase-2"));
    }

    #[test]
    fn decompose_only_p_tasks_no_consolidated() {
        let feat = feature_fixture(
            "002-foo",
            "## Phase 1: Setup\n\
             - [ ] T001 [P] one\n\
             - [ ] T002 [P] two\n\
             - [ ] T003 [P] three\n\
             - [ ] T004 [P] four\n",
        );
        let entries = decompose_feature(&feat);
        assert_eq!(entries.len(), 4);
        assert!(entries.iter().all(|e| e.branch.starts_with("task/")));
    }

    #[test]
    fn decompose_only_non_p_one_consolidated_entry() {
        let feat = feature_fixture(
            "002-foo",
            "## Phase 1: Setup\n\
             - [ ] T001 one\n\
             - [ ] T002 two\n\
             - [ ] T003 three\n",
        );
        let entries = decompose_feature(&feat);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].branch.starts_with("phase/"));
        assert!(entries[0].prompt.contains("T001"));
        assert!(entries[0].prompt.contains("T002"));
        assert!(entries[0].prompt.contains("T003"));
    }

    #[test]
    fn decompose_single_non_p_still_uses_phase_branch() {
        let feat = feature_fixture("002-foo", "## Phase 1: Setup\n- [ ] T001 only\n");
        let entries = decompose_feature(&feat);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].branch.starts_with("phase/"));
    }

    #[test]
    fn decompose_fully_complete_yields_nothing() {
        let feat = feature_fixture(
            "001-foo",
            "## Phase 1: Setup\n- [x] T001 done\n- [x] T002 done\n",
        );
        let entries = decompose_feature(&feat);
        assert!(entries.is_empty());
    }

    #[test]
    fn decompose_empty_tasks_md_yields_nothing() {
        let feat = feature_fixture("001-foo", "");
        let entries = decompose_feature(&feat);
        assert!(entries.is_empty());
    }

    #[test]
    fn decompose_owned_files_is_none() {
        let feat = feature_fixture(
            "001-foo",
            "## Phase 1: Setup\n- [ ] T001 [P] do thing\n- [ ] T002 do other\n",
        );
        for entry in decompose_feature(&feat) {
            assert!(entry.owned_files.is_none(), "id={}", entry.id);
            assert!(entry.cli.is_none(), "id={}", entry.id);
        }
    }

    #[test]
    fn decompose_branch_shapes() {
        let feat = feature_fixture(
            "003-user-list",
            "## Phase 2: Foundational\n\
             - [ ] T009 [P] Add login form component\n\
             - [ ] T010 Setup database schema\n",
        );
        let entries = decompose_feature(&feat);
        let task_entry = entries
            .iter()
            .find(|e| e.id == "003-user-list-T009")
            .unwrap();
        assert_eq!(task_entry.branch, "task/t009-add-login-form-component");

        let phase_entry = entries
            .iter()
            .find(|e| e.id == "003-user-list-phase-2")
            .unwrap();
        assert_eq!(phase_entry.branch, "phase/003-user-list-foundational");
    }

    #[test]
    fn decompose_branches_use_safe_char_set() {
        let feat = feature_fixture(
            "003-user-list",
            "## Phase 2: User Story #1!\n\
             - [ ] T001 [P] Punctuation & symbols (yes, with commas)\n\
             - [ ] T002 plain task\n",
        );
        let entries = decompose_feature(&feat);
        for entry in &entries {
            let stripped = entry
                .branch
                .strip_prefix("task/")
                .or_else(|| entry.branch.strip_prefix("phase/"))
                .unwrap();
            for c in stripped.chars() {
                assert!(
                    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_',
                    "unsafe char {c:?} in branch {}",
                    entry.branch
                );
            }
        }
    }

    // --- build_prompt ---

    #[test]
    fn prompt_includes_spec_and_plan() {
        let feat = feature_fixture("003-user-list", "## Phase 1: Setup\n- [ ] T001 one\n");
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(prompt.contains("## Feature Context"));
        assert!(prompt.contains("SPEC"));
        assert!(prompt.contains("## Implementation Plan"));
        assert!(prompt.contains("PLAN"));
        assert!(prompt.contains("T001"));
    }

    #[test]
    fn prompt_omits_plan_when_missing() {
        let mut feat = feature_fixture("003-user-list", "## Phase 1: Setup\n- [ ] T001 one\n");
        feat.plan_md = None;
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(prompt.contains("## Feature Context"));
        assert!(!prompt.contains("## Implementation Plan"));
    }

    #[test]
    fn prompt_includes_checklists_when_present() {
        let mut feat = feature_fixture("003-user-list", "## Phase 1: Setup\n- [ ] T001 one\n");
        feat.checklists = vec![
            ("auth.md".to_string(), "auth criteria".to_string()),
            ("data.md".to_string(), "data criteria".to_string()),
        ];
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(prompt.contains("## Validation Criteria (advisory)"));
        assert!(prompt.contains("### auth.md"));
        assert!(prompt.contains("auth criteria"));
        assert!(prompt.contains("### data.md"));
        assert!(prompt.contains("data criteria"));
        assert!(prompt.contains("advisory"));
    }

    #[test]
    fn prompt_omits_checklists_when_empty() {
        let feat = feature_fixture("003-user-list", "## Phase 1: Setup\n- [ ] T001 one\n");
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(!prompt.contains("Validation Criteria"));
    }

    #[test]
    fn consolidated_prompt_lists_tasks_in_order_with_ids() {
        let feat = feature_fixture(
            "003-user-list",
            "## Phase 2: Foundational\n\
             - [ ] T004 Setup database schema\n\
             - [ ] T005 Create auth tables\n\
             - [ ] T006 Seed test data\n",
        );
        let phase = current_phase(&feat.phases).unwrap();
        let tasks: Vec<&Task> = phase.tasks.iter().filter(|t| !t.p_marker).collect();
        let kind = EntryKind::Consolidated {
            tasks,
            phase_number: phase.number,
            phase_name: &phase.name,
        };
        let prompt = build_prompt(&feat, &kind);

        let p4 = prompt.find("T004").unwrap();
        let p5 = prompt.find("T005").unwrap();
        let p6 = prompt.find("T006").unwrap();
        assert!(p4 < p5 && p5 < p6, "tasks must appear in source order");
        assert!(prompt.contains("Setup database schema"));
        assert!(prompt.contains("Create auth tables"));
        assert!(prompt.contains("Seed test data"));
        assert!(prompt.contains("`- [x]`"));
        assert!(prompt.contains("agent.done"));
    }

    #[test]
    fn single_prompt_omits_sequential_instruction() {
        let feat = feature_fixture(
            "003-user-list",
            "## Phase 1: Setup\n- [ ] T009 [P] only one task\n",
        );
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(prompt.contains("T009"));
        assert!(prompt.contains("only one task"));
        assert!(!prompt.contains("sequentially"));
        assert!(!prompt.contains("agent.done"));
    }

    #[test]
    fn prompt_sections_separated_by_delimiter() {
        let feat = feature_fixture("003-user-list", "## Phase 1: Setup\n- [ ] T001 one\n");
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(prompt.contains("\n\n---\n\n"));
    }

    // Maps to scenario `Boot prompt omits Implementation Plan when plan.md is
    // missing` from spec-kit-format. Uses on-disk fixture via `read_feature`
    // so the path that real scans take is exercised end-to-end.
    // (test-coverage-v0-5-0 task 11.2)
    #[test]
    fn boot_prompt_omits_plan_section_when_plan_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let feat_dir = tmp.path().join("009-no-plan");
        fs::create_dir(&feat_dir).unwrap();
        fs::write(feat_dir.join("spec.md"), "feature spec body").unwrap();
        fs::write(
            feat_dir.join("tasks.md"),
            "## Phase 1: Setup\n- [ ] T001 do thing\n",
        )
        .unwrap();
        // Explicitly no plan.md on disk.

        let feature = read_feature(&feat_dir).unwrap().expect("feature loads");
        let phase = current_phase(&feature.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feature, &EntryKind::Single { task });
        assert!(
            !prompt.contains("Implementation Plan"),
            "boot prompt must omit the Implementation Plan section when plan.md is missing; got:\n{prompt}"
        );
    }

    // Maps to scenario `Boot prompt includes checklists when present` from
    // spec-kit-format. (test-coverage-v0-5-0 task 11.3)
    #[test]
    fn boot_prompt_includes_checklists_section_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let feat_dir = tmp.path().join("010-checklisted");
        fs::create_dir(&feat_dir).unwrap();
        fs::write(feat_dir.join("spec.md"), "spec body").unwrap();
        fs::write(
            feat_dir.join("tasks.md"),
            "## Phase 1: Setup\n- [ ] T001 do thing\n",
        )
        .unwrap();
        fs::create_dir(feat_dir.join("checklists")).unwrap();
        fs::write(
            feat_dir.join("checklists/auth-checklist.md"),
            "auth criteria text",
        )
        .unwrap();
        fs::write(
            feat_dir.join("checklists/data-checklist.md"),
            "data criteria text",
        )
        .unwrap();

        let feature = read_feature(&feat_dir).unwrap().expect("feature loads");
        let phase = current_phase(&feature.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feature, &EntryKind::Single { task });
        assert!(
            prompt.contains("Validation Criteria"),
            "boot prompt should include the Validation Criteria section; got:\n{prompt}"
        );
        assert!(
            prompt.contains("auth criteria text"),
            "boot prompt should include the auth checklist content; got:\n{prompt}"
        );
        assert!(
            prompt.contains("data criteria text"),
            "boot prompt should include the data checklist content; got:\n{prompt}"
        );
    }

    // Maps to scenario `Single-[P] boot prompt contains one task description`
    // from spec-kit-format. (test-coverage-v0-5-0 task 11.4)
    #[test]
    fn single_p_boot_prompt_contains_one_task_description() {
        let feat = feature_fixture(
            "011-login",
            "## Phase 1: Build\n- [ ] T009 [P] Add login form\n",
        );
        let phase = current_phase(&feat.phases).unwrap();
        let task = &phase.tasks[0];
        let prompt = build_prompt(&feat, &EntryKind::Single { task });
        assert!(
            prompt.contains("T009"),
            "prompt should include task id; got:\n{prompt}"
        );
        assert!(
            prompt.contains("Add login form"),
            "prompt should include the description; got:\n{prompt}"
        );
        assert!(
            !prompt.contains("agent.done only when"),
            "single-[P] prompt must not carry the consolidated-set sequential instruction; got:\n{prompt}"
        );
        assert!(
            !prompt.contains("sequentially"),
            "single-[P] prompt must not carry sequential ordering text; got:\n{prompt}"
        );
    }

    // --- detect_constitution ---

    #[test]
    fn detect_constitution_present() {
        let tmp = tempfile::tempdir().unwrap();
        let specify = tmp.path().join(".specify");
        let specs = specify.join("specs");
        let memory = specify.join("memory");
        fs::create_dir_all(&specs).unwrap();
        fs::create_dir_all(&memory).unwrap();
        let cons = memory.join("constitution.md");
        fs::write(&cons, "Be excellent.").unwrap();

        let detected = detect_constitution(&specs).unwrap();
        assert_eq!(detected, cons);
    }

    #[test]
    fn detect_constitution_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let specs = tmp.path().join(".specify").join("specs");
        fs::create_dir_all(&specs).unwrap();
        assert!(detect_constitution(&specs).is_none());
    }

    #[test]
    fn detect_constitution_no_parent() {
        // The root directory `/` has no parent — defensive: returns None.
        let root = Path::new("/");
        // We do not assert presence of /memory/constitution.md; we only
        // assert the call returns None when no parent exists. On Unix `/`
        // has no parent so `parent()` returns None. On Windows roots also
        // return None.
        assert!(detect_constitution(root).is_none());
    }

    // --- Round-trip: scan a fixture .specify/specs/ tree ---

    #[test]
    fn scan_multi_feature_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs");
        fs::create_dir_all(&specs_dir).unwrap();

        // Feature 1: phase 1 done, phase 2 mixed → 1 [P] entry + 1 consolidated.
        let f1 = specs_dir.join("001-alpha");
        fs::create_dir(&f1).unwrap();
        fs::write(f1.join("spec.md"), "alpha spec").unwrap();
        fs::write(f1.join("plan.md"), "alpha plan").unwrap();
        fs::write(
            f1.join("tasks.md"),
            "## Phase 1: Setup\n\
             - [x] T001 done\n\
             ## Phase 2: Foundational\n\
             - [ ] T002 [P] parallel one\n\
             - [ ] T003 sequential task\n\
             - [ ] T004 sequential other\n",
        )
        .unwrap();

        // Feature 2: phase 1 only [P] (2 entries), phase 2 only non-[P] (deferred).
        let f2 = specs_dir.join("002-beta");
        fs::create_dir(&f2).unwrap();
        fs::write(f2.join("spec.md"), "beta spec").unwrap();
        fs::write(
            f2.join("tasks.md"),
            "## Phase 1: Setup\n\
             - [ ] T010 [P] alpha\n\
             - [ ] T011 [P] beta\n\
             ## Phase 2: Polish\n\
             - [ ] T020 deferred\n",
        )
        .unwrap();

        // Feature 3: fully complete (skipped).
        let f3 = specs_dir.join("003-gamma");
        fs::create_dir(&f3).unwrap();
        fs::write(f3.join("tasks.md"), "## Phase 1: Setup\n- [x] T030 done\n").unwrap();

        let backend = SpecKitBackend;
        let entries = backend.scan(&specs_dir).unwrap();
        // F1: 1 [P] + 1 consolidated = 2.  F2: 2 [P] = 2.  F3: 0.
        assert_eq!(entries.len(), 4, "got entries: {entries:?}");
        let ids: std::collections::HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();
        assert!(ids.contains("001-alpha-T002"));
        assert!(ids.contains("001-alpha-phase-2"));
        assert!(ids.contains("002-beta-T010"));
        assert!(ids.contains("002-beta-T011"));
        // Gamma is skipped.
        assert!(!ids.iter().any(|id| id.starts_with("003-gamma")));

        // Spec/plan content is included.
        let alpha = entries
            .iter()
            .find(|e| e.id == "001-alpha-phase-2")
            .unwrap();
        assert!(alpha.prompt.contains("alpha spec"));
        assert!(alpha.prompt.contains("alpha plan"));

        let beta = entries.iter().find(|e| e.id == "002-beta-T010").unwrap();
        assert!(beta.prompt.contains("beta spec"));
        // No plan.md for beta — plan section is absent.
        assert!(!beta.prompt.contains("## Implementation Plan"));
    }

    #[test]
    fn scan_advances_phase_when_phase_one_clears() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs");
        let feat = specs_dir.join("001-feature");
        fs::create_dir_all(&feat).unwrap();
        let tasks_path = feat.join("tasks.md");

        // Initial state: phase 1 has one incomplete; phase 2 deferred.
        fs::write(
            &tasks_path,
            "## Phase 1: Setup\n- [ ] T001 a\n## Phase 2: Build\n- [ ] T002 b\n",
        )
        .unwrap();
        let backend = SpecKitBackend;
        let entries = backend.scan(&specs_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "001-feature-phase-1");

        // Clear phase 1 — phase 2 becomes current.
        fs::write(
            &tasks_path,
            "## Phase 1: Setup\n- [x] T001 a\n## Phase 2: Build\n- [ ] T002 b\n",
        )
        .unwrap();
        let entries = backend.scan(&specs_dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "001-feature-phase-2");
    }
}
