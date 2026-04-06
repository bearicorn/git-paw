//! Replay captured session logs.
//!
//! Reads session logs, strips ANSI escape codes for clean display,
//! and supports colored output via `less -R`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::PawError;
use crate::logging;

// ---------------------------------------------------------------------------
// ANSI stripping
// ---------------------------------------------------------------------------

/// State machine states for ANSI escape code parsing.
#[derive(Debug, Clone, Copy)]
enum AnsiState {
    Normal,
    SeenEsc,
    InCsi,
    InOsc,
    InOscEscSeen,
}

/// Strips ANSI CSI and OSC escape sequences from the input using a
/// char-by-char state machine. Non-escape content passes through unchanged.
///
/// Handles:
/// - CSI sequences: `ESC [` followed by parameters until a final byte (0x40-0x7E)
/// - OSC sequences: `ESC ]` followed by content until BEL (`\x07`) or ST (`ESC \`)
pub fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut state = AnsiState::Normal;

    for ch in input.chars() {
        match state {
            AnsiState::Normal => {
                if ch == '\x1b' {
                    state = AnsiState::SeenEsc;
                } else {
                    output.push(ch);
                }
            }
            AnsiState::SeenEsc => {
                if ch == '[' {
                    state = AnsiState::InCsi;
                } else if ch == ']' {
                    state = AnsiState::InOsc;
                } else {
                    // Not a CSI/OSC sequence — drop ESC, emit the character
                    output.push(ch);
                    state = AnsiState::Normal;
                }
            }
            AnsiState::InCsi => {
                // Final byte of a CSI sequence is in the range 0x40–0x7E
                if ('@'..='~').contains(&ch) {
                    state = AnsiState::Normal;
                }
                // Parameter and intermediate bytes are consumed silently
            }
            AnsiState::InOsc => {
                if ch == '\x07' {
                    // BEL terminates the OSC sequence
                    state = AnsiState::Normal;
                } else if ch == '\x1b' {
                    // Possible start of ST (ESC \)
                    state = AnsiState::InOscEscSeen;
                }
                // All other characters inside OSC are consumed silently
            }
            AnsiState::InOscEscSeen => {
                if ch == '\\' {
                    // ST (ESC \) terminates the OSC sequence
                    state = AnsiState::Normal;
                } else {
                    // False alarm — still inside OSC, re-evaluate this char
                    // (the ESC was part of the OSC payload, which is unusual
                    // but we stay in OSC to be safe)
                    state = AnsiState::InOsc;
                }
            }
        }
    }
    // Incomplete sequences at end of input are silently dropped
    output
}

// ---------------------------------------------------------------------------
// Session selection
// ---------------------------------------------------------------------------

/// Resolves which session to replay from.
///
/// If `session_flag` is `Some`, validates it exists and returns it.
/// Otherwise, returns the session with the most recent modification time.
pub fn resolve_session(repo_root: &Path, session_flag: Option<&str>) -> Result<String, PawError> {
    let sessions = logging::list_log_sessions(repo_root)?;

    if sessions.is_empty() {
        return Err(PawError::ReplayError(
            "No log sessions found. Session logging may not be enabled.".to_string(),
        ));
    }

    if let Some(name) = session_flag {
        if sessions.contains(&name.to_string()) {
            return Ok(name.to_string());
        }
        return Err(PawError::ReplayError(format!(
            "Session '{name}' not found. Run `git paw replay --list` to see available sessions."
        )));
    }

    // Default to most recent by directory mtime
    let logs_base = repo_root.join(".git-paw").join("logs");
    let mut best: Option<(String, std::time::SystemTime)> = None;

    for session in &sessions {
        let path = logs_base.join(session);
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(mtime) = meta.modified()
            && best.as_ref().is_none_or(|(_, t)| mtime > *t)
        {
            best = Some((session.clone(), mtime));
        }
    }

    best.map(|(name, _)| name)
        .ok_or_else(|| PawError::ReplayError("No accessible log sessions found.".to_string()))
}

// ---------------------------------------------------------------------------
// Branch matching
// ---------------------------------------------------------------------------

