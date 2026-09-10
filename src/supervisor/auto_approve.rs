//! Safe-command classification for the auto-approve feature.
//!
//! Filled out in Section 2 of `openspec/changes/auto-approve-patterns/tasks.md`.
//! Section 1 only needs `default_safe_commands()` so [`crate::config::AutoApproveConfig`]
//! can compute its effective whitelist.
//!
//! The `auto-approve-scope-v0-6-x` change adds a *worktree file-operation*
//! category on top of the shell-command whitelist: a Claude
//! write / edit / create permission prompt is auto-approved when the target
//! path resolves *inside* the agent's own worktree root. The boundary check
//! canonicalises the path before the `starts_with(worktree_root)` test so a
//! `..`/symlink escape cannot smuggle an out-of-worktree path past the gate.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::agents::is_managed_path;
use crate::config::claude_prompt_profile;

/// Read-mostly command verbs eligible for auto-approval.
///
/// These leading verbs are routine, low-risk operations (reads, searches,
/// localised writes) that an unattended agent runs constantly. They are part
/// of the built-in whitelist but remain **subordinate to the danger-list**:
/// `is_safe_command` matching one of these (e.g. `git`) never overrides a
/// danger-list match (e.g. `git push`) — see [`is_dangerous`]. The same set
/// gates the permanent broad-grant rule (a `git status` broad grant is fine; a
/// `python -c` one is not).
///
/// The list is **stack-neutral**: toolchain verbs (`cargo`, `openspec`,
/// `just`, …) are never built in — projects receive them through the resolved
/// `[supervisor.common_dev_allowlist]` stack presets and/or configured
/// extensions (see `AutoApproveConfig::effective_whitelist`).
///
/// Exported so the broad-grant rule and the bundled `sweep.sh` helper can
/// assert parity with the Rust classifier.
pub const READ_MOSTLY_VERBS: &[&str] = &[
    "curl", "cat", "ls", "grep", "rg", "git", "echo", "sed", "awk", "find", "wc", "head", "tail",
    "jq", "mkdir", "touch", "export", "tmux", "env",
];

/// Built-in whitelist of command prefixes eligible for auto-approval.
///
/// Contains only **stack-neutral** entries: `git commit`, `git push`, the
/// broker-localhost `curl` prefix, and the [`READ_MOSTLY_VERBS`] allowlist.
/// Stack- or tool-specific patterns (`cargo …`, `openspec`, `just`) are NOT
/// built in — they enter the effective whitelist through the resolved
/// `[supervisor.common_dev_allowlist]` stack presets and `extra`, plus
/// `[supervisor.auto_approve] safe_commands` (see
/// `AutoApproveConfig::effective_whitelist`). Each entry is matched against
/// captured command text via prefix + whitespace boundary semantics in
/// [`is_safe_command`]. Every whitelist match is **subordinate to the
/// danger-list**: the poll loop evaluates [`is_dangerous`] first, so a
/// whitelisted verb that also matches a danger pattern still escalates.
#[must_use]
pub fn default_safe_commands() -> &'static [&'static str] {
    &[
        "git commit",
        "git push",
        "curl http://127.0.0.1:",
        // Read-mostly verb allowlist (kept in sync with READ_MOSTLY_VERBS —
        // see the `read_mostly_verbs_are_whitelisted` test).
        "curl",
        "cat",
        "ls",
        "grep",
        "rg",
        "git",
        "echo",
        "sed",
        "awk",
        "find",
        "wc",
        "head",
        "tail",
        "jq",
        "mkdir",
        "touch",
        "export",
        "tmux",
        "env",
    ]
}

/// Returns `true` if `captured` begins with any whitelist entry followed
/// by either end-of-string or ASCII whitespace.
///
/// Prefix matching is intentional: a single entry like `cargo test` should
/// match `cargo test --no-run --workspace` without the user having to
/// enumerate every flag combination. The whitespace boundary prevents
/// `cargotest --foo` from matching the `cargo test` prefix.
#[must_use]
pub fn is_safe_command(captured: &str, whitelist: &[String]) -> bool {
    let captured = captured.trim_start();
    whitelist.iter().any(|entry| {
        let entry = entry.as_str();
        if !captured.starts_with(entry) {
            return false;
        }
        match captured.as_bytes().get(entry.len()) {
            None => true,
            Some(b) => b.is_ascii_whitespace(),
        }
    })
}

/// Compiled regex matching the lead-in of a Claude filesystem-operation
/// permission prompt and capturing the target path.
///
/// Covers the documented write / edit / create prompt shapes, e.g.
/// `"Do you want to allow this write to <path>?"` and
/// `"Do you want to make this edit to <path>?"`. The capture group runs to
/// the end of the line (minus a trailing `?`), so the path may contain
/// spaces. Matching is case-insensitive.
///
/// Sourced from the embedded Claude Code profile's `file_prompt_pattern`
/// (`cli-prompt-profiles` capability) rather than a compiled literal.
fn file_prompt_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&claude_prompt_profile().file_prompt_pattern)
            .expect("embedded file-prompt pattern is valid")
    })
}

/// Extracts the target path from a Claude filesystem-operation permission
/// prompt, or `None` when the prompt is not a recognised file-op prompt.
///
/// The prompt text is scanned line by line; the first line whose lead-in
/// matches a documented write / edit / create shape yields its captured
/// path. Surrounding quotes and backticks are stripped. The returned string
/// is the raw path as it appeared in the prompt — resolution against the
/// worktree root is the caller's job via [`is_path_inside_worktree`].
#[must_use]
pub fn extract_path_from_file_prompt(prompt: &str) -> Option<String> {
    let re = file_prompt_regex();
    for line in prompt.lines() {
        if let Some(caps) = re.captures(line) {
            let raw = caps.get(1)?.as_str().trim();
            let cleaned = raw
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

/// Resolves `target` to an absolute, symlink/`..`-collapsed path for the
/// worktree-boundary check, even when the target does not exist yet.
///
/// `canonicalize` fails for a not-yet-created file (the common case for a
/// file-create prompt), so this walks up to the deepest *existing* ancestor,
/// canonicalises that, and re-appends the non-existent suffix. Any component
/// that is `..` (a `Path::file_name` of `None`) aborts the walk and returns
/// `None`, which the caller treats as "outside the worktree" — fail-closed.
fn resolve_for_boundary(target: &Path) -> Option<PathBuf> {
    let mut suffix: Vec<OsString> = Vec::new();
    let mut cur = target.to_path_buf();
    loop {
        if let Ok(base) = cur.canonicalize() {
            let mut resolved = base;
            for comp in suffix.iter().rev() {
                resolved.push(comp);
            }
            return Some(resolved);
        }
        let file = cur.file_name()?.to_os_string();
        if !cur.pop() {
            return None;
        }
        suffix.push(file);
    }
}

/// Returns `true` when `path` (resolved against `worktree_root`) lies inside
/// the worktree.
///
/// Both sides are canonicalised before the `starts_with` comparison so a
/// `..`-relative path or a symlink that escapes the worktree fails the
/// check. A path that cannot be resolved (e.g. one containing a bare `..`
/// that walks off the filesystem) is treated as outside the worktree.
#[must_use]
pub fn is_path_inside_worktree(path: &str, worktree_root: &Path) -> bool {
    let target = Path::new(path);
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        worktree_root.join(target)
    };
    let Some(resolved) = resolve_for_boundary(&joined) else {
        return false;
    };
    let root = worktree_root
        .canonicalize()
        .unwrap_or_else(|_| worktree_root.to_path_buf());
    resolved.starts_with(&root)
}

/// Classifies a captured permission prompt as a safe worktree file operation.
///
/// Returns `true` only when (a) `approve_worktree_writes` is enabled, (b) the
/// prompt matches a documented Claude file-op shape, and (c) the extracted
/// target path resolves inside `worktree_root`. Out-of-worktree paths,
/// symlink-escape attempts, and shell-command prompts all return `false` —
/// those continue through the shell whitelist or the manual-prompt flow.
#[must_use]
pub fn is_worktree_file_op(
    prompt: &str,
    worktree_root: &Path,
    approve_worktree_writes: bool,
) -> bool {
    if !approve_worktree_writes {
        return false;
    }
    match extract_path_from_file_prompt(prompt) {
        Some(path) => is_path_inside_worktree(&path, worktree_root),
        None => false,
    }
}

/// Classifies a command slice as a worktree-confined `git add` / `git commit`
/// pre-approval — the F2 keystone that lets an unattended agent stage and
/// commit its own work without stalling on the approval prompt.
///
/// Returns `true` only when (a) the slice's verb is `git add` or `git commit`
/// and (b) the agent's `worktree_root` resolves to a real directory, reusing
/// the same canonicalize-then-`starts_with(worktree_root)` boundary check as
/// [`is_worktree_file_op`] (the agent's cwd is its isolated worktree).
///
/// `git push` is deliberately NOT covered — it is on the danger-list and the
/// caller evaluates [`is_dangerous`] first, so a `git push` slice escalates
/// before this function is ever consulted.
#[must_use]
pub fn is_worktree_git_op(slice: &str, worktree_root: &Path) -> bool {
    let s = slice.trim_start();
    let is_add_or_commit = is_safe_command(s, &["git add".to_string()])
        || is_safe_command(s, &["git commit".to_string()]);
    if !is_add_or_commit {
        return false;
    }
    // The worktree root must be a real, canonicalisable directory — the same
    // boundary primitive used for file edits, applied to the worktree itself.
    is_path_inside_worktree(".", worktree_root)
}

/// Interpreter verbs eligible for the worktree-confined script-run rule.
///
/// None of these are read-mostly verbs, so a matching prompt only ever
/// receives the one-time approval — the broad-grant rule in
/// [`select_option_index`] never grants them permanently.
const WORKTREE_INTERPRETERS: &[&str] = &["bash", "sh", "python3", "python", "node"];

/// Shell metacharacters that disqualify a slice from the worktree-confined
/// dev-test rules: separators, substitution, and redirection can smuggle a
/// second command or an out-of-worktree effect past the per-token path
/// check, so their presence fails the match (fail-closed).
const DEV_TEST_METACHARS: &[char] = &[';', '|', '&', '$', '`', '>', '<'];

/// Classifies a command slice as a worktree-confined dev-test operation —
/// the rider rules of `safe-command-classification`:
///
/// - `bash -n <path>` — shell syntax check on a worktree-resident script;
/// - non-recursive `chmod <mode> <path...>` with every path worktree-resident
///   (`chmod -R` is danger-listed and escalates before this rule is ever
///   consulted; the rule itself also refuses flag tokens, fail-closed);
/// - `mktemp` / `mktemp -d` (flags only — a template path argument does not
///   match);
/// - interpreter execution of a worktree-resident script (`bash`, `sh`,
///   `python3`, `python`, `node` followed by a worktree-resident file path,
///   with no path argument resolving outside the worktree).
///
/// Inline code strings (a `-c` flag) never match. Path resolution reuses
/// [`is_path_inside_worktree`] (canonicalised, fail-closed). The rules apply
/// only when a worktree root is known and resolvable — the supervisor pane,
/// which has none, is unaffected.
///
/// Interpreter matches are safe for ONE-TIME approval only: none of the
/// covered verbs are read-mostly, so [`select_option_index`] always picks
/// the one-time option on a 3-option prompt, never the permanent broad
/// grant.
#[must_use]
pub fn is_worktree_dev_test_op(slice: &str, worktree_root: &Path) -> bool {
    let slice = slice.trim();
    // Fail-closed: no metacharacter smuggling, and the worktree root itself
    // must be a real directory (a pane without a known worktree never
    // matches).
    if slice.contains(DEV_TEST_METACHARS) || !worktree_root.is_dir() {
        return false;
    }
    // Skip leading VAR=value assignments, mirroring `leading_verb`.
    let mut tokens = slice.split_whitespace().skip_while(|tok| {
        tok.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    });
    let Some(first) = tokens.next() else {
        return false;
    };
    let verb = first.rsplit('/').next().unwrap_or(first);
    let args: Vec<&str> = tokens.collect();
    // Inline code strings never match these rules.
    if args.contains(&"-c") {
        return false;
    }
    match verb {
        // `mktemp` / `mktemp -d`: flags only, never a template path argument.
        "mktemp" => args.iter().all(|t| t.starts_with('-')),
        // Non-recursive chmod: <mode> then worktree-resident paths only. Any
        // flag token (including `-R`) fails the match.
        "chmod" => {
            if args.iter().any(|t| t.starts_with('-')) {
                return false;
            }
            let paths = args.get(1..).unwrap_or_default();
            !paths.is_empty()
                && paths
                    .iter()
                    .all(|p| is_path_inside_worktree(p, worktree_root))
        }
        // `bash -n <script>` and interpreter-of-worktree-script: every
        // non-flag argument must resolve inside the worktree.
        v if WORKTREE_INTERPRETERS.contains(&v) => {
            let paths: Vec<&str> = args
                .iter()
                .filter(|t| !t.starts_with('-'))
                .copied()
                .collect();
            !paths.is_empty()
                && paths
                    .iter()
                    .all(|p| is_path_inside_worktree(p, worktree_root))
        }
        _ => false,
    }
}

/// Interpreters through which a managed helper script may be invoked
/// (`bash .git-paw/scripts/broker.sh …`), in addition to direct invocation.
const SCRIPT_INTERPRETERS: &[&str] = &["bash", "sh"];

/// Filenames of git-paw's own bundled helper scripts eligible for the GP-02b
/// safe-invocation rule.
const MANAGED_HELPER_SCRIPTS: &[&str] = &["broker.sh", "sweep.sh", "docs-fetch.sh"];

/// Returns `true` when `token` names one of [`MANAGED_HELPER_SCRIPTS`],
/// resolved against `worktree_root` when the token is a relative path.
///
/// Delegates the "is this under git-paw's own managed subtree" question to
/// [`is_managed_path`] — the single source of truth `agent-memory-isolation`
/// and `remove` also consult; this function narrows it to the specific
/// helper-script filenames the safe-invocation rule covers (an arbitrary
/// `.git-paw/` file, e.g. `config.toml`, is managed bookkeeping but is NOT a
/// script git-paw ever executes on an agent's behalf).
fn is_managed_script_path(token: &str, worktree_root: Option<&Path>) -> bool {
    let path = Path::new(token);
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !MANAGED_HELPER_SCRIPTS.contains(&name) {
        return false;
    }
    let Some(root) = worktree_root else {
        // No worktree root known: fall back to the unambiguous relative
        // shape `.git-paw/scripts/<name>` so the rule still applies where it
        // safely can.
        return path
            .to_str()
            .is_some_and(|s| s == format!(".git-paw/scripts/{name}"));
    };
    let rel = if path.is_absolute() {
        let root_resolved = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let resolved = resolve_for_boundary(path).unwrap_or_else(|| path.to_path_buf());
        match resolved.strip_prefix(&root_resolved) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => return false,
        }
    } else {
        path.to_path_buf()
    };
    rel.to_str().is_some_and(|s| is_managed_path(root, s))
}

/// Classifies a command slice as an invocation of one of git-paw's own
/// managed helper scripts (`.git-paw/scripts/{broker,sweep,docs-fetch}.sh`) —
/// the GP-02b safe rule. git-paw authors these scripts and they perform only
/// bounded coordination actions, so an unattended agent's boot-time call into
/// one of them is safe by construction.
///
/// Matches direct invocation (`.git-paw/scripts/broker.sh …`) and
/// interpreter-invoked forms (`bash .git-paw/scripts/broker.sh …`). Callers
/// evaluate this AFTER the danger-list, so a slice that chains a
/// managed-script call with a danger-class operation
/// (`sweep.sh snapshot && rm -rf /`) still escalates — the danger-list match
/// is found first and this rule is never consulted for that slice.
#[must_use]
pub fn is_managed_script_invocation(slice: &str, worktree_root: Option<&Path>) -> bool {
    let mut tokens = slice
        .split_whitespace()
        .skip_while(|tok| is_assignment_token(tok));
    let Some(first) = tokens.next() else {
        return false;
    };
    let path_token = if SCRIPT_INTERPRETERS.contains(&first) {
        match tokens.find(|t| !t.starts_with('-')) {
            Some(t) => t,
            None => return false,
        }
    } else {
        first
    };
    is_managed_script_path(path_token, worktree_root)
}

// ---------------------------------------------------------------------------
// Command-slice extraction (Section 1)
// ---------------------------------------------------------------------------

/// Strips leading TUI decoration (box-drawing glyphs, bullets, the selection
/// caret, and surrounding whitespace) from a captured line so command
/// extraction and boundary detection are not confused by pane chrome.
///
/// A bare `>` is deliberately NOT stripped: it is significant to the
/// `> /dev/` danger pattern when it appears mid-line as a redirect.
fn strip_decoration(line: &str) -> &str {
    line.trim_start_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '│' | '─'
                    | '╭'
                    | '╮'
                    | '╰'
                    | '╯'
                    | '├'
                    | '┤'
                    | '┐'
                    | '└'
                    | '┘'
                    | '┌'
                    | '⎿'
                    | '❯'
                    | '●'
                    | '•'
                    | '*'
                    | '·'
            )
    })
    .trim_end()
}

