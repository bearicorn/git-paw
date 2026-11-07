//! What a session git-paw launched has spent, as its CLI reports it. A
//! provider that cannot read usage answers [`UsageReport::Unknown`] with a
//! reason. Only the usage fields are deserialised: report content never
//! reaches an error message or a log.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use serde::Deserialize;

/// What a provider is asked when resolving one session's usage.
#[derive(Debug, Clone, Copy)]
pub struct UsageQuery<'a> {
    /// The CLI id, named in an unknown reason.
    pub cli_id: &'a str,
    /// The session id git-paw assigned at launch.
    pub session_id: &'a str,
    /// The worktree the session runs in.
    pub worktree_path: &'a Path,
    /// The CLI config dir when the entry sets one (the parent of
    /// `settings_path`).
    pub config_dir: Option<&'a Path>,
    /// Reads kept from earlier calls in this run, so only appended report
    /// bytes are parsed. `None` reads every report in full.
    pub cache: Option<&'a UsageCache>,
}

/// Report state kept across reads in one run, so reads parse only new lines.
#[derive(Debug, Default)]
pub struct UsageCache {
    sessions: Mutex<HashMap<PathBuf, SessionCache>>,
    lines_parsed: AtomicU64,
}

#[derive(Debug, Default)]
struct SessionCache {
    parser: ClaudeUsageParser,
    files: HashMap<PathBuf, FileCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FileCursor {
    len: u64,
    modified: Option<SystemTime>,
    offset: u64,
    lines: usize,
}

impl UsageCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Report lines parsed through this cache so far.
    #[must_use]
    pub fn lines_parsed(&self) -> u64 {
        self.lines_parsed.load(Ordering::Relaxed)
    }

    /// Totals for the session's report files (main transcript first). A file
    /// that shrank, or was rewritten (new mtime without growing), restarts
    /// the session.
    fn read_session(&self, files: &[PathBuf]) -> UsageReport {
        // A panic elsewhere must not stop counting: the map is still sound.
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(key) = files.first().cloned() else {
            return UsageReport::Unknown("no usage report files".to_string());
        };
        let mut session = sessions.remove(&key).unwrap_or_default();
        let rewritten = session.files.iter().any(|(path, cursor)| {
            std::fs::metadata(path).map_or(true, |m| {
                m.len() < cursor.offset
                    || (m.modified().ok() != cursor.modified && m.len() <= cursor.len)
            })
        });
        if rewritten {
            session = SessionCache::default();
        }
        for (n, path) in files.iter().enumerate() {
            if let Err(report) = self.advance(&mut session, path, n) {
                // The session is dropped: the next read starts over.
                return report;
            }
        }
        let totals = session.parser.totals();
        sessions.insert(key, session);
        UsageReport::Known(totals)
    }

    /// A line without its newline is still being written: leave it for the
    /// next read.
    fn advance(
        &self,
        session: &mut SessionCache,
        path: &Path,
        file: usize,
    ) -> Result<(), UsageReport> {
        let unreadable_file =
            || UsageReport::Unknown(format!("usage report {} could not be read", file + 1));
        let meta = std::fs::metadata(path).map_err(|_| unreadable_file())?;
        let (len, modified) = (meta.len(), meta.modified().ok());
        let mut cursor = session.files.get(path).copied().unwrap_or(FileCursor {
            len: 0,
            modified: None,
            offset: 0,
            lines: 0,
        });
        if session.files.contains_key(path) && cursor.len == len && cursor.modified == modified {
            return Ok(());
        }
        let mut handle = File::open(path).map_err(|_| unreadable_file())?;
        handle
            .seek(SeekFrom::Start(cursor.offset))
            .map_err(|_| unreadable_file())?;
        let mut reader = BufReader::new(handle);
        let mut line = String::new();
        loop {
            line.clear();
            let read = reader.read_line(&mut line).map_err(|_| unreadable_file())?;
            if read == 0 || !line.ends_with('\n') {
                break;
            }
            self.lines_parsed.fetch_add(1, Ordering::Relaxed);
            if session.parser.feed(&line).is_err() {
                return Err(unreadable(file, cursor.lines));
            }
            cursor.offset += read as u64;
            cursor.lines += 1;
        }
        cursor.len = len;
        cursor.modified = modified;
        session.files.insert(path.to_path_buf(), cursor);
        Ok(())
    }
}