/// Finds the log file matching `branch_query` within the given session.
///
/// Matches against both the original branch name and the sanitized filename
/// stem, so `feat/add-auth` and `feat--add-auth` both work.
pub fn find_log(repo_root: &Path, session: &str, branch_query: &str) -> Result<PathBuf, PawError> {
    let entries = logging::list_logs_for_session(repo_root, session)?;

    if entries.is_empty() {
        return Err(PawError::ReplayError(format!(
            "No logs found in session '{session}'."
        )));
    }

    for entry in &entries {
        let filename = entry.path.file_name().unwrap_or_default().to_string_lossy();
        let stem = filename.trim_end_matches(".log");
        if entry.branch == branch_query || stem == branch_query || *filename == *branch_query {
            return Ok(entry.path.clone());
        }
    }

    let available: Vec<String> = entries
        .iter()
        .map(|e| {
            let filename = e.path.file_name().unwrap_or_default().to_string_lossy();
            format!("  {filename}  \u{2192}  {}", e.branch)
        })
        .collect();
    Err(PawError::ReplayError(format!(
        "No log matching '{branch_query}' in session '{session}'. Available branches:\n{}",
        available.join("\n")
    )))
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

/// Reads the log file, strips ANSI codes, and writes to stdout.
pub fn replay_stripped(log_path: &Path) -> Result<(), PawError> {
    let content = fs::read_to_string(log_path)
        .map_err(|e| PawError::ReplayError(format!("cannot read log file: {e}")))?;

    if content.is_empty() {
        return Ok(());
    }

    let stripped = strip_ansi(&content);
    std::io::stdout()
        .write_all(stripped.as_bytes())
        .map_err(|e| PawError::ReplayError(format!("cannot write to stdout: {e}")))?;
    Ok(())
}

/// Pipes the raw log content through `less -R` for colored viewing.
///
/// Falls back to printing raw output with a warning if `less` is not found.
pub fn replay_colored(log_path: &Path) -> Result<(), PawError> {
    let content = fs::read(log_path)
        .map_err(|e| PawError::ReplayError(format!("cannot read log file: {e}")))?;

    if content.is_empty() {
        return Ok(());
    }

    if which::which("less").is_ok() {
        let mut child = std::process::Command::new("less")
            .arg("-R")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| PawError::ReplayError(format!("cannot start less: {e}")))?;

        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(&content);
        }

        let _ = child.wait();
    } else {
        eprintln!("warning: 'less' not found, printing raw output");
        std::io::stdout()
            .write_all(&content)
            .map_err(|e| PawError::ReplayError(format!("cannot write to stdout: {e}")))?;
    }

    Ok(())
}