/// Compiled regex for the embedded profile's `option_line_pattern`
/// (`cli-prompt-profiles` capability), matching a numbered option line
/// (`1. …`, `2) …`) once leading whitespace is trimmed.
fn option_line_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&claude_prompt_profile().option_line_pattern)
            .expect("embedded option-line pattern is valid")
    })
}

/// Returns `true` when a cleaned line is a numbered option (`1. …`, `2) …`).
fn is_option_line(line: &str) -> bool {
    option_line_regex().is_match(line.trim_start())
}

/// Returns `true` when a cleaned line marks the end of the command block —
/// the confirmation question, an approval marker, or the option list.
///
/// The confirmation-question and approval-marker checks source their
/// literals from the embedded Claude Code profile's `live_prompt_markers`
/// and `approval_markers` (`cli-prompt-profiles` capability).
fn is_confirmation_boundary(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let profile = claude_prompt_profile();
    profile
        .live_prompt_markers
        .iter()
        .any(|m| lower.starts_with(m.as_str()))
        || profile
            .approval_markers
            .iter()
            .any(|m| lower.contains(&m.to_ascii_lowercase()))
        || is_option_line(line)
}

/// Returns `true` when a cleaned line is a `Bash command` prompt header.
///
/// Sources its literal from the embedded profile's `command_header_markers`
/// (`cli-prompt-profiles` capability) — a header-style marker is one that
/// does NOT end in `(` (an inline-embedding marker like `Bash(`, handled
/// separately by [`extract_command_slice`]'s inline form).
fn is_bash_command_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    claude_prompt_profile()
        .command_header_markers
        .iter()
        .filter(|m| !m.ends_with('('))
        .any(|m| lower.starts_with(&m.to_ascii_lowercase()))
}

