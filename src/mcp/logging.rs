//! Minimal stderr logger for the MCP server (design D5).
//!
//! stdio MCP servers MUST keep **stdout** reserved for JSON-RPC frames, so all
//! diagnostics go to **stderr** (and, optionally, a `--log-file`). Verbosity is
//! controlled by the `RUST_LOG` environment variable (a recognised level
//! keyword anywhere in the value), defaulting to `warn` — quiet by default.
//!
//! This is intentionally a tiny homegrown facility rather than a new
//! `tracing-subscriber` dependency: the project minimises its dependency
//! surface (cf. the homegrown `crate::dirs`), and the only audited
//! requirements are "logging to stderr, `RUST_LOG`-controlled, default warn,
//! optional file tee" — all satisfied here without pulling in a logging stack.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::error::PawError;

/// Severity levels, ordered least- to most-verbose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Errors that prevented an operation.
    Error,
    /// Warnings — the default threshold.
    Warn,
    /// Informational lifecycle messages.
    Info,
    /// Debugging detail.
    Debug,
    /// Very verbose tracing.
    Trace,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

/// Parses a verbosity threshold from a `RUST_LOG`-style value. Recognises a
/// level keyword appearing anywhere in the string (case-insensitive); defaults
/// to [`Level::Warn`] when none is found.
#[must_use]
pub fn parse_level(rust_log: Option<&str>) -> Level {
    let Some(value) = rust_log else {
        return Level::Warn;
    };
    let v = value.to_ascii_lowercase();
    // Most-verbose wins if multiple appear, so check from the top down.
    if v.contains("trace") {
        Level::Trace
    } else if v.contains("debug") {
        Level::Debug
    } else if v.contains("info") {
        Level::Info
    } else if v.contains("error") && !v.contains("warn") {
        Level::Error
    } else {
        Level::Warn
    }
}

struct Logger {
    threshold: Level,
    file: Option<Mutex<std::fs::File>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Initialises the global logger. Idempotent: a second call is a no-op (the
/// first configuration wins), which keeps tests that spin up the server
/// repeatedly from panicking.
pub fn init(log_file: Option<&Path>) -> Result<(), PawError> {
    let threshold = parse_level(std::env::var("RUST_LOG").ok().as_deref());
    let file = match log_file {
        Some(path) => {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| {
                    PawError::McpError(format!("could not open --log-file {}: {e}", path.display()))
                })?;
            Some(Mutex::new(f))
        }
        None => None,
    };
    // First writer wins; ignore if already initialised.
    let _ = LOGGER.set(Logger { threshold, file });
    Ok(())
}

/// Emits a log line at `level` to stderr and the log file (when enabled),
/// gated by the configured threshold. Before [`init`], falls back to the
/// default `warn` threshold against stderr only.
pub fn log(level: Level, message: &str) {
    match LOGGER.get() {
        Some(logger) => {
            if level > logger.threshold {
                return;
            }
            let line = format!("[git-paw mcp] {}: {message}\n", level.label());
            // stderr is always the primary sink.
            let _ = std::io::stderr().write_all(line.as_bytes());
            if let Some(file) = logger.file.as_ref()
                && let Ok(mut f) = file.lock()
            {
                let _ = f.write_all(line.as_bytes());
            }
        }
        None => {
            if level <= Level::Warn {
                let _ = std::io::stderr()
                    .write_all(format!("[git-paw mcp] {}: {message}\n", level.label()).as_bytes());
            }
        }
    }
}

/// Logs at [`Level::Info`].
pub fn info(message: &str) {
    log(Level::Info, message);
}

/// Logs at [`Level::Warn`].
pub fn warn(message: &str) {
    log(Level::Warn, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_level_defaults_to_warn() {
        assert_eq!(parse_level(None), Level::Warn);
        assert_eq!(parse_level(Some("")), Level::Warn);
    }

    #[test]
    fn parse_level_recognises_keywords() {
        assert_eq!(parse_level(Some("debug")), Level::Debug);
        assert_eq!(parse_level(Some("info")), Level::Info);
        assert_eq!(parse_level(Some("trace")), Level::Trace);
        assert_eq!(parse_level(Some("git_paw=debug,hyper=warn")), Level::Debug);
    }

    #[test]
    fn level_ordering_is_least_to_most_verbose() {
        assert!(Level::Error < Level::Warn);
        assert!(Level::Warn < Level::Debug);
        assert!(Level::Debug < Level::Trace);
    }
}