/// Token totals for one or more sessions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UsageTotals {
    /// Input tokens not served from the prompt cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
    /// The cost the CLI itself reported, in its own currency.
    pub cost: Option<f64>,
}

impl UsageTotals {
    /// Input, output, cache-read and cache-write tokens: the figure
    /// `budget_max_tokens` caps.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    /// The sum of `items`. The cost is `None` unless every item has one, so a
    /// partial cost is never shown as the total.
    #[must_use]
    pub fn sum<'a>(items: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut total = Self::default();
        let mut cost = Some(0.0);
        let mut any = false;
        for item in items {
            any = true;
            total.input = total.input.saturating_add(item.input);
            total.output = total.output.saturating_add(item.output);
            total.cache_read = total.cache_read.saturating_add(item.cache_read);
            total.cache_write = total.cache_write.saturating_add(item.cache_write);
            cost = cost.zip(item.cost).map(|(a, b)| a + b);
        }
        total.cost = if any { cost } else { None };
        total
    }
}

/// One session's usage, or why it is not known.
#[derive(Debug, Clone, PartialEq)]
pub enum UsageReport {
    /// The figures the CLI reported.
    Known(UsageTotals),
    /// Usage cannot be attributed; the reason is shown to the operator.
    Unknown(String),
}

impl UsageReport {
    /// The figures when known.
    #[must_use]
    pub fn known(&self) -> Option<&UsageTotals> {
        match self {
            Self::Known(totals) => Some(totals),
            Self::Unknown(_) => None,
        }
    }
}

/// Attributed usage over several sessions: the known figures in total and by
/// CLI id, and the sessions whose usage is unknown. Unknown is never counted
/// as zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSummary {
    /// Known figures per CLI id.
    pub by_cli: BTreeMap<String, UsageTotals>,
    /// The sum of every known figure.
    pub total: UsageTotals,
    /// `(label, reason)` of each session whose usage is unknown.
    pub unknown: Vec<(String, String)>,
}

impl UsageSummary {
    /// Summarises `(cli id, label, report)` triples.
    #[must_use]
    pub fn from_reports<'a>(
        reports: impl IntoIterator<Item = (&'a str, &'a str, &'a UsageReport)>,
    ) -> Self {
        let mut grouped: BTreeMap<String, Vec<UsageTotals>> = BTreeMap::new();
        let mut unknown = Vec::new();
        for (cli, label, report) in reports {
            match report {
                UsageReport::Known(t) => grouped.entry(cli.to_string()).or_default().push(*t),
                UsageReport::Unknown(reason) => unknown.push((label.to_string(), reason.clone())),
            }
        }
        let by_cli: BTreeMap<String, UsageTotals> = grouped
            .into_iter()
            .map(|(cli, items)| (cli, UsageTotals::sum(&items)))
            .collect();
        let total = UsageTotals::sum(by_cli.values());
        Self {
            by_cli,
            total,
            unknown,
        }
    }

    /// Whether any session's usage is known.
    #[must_use]
    pub fn any_known(&self) -> bool {
        !self.by_cli.is_empty()
    }
}

/// The reason a CLI with no usage source reports.
#[must_use]
pub fn no_source_reason(cli_id: &str) -> String {
    format!("{cli_id} reports no usage git-paw can read")
}

/// Accumulates Claude Code transcript lines into [`UsageTotals`].
///
/// Each `message.id` counts once: Claude Code repeats a response's usage on
/// every content-block line, so feed all of a session's transcripts to one
/// parser.
#[derive(Debug, Default)]
pub struct ClaudeUsageParser {
    seen: HashSet<String>,
    totals: UsageTotals,
}