/// Enumerates all sessions and their branches, printing a formatted list.
pub fn display_list(repo_root: &Path) -> Result<(), PawError> {
    let sessions = logging::list_log_sessions(repo_root)?;

    if sessions.is_empty() {
        println!("No log sessions found. Start a session with logging enabled to capture output.");
        return Ok(());
    }

    for session in &sessions {
        let entries = logging::list_logs_for_session(repo_root, session)?;
        let label = if entries.len() == 1 {
            "branch"
        } else {
            "branches"
        };
        println!("{session} ({} {label})", entries.len());
        for entry in &entries {
            let filename = entry.path.file_name().unwrap_or_default().to_string_lossy();
            println!("  {filename}  \u{2192}  {}", entry.branch);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- strip_ansi --

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn sgr_removed() {
        assert_eq!(strip_ansi("\x1b[31mred text\x1b[0m"), "red text");
    }

    #[test]
    fn cursor_sequences_removed() {
        assert_eq!(strip_ansi("\x1b[Hhello\x1b[2J"), "hello");
    }

    #[test]
    fn multiple_sequences_per_line() {
        assert_eq!(
            strip_ansi("\x1b[1m\x1b[31mBold Red\x1b[0m Normal"),
            "Bold Red Normal"
        );
    }

    #[test]
    fn incomplete_csi_at_end() {
        assert_eq!(strip_ansi("hello\x1b["), "hello");
    }

    #[test]
    fn incomplete_esc_at_end() {
        assert_eq!(strip_ansi("hello\x1b"), "hello");
    }

    #[test]
    fn empty_input() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn cursor_movement_codes() {
        assert_eq!(strip_ansi("\x1b[5Aup\x1b[3Bdown"), "updown");
    }

    #[test]
    fn erase_line_codes() {
        assert_eq!(strip_ansi("\x1b[Ktext\x1b[2K"), "text");
    }

    #[test]
    fn complex_sgr_params() {
        assert_eq!(strip_ansi("\x1b[38;5;196mcolor\x1b[0m"), "color");
    }

    // -- OSC stripping --

    #[test]
    fn osc_title_bel_terminated() {
        // \x1b]0;title\x07 — set terminal title, terminated by BEL
        assert_eq!(strip_ansi("\x1b]0;my title\x07hello"), "hello");
    }

    #[test]
    fn osc_title_st_terminated() {
        // \x1b]0;title\x1b\\ — set terminal title, terminated by ST
        assert_eq!(strip_ansi("\x1b]0;my title\x1b\\hello"), "hello");
    }

    #[test]
    fn osc_file_uri() {
        // Common OSC 7 sequence from shells reporting CWD
        assert_eq!(
            strip_ansi("\x1b]7;file:///Users/me/project\x07prompt$ "),
            "prompt$ "
        );
    }

    #[test]
    fn osc_hyperlink() {
        // OSC 8 hyperlink: \x1b]8;;url\x07text\x1b]8;;\x07
        assert_eq!(
            strip_ansi("\x1b]8;;https://example.com\x07click me\x1b]8;;\x07"),
            "click me"
        );
    }

    #[test]
    fn osc_mixed_with_csi() {
        // OSC title + CSI color in the same string
        assert_eq!(
            strip_ansi("\x1b]0;title\x07\x1b[31mred\x1b[0m plain"),
            "red plain"
        );
    }

    #[test]
    fn osc_incomplete_at_end() {
        // Incomplete OSC at end of input is silently dropped
        assert_eq!(strip_ansi("hello\x1b]0;partial title"), "hello");
    }

    #[test]
    fn osc_incomplete_st_at_end() {
        // OSC with ESC at end but no backslash
        assert_eq!(strip_ansi("hello\x1b]0;title\x1b"), "hello");
    }

    // -- resolve_session --

    fn setup_log_dir(root: &Path, session: &str, files: &[&str]) {
        let dir = root.join(".git-paw").join("logs").join(session);
        fs::create_dir_all(&dir).unwrap();
        for f in files {
            fs::write(dir.join(f), "log content").unwrap();
        }
    }

    #[test]
    fn resolve_explicit_session_found() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "paw-test", &["main.log"]);
        assert_eq!(
            resolve_session(tmp.path(), Some("paw-test")).unwrap(),
            "paw-test"
        );
    }

    #[test]
    fn resolve_explicit_session_not_found() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "paw-test", &["main.log"]);
        let err = resolve_session(tmp.path(), Some("nope")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("nope"));
        assert!(msg.contains("--list"));
    }

    #[test]
    fn resolve_default_most_recent() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "paw-old", &["main.log"]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        setup_log_dir(tmp.path(), "paw-new", &["main.log"]);
        assert_eq!(resolve_session(tmp.path(), None).unwrap(), "paw-new");
    }

    #[test]
    fn resolve_no_sessions_error() {
        let tmp = TempDir::new().unwrap();
        let err = resolve_session(tmp.path(), None).unwrap_err();
        assert!(err.to_string().contains("No log sessions"));
    }

    // --- Gap #8: No logs directory at all ---

    #[test]
    fn resolve_session_no_logs_dir_mentions_logging() {
        let tmp = TempDir::new().unwrap();
        // No .git-paw/logs/ directory exists at all
        let err = resolve_session(tmp.path(), None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("logging"),
            "error should mention logging not enabled, got: {msg}"
        );
    }

    // -- find_log --

    #[test]
    fn find_log_by_original_branch() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "s", &["feat--add-auth.log"]);
        assert!(find_log(tmp.path(), "s", "feat/add-auth").is_ok());
    }

    #[test]
    fn find_log_by_sanitized_name() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "s", &["feat--add-auth.log"]);
        assert!(find_log(tmp.path(), "s", "feat--add-auth").is_ok());
    }

    #[test]
    fn find_log_no_match_lists_available() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "s", &["main.log", "feat--auth.log"]);
        let err = find_log(tmp.path(), "s", "nonexistent").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("nonexistent"));
        assert!(msg.contains("main"));
    }

    // -- display_list --

    #[test]
    fn display_list_no_sessions() {
        let tmp = TempDir::new().unwrap();
        assert!(display_list(tmp.path()).is_ok());
    }

    #[test]
    fn display_list_with_sessions() {
        let tmp = TempDir::new().unwrap();
        setup_log_dir(tmp.path(), "paw-proj", &["main.log", "feat--x.log"]);
        assert!(display_list(tmp.path()).is_ok());
    }

    // -- replay_stripped --

    #[test]
    fn replay_stripped_empty_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("empty.log");
        fs::write(&p, "").unwrap();
        assert!(replay_stripped(&p).is_ok());
    }

    #[test]
    fn replay_stripped_strips_ansi() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("colored.log");
        fs::write(&p, "\x1b[31mred\x1b[0m plain").unwrap();
        assert!(replay_stripped(&p).is_ok());
    }

    // --- Gap #15: replay_colored happy path ---

    #[test]
    fn replay_colored_succeeds_with_ansi_content() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("colored.log");
        fs::write(&p, "\x1b[31mred text\x1b[0m and plain").unwrap();

        // If less is on PATH (it usually is), this should succeed.
        // If less is missing, it falls back to raw output, which also succeeds.
        let result = replay_colored(&p);
        assert!(
            result.is_ok(),
            "replay_colored should succeed, got: {result:?}"
        );
    }

    #[test]
    fn replay_colored_empty_file_succeeds() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("empty.log");
        fs::write(&p, "").unwrap();
        assert!(replay_colored(&p).is_ok());
    }
}