/// Extracts the prompted command slice from a pane capture: the command text
/// between the `Bash command` / `Bash(` header and the confirmation question,
/// ignoring surrounding narration.
///
/// The LAST header in the capture wins — when a pane shows earlier narration
/// followed by the live prompt, the live command is the most recent one. The
/// `Bash(<cmd>)` inline form is read from inside the parentheses; the
/// `Bash command` header form takes the first non-empty line after the header
/// (the command itself), stopping before any description line or the
/// confirmation question. Returns `None` when no recognised header is present.
#[must_use]
pub fn extract_command_slice(capture: &str) -> Option<String> {
    let inline_markers: Vec<&str> = claude_prompt_profile()
        .command_header_markers
        .iter()
        .filter(|m| m.ends_with('('))
        .map(String::as_str)
        .collect();
    let lines: Vec<&str> = capture.lines().collect();
    for (idx, raw) in lines.iter().enumerate().rev() {
        let line = strip_decoration(raw);

        // Inline `Bash(<cmd>)` form.
        if let Some((start, marker)) = inline_markers
            .iter()
            .find_map(|m| line.find(m).map(|start| (start, *m)))
        {
            let after = &line[start + marker.len()..];
            if let Some(end) = after.rfind(')') {
                let cmd = after[..end].trim();
                if !cmd.is_empty() {
                    return Some(cmd.to_string());
                }
            }
        }

        // `Bash command` header form: the command is the first non-empty
        // cleaned line that follows, up to the confirmation question.
        if is_bash_command_header(line) {
            for next in &lines[idx + 1..] {
                let cleaned = strip_decoration(next);
                if cleaned.is_empty() {
                    continue;
                }
                if is_confirmation_boundary(cleaned) {
                    break;
                }
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

/// Discard-redirect suffixes [`normalize_command`] strips: the two spellings of
/// a `/dev/null` redirect wrapped onto a command purely to silence its output.
///
/// Only the `/dev/null` target is listed. A redirect to any other device (e.g.
/// `> /dev/sda`) is left in place so the `> /dev/` danger pattern still matches
/// it.
const DISCARD_REDIRECTS: &[&str] = &[">/dev/null 2>&1", "> /dev/null 2>&1"];

/// Strips a trailing exit-code-probe clause (`; echo …$?`, `; RC=$?`) from
/// `cmd`, or returns `None` when the command carries none.
///
/// Only the clause after the LAST `;` is considered, and only when it is
/// recognisably a probe: an `echo` mentioning `$?`, or a bare
/// `NAME=$?` assignment. Anything else — including a second real command — is
/// left intact.
fn strip_exit_probe(cmd: &str) -> Option<&str> {
    let (head, tail) = cmd.rsplit_once(';')?;
    let tail = tail.trim();
    let is_echo_probe = tail
        .strip_prefix("echo")
        .is_some_and(|rest| rest.starts_with(char::is_whitespace) && rest.contains("$?"));
    let is_assign_probe = tail.strip_suffix("=$?").is_some_and(|name| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    (is_echo_probe || is_assign_probe).then(|| head.trim_end())
}

/// Strips a trailing [`DISCARD_REDIRECTS`] suffix from `cmd`, or returns `None`
/// when the command ends in no such redirect.
fn strip_discard_redirect(cmd: &str) -> Option<&str> {
    DISCARD_REDIRECTS
        .iter()
        .find_map(|redirect| cmd.strip_suffix(redirect))
        .map(str::trim_end)
}

/// Returns `true` when `tok` is strict shell-assignment syntax: `NAME=value`
/// with a non-empty alphanumeric/underscore name. A value that merely
/// contains `=` (e.g. a URL argument) never matches unless its own leading
/// segment happens to look like a bare identifier followed by `=`.
fn is_assignment_token(tok: &str) -> bool {
    tok.split_once('=').is_some_and(|(k, _)| {
        !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Splits `s` (assumed free of leading whitespace) into its first
/// whitespace-delimited token and the remainder — which may itself start
/// with whitespace, since the split is on the position of the boundary, not
/// past it. Returns `None` for an empty string.
fn split_first_token(s: &str) -> Option<(&str, &str)> {
    if s.is_empty() {
        return None;
    }
    match s.find(char::is_whitespace) {
        Some(idx) => Some((&s[..idx], &s[idx..])),
        None => Some((s, "")),
    }
}

/// Returns the remainder after a leading `verb` token followed by a
/// whitespace boundary or end-of-string (so `envfoo` does not match `env`),
/// or `None` when `s` does not start with that verb.
fn strip_wrapper_verb<'a>(s: &'a str, verb: &str) -> Option<&'a str> {
    let (tok, after) = split_first_token(s)?;
    (tok == verb).then_some(after)
}

/// Strips a leading run of `NAME=value` assignments, then a leading `env`
/// (together with its own assignments and `-i` / `-u NAME` options) or
/// `nohup` wrapper, from `cmd`. Returns `None` when `cmd` carries no such
/// prefix, so [`normalize_command`]'s fixpoint loop can detect it stopped
/// changing.
fn strip_leading_prefix(cmd: &str) -> Option<&str> {
    let mut rest = cmd;
    let mut changed = false;

    // A run of leading `NAME=value` assignments.
    loop {
        let trimmed = rest.trim_start();
        let Some((tok, after)) = split_first_token(trimmed) else {
            break;
        };
        if !is_assignment_token(tok) {
            break;
        }
        rest = after;
        changed = true;
    }

    // A leading `env` wrapper (with its own assignments and `-i` / `-u NAME`
    // options) or a leading `nohup` wrapper.
    let trimmed = rest.trim_start();
    if let Some(after_verb) = strip_wrapper_verb(trimmed, "env") {
        rest = after_verb;
        changed = true;
        loop {
            let t = rest.trim_start();
            let Some((tok, after)) = split_first_token(t) else {
                break;
            };
            if is_assignment_token(tok) || tok == "-i" {
                rest = after;
                continue;
            }
            if tok == "-u" {
                let after_trim = after.trim_start();
                if let Some((_name, after2)) = split_first_token(after_trim) {
                    rest = after2;
                    continue;
                }
            }
            break;
        }
    } else if let Some(after_verb) = strip_wrapper_verb(trimmed, "nohup") {
        rest = after_verb;
        changed = true;
    }

    changed.then_some(rest)
}

/// Normalises a command slice for classification by removing the gate-reporting
/// wrappers an agent appends to a command it is only observing the exit status
/// of: a trailing `; echo …$?` / `; RC=$?` exit-code probe, a trailing
/// `>/dev/null 2>&1` discard redirect, and a leading run of `NAME=value`
/// environment-variable assignments plus a leading `env` / `nohup` invocation
/// wrapper (GP-02a) — the run-environment prefixes that varied on every dogfood
/// invocation (`TMPDIR=… cargo test`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test`,
/// `env FOO=bar …`, `nohup … &`) and so never accumulated a matching grant.
///
/// The probe text and the assignment values differ on every run, so an
/// un-normalised slice never matches a prefix allowlist entry and a routine
/// gate command re-prompts forever. This is deliberately not a shell parser:
/// only these specific, conservatively-recognised forms are stripped, and
/// anything unrecognised is returned unchanged.
///
/// Normalisation is a pure rewrite performed BEFORE any classification, so the
/// danger-list ([`is_dangerous`]), the worktree-confinement rules, and the
/// protected-path rule all run against the normalised command and stripping can
/// never downgrade an escalation — `git push; echo $?` normalises to `git push`
/// and still escalates, and `FOO=bar rm -rf /` normalises to `rm -rf /` and
/// still escalates.
#[must_use]
pub fn normalize_command(slice: &str) -> String {
    let mut cmd = slice.trim();
    loop {
        if let Some(head) = strip_leading_prefix(cmd) {
            cmd = head.trim_start();
        } else if let Some(head) = strip_exit_probe(cmd) {
            cmd = head;
        } else if let Some(head) = strip_discard_redirect(cmd) {
            cmd = head;
        } else {
            break;
        }
    }
    cmd.to_string()
}

// ---------------------------------------------------------------------------
// Curated danger-list (Section 2)
// ---------------------------------------------------------------------------

/// Shared, OS-independent danger patterns. A command slice matching any of
/// these escalates to the human and is NEVER auto-approved — subject only to
/// the `rm -rf` scratch-path exception (see [`is_dangerous`] / [`is_scratch_rm`]).
///
/// Exported so unit tests and the bundled `sweep.sh` helper can assert parity
/// with the Rust classifier.
pub const DANGER_BASE: &[&str] = &[
    "rm -rf",
    "rm -fr",
    "git push",
    "--force",
    "force-push",
    "reset --hard",
    "git rebase",
    "git checkout ",
    "branch -D",
    "git worktree remove",
    "clean -fd",
    "clean -fdx",
    "sudo",
    "mkfs",
    "dd if=",
    "> /dev/",
    "chmod -R",
    "chown -R",
    "pkill",
    "kill",
];

/// macOS-specific danger addendum, compiled on macOS only.
///
/// `diskutil` and raw `/dev/disk*` writes are macOS-specific destructive
/// surfaces. macOS deletes targeting `/Volumes/…` and `rm -rf ~/Library/…`
/// are caught by the OS-independent `rm -rf` non-scratch rule.
#[cfg(target_os = "macos")]
#[must_use]
pub fn os_addendum() -> &'static [&'static str] {
    &["diskutil", "/dev/disk"]
}

/// Linux/WSL danger addendum, compiled on every non-macOS platform.
///
/// Windows is treated as WSL = Linux. Raw block devices (`/dev/sd*`,
/// `/dev/nvme*`) and filesystem creation (`mkfs*`) are the device-destroying
/// surface gated here.
#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn os_addendum() -> &'static [&'static str] {
    &["/dev/sd", "/dev/nvme", "mkfs"]
}

/// Returns `true` when `word` occurs in `haystack` bounded by word edges
/// (so `kill` does not match inside `skill` or `skills`).
fn contains_word(haystack: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(word) {
        let abs = start + pos;
        let before_ok = abs == 0 || !haystack[..abs].chars().next_back().is_some_and(is_word);
        let after = abs + word.len();
        let after_ok =
            after >= haystack.len() || !haystack[after..].chars().next().is_some_and(is_word);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
        if start >= haystack.len() {
            break;
        }
    }
    false
}

/// Matches a single danger pattern against a command slice.
///
/// Verb-like patterns (`sudo`, `kill`, `pkill`) use word-boundary matching to
/// avoid false positives inside larger identifiers; every other pattern is a
/// plain substring match.
fn danger_pattern_matches(slice: &str, pattern: &str) -> bool {
    match pattern {
        "sudo" | "kill" | "pkill" => contains_word(slice, pattern),
        _ => slice.contains(pattern),
    }
}

/// Returns `true` when the command slice matches the curated danger-list
/// (shared base + per-OS addendum) and must escalate to the human.
///
/// The `rm -rf` / `rm -fr` patterns are subject to the scratch-path exception:
/// a delete whose every target is repo/OS scratch does not escalate. Any other
/// danger pattern is a terminal escalate decision, even when it co-occurs with
/// a scratch-only `rm` in a compound command.
#[must_use]
pub fn is_dangerous(slice: &str) -> bool {
    let slice = slice.trim();
    for pattern in DANGER_BASE.iter().chain(os_addendum().iter()) {
        if danger_pattern_matches(slice, pattern) {
            if (*pattern == "rm -rf" || *pattern == "rm -fr") && rm_rf_all_targets_scratch(slice) {
                continue;
            }
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// rm -rf scratch-path exception (Section 3)
// ---------------------------------------------------------------------------

/// Returns `true` when `path` is a recognised repo/OS scratch location:
/// `/tmp/paw-*`, `/private/tmp/paw-*` (macOS symlinks `/tmp`→`/private/tmp`),
/// a `$TMPDIR`-rooted `paw-*` directory, or any path under `.git-paw/tmp/`.
#[must_use]
pub fn is_scratch_path(path: &str) -> bool {
    let p = path.trim().trim_matches(|c| c == '"' || c == '\'');
    if p.starts_with("/tmp/paw-") || p.starts_with("/private/tmp/paw-") {
        return true;
    }
    if p.starts_with(".git-paw/tmp/") || p.contains("/.git-paw/tmp/") {
        return true;
    }
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let base = tmpdir.trim_end_matches('/');
        if !base.is_empty() && p.starts_with(&format!("{base}/paw-")) {
            return true;
        }
    }
    false
}

/// Parses a `$VAR` / `${VAR}` reference, returning the bare variable name.
fn parse_var_ref(token: &str) -> Option<String> {
    let rest = token.strip_prefix('$')?;
    let rest = rest
        .strip_prefix('{')
        .map_or(rest, |r| r.trim_end_matches('}'));
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(rest.to_string())
    } else {
        None
    }
}

/// Resolves an `rm` target token to a concrete path, using a preceding
/// `VAR=value` assignment or the captured environment for `$VAR` references
/// and `$TMPDIR` substitution. Returns `None` when a variable cannot be
/// resolved — the caller treats that as "escalate" (fail-safe).
fn resolve_target(token: &str, assignments: &HashMap<String, String>) -> Option<String> {
    let t = token.trim_matches(|c| c == '"' || c == '\'');
    if let Some(name) = parse_var_ref(t) {
        if let Some(v) = assignments.get(&name) {
            return Some(v.clone());
        }
        return std::env::var(&name).ok();
    }
    if t.contains("$TMPDIR") {
        return std::env::var("TMPDIR")
            .ok()
            .map(|tmp| t.replace("$TMPDIR", tmp.trim_end_matches('/')));
    }
    Some(t.to_string())
}

/// Extracts the resolved removal targets of an `rm` command slice, or `None`
/// when any target is an unresolvable variable. Leading `VAR=value`
/// assignments feed `$VAR` resolution; parsing stops at the first command
/// separator so only the `rm`'s own targets are considered.
fn rm_rf_targets(slice: &str) -> Option<Vec<String>> {
    let mut assignments: HashMap<String, String> = HashMap::new();
    let mut targets: Vec<String> = Vec::new();
    let mut seen_rm = false;
    for tok in slice.split_whitespace() {
        if matches!(tok, "&&" | "||" | ";" | "|") {
            break;
        }
        if !seen_rm {
            if let Some((k, v)) = tok.split_once('=')
                && !k.is_empty()
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                assignments.insert(k.to_string(), v.to_string());
                continue;
            }
            if tok == "rm" {
                seen_rm = true;
            }
            continue;
        }
        if tok.starts_with('-') {
            continue;
        }
        targets.push(resolve_target(tok, &assignments)?);
    }
    Some(targets)
}

/// Returns `true` when an `rm` command's every target is scratch (and there is
/// at least one target). An unresolved variable or zero targets returns
/// `false` (escalate).
fn rm_rf_all_targets_scratch(slice: &str) -> bool {
    match rm_rf_targets(slice) {
        Some(targets) if !targets.is_empty() => targets.iter().all(|t| is_scratch_path(t)),
        _ => false,
    }
}

/// Returns `true` when the slice is an `rm -rf` / `rm -fr` whose every target
/// is scratch AND which carries no other danger pattern — the scratch
/// exception classifies it safe-by-pattern rather than escalating.
#[must_use]
pub fn is_scratch_rm(slice: &str) -> bool {
    let slice = slice.trim();
    if !slice.contains("rm -rf") && !slice.contains("rm -fr") {
        return false;
    }
    rm_rf_all_targets_scratch(slice) && !is_dangerous(slice)
}

// ---------------------------------------------------------------------------
// Protected-path set (agent-memory-isolation)
// ---------------------------------------------------------------------------

/// Config-driven protected-path set covering operator configuration and
/// memory territory (`agent-memory-isolation`).
///
/// The set derives from configuration and well-known defaults — never from
/// hardcoded CLI product names (the export-agnosticism policy). Sources:
///
/// - the documented claude-format default config dir (`~/.claude`);
/// - the directory named by `CLAUDE_CONFIG_DIR` when set;
/// - the parent directory of every configured `[clis.<name>].settings_path`;
/// - the `projects/**/memory` subtrees beneath the directories above;
/// - the host repository root's `.claude/` and `.git-paw/` directories
///   (the agent's own worktree subtree is carved out at match time, so an
///   embedded worktree under `.git-paw/worktrees/` is unaffected).
///
/// Entries are canonicalized at derivation time when they exist; entries
/// whose paths do not exist keep their lexically-normalized absolute form
/// and are still matched syntactically (fail-closed).
///
/// The default value is the EMPTY set (matches nothing) — production callers
/// build the real set via [`ProtectedPaths::derive`].
#[derive(Debug, Clone, Default)]
pub struct ProtectedPaths {
    /// Absolute directory prefixes: the lexically-normalized spelling of each
    /// source, plus its canonical spelling when the two differ.
    entries: Vec<PathBuf>,
    /// Home directory captured at derivation time, used to expand `~` in
    /// match targets deterministically.
    home: Option<PathBuf>,
}

impl ProtectedPaths {
    /// Derives the protected-path set from the loaded config, the process
    /// environment (`CLAUDE_CONFIG_DIR`), and the home directory.
    ///
    /// `repo_root` is the host repository root; pass it whenever known so the
    /// repo-level `.claude/` and `.git-paw/` control directories join the set.
    #[must_use]
    pub fn derive(config: &crate::config::PawConfig, repo_root: Option<&Path>) -> Self {
        let settings_paths: Vec<String> = config
            .clis
            .values()
            .filter_map(|c| c.settings_path.clone())
            .collect();
        Self::derive_from_parts(
            &settings_paths,
            std::env::var("CLAUDE_CONFIG_DIR").ok().as_deref(),
            crate::dirs::home_dir(),
            repo_root,
        )
    }

    /// Testable core of [`ProtectedPaths::derive`]: the environment inputs
    /// (`CLAUDE_CONFIG_DIR` value, home directory) are passed explicitly so
    /// unit tests never mutate process-global state.
    fn derive_from_parts(
        settings_paths: &[String],
        claude_config_dir: Option<&str>,
        home: Option<PathBuf>,
        repo_root: Option<&Path>,
    ) -> Self {
        let mut sources: Vec<PathBuf> = Vec::new();
        // The documented claude-format default config dir — the only
        // built-in; every other entry traces to config or environment.
        if let Some(h) = &home {
            sources.push(h.join(".claude"));
        }
        if let Some(dir) = claude_config_dir {
            let dir = dir.trim();
            if !dir.is_empty() {
                sources.push(expand_tilde(dir, home.as_deref()));
            }
        }
        for raw in settings_paths {
            let expanded = expand_tilde(raw, home.as_deref());
            if let Some(parent) = expanded.parent() {
                sources.push(parent.to_path_buf());
            }
        }
        // Existing `projects/*/memory` subtrees beneath the config dirs.
        // Their protected parents already cover them by prefix; enumerating
        // the memory territory keeps the set explicit for auditability.
        let memory: Vec<PathBuf> = sources.iter().flat_map(|d| memory_dirs_under(d)).collect();
        sources.extend(memory);
        if let Some(root) = repo_root {
            sources.push(root.join(".claude"));
            sources.push(root.join(".git-paw"));
        }

        let mut entries: Vec<PathBuf> = Vec::new();
        for src in sources {
            let lexical = lexical_normalize(&src);
            if !entries.contains(&lexical) {
                entries.push(lexical);
            }
            // Canonical spelling via the deepest EXISTING ancestor, so an
            // entry that does not exist yet still gets the same spelling a
            // resolvable target will resolve to (e.g. a symlinked temp dir).
            if let Some(resolved) = resolve_for_boundary(&src) {
                let resolved = lexical_normalize(&resolved);
                if !entries.contains(&resolved) {
                    entries.push(resolved);
                }
            }
        }
        Self { entries, home }
    }

    /// Returns `true` when `dir` is an entry of the derived set (compared by
    /// lexical or canonical spelling). Exposed so callers and tests can audit
    /// what the derivation produced.
    #[must_use]
    pub fn contains_dir(&self, dir: &Path) -> bool {
        let lexical = lexical_normalize(dir);
        if self.entries.contains(&lexical) {
            return true;
        }
        dir.canonicalize().is_ok_and(|c| self.entries.contains(&c))
    }

    /// Returns `true` when `target` resolves inside the protected set —
    /// excluding anything inside the agent's own worktree.
    ///
    /// `target` may be relative (resolved against `worktree_root`) or
    /// `~`-prefixed (expanded against the derivation-time home directory).
    /// Resolution is fail-closed: a target that cannot be canonicalized is
    /// collapsed lexically, so a `..`/`~` spelling that syntactically reaches
    /// into the protected set still matches. A relative target with no known
    /// worktree root never matches (other classification rules decide).
    #[must_use]
    pub fn matches_target(&self, target: &str, worktree_root: Option<&Path>) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let cleaned = target
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '`');
        if cleaned.is_empty() {
            return false;
        }
        let expanded = expand_tilde(cleaned, self.home.as_deref());
        let joined = if expanded.is_absolute() {
            expanded
        } else if let Some(root) = worktree_root {
            root.join(expanded)
        } else {
            return false;
        };
        let resolved = resolve_for_boundary(&joined).unwrap_or_else(|| lexical_normalize(&joined));
        // The agent's own worktree subtree is never protected — in-worktree
        // writes stay governed by the worktree-boundary rules. This is also
        // what carves an embedded worktree out of the repo-root `.git-paw/`
        // entry.
        if let Some(root) = worktree_root {
            let root_resolved = root
                .canonicalize()
                .unwrap_or_else(|_| lexical_normalize(root));
            if resolved.starts_with(&root_resolved) {
                return false;
            }
        }
        self.entries.iter().any(|e| resolved.starts_with(e))
    }
}

/// Expands a leading `~` / `~/` against `home`. Any other form (including
/// `~user`, which the CLIs git-paw drives never emit) is returned as-is.
fn expand_tilde(raw: &str, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from(raw);
    };
    if raw == "~" {
        return home.to_path_buf();
    }
    match raw.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(raw),
    }
}