#[derive(Deserialize)]
struct RecordKind {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct AssistantRecord {
    message: Option<AssistantMessage>,
}

#[derive(Deserialize)]
struct AssistantMessage {
    id: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    #[serde(rename = "input_tokens")]
    input: u64,
    #[serde(rename = "output_tokens")]
    output: u64,
    #[serde(default, rename = "cache_read_input_tokens")]
    cache_read: Option<u64>,
    #[serde(default, rename = "cache_creation_input_tokens")]
    cache_write: Option<u64>,
}

/// A transcript line the parser could not read. Holds no report content, so
/// it is safe to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a usage record could not be parsed")]
pub struct UnreadableRecord;

impl ClaudeUsageParser {
    /// A parser with nothing counted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Counts one transcript line. A blank line is ignored.
    ///
    /// # Errors
    ///
    /// [`UnreadableRecord`] when the line is not JSON or an `assistant`
    /// record's usage is not the expected shape.
    pub fn feed(&mut self, line: &str) -> Result<(), UnreadableRecord> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(());
        }
        let kind: RecordKind = serde_json::from_str(line).map_err(|_| UnreadableRecord)?;
        if kind.kind.as_deref() != Some("assistant") {
            return Ok(());
        }
        let record: AssistantRecord = serde_json::from_str(line).map_err(|_| UnreadableRecord)?;
        let Some(AssistantMessage {
            id,
            usage: Some(usage),
        }) = record.message
        else {
            return Ok(());
        };
        if let Some(id) = id
            && !self.seen.insert(id)
        {
            return Ok(());
        }
        let t = &mut self.totals;
        t.input = t.input.saturating_add(usage.input);
        t.output = t.output.saturating_add(usage.output);
        t.cache_read = t.cache_read.saturating_add(usage.cache_read.unwrap_or(0));
        t.cache_write = t.cache_write.saturating_add(usage.cache_write.unwrap_or(0));
        Ok(())
    }

    /// The totals counted so far. Claude Code reports no cost.
    #[must_use]
    pub fn totals(&self) -> UsageTotals {
        self.totals
    }
}

/// Parses whole transcripts of one session into a report: the totals, or
/// unknown when any record is unreadable — never a partial figure.
#[must_use]
pub fn parse_claude_transcripts(transcripts: &[&str]) -> UsageReport {
    let mut parser = ClaudeUsageParser::new();
    for (file, text) in transcripts.iter().enumerate() {
        for (n, line) in text.lines().enumerate() {
            if parser.feed(line).is_err() {
                return unreadable(file, n);
            }
        }
    }
    UsageReport::Known(parser.totals())
}

fn unreadable(file: usize, line: usize) -> UsageReport {
    UsageReport::Unknown(format!(
        "usage report {} line {} could not be parsed",
        file + 1,
        line + 1
    ))
}

/// The directory name Claude Code keeps a working directory's transcripts
/// under: the path with every character outside `[A-Za-z0-9]` replaced by `-`.
#[must_use]
pub fn claude_project_slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The Claude Code configuration directories to search, in order: the
/// entry's relocated directory, `CLAUDE_CONFIG_DIR`, then `~/.claude`.
#[must_use]
pub fn claude_config_dirs(
    entry_dir: Option<&Path>,
    env_dir: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for dir in [
        entry_dir.map(Path::to_path_buf),
        env_dir.filter(|d| !d.as_os_str().is_empty()),
        home.map(|h| h.join(".claude")),
    ]
    .into_iter()
    .flatten()
    {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// The transcript files of `session_id` under `config_dir`: the main
/// transcript, then its subagent transcripts. Empty when there is none.
///
/// The main transcript is looked up under the worktree's slug first, then in
/// any project directory (a session id is unique), so a change in how the CLI
/// names project directories does not lose the report.
#[must_use]
pub fn claude_transcripts(config_dir: &Path, worktree: &Path, session_id: &str) -> Vec<PathBuf> {
    // The id becomes a file name: anything but a plain id could leave `projects/`.
    if session_id.is_empty()
        || !session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Vec::new();
    }
    let projects = config_dir.join("projects");
    let file = format!("{session_id}.jsonl");
    let slugged = projects.join(claude_project_slug(worktree)).join(&file);
    let main = if slugged.is_file() {
        Some(slugged)
    } else {
        std::fs::read_dir(&projects).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|e| e.path().join(&file))
                .find(|p| p.is_file())
        })
    };
    let Some(main) = main else {
        return Vec::new();
    };
    let mut subagents: Vec<PathBuf> = main
        .parent()
        .map(|dir| dir.join(session_id).join("subagents"))
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .collect()
        })
        .unwrap_or_default();
    subagents.sort();
    std::iter::once(main).chain(subagents).collect()
}