/// Collapses `.` / `..` components lexically — no filesystem access — for the
/// fail-closed syntactic matching of paths that cannot be canonicalized.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                // Popping past the root leaves the root in place, matching
                // how the OS resolves `/..`.
                let _ = out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Enumerates the existing `projects/*/memory` directories beneath `dir` —
/// the per-project memory territory of a claude-format config dir.
fn memory_dirs_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(projects) = std::fs::read_dir(dir.join("projects")) else {
        return out;
    };
    for entry in projects.flatten() {
        let memory = entry.path().join("memory");
        if memory.is_dir() {
            out.push(memory);
        }
    }
    out
}

/// Write-target extraction for the protected-path rule: the candidate paths a
/// command slice writes to. Covers redirection targets (`>`, `>>`, attached
/// and fd-prefixed forms) and the targets of common mutating verbs (`tee`,
/// `cp`/`mv`/`ln` destinations, `touch`, `mkdir`, `rm`, `rmdir`, `truncate`,
/// and in-place `sed -i`). Read-only operations contribute no targets.
fn slice_write_targets(slice: &str) -> Vec<String> {
    let mut targets: Vec<String> = Vec::new();
    let mut segment: Vec<&str> = Vec::new();
    for tok in slice.split_whitespace() {
        if matches!(tok, "&&" | "||" | ";" | "|") {
            collect_segment_write_targets(&segment, &mut targets);
            segment.clear();
        } else {
            segment.push(tok);
        }
    }
    collect_segment_write_targets(&segment, &mut targets);
    targets
}

/// Collects the write targets of one pipeline segment into `out`.
fn collect_segment_write_targets(tokens: &[&str], out: &mut Vec<String>) {
    // Redirection targets anywhere in the segment; the remaining tokens feed
    // the mutating-verb scan below.
    let mut redirect_pending = false;
    let mut rest: Vec<&str> = Vec::new();
    for tok in tokens {
        if redirect_pending {
            out.push((*tok).to_string());
            redirect_pending = false;
            continue;
        }
        let stripped = tok.trim_start_matches(|c: char| c.is_ascii_digit() || c == '&');
        if let Some(after) = stripped.strip_prefix('>') {
            let after = after.strip_prefix('>').unwrap_or(after);
            if after.is_empty() {
                redirect_pending = true;
            } else if !after.starts_with('&') {
                // `2>&1`-style fd duplication has no path target.
                out.push(after.to_string());
            }
            continue;
        }
        rest.push(tok);
    }

    // Mutating-verb targets, with leading VAR=value assignments skipped
    // (mirroring `leading_verb`).
    let mut toks = rest.iter().copied().skip_while(|tok| {
        tok.split_once('=').is_some_and(|(k, _)| {
            !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    });
    let Some(first) = toks.next() else {
        return;
    };
    let verb = first.rsplit('/').next().unwrap_or(first);
    let args: Vec<&str> = toks.collect();
    let paths: Vec<&str> = args
        .iter()
        .filter(|t| !t.starts_with('-'))
        .copied()
        .collect();
    match verb {
        "tee" | "touch" | "mkdir" | "rm" | "rmdir" | "truncate" => {
            out.extend(paths.iter().map(|p| (*p).to_string()));
        }
        // Only the destination (last path argument) is a write target — the
        // source of a `cp`/`mv` is a read.
        "cp" | "mv" | "ln" => {
            if paths.len() >= 2
                && let Some(last) = paths.last()
            {
                out.push((*last).to_string());
            }
        }
        // In-place edit only; a plain `sed` is a read.
        "sed" if args.iter().any(|t| t.starts_with("-i")) => {
            out.extend(paths.iter().map(|p| (*p).to_string()));
        }
        _ => {}
    }
}

/// Returns `true` when a filesystem prompt or its command slice targets a
/// write inside the protected set — the danger-class protected-path rule of
/// `safe-command-classification`.
///
/// Callers evaluate this at the same precedence as the curated danger-list:
/// a match is a terminal escalation, never auto-approved. Reads never match
/// (only write targets are extracted). `worktree_root` carves out the
/// agent's own worktree.
#[must_use]
pub fn is_protected_path_violation(
    captured: &str,
    slice: &str,
    protected: &ProtectedPaths,
    worktree_root: Option<&Path>,
) -> bool {
    if let Some(path) = extract_path_from_file_prompt(captured)
        && protected.matches_target(&path, worktree_root)
    {
        return true;
    }
    slice_write_targets(slice)
        .iter()
        .any(|t| protected.matches_target(t, worktree_root))
}

// ---------------------------------------------------------------------------
// `.git/`-write danger rule (GP-04b)
// ---------------------------------------------------------------------------

/// Returns `true` when `path` resolves to a location strictly inside a
/// `.git/` directory — a component named exactly `.git` followed by at least
/// one further component, e.g. `.git/config`, `.git/info/exclude`,
/// `.git/hooks/pre-commit`.
///
/// Resolution mirrors the worktree-boundary check: canonicalise via the
/// deepest existing ancestor when possible ([`resolve_for_boundary`]),
/// falling back to a lexical `.`/`..` collapse (fail-closed) so a target that
/// cannot be canonicalized but syntactically reaches into `.git/` still
/// matches. A relative `path` is resolved against `worktree_root` when known;
/// with no root known it is matched against its own lexical form, so the
/// rule still applies to the common relative-path case.
fn is_inside_dot_git(path: &str, worktree_root: Option<&Path>) -> bool {
    let cleaned = path
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`');
    if cleaned.is_empty() {
        return false;
    }
    let target = Path::new(cleaned);
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else if let Some(root) = worktree_root {
        root.join(target)
    } else {
        PathBuf::from(cleaned)
    };
    let resolved = resolve_for_boundary(&joined).unwrap_or_else(|| lexical_normalize(&joined));
    let mut saw_git_dir = false;
    for comp in resolved.components() {
        if saw_git_dir {
            return true;
        }
        if comp.as_os_str() == ".git" {
            saw_git_dir = true;
        }
    }
    false
}

/// Returns `true` when a filesystem prompt or its command slice targets a
/// write inside a repository `.git/` directory — the danger-class rule of
/// `approval-command-safety` (GP-04b). Callers evaluate this at the same
/// precedence as the curated danger-list: a match is a terminal escalation,
/// never auto-approved.
///
/// Read-only operations never match (only write targets are extracted via
/// [`slice_write_targets`], mirroring [`is_protected_path_violation`]).
/// Ordinary `git` subcommands (`git commit`, `git add`, …) are classified by
/// the git-verb rules and are unaffected — `git` is not among the mutating
/// verbs [`slice_write_targets`] extracts targets for.
#[must_use]
pub fn is_git_dir_write(captured: &str, slice: &str, worktree_root: Option<&Path>) -> bool {
    if let Some(path) = extract_path_from_file_prompt(captured)
        && is_inside_dot_git(&path, worktree_root)
    {
        return true;
    }
    slice_write_targets(slice)
        .iter()
        .any(|t| is_inside_dot_git(t, worktree_root))
}

// ---------------------------------------------------------------------------
// Live-prompt gate (Section 6)
// ---------------------------------------------------------------------------

/// Trailing non-blank lines that must ANCHOR the prompt: a textual marker or
/// a numbered option line must sit within this window for the capture to be
/// live. Output printed after an answered prompt evicts it from the anchor,
/// so a prompt scrolled into history never counts.
pub const LIVE_PROMPT_TAIL: usize = 4;

/// Widest window of trailing non-blank lines inspected for the prompt's
/// textual markers — wide enough to span a full multi-option prompt block
/// (`Do you want to proceed?` + numbered options, possibly wrapped, + the
/// footer), so a prompt offering a "don't ask again" option is detected
/// rather than missed by a narrow tail.
pub const LIVE_PROMPT_BLOCK: usize = 15;

/// Shared structural live-prompt check: `true` when a textual marker sits in
/// the last [`LIVE_PROMPT_TAIL`] non-blank lines, OR a numbered option line
/// anchors that tail while a textual marker sits within the last
/// [`LIVE_PROMPT_BLOCK`] non-blank lines (a multi-option prompt bottoms out
/// in its option list, with the question above it).
///
/// Textual markers are matched case-insensitively as substrings. This is the
/// one structural algorithm behind [`is_live_prompt`] and the
/// `broker-mediated-approvals` re-confirm
/// ([`crate::supervisor::approval_gate::live_prompt_in_tail`]), so detection
/// and send-time re-confirm agree on what "live" means.
#[must_use]
pub fn live_prompt_markers_at_tail(capture: &str, textual_markers: &[&str]) -> bool {
    let nonblank: Vec<&str> = capture.lines().filter(|l| !l.trim().is_empty()).collect();
    let has_textual = |lines: &[&str]| {
        lines.iter().any(|line| {
            let lower = line.to_ascii_lowercase();
            textual_markers
                .iter()
                .any(|m| lower.contains(&m.to_ascii_lowercase()))
        })
    };
    let tail = &nonblank[nonblank.len().saturating_sub(LIVE_PROMPT_TAIL)..];
    if has_textual(tail) {
        return true;
    }
    let block = &nonblank[nonblank.len().saturating_sub(LIVE_PROMPT_BLOCK)..];
    tail.iter().any(|l| is_option_line(strip_decoration(l))) && has_textual(block)
}

/// Returns `true` when the capture shows a LIVE prompt: its structural
/// markers — the numbered option glyphs and/or the `Do you want to …`
/// question, together with the `Esc to cancel` footer — are present at the
/// capture's tail, over a window wide enough to span a full multi-option
/// prompt block (see [`live_prompt_markers_at_tail`]).
///
/// This is the precondition for any keystroke dispatch — a supervisor merely
/// narrating about a pane, or an earlier prompt that has scrolled away, will
/// not have the markers in the live window and so cannot trip a phantom
/// approval.
///
/// Sources its markers from the embedded Claude Code profile's
/// `live_prompt_markers` (`cli-prompt-profiles` capability).
#[must_use]
pub fn is_live_prompt(capture: &str) -> bool {
    let markers: Vec<&str> = claude_prompt_profile()
        .live_prompt_markers
        .iter()
        .map(String::as_str)
        .collect();
    live_prompt_markers_at_tail(capture, &markers)
}

// ---------------------------------------------------------------------------
// Option-index selection and broad-grant rule (Section 7)
// ---------------------------------------------------------------------------

/// Shape of a detected permission prompt's option list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptShape {
    /// A 2-option `Yes` / `No` prompt. Option 1 is `Yes`.
    TwoOption,
    /// A 3-option `Yes` / `Yes, and don't ask again` / `No` prompt. Option 2
    /// is the permanent broad grant.
    ThreeOption,
}

/// Detects the prompt shape: [`PromptShape::ThreeOption`] when a permanent
/// broad-grant option (a "don't ask again" choice) is present, otherwise
/// [`PromptShape::TwoOption`].
///
/// Sources its marker from the embedded Claude Code profile's
/// `broad_grant_marker` (`cli-prompt-profiles` capability).
#[must_use]
pub fn detect_prompt_shape(capture: &str) -> PromptShape {
    let lower = capture.to_ascii_lowercase();
    let marker = claude_prompt_profile()
        .broad_grant_marker
        .to_ascii_lowercase();
    // Match both the ASCII apostrophe and the Unicode right-single-quote that
    // some terminals render.
    let unicode_apostrophe = marker.replace('\'', "\u{2019}");
    if lower.contains(&marker) || lower.contains(&unicode_apostrophe) {
        PromptShape::ThreeOption
    } else {
        PromptShape::TwoOption
    }
}

/// Returns the leading command verb (basename), skipping any leading
/// `VAR=value` environment-assignment tokens.
fn leading_verb(slice: &str) -> Option<&str> {
    for tok in slice.split_whitespace() {
        if let Some((k, _)) = tok.split_once('=')
            && !k.is_empty()
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        return Some(tok.rsplit('/').next().unwrap_or(tok));
    }
    None
}

/// Returns `true` when the command slice runs arbitrary code: a `python`,
/// `python3`, `node`, or `eval` verb, or a `bash -c` / `sh -c` / bare ` -c `
/// code-string flag. Such commands NEVER receive a permanent broad grant.
#[must_use]
pub fn is_arbitrary_code_runner(slice: &str) -> bool {
    if matches!(
        leading_verb(slice),
        Some("python" | "python3" | "node" | "eval")
    ) {
        return true;
    }
    slice.contains("bash -c") || slice.contains("sh -c") || slice.contains(" -c ")
}

/// Returns `true` when the slice's leading verb is in [`READ_MOSTLY_VERBS`].
fn verb_is_read_mostly(slice: &str) -> bool {
    leading_verb(slice).is_some_and(|v| READ_MOSTLY_VERBS.contains(&v))
}

/// Selects the 1-based option index to dispatch for a `slice` at a prompt of
/// the given `shape`.
///
/// - 2-option → option 1 (`Yes`); there is no durable option to prefer.
/// - 3-option → option 2 (the permanent broad grant) when the slice is NOT an
///   arbitrary-code runner AND either the caller has already classified the
///   command safe / worktree-confined (`classified_safe`) or the slice's verb is
///   read-mostly-allowlisted; otherwise option 1 (one-time `Yes`).
///
/// `classified_safe` is what makes a routine prompt permanently allowed instead
/// of re-prompting on every identical occurrence: a stack command such as
/// `cargo test` classifies safe through the resolved allowlist even though
/// `cargo` is not a read-mostly verb. Callers that have not classified the
/// command pass `false` and get the narrower read-mostly rule unchanged. An
/// arbitrary-code runner NEVER receives a durable grant, whatever the
/// classification.
#[must_use]
pub fn select_option_index(shape: PromptShape, slice: &str, classified_safe: bool) -> u8 {
    match shape {
        PromptShape::TwoOption => 1,
        PromptShape::ThreeOption => {
            if is_arbitrary_code_runner(slice) {
                1
            } else if classified_safe || verb_is_read_mostly(slice) {
                2
            } else {
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec scenario "Default whitelist is stack-neutral": the built-ins carry
    /// only stack-neutral entries — no toolchain verbs.
    #[test]
    fn defaults_contain_documented_classes() {
        let defaults = default_safe_commands();
        assert!(defaults.contains(&"git commit"));
        assert!(defaults.contains(&"git push"));
        assert!(defaults.contains(&"curl http://127.0.0.1:"));
        // Previously hardcoded stack-specific entries are no longer built in.
        for gone in [
            "cargo fmt",
            "cargo clippy",
            "cargo test",
            "cargo build",
            "openspec",
            "just",
        ] {
            assert!(
                !defaults.contains(&gone),
                "built-ins must be stack-neutral; found {gone}"
            );
        }
    }

    #[test]
    fn prefix_match_accepts_flag_variations() {
        let whitelist = vec!["cargo test".to_string()];
        assert!(is_safe_command(
            "cargo test --no-run --workspace",
            &whitelist
        ));
        assert!(is_safe_command("cargo test", &whitelist));
    }

    #[test]
    fn prefix_match_requires_word_boundary() {
        let whitelist = vec!["cargo test".to_string()];
        assert!(
            !is_safe_command("cargotest --foo", &whitelist),
            "no whitespace boundary should fail"
        );
    }

    #[test]
    fn unknown_command_is_not_safe() {
        let whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        assert!(!is_safe_command("rm -rf /tmp/foo", &whitelist));
    }

    #[test]
    fn config_extras_extend_whitelist() {
        let mut whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        whitelist.push("just smoke".to_string());
        assert!(is_safe_command("just smoke -v", &whitelist));
    }

    #[test]
    fn empty_extras_keeps_defaults() {
        let whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        assert!(is_safe_command("grep -rn \"foo\" src/", &whitelist));
        assert!(is_safe_command("git commit -m hi", &whitelist));
    }

    #[test]
    fn leading_whitespace_is_tolerated() {
        let whitelist = vec!["cargo fmt".to_string()];
        assert!(is_safe_command("   cargo fmt --check", &whitelist));
    }

    // --- Bug 3: worktree file-operation classifier ---

    use tempfile::TempDir;

    #[test]
    fn extracts_path_from_write_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to Containerfile?"),
            Some("Containerfile".to_string())
        );
    }

    #[test]
    fn extracts_path_from_edit_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to make this edit to src/main.rs?"),
            Some("src/main.rs".to_string())
        );
    }

    #[test]
    fn extracts_absolute_path_from_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to /etc/hosts?"),
            Some("/etc/hosts".to_string())
        );
    }

    #[test]
    fn shell_prompt_yields_no_file_path() {
        // A shell-command prompt must not be mistaken for a file op.
        assert_eq!(
            extract_path_from_file_prompt("Do you want to proceed?\ncargo build --release"),
            None
        );
    }

    /// Spec scenario: In-worktree file create is auto-approved.
    #[test]
    fn in_worktree_file_create_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Containerfile does not exist yet — the create case.
        let prompt = "Do you want to allow this write to Containerfile?";
        assert!(is_worktree_file_op(prompt, root, true));
    }

    /// Spec scenario: Out-of-worktree file create requires manual approval.
    #[test]
    fn out_of_worktree_path_is_not_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let prompt = "Do you want to allow this write to /etc/hosts?";
        assert!(!is_worktree_file_op(prompt, root, true));
    }

    /// Spec scenario: Symlink/`..`-escape attempt does not bypass the boundary.
    #[test]
    fn dotdot_escape_attempt_is_rejected() {
        let tmp = TempDir::new().unwrap();
        // Nest the worktree so `../../` resolves above it but still exists.
        let root = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&root).unwrap();
        let prompt = "Do you want to allow this write to ../../escaped.txt?";
        assert!(!is_worktree_file_op(prompt, &root, true));
    }

    #[test]
    fn nested_in_worktree_path_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let prompt = "Do you want to make this edit to deep/nested/new_file.rs?";
        assert!(is_worktree_file_op(prompt, root, true));
    }

    /// Task 1.6 / spec scenario: shell auto-approve is unaffected — a shell
    /// prompt is never classified as a worktree file op.
    #[test]
    fn shell_prompt_is_not_a_worktree_file_op() {
        let tmp = TempDir::new().unwrap();
        let prompt = "Do you want to proceed?\ncargo test --workspace";
        assert!(!is_worktree_file_op(prompt, tmp.path(), true));
        // The shell whitelist still matches it.
        assert!(is_safe_command(
            "cargo test --workspace",
            &["cargo test".to_string()]
        ));
    }

    /// Task 2.3 / spec scenario: explicit `approve_worktree_writes = false`
    /// reverts a worktree-confined prompt to manual.
    #[test]
    fn disabled_flag_rejects_in_worktree_path() {
        let tmp = TempDir::new().unwrap();
        let prompt = "Do you want to allow this write to Containerfile?";
        assert!(
            !is_worktree_file_op(prompt, tmp.path(), false),
            "approve_worktree_writes=false must not classify any file op as safe"
        );
    }

    // --- Section 1: command-slice extraction ---

    /// Task 1.2 / spec scenario "Narration about a dangerous command is not
    /// classified as danger": prose mentioning `rm -rf /` elsewhere in the
    /// capture is ignored; the extracted slice is the live `cargo test`.
    #[test]
    fn command_slice_ignores_narration_prose() {
        let capture = "\
I will avoid running rm -rf / because it is destructive.
Let me run the tests instead.

  Bash command
    cargo test --workspace
    Run the test suite

  Do you want to proceed?
  ❯ 1. Yes
    2. No";
        let slice = extract_command_slice(capture).expect("a command slice");
        assert_eq!(slice, "cargo test --workspace");
        assert!(!is_dangerous(&slice), "extracted slice must not be danger");
    }

    #[test]
    fn command_slice_reads_bash_paren_form() {
        let capture = "Bash(git push origin main)\n  Push the branch\nDo you want to proceed?";
        assert_eq!(
            extract_command_slice(capture),
            Some("git push origin main".to_string())
        );
    }

    #[test]
    fn command_slice_none_without_header() {
        assert_eq!(extract_command_slice("just some prose\n$ ls\n"), None);
    }

    #[test]
    fn command_slice_prefers_last_header() {
        let capture = "\
Bash command
  git status

Bash command
  git push origin main
Do you want to proceed?";
        assert_eq!(
            extract_command_slice(capture),
            Some("git push origin main".to_string())
        );
    }

    // --- Section 2: curated danger-list ---

    #[test]
    fn force_push_escalates() {
        assert!(is_dangerous("git push --force origin main"));
    }

    #[test]
    fn hard_reset_escalates() {
        assert!(is_dangerous("git reset --hard HEAD~3"));
    }

    #[test]
    fn branch_switch_escalates() {
        assert!(is_dangerous("git checkout main"));
    }

    #[test]
    fn rebase_and_branch_delete_and_worktree_remove_escalate() {
        assert!(is_dangerous("git rebase -i HEAD~2"));
        assert!(is_dangerous("git branch -D feature"));
        assert!(is_dangerous("git worktree remove ../wt"));
        assert!(is_dangerous("git clean -fdx"));
    }

    #[test]
    fn privileged_and_device_commands_escalate() {
        assert!(is_dangerous("sudo apt install x"));
        assert!(is_dangerous("dd if=/dev/zero of=disk.img"));
        assert!(is_dangerous("chmod -R 777 /etc"));
        assert!(is_dangerous("chown -R root /srv"));
        assert!(is_dangerous("cat secrets > /dev/sda"));
    }

    #[test]
    fn process_killing_commands_escalate() {
        assert!(is_dangerous("pkill -9 node"));
        assert!(is_dangerous("kill -9 1234"));
    }

    /// Word-boundary matching: `kill`/`sudo` must not fire inside larger
    /// identifiers such as `skills` or `sudoers`.
    #[test]
    fn kill_and_sudo_do_not_false_match_substrings() {
        assert!(!is_dangerous("cat src/skills.rs"));
        assert!(!is_dangerous("grep skill docs/"));
        assert!(!is_dangerous("cat /etc/sudoers"));
    }

    /// Spec scenario "Danger match overrides a whitelist match": `git` is a
    /// read-mostly safe verb yet `git push` is danger — danger wins.
    #[test]
    fn danger_overrides_whitelisted_git_verb() {
        let whitelist = vec!["git".to_string()];
        assert!(
            is_safe_command("git push origin main", &whitelist),
            "git verb matches the whitelist in isolation"
        );
        assert!(
            is_dangerous("git push origin main"),
            "but the danger-list must escalate it"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_diskutil_escalates_on_macos() {
        assert!(is_dangerous("diskutil eraseDisk JHFS+ x /dev/disk2"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn linux_device_write_escalates_on_linux() {
        assert!(is_dangerous("dd if=image.iso of=/dev/sda"));
        assert!(is_dangerous("dd if=image.iso of=/dev/nvme0n1"));
    }

    // --- Section 3: rm -rf scratch-path exception ---

    #[test]
    fn scratch_tmp_paw_delete_is_safe() {
        let slice = "rm -rf /tmp/paw-build-123";
        assert!(!is_dangerous(slice), "scratch delete must not escalate");
        assert!(is_scratch_rm(slice), "scratch delete classifies safe");
    }

    #[test]
    fn scratch_private_tmp_paw_delete_is_safe() {
        let slice = "rm -rf /private/tmp/paw-cache";
        assert!(!is_dangerous(slice));
        assert!(is_scratch_rm(slice));
    }

    #[test]
    fn scratch_repo_local_delete_is_safe() {
        let slice = "rm -rf .git-paw/tmp/wave-7";
        assert!(!is_dangerous(slice));
        assert!(is_scratch_rm(slice));
    }

    #[test]
    fn scratch_var_resolves_to_scratch_is_safe() {
        let slice = "SCRATCH=/tmp/paw-x rm -rf \"$SCRATCH\"";
        assert!(!is_dangerous(slice));
        assert!(is_scratch_rm(slice));
    }

    #[test]
    fn non_scratch_rm_escalates() {
        let slice = "rm -rf ~/Documents";
        assert!(is_dangerous(slice), "non-scratch delete must escalate");
        assert!(!is_scratch_rm(slice));
    }

    #[test]
    fn mixed_scratch_and_non_scratch_targets_escalate() {
        let slice = "rm -rf /tmp/paw-x /etc/important";
        assert!(is_dangerous(slice), "any non-scratch target escalates");
        assert!(!is_scratch_rm(slice));
    }

    #[test]
    fn unresolved_var_in_rm_escalates_fail_safe() {
        // `$NOPE` is bound by neither an assignment nor (in test) the env.
        let slice = "rm -rf \"$NOPE_UNSET_VAR_XYZ\"";
        assert!(is_dangerous(slice), "unresolved var must fail safe");
        assert!(!is_scratch_rm(slice));
    }

    // --- Section 4: read-mostly verb allowlist ---

    /// Spec scenario "Read-mostly verb is whitelisted": a `grep`/`cat`/`ls`
    /// command classifies safe; `rm` is deliberately excluded.
    #[test]
    fn read_mostly_verb_classifies_safe() {
        let whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        assert!(is_safe_command("grep -rn \"foo\" src/", &whitelist));
        assert!(is_safe_command("cat src/main.rs", &whitelist));
        assert!(is_safe_command("ls -la", &whitelist));
        assert!(is_safe_command("rg pattern", &whitelist));
        assert!(
            !is_safe_command("rm -rf /tmp/foo", &whitelist),
            "rm is not a read-mostly verb"
        );
    }

    /// Guards against drift between [`READ_MOSTLY_VERBS`] and the entries baked
    /// into [`default_safe_commands`], and against stack-specific verbs
    /// creeping back into the read-mostly set.
    #[test]
    fn read_mostly_verbs_are_whitelisted() {
        let defaults = default_safe_commands();
        for verb in READ_MOSTLY_VERBS {
            assert!(
                defaults.contains(verb),
                "{verb} must be in default_safe_commands"
            );
        }
        for gone in ["openspec", "just", "cargo"] {
            assert!(
                !READ_MOSTLY_VERBS.contains(&gone),
                "read-mostly verbs must stay stack-neutral; found {gone}"
            );
        }
    }

    // --- Section 5: worktree-confined git add / git commit ---

    /// Spec scenario "Worktree git commit auto-approves".
    #[test]
    fn in_worktree_git_commit_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        assert!(is_worktree_git_op("git commit -m \"feat: x\"", tmp.path()));
    }

    /// Spec scenario "Worktree git add auto-approves".
    #[test]
    fn in_worktree_git_add_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        assert!(is_worktree_git_op("git add -A", tmp.path()));
    }

    /// Spec scenario "git push still escalates despite worktree confinement":
    /// `git push` is neither an add/commit nor exempt from the danger-list.
    #[test]
    fn git_push_is_not_a_worktree_git_op_and_escalates() {
        let tmp = TempDir::new().unwrap();
        assert!(
            !is_worktree_git_op("git push origin main", tmp.path()),
            "git push is not an add/commit pre-approval"
        );
        assert!(
            is_dangerous("git push origin main"),
            "git push escalates via the danger-list"
        );
    }

    #[test]
    fn git_status_is_not_a_worktree_git_op() {
        // Reads are handled by the read-mostly verb path, not the add/commit
        // pre-approval.
        let tmp = TempDir::new().unwrap();
        assert!(!is_worktree_git_op("git status", tmp.path()));
    }

    // --- Rider: worktree-confined dev-test commands ---

    /// Spec scenario "bash -n on a worktree script is safe".
    #[test]
    fn bash_n_on_worktree_script_is_safe() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("scripts")).unwrap();
        std::fs::write(tmp.path().join("scripts/helper.sh"), "echo hi\n").unwrap();
        assert!(is_worktree_dev_test_op(
            "bash -n scripts/helper.sh",
            tmp.path()
        ));
    }

    /// Spec scenario "chmod on own file is safe, recursive stays danger".
    #[test]
    fn chmod_on_worktree_file_is_safe_recursive_stays_danger() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("scripts")).unwrap();
        std::fs::write(tmp.path().join("scripts/helper.sh"), "echo hi\n").unwrap();
        assert!(is_worktree_dev_test_op(
            "chmod +x scripts/helper.sh",
            tmp.path()
        ));
        // `chmod -R` matches the danger-list (which the callers evaluate
        // first) and never matches the dev-test rule either.
        assert!(is_dangerous("chmod -R 755 ."));
        assert!(!is_worktree_dev_test_op("chmod -R 755 .", tmp.path()));
    }

    /// Spec scenario "mktemp is safe".
    #[test]
    fn mktemp_is_safe() {
        let tmp = TempDir::new().unwrap();
        assert!(is_worktree_dev_test_op("mktemp -d", tmp.path()));
        assert!(is_worktree_dev_test_op("mktemp", tmp.path()));
        // A template path argument does not match (fail-closed).
        assert!(!is_worktree_dev_test_op(
            "mktemp /etc/passwd.XXXXXX",
            tmp.path()
        ));
    }

    /// Spec scenario "Interpreter run of a worktree script is one-time safe":
    /// the slice classifies safe-by-pattern, and on a 3-option prompt the
    /// selector picks the one-time option, never the permanent broad grant.
    #[test]
    fn interpreter_run_of_worktree_script_is_one_time_safe() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("tools")).unwrap();
        std::fs::write(tmp.path().join("tools/gen.py"), "print(1)\n").unwrap();
        assert!(is_worktree_dev_test_op("python3 tools/gen.py", tmp.path()));
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "python3 tools/gen.py", true),
            1,
            "interpreter runs must take the one-time option even once classified safe"
        );
    }

    /// Spec scenario "Inline code strings do not match".
    #[test]
    fn inline_code_strings_do_not_match_dev_test_rules() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_worktree_dev_test_op(
            "python3 -c \"import os\"",
            tmp.path()
        ));
        assert!(!is_worktree_dev_test_op("bash -c \"do-thing\"", tmp.path()));
    }

    /// Spec scenario "Out-of-worktree script does not match".
    #[test]
    fn out_of_worktree_script_does_not_match() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_worktree_dev_test_op(
            "bash /etc/init.d/thing",
            tmp.path()
        ));
        // A worktree script with an out-of-worktree path argument fails too.
        std::fs::write(tmp.path().join("gen.py"), "print(1)\n").unwrap();
        assert!(!is_worktree_dev_test_op(
            "python3 gen.py /etc/passwd",
            tmp.path()
        ));
    }

    /// Command separators / substitution metacharacters never match — a
    /// second command cannot ride along a worktree-confined shape.
    #[test]
    fn dev_test_rules_reject_metacharacter_smuggling() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("x.sh"), "echo hi\n").unwrap();
        assert!(!is_worktree_dev_test_op(
            "bash -n x.sh && curl evil.example",
            tmp.path()
        ));
        assert!(!is_worktree_dev_test_op("mktemp -d; do-thing", tmp.path()));
    }

    /// An unresolvable worktree root (no worktree known) never matches —
    /// the supervisor pane is unaffected by the dev-test rules.
    #[test]
    fn dev_test_rules_require_resolvable_worktree_root() {
        let missing = Path::new("/nonexistent/paw-worktree-xyz");
        assert!(!is_worktree_dev_test_op("mktemp -d", missing));
        assert!(!is_worktree_dev_test_op("bash -n x.sh", missing));
    }

    // --- Protected-path set (agent-memory-isolation) ---

    /// Builds a set from explicit parts with a temp home — no env mutation.
    fn protected_from(
        settings_paths: &[&str],
        claude_config_dir: Option<&str>,
        home: &Path,
        repo_root: Option<&Path>,
    ) -> ProtectedPaths {
        let settings: Vec<String> = settings_paths.iter().map(|s| (*s).to_string()).collect();
        ProtectedPaths::derive_from_parts(
            &settings,
            claude_config_dir,
            Some(home.to_path_buf()),
            repo_root,
        )
    }

    /// Spec scenario "Configured `settings_path` parent joins the set": the
    /// tilde-expanded parent of `[clis.myvariant].settings_path` is protected.
    #[test]
    fn configured_settings_path_parent_joins_the_set() {
        let home = TempDir::new().unwrap();
        let set = protected_from(&["~/.myvariant/settings.json"], None, home.path(), None);
        assert!(set.contains_dir(&home.path().join(".myvariant")));
    }

    /// The same scenario through the config-driven entry point: a `PawConfig`
    /// with `[clis.myvariant] settings_path` feeds the derivation.
    #[test]
    fn derive_reads_settings_paths_from_config() {
        let mut config = crate::config::PawConfig::default();
        config.clis.insert(
            "myvariant".to_string(),
            crate::config::CustomCli {
                command: "myvariant".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: Some("/opt/myvariant-home/settings.json".to_string()),
                approval_args: std::collections::HashMap::new(),
                prompt_profile: None,
            },
        );
        let set = ProtectedPaths::derive(&config, None);
        assert!(set.contains_dir(Path::new("/opt/myvariant-home")));
    }

    /// Spec scenario "Repo-root control dirs are protected for embedded
    /// worktrees": `<repo>/.claude` and `<repo>/.git-paw` join the set, while
    /// the agent's own worktree subtree is carved out.
    #[test]
    fn repo_root_control_dirs_protected_for_embedded_worktrees() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let worktree = repo.path().join(".git-paw/worktrees/feat-x");
        std::fs::create_dir_all(&worktree).unwrap();
        let set = protected_from(&[], None, home.path(), Some(repo.path()));
        assert!(set.contains_dir(&repo.path().join(".claude")));
        assert!(set.contains_dir(&repo.path().join(".git-paw")));
        // A write to the repo-root control area escalates...
        let config_toml = repo.path().join(".git-paw/config.toml");
        assert!(set.matches_target(&config_toml.to_string_lossy(), Some(&worktree)));
        // ...but the agent's own worktree subtree (inside .git-paw) does not.
        assert!(!set.matches_target("notes/memory.md", Some(&worktree)));
        let own_file = worktree.join("src/lib.rs");
        assert!(!set.matches_target(&own_file.to_string_lossy(), Some(&worktree)));
    }

    /// Spec scenario "No CLI product names are hardcoded beyond the
    /// claude-format default": with empty config and no env, the set is the
    /// home-level claude-format default only.
    #[test]
    fn derivation_has_no_hardcoded_cli_names() {
        let home = TempDir::new().unwrap();
        let set = protected_from(&[], None, home.path(), None);
        assert!(set.contains_dir(&home.path().join(".claude")));
        for product_dir in [".claude-oss", ".codex", ".gemini", ".agents", ".aider"] {
            assert!(
                !set.contains_dir(&home.path().join(product_dir)),
                "{product_dir} must not be built in — entries trace to config/env only"
            );
        }
    }

    /// `CLAUDE_CONFIG_DIR` (passed explicitly) joins the set.
    #[test]
    fn claude_config_dir_joins_the_set() {
        let home = TempDir::new().unwrap();
        let custom = TempDir::new().unwrap();
        let set = protected_from(
            &[],
            Some(&custom.path().to_string_lossy()),
            home.path(),
            None,
        );
        assert!(set.contains_dir(custom.path()));
        let target = custom.path().join("settings.json");
        assert!(set.matches_target(&target.to_string_lossy(), None));
    }

    /// `projects/**/memory` subtrees beneath a config dir join the set.
    #[test]
    fn memory_subtrees_join_the_set() {
        let home = TempDir::new().unwrap();
        let memory = home.path().join(".claude/projects/-x-repo/memory");
        std::fs::create_dir_all(&memory).unwrap();
        let set = protected_from(&[], None, home.path(), None);
        assert!(set.contains_dir(&memory));
    }

    /// Fail-closed: an entry whose path does not exist is still matched
    /// syntactically.
    #[test]
    fn nonexistent_entry_matches_syntactically() {
        let home = TempDir::new().unwrap();
        // `~/.myvariant` is never created.
        let set = protected_from(&["~/.myvariant/settings.json"], None, home.path(), None);
        assert!(set.matches_target("~/.myvariant/settings.json", None));
    }

    // --- Protected-path violation rule (safe-command-classification) ---

    /// Spec scenario "Write to operator memory escalates as danger": a
    /// filesystem prompt targeting the operator's memory territory matches.
    #[test]
    fn operator_memory_write_prompt_is_violation() {
        let home = TempDir::new().unwrap();
        let memory = home.path().join(".claude/projects/-x-repo/memory");
        std::fs::create_dir_all(&memory).unwrap();
        let set = protected_from(&[], None, home.path(), None);
        let target = memory.join("MEMORY.md");
        let prompt = format!(
            "Do you want to allow this write to {}?",
            target.to_string_lossy()
        );
        let worktree = TempDir::new().unwrap();
        assert!(is_protected_path_violation(
            &prompt,
            &prompt,
            &set,
            Some(worktree.path())
        ));
    }

    /// Spec scenario "Shell append to a configured settings file escalates":
    /// a `>>` redirect into a configured settings dir matches.
    #[test]
    fn shell_append_to_configured_settings_is_violation() {
        let home = TempDir::new().unwrap();
        let set = protected_from(&["~/.claude-oss/settings.json"], None, home.path(), None);
        let capture =
            "Bash command\n  echo '{}' >> ~/.claude-oss/settings.json\nDo you want to proceed?";
        let slice = "echo '{}' >> ~/.claude-oss/settings.json";
        assert!(is_protected_path_violation(capture, slice, &set, None));
    }

    /// Spec scenario "In-worktree writes are unaffected": a relative write
    /// inside the agent's own worktree never matches this rule.
    #[test]
    fn in_worktree_write_is_not_violation() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let worktree = repo.path().join(".git-paw/worktrees/feat-x");
        std::fs::create_dir_all(&worktree).unwrap();
        let set = protected_from(&[], None, home.path(), Some(repo.path()));
        let prompt = "Do you want to allow this write to notes/memory.md?";
        assert!(!is_protected_path_violation(
            prompt,
            prompt,
            &set,
            Some(&worktree)
        ));
        // The existing worktree-confined classification still applies.
        assert!(is_worktree_file_op(prompt, &worktree, true));
    }

    /// Spec scenario "Reads of operator config are not matched by this rule".
    #[test]
    fn reads_of_operator_config_are_not_violation() {
        let home = TempDir::new().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        let set = protected_from(&[], None, home.path(), None);
        let slice = "cat ~/.claude/settings.json";
        let capture = format!("Bash command\n  {slice}\nDo you want to proceed?");
        assert!(!is_protected_path_violation(&capture, slice, &set, None));
        // Reading via cp FROM the protected dir is a read of the source too.
        let slice = "cp ~/.claude/settings.json backup.json";
        let worktree = TempDir::new().unwrap();
        assert!(!is_protected_path_violation(
            slice,
            slice,
            &set,
            Some(worktree.path())
        ));
    }

    /// Spec scenario "Path-escape into the protected set is caught": a
    /// `..`-spelled target that resolves into `<repo>/.claude` matches.
    #[test]
    fn dotdot_escape_into_protected_set_is_violation() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let worktree = repo.path().join(".git-paw/worktrees/feat-x");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
        let set = protected_from(&[], None, home.path(), Some(repo.path()));
        let target = format!(
            "{}/../../../.claude/settings.json",
            worktree.to_string_lossy()
        );
        let prompt = format!("Do you want to allow this write to {target}?");
        assert!(is_protected_path_violation(
            &prompt,
            &prompt,
            &set,
            Some(&worktree)
        ));
    }

    /// Mutating verbs (not just redirects) are covered: `mkdir`/`touch`/`rm`
    /// into a protected dir match; `tee` and `sed -i` too.
    #[test]
    fn mutating_verbs_into_protected_set_are_violations() {
        let home = TempDir::new().unwrap();
        std::fs::create_dir_all(home.path().join(".claude")).unwrap();
        let set = protected_from(&[], None, home.path(), None);
        let worktree = TempDir::new().unwrap();
        let root = Some(worktree.path());
        for slice in [
            "mkdir -p ~/.claude/hooks",
            "touch ~/.claude/settings.json",
            "rm ~/.claude/settings.json",
            "echo x | tee -a ~/.claude/settings.json",
            "sed -i s/a/b/ ~/.claude/settings.json",
            "cp evil.json ~/.claude/settings.json",
        ] {
            assert!(
                is_protected_path_violation(slice, slice, &set, root),
                "{slice} must match the protected-path rule"
            );
        }
        // Plain sed (no -i) is a read.
        let slice = "sed -n 1,10p ~/.claude/settings.json";
        assert!(!is_protected_path_violation(slice, slice, &set, root));
    }

    /// The empty (default) set matches nothing — production derivation is
    /// what arms the rule.
    #[test]
    fn empty_protected_set_matches_nothing() {
        let set = ProtectedPaths::default();
        assert!(!is_protected_path_violation(
            "Do you want to allow this write to /etc/hosts?",
            "echo x >> /etc/hosts",
            &set,
            None
        ));
    }

    // --- Section 6: live-prompt gate ---

    /// Spec scenario "Live prompt fires": the footer is within the last lines.
    #[test]
    fn live_prompt_with_footer_fires() {
        let capture = "\
Bash command
  cargo test
Do you want to proceed?
❯ 1. Yes
  2. No
  (esc to cancel)";
        assert!(is_live_prompt(capture));
    }

    /// Spec scenario "Footer absent does not fire".
    #[test]
    fn live_prompt_without_footer_does_not_fire() {
        let capture = "\
I might run cargo test soon.
Here is some narration about the plan.
$ ls -la
done.";
        assert!(!is_live_prompt(capture));
    }

    /// Spec scenario "Footer scrolled out of the live window does not fire":
    /// `Esc to cancel` appears but is followed by more than ~4 non-blank lines.
    #[test]
    fn live_prompt_scrolled_out_does_not_fire() {
        let capture = "\
Do you want to proceed?
  Esc to cancel
output line 1
output line 2
output line 3
output line 4
output line 5";
        assert!(
            !is_live_prompt(capture),
            "footer scrolled past the last 4 non-blank lines is not live"
        );
    }

    /// Spec scenario "Live multi-option prompt is detected": the proceed
    /// question sits above a 3-option list (with a "don't ask again" option)
    /// whose last line carries only an inline `(esc)` — no separate footer.
    /// The option glyphs anchor the tail; the question is within the block.
    #[test]
    fn live_prompt_multi_option_block_is_detected() {
        let capture = "\
╭──────────────────────────────────────────────────────────────╮
│ Bash command                                                 │
│   cargo test --workspace                                     │
│   Run the full test suite                                    │
│ Do you want to proceed?                                      │
│ ❯ 1. Yes                                                     │
│   2. Yes, and don't ask again for cargo test in this project │
│   3. No, and tell Claude what to do differently (esc)        │
╰──────────────────────────────────────────────────────────────╯";
        assert!(
            is_live_prompt(capture),
            "a multi-option prompt block must be detected as live"
        );
    }

    /// Spec scenario "Live multi-option prompt is detected" (footer variant):
    /// the same block with an explicit `Esc to cancel` footer line, the
    /// proceed question more than 4 non-blank lines above the bottom.
    #[test]
    fn live_prompt_multi_option_with_footer_is_detected() {
        let capture = "\
Do you want to proceed?
❯ 1. Yes
  2. Yes, and don't ask again for: git status
  3. No
  Esc to cancel";
        assert!(is_live_prompt(capture));
    }

    /// A numbered list at the tail of plain narration (no proceed question or
    /// footer anywhere in the block window) is NOT a live prompt — option
    /// glyphs alone cannot bridge to liveness.
    #[test]
    fn numbered_list_without_prompt_markers_is_not_live() {
        let capture = "\
Here is my plan:
1. run the tests
2. commit the work
3. stand by for verification";
        assert!(
            !is_live_prompt(capture),
            "a prose numbered list must not be detected as live"
        );
    }

    // --- Section 7: option-index selection and broad-grant rule ---

    #[test]
    fn detects_two_and_three_option_shapes() {
        let two = "Do you want to proceed?\n❯ 1. Yes\n  2. No";
        let three = "Do you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for: git status\n  3. No";
        assert_eq!(detect_prompt_shape(two), PromptShape::TwoOption);
        assert_eq!(detect_prompt_shape(three), PromptShape::ThreeOption);
    }

    #[test]
    fn arbitrary_code_runner_predicate() {
        assert!(is_arbitrary_code_runner("python3 -c \"import os\""));
        assert!(is_arbitrary_code_runner("python script.py"));
        assert!(is_arbitrary_code_runner("bash -c \"do-thing\""));
        assert!(is_arbitrary_code_runner("sh -c 'x'"));
        assert!(is_arbitrary_code_runner("node -e \"x\" -c more"));
        assert!(is_arbitrary_code_runner("eval \"$(thing)\""));
        assert!(!is_arbitrary_code_runner("git status"));
        assert!(!is_arbitrary_code_runner("cargo test"));
    }

    /// Spec scenarios "Two-option prompt selects Yes" and "A safe prompt with no
    /// durable option uses the once-only option": with no durable option on
    /// offer there is nothing to prefer, classified safe or not.
    #[test]
    fn two_option_selects_yes() {
        assert_eq!(
            select_option_index(PromptShape::TwoOption, "git status", false),
            1
        );
        assert_eq!(
            select_option_index(PromptShape::TwoOption, "cargo test", true),
            1
        );
    }

    /// Spec scenario "Allowlisted verb takes the broad grant": a read-mostly verb
    /// takes the durable option even when the caller has not classified it.
    #[test]
    fn three_option_allowlisted_takes_broad_grant() {
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "git status", false),
            2
        );
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "grep foo", false),
            2
        );
    }

    /// Spec scenario "A safe prompt offering a durable option takes it": a
    /// stack command that classifies safe through the resolved allowlist takes
    /// the durable grant even though its verb is not read-mostly.
    #[test]
    fn three_option_safe_classified_takes_durable_grant() {
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "cargo test --lib", true),
            2
        );
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "mdbook build docs/", true),
            2
        );
    }

    /// Spec scenario "A non-safe prompt is never granted durably": an
    /// unclassified non-read-mostly command stays on the once-only option.
    #[test]
    fn three_option_unclassified_command_gets_no_durable_grant() {
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "cargo test --lib", false),
            1
        );
        assert_eq!(
            select_option_index(PromptShape::ThreeOption, "frobnicate --all", false),
            1
        );
    }

    /// Spec scenarios "python -c / bash -c never gets a permanent broad grant":
    /// arbitrary-code runners take the one-time Yes (option 1) even when the
    /// caller classified them safe (a worktree-resident script run).
    #[test]
    fn arbitrary_code_never_takes_broad_grant() {
        for classified_safe in [false, true] {
            assert_eq!(
                select_option_index(
                    PromptShape::ThreeOption,
                    "python3 -c \"import os; os.remove('x')\"",
                    classified_safe
                ),
                1
            );
            assert_eq!(
                select_option_index(
                    PromptShape::ThreeOption,
                    "bash -c \"do-thing\"",
                    classified_safe
                ),
                1
            );
            assert_eq!(
                select_option_index(PromptShape::ThreeOption, "python3 tools/gen.py", true),
                1,
                "an interpreter run of a worktree script is one-time safe only"
            );
        }
    }

    // --- Section: `.git/`-write danger rule (GP-04b) -------------------------

    /// Spec scenario "Append to .git/info/exclude escalates as danger".
    #[test]
    fn git_info_exclude_append_escalates_as_danger() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(tmp.path());
        let captured =
            "Bash command\n  echo '.git-paw/' >> .git/info/exclude\nDo you want to proceed?";
        let slice = "echo '.git-paw/' >> .git/info/exclude";
        assert!(is_git_dir_write(captured, slice, Some(tmp.path())));
    }

    /// Spec scenario "Write to .git/config escalates as danger".
    #[test]
    fn git_config_write_escalates_as_danger() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(tmp.path());
        let target = tmp.path().join(".git").join("config");
        let prompt = format!(
            "Do you want to allow this write to {}?",
            target.to_string_lossy()
        );
        assert!(is_git_dir_write(&prompt, "", Some(tmp.path())));
    }

    /// Spec scenario "Reading .git metadata is not matched by this rule".
    #[test]
    fn reading_git_config_is_not_matched() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(tmp.path());
        assert!(!is_git_dir_write(
            "Bash command\n  cat .git/config\nDo you want to proceed?",
            "cat .git/config",
            Some(tmp.path())
        ));
    }

    /// A relative `.git/` target with no known worktree root is still caught
    /// (fail-closed, matching the lexical fallback).
    #[test]
    fn git_dir_write_matches_without_a_worktree_root() {
        assert!(is_git_dir_write("", "echo x >> .git/info/exclude", None));
    }

    /// Ordinary `git` subcommands (which target no `.git/`-internal file
    /// path via the mutating-verb scan) are unaffected by this rule.
    #[test]
    fn ordinary_git_subcommands_are_unaffected() {
        let tmp = TempDir::new().unwrap();
        init_git_repo(tmp.path());
        for cmd in ["git commit -m wip", "git add .", "git status"] {
            assert!(
                !is_git_dir_write("", cmd, Some(tmp.path())),
                "{cmd} must not match the .git/-write rule"
            );
        }
    }

    /// A helper to initialise a minimal git repo at `dir`, mirroring the
    /// fixture used elsewhere in this crate for tests that need a real
    /// `.git/` directory to canonicalize against.
    fn init_git_repo(dir: &Path) {
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .current_dir(dir)
                .args(args)
                .status()
                .expect("git command");
            assert!(status.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
    }

    // --- Section: exit-probe / discard-redirect normalization ---------------

    /// Spec scenario "A safe command with a trailing exit-code probe classifies
    /// safe": the probe is stripped, so the remainder matches the allowlist the
    /// same way the bare command does.
    #[test]
    fn normalize_strips_trailing_exit_code_probe() {
        let whitelist = vec!["cargo test".to_string()];
        assert_eq!(
            normalize_command("cargo test --lib; echo test-exit=$?"),
            "cargo test --lib"
        );
        assert!(is_safe_command(
            &normalize_command("cargo test --lib; echo test-exit=$?"),
            &whitelist
        ));
        assert_eq!(normalize_command("just check; RC=$?"), "just check");
        assert_eq!(normalize_command("just check ; echo $?"), "just check");
    }

    /// Spec scenario "A trailing redirect is normalized away".
    #[test]
    fn normalize_strips_trailing_discard_redirect() {
        assert_eq!(
            normalize_command("mdbook build docs/ >/dev/null 2>&1"),
            "mdbook build docs/"
        );
        assert_eq!(
            normalize_command("mdbook build docs/ > /dev/null 2>&1"),
            "mdbook build docs/"
        );
        // Both wrappers at once still reduce to the bare command.
        assert_eq!(
            normalize_command("mdbook build docs/ >/dev/null 2>&1; echo $?"),
            "mdbook build docs/"
        );
    }

    /// Spec scenario "Normalization does not rescue a danger command": the
    /// danger-list runs against the normalized command, so stripping the probe
    /// cannot downgrade the escalation.
    #[test]
    fn normalize_does_not_rescue_a_danger_command() {
        for wrapped in [
            "git push --force; echo $?",
            "sudo rm -rf /etc; echo rc=$?",
            "git push >/dev/null 2>&1",
        ] {
            assert!(
                is_dangerous(&normalize_command(wrapped)),
                "{wrapped} must still escalate after normalization"
            );
        }
        // A redirect to a real device is NOT a discard redirect and stays put.
        assert!(is_dangerous(&normalize_command("cat secrets > /dev/sda")));
    }

    /// Normalization is conservative: only the documented suffix forms are
    /// removed, and anything else is returned unchanged.
    #[test]
    fn normalize_leaves_unrecognised_suffixes_intact() {
        for unchanged in [
            "cargo test --lib",
            "cargo test; cargo build",
            "echo $? > rc.txt",
            "grep -rn 'echo $?' src/",
            "cargo test > out.log 2>&1",
        ] {
            assert_eq!(normalize_command(unchanged), unchanged);
        }
    }

    // --- Section: leading assignment / env / nohup normalization (GP-02a) ---

    /// Spec scenario "A leading VAR=value assignment is normalized away".
    #[test]
    fn normalize_strips_leading_assignment() {
        assert_eq!(
            normalize_command("TMPDIR=/tmp/x cargo test --lib"),
            "cargo test --lib"
        );
    }

    /// Spec scenario "Multiple leading assignments plus a trailing probe are
    /// both normalized".
    #[test]
    fn normalize_strips_multiple_leading_assignments_and_trailing_probe() {
        assert_eq!(
            normalize_command(
                "GIT_PAW_ALLOW_LIVE_SESSION=1 TMPDIR=/tmp/x cargo test; echo exit=$?"
            ),
            "cargo test"
        );
    }

    /// Spec scenario "A leading env wrapper is normalized away".
    #[test]
    fn normalize_strips_leading_env_wrapper() {
        assert_eq!(normalize_command("env FOO=bar cargo build"), "cargo build");
    }

    /// `env`'s own `-i` / `-u NAME` options are stripped along with its
    /// assignments, per the requirement text.
    #[test]
    fn normalize_strips_env_options() {
        assert_eq!(
            normalize_command("env -i FOO=bar -u PATH cargo build"),
            "cargo build"
        );
    }

    /// Spec scenario "A leading nohup wrapper is normalized away".
    #[test]
    fn normalize_strips_leading_nohup_wrapper() {
        assert_eq!(normalize_command("nohup just check"), "just check");
    }

    /// Spec scenario "Normalization does not rescue a danger command behind
    /// assignments": stripping the leading prefix must not downgrade an
    /// escalation.
    #[test]
    fn normalize_leading_prefix_does_not_rescue_a_danger_command() {
        assert_eq!(normalize_command("FOO=bar rm -rf /"), "rm -rf /");
        assert!(is_dangerous(&normalize_command("FOO=bar rm -rf /")));
    }

    /// A value that merely contains `=` (not a leading identifier) is never
    /// mistaken for an assignment and stays part of the command.
    #[test]
    fn normalize_does_not_strip_non_assignment_equals() {
        assert_eq!(
            normalize_command("curl 'http://example.com/?a=b' -o out"),
            "curl 'http://example.com/?a=b' -o out"
        );
    }

    // --- Section: managed helper-script safe rule (GP-02b) ------------------

    /// Spec scenario "A bundled broker.sh boot call classifies safe".
    #[test]
    fn managed_broker_script_invocation_is_safe() {
        assert!(is_managed_script_invocation(
            ".git-paw/scripts/broker.sh --agent feat-x status booting",
            None
        ));
    }

    /// Spec scenario "A bundled sweep.sh call classifies safe".
    #[test]
    fn managed_sweep_script_invocation_is_safe() {
        assert!(is_managed_script_invocation(
            ".git-paw/scripts/sweep.sh status-publish",
            None
        ));
    }

    /// The rule also covers `docs-fetch.sh` and interpreter-invoked forms.
    #[test]
    fn managed_docs_fetch_script_and_interpreter_invocation_are_safe() {
        assert!(is_managed_script_invocation(
            ".git-paw/scripts/docs-fetch.sh fetch foo",
            None
        ));
        assert!(is_managed_script_invocation(
            "bash .git-paw/scripts/broker.sh --agent feat-x status booting",
            None
        ));
    }

    /// Spec scenario "A managed-script invocation chained with a danger
    /// command still escalates": the safe rule itself does not gate on
    /// danger (that precedence lives in the caller), but the danger-list
    /// match on the full slice must not be defeated by the managed-script
    /// prefix.
    #[test]
    fn managed_script_chained_with_danger_command_still_escalates() {
        let slice = ".git-paw/scripts/sweep.sh snapshot && rm -rf /";
        assert!(is_dangerous(slice), "danger-list must match the full slice");
    }

    /// An arbitrary `.git-paw/` file that is not one of the three bundled
    /// scripts does not classify safe merely for living under `.git-paw/`.
    #[test]
    fn non_script_git_paw_path_is_not_a_managed_script_invocation() {
        assert!(!is_managed_script_invocation(
            ".git-paw/config.toml --dump",
            None
        ));
    }

    /// A worktree-relative managed-script path resolves through
    /// `is_managed_path` when a worktree root is known.
    #[test]
    fn managed_script_invocation_resolves_against_worktree_root() {
        let tmp = TempDir::new().unwrap();
        assert!(is_managed_script_invocation(
            ".git-paw/scripts/broker.sh status booting",
            Some(tmp.path())
        ));
    }
}