/// Reads a Claude-family session's usage from its transcripts, line by line,
/// through the query's cache when it has one.
///
/// An unterminated last line is a record the CLI is still writing and is
/// skipped; any other unreadable line makes the whole report unknown.
#[must_use]
pub fn read_claude_usage(query: &UsageQuery<'_>) -> UsageReport {
    let dirs = claude_config_dirs(
        query.config_dir,
        std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        crate::dirs::home_dir(),
    );
    let Some(files) = dirs
        .iter()
        .map(|dir| claude_transcripts(dir, query.worktree_path, query.session_id))
        .find(|files| !files.is_empty())
    else {
        return UsageReport::Unknown(format!(
            "no usage report found for {} session {}",
            query.cli_id, query.session_id
        ));
    };
    match query.cache {
        Some(cache) => cache.read_session(&files),
        None => UsageCache::new().read_session(&files),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::agent_cli::CliProfile;
    use crate::config::CustomCli;

    const SESSION: &str = include_str!("../../tests/fixtures/claude_usage/session.jsonl");
    const SUBAGENT: &str = include_str!("../../tests/fixtures/claude_usage/subagent.jsonl");
    const MALFORMED: &str = include_str!("../../tests/fixtures/claude_usage/malformed.jsonl");
    const SID: &str = "11111111-2222-3333-4444-555555555555";

    fn totals(input: u64, output: u64, cache_read: u64, cache_write: u64) -> UsageTotals {
        UsageTotals {
            input,
            output,
            cache_read,
            cache_write,
            cost: None,
        }
    }

    /// Scenario "A known report yields figures".
    #[test]
    fn a_known_report_yields_the_sums_counting_each_response_once() {
        assert_eq!(
            parse_claude_transcripts(&[SESSION]),
            UsageReport::Known(totals(5, 150, 2500, 520))
        );
    }

    #[test]
    fn subagent_transcripts_add_to_the_session_total() {
        assert_eq!(
            parse_claude_transcripts(&[SESSION, SUBAGENT]),
            UsageReport::Known(totals(10, 160, 2500, 820))
        );
    }

    /// Scenario "A malformed report yields unknown".
    #[test]
    fn a_malformed_report_is_unknown_not_a_partial_total() {
        let report = parse_claude_transcripts(&[MALFORMED]);
        let UsageReport::Unknown(reason) = report else {
            panic!("expected unknown, got {report:?}");
        };
        assert!(
            !reason.contains("SECRET"),
            "the reason must carry no report content: {reason}"
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_unknown() {
        assert!(matches!(
            parse_claude_transcripts(&["{\"type\":\"assistant\""]),
            UsageReport::Unknown(_)
        ));
    }

    /// Scenario "A generic CLI's usage is unknown".
    #[test]
    fn a_generic_cli_is_unknown_with_a_reason_naming_the_cli() {
        let profile = CliProfile::resolve("aider", &HashMap::new());
        let report = profile.usage(Some(SID), Path::new("/repo"));
        let UsageReport::Unknown(reason) = report else {
            panic!("expected unknown, got {report:?}");
        };
        assert!(reason.contains("aider"), "got {reason}");
    }

    #[test]
    fn a_session_without_an_assigned_id_is_unknown() {
        let profile = CliProfile::resolve("claude", &HashMap::new());
        let UsageReport::Unknown(reason) = profile.usage(None, Path::new("/repo")) else {
            panic!("a session without an id must be unknown");
        };
        assert!(reason.contains("no git-paw-assigned id"), "got {reason}");
    }

    /// A claude-family entry whose `settings_path` relocates its config dir.
    fn relocated(config_dir: &Path) -> HashMap<String, CustomCli> {
        HashMap::from([(
            "claude-oss".to_string(),
            CustomCli {
                command: "claude".to_string(),
                family: Some("claude".to_string()),
                settings_path: Some(config_dir.join("settings.json").display().to_string()),
                ..CustomCli::default()
            },
        )])
    }

    fn write_session(project_dir: &Path, main: &str, subagent: Option<&str>) {
        std::fs::create_dir_all(project_dir).unwrap();
        std::fs::write(project_dir.join(format!("{SID}.jsonl")), main).unwrap();
        if let Some(sub) = subagent {
            let dir = project_dir.join(SID).join("subagents");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("agent-1.jsonl"), sub).unwrap();
        }
    }

    #[test]
    fn the_adapter_reads_the_session_and_its_subagents_from_the_relocated_dir() {
        let config = tempfile::tempdir().unwrap();
        let worktree = Path::new("/work/repo/.git-paw/worktrees/feat-x");
        let project = config
            .path()
            .join("projects")
            .join(claude_project_slug(worktree));
        write_session(&project, SESSION, Some(SUBAGENT));
        let profile = CliProfile::resolve("claude-oss", &relocated(config.path()));
        assert_eq!(
            profile.usage(Some(SID), worktree),
            UsageReport::Known(totals(10, 160, 2500, 820))
        );
    }

    #[test]
    fn the_adapter_finds_a_session_filed_under_another_project_name() {
        let config = tempfile::tempdir().unwrap();
        write_session(
            &config.path().join("projects").join("elsewhere"),
            SESSION,
            None,
        );
        let profile = CliProfile::resolve("claude-oss", &relocated(config.path()));
        assert_eq!(
            profile.usage(Some(SID), Path::new("/somewhere/else")),
            UsageReport::Known(totals(5, 150, 2500, 520))
        );
    }

    #[test]
    fn the_adapter_skips_a_record_still_being_written_but_not_a_malformed_one() {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects").join("p");
        let profile = CliProfile::resolve("claude-oss", &relocated(config.path()));

        write_session(&project, &format!("{SESSION}{{\"type\":\"assist"), None);
        assert_eq!(
            profile.usage(Some(SID), Path::new("/x")),
            UsageReport::Known(totals(5, 150, 2500, 520))
        );

        write_session(&project, MALFORMED, None);
        let report = profile.usage(Some(SID), Path::new("/x"));
        let UsageReport::Unknown(reason) = report else {
            panic!("expected unknown, got {report:?}");
        };
        assert!(!reason.contains("SECRET"), "got {reason}");
    }

    /// Scenario "A re-read parses only appended records".
    #[test]
    fn a_cached_re_read_parses_only_the_appended_lines() {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects").join("p");
        let profile = CliProfile::resolve("claude-oss", &relocated(config.path()));
        let cache = UsageCache::new();
        let read = || profile.usage_cached(Some(SID), Path::new("/x"), Some(&cache));

        write_session(&project, SESSION, None);
        assert_eq!(read(), UsageReport::Known(totals(5, 150, 2500, 520)));
        let first = cache.lines_parsed();
        assert_eq!(first, SESSION.lines().count() as u64);

        assert_eq!(read(), UsageReport::Known(totals(5, 150, 2500, 520)));
        assert_eq!(
            cache.lines_parsed(),
            first,
            "an unchanged report is not re-parsed"
        );

        write_session(&project, &format!("{SESSION}{SUBAGENT}"), None);
        assert_eq!(read(), UsageReport::Known(totals(10, 160, 2500, 820)));
        assert_eq!(
            cache.lines_parsed(),
            first + SUBAGENT.lines().count() as u64,
            "only the appended lines are parsed"
        );

        // A rewritten, shorter report starts over rather than keeping stale totals.
        write_session(&project, SUBAGENT, None);
        assert_eq!(read(), UsageReport::Known(totals(5, 10, 0, 300)));
    }

    /// Scenario "A rewritten report is read again from its start".
    #[test]
    fn a_report_rewritten_at_the_same_length_is_read_again_from_its_start() {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects").join("p");
        let profile = CliProfile::resolve("claude-oss", &relocated(config.path()));
        let cache = UsageCache::new();
        let read = || profile.usage_cached(Some(SID), Path::new("/x"), Some(&cache));

        write_session(&project, SESSION, None);
        assert_eq!(read(), UsageReport::Known(totals(5, 150, 2500, 520)));

        let rewritten = SESSION.replace("\"output_tokens\":100", "\"output_tokens\":900");
        assert_eq!(rewritten.len(), SESSION.len(), "same length");
        write_session(&project, &rewritten, None);
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(project.join(format!("{SID}.jsonl")))
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(read(), UsageReport::Known(totals(5, 950, 2500, 520)));
    }

    #[test]
    fn a_missing_report_is_found_nowhere() {
        // Searches only the temp dir: the full adapter would also fall back
        // to the developer's real CLAUDE_CONFIG_DIR and home.
        let config = tempfile::tempdir().unwrap();
        assert!(claude_transcripts(config.path(), Path::new("/x"), SID).is_empty());
        std::fs::create_dir_all(config.path().join("projects").join("p")).unwrap();
        assert!(claude_transcripts(config.path(), Path::new("/x"), SID).is_empty());
    }

    #[test]
    fn a_session_id_that_is_not_a_plain_id_is_never_used_as_a_path() {
        let config = tempfile::tempdir().unwrap();
        let projects = config.path().join("projects");
        write_session(&projects.join("p"), SESSION, None);
        // Paths the guard must block: slug dir `-x` climbing into `p/`, and a nested file.
        std::fs::create_dir_all(projects.join("-x")).unwrap();
        std::fs::create_dir_all(projects.join("p").join("a")).unwrap();
        std::fs::write(projects.join("p").join("a").join("b.jsonl"), SESSION).unwrap();
        let traversal = format!("../p/{SID}");
        assert!(
            projects
                .join("-x")
                .join(format!("{traversal}.jsonl"))
                .is_file()
        );
        assert!(!claude_transcripts(config.path(), Path::new("/x"), SID).is_empty());
        for bad in ["", traversal.as_str(), "a/b", ".."] {
            assert!(
                claude_transcripts(config.path(), Path::new("/x"), bad).is_empty(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_project_slug_replaces_every_non_alphanumeric_character() {
        assert_eq!(claude_project_slug(Path::new("/a/b.c/.d_e")), "-a-b-c--d-e");
    }

    #[test]
    fn config_dirs_are_searched_entry_then_env_then_home() {
        let dirs = claude_config_dirs(
            Some(Path::new("/oss")),
            Some(PathBuf::from("/env")),
            Some(PathBuf::from("/home/u")),
        );
        assert_eq!(
            dirs,
            [
                PathBuf::from("/oss"),
                PathBuf::from("/env"),
                PathBuf::from("/home/u/.claude")
            ]
        );
        assert_eq!(
            claude_config_dirs(None, Some(PathBuf::new()), Some(PathBuf::from("/h"))),
            [PathBuf::from("/h/.claude")]
        );
    }

    #[test]
    fn a_summary_groups_known_usage_by_cli_and_lists_the_unknown() {
        let a = UsageReport::Known(totals(1, 10, 0, 0));
        let b = UsageReport::Known(totals(2, 20, 0, 0));
        let c = UsageReport::Known(totals(4, 40, 0, 0));
        let none = UsageReport::Unknown("aider reports no usage git-paw can read".to_string());
        let summary = UsageSummary::from_reports([
            ("sonnet", "feat/a", &a),
            ("sonnet", "feat/b", &b),
            ("opus", "feat/c", &c),
            ("aider", "feat/d", &none),
        ]);
        assert_eq!(summary.by_cli["sonnet"].tokens(), 33);
        assert_eq!(summary.by_cli["opus"].tokens(), 44);
        assert!(!summary.by_cli.contains_key("aider"), "unknown is not zero");
        assert_eq!(summary.total.tokens(), 77);
        assert_eq!(summary.unknown.len(), 1);
        assert_eq!(summary.unknown[0].0, "feat/d");
        assert!(summary.any_known());
        assert!(!UsageSummary::from_reports([("aider", "x", &none)]).any_known());
    }

    #[test]
    fn a_sum_shows_cost_only_when_every_item_reports_one() {
        let with = UsageTotals {
            cost: Some(1.5),
            ..totals(1, 2, 3, 4)
        };
        let without = totals(10, 20, 30, 40);
        assert_eq!(UsageTotals::sum([&with, &with]).cost, Some(3.0));
        let mixed = UsageTotals::sum([&with, &without]);
        assert_eq!(mixed.cost, None);
        assert_eq!(mixed.tokens(), 110, "all four token kinds of both items");
        assert_eq!(UsageTotals::sum([]), UsageTotals::default());
    }
}
