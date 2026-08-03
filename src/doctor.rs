//! Read-only preflight diagnostics for `git paw doctor`.
//!
//! Answers "why won't it launch?" in one command. Doctor probes the
//! environment, the AI CLIs on `PATH`, the parsed configuration, the resolved
//! spec system, the bundled helper scripts, the broker port, the supervisor's
//! gate commands, and repository hygiene, then prints a grouped report where
//! every check carries a ✓ / ⚠ / ✗ status and every non-✓ check carries an
//! actionable remedy.
//!
//! The module has two layers (design D3):
//!
//! - a **probe layer** ([`collect_probes`]) that performs every side-effecting
//!   read — `which` lookups, `--version` banners, config and `.gitignore`
//!   reads, a TCP port probe, directory listings — and captures the answers as
//!   plain data;
//! - **pure check functions** ([`check_environment`], [`check_clis`],
//!   [`check_config`], [`check_spec_system`], [`check_bundled_scripts`],
//!   [`check_broker`], [`check_supervisor`], [`check_hygiene`]) that map probed
//!   data to [`CheckResult`]s. They do no I/O, so every ✓/⚠/✗ decision is
//!   unit-testable without a real environment.
//!
//! Doctor is **diagnose-only**: it never creates, modifies, or deletes a file,
//! config, session, or any other persistent state, and it exposes no repair
//! mode (`--fix` is deferred past v0.13.0).
//!
//! Ambiguous states resolve to ⚠ rather than ✗ so a diagnostic never blocks a
//! launch spuriously; only a genuinely missing prerequisite is ✗.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::config::{PawConfig, WorktreePlacement};
use crate::error::PawError;

/// Environment group heading.
pub const GROUP_ENVIRONMENT: &str = "Environment";
/// AI-CLI availability group heading.
pub const GROUP_CLIS: &str = "CLIs";
/// Configuration group heading.
pub const GROUP_CONFIG: &str = "Config";
/// Spec-system group heading.
pub const GROUP_SPEC_SYSTEM: &str = "Spec system";
/// Bundled helper-script group heading.
pub const GROUP_BUNDLED_SCRIPTS: &str = "Bundled scripts";
/// Broker group heading.
pub const GROUP_BROKER: &str = "Broker";
/// Supervisor group heading.
pub const GROUP_SUPERVISOR: &str = "Supervisor";
/// Repository-hygiene group heading.
pub const GROUP_HYGIENE: &str = "Hygiene";

/// Minimum supported `git` version — `git worktree`, the primitive git-paw is
/// built on, landed in 2.5.
pub const MIN_GIT_VERSION: (u32, u32) = (2, 5);

/// Minimum supported `tmux` version — pane zoom (`-Z`) and the `#{…}` format
/// vocabulary the layout code depends on landed in 1.8.
pub const MIN_TMUX_VERSION: (u32, u32) = (1, 8);

/// Status of a single diagnostic check.
///
/// The ordering is severity ordering (`Ok` < `Warn` < `Fail`), so the report's
/// overall status is the maximum over its checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    /// The check passed, or is informational.
    Ok,
    /// The check found something worth fixing that does not block a launch.
    Warn,
    /// The check found a hard blocker.
    Fail,
}

impl CheckStatus {
    /// Returns the report glyph for this status (`✓`, `⚠`, or `✗`).
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Ok => "\u{2713}",
            Self::Warn => "\u{26a0}",
            Self::Fail => "\u{2717}",
        }
    }
}

/// The outcome of one diagnostic check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    /// Report heading this check is printed under (one of the `GROUP_*`
    /// constants).
    pub group: &'static str,
    /// Short check name, unique within its group.
    pub name: String,
    /// The check's verdict.
    pub status: CheckStatus,
    /// What was observed.
    pub detail: String,
    /// How to fix it. Always `Some` for a non-[`CheckStatus::Ok`] check, and
    /// always `None` for an `Ok` one.
    pub remedy: Option<String>,
}

impl CheckResult {
    /// Builds a passing (or informational) check.
    #[must_use]
    pub fn ok(group: &'static str, name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            group,
            name: name.into(),
            status: CheckStatus::Ok,
            detail: detail.into(),
            remedy: None,
        }
    }

    /// Builds a warning check — something to fix that does not block a launch.
    #[must_use]
    pub fn warn(
        group: &'static str,
        name: impl Into<String>,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            group,
            name: name.into(),
            status: CheckStatus::Warn,
            detail: detail.into(),
            remedy: Some(remedy.into()),
        }
    }

    /// Builds a failing check — a hard blocker.
    #[must_use]
    pub fn fail(
        group: &'static str,
        name: impl Into<String>,
        detail: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            group,
            name: name.into(),
            status: CheckStatus::Fail,
            detail: detail.into(),
            remedy: Some(remedy.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Probed inputs
// ---------------------------------------------------------------------------

/// An external tool looked up on `PATH`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolProbe {
    /// Resolved path to the binary, or `None` when it is not on `PATH`.
    pub path: Option<String>,
    /// First line of the tool's version banner, when it could be read.
    pub version: Option<String>,
}

/// Environment facts: the two hard tool dependencies and repository presence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentProbe {
    /// The `git` binary.
    pub git: ToolProbe,
    /// The `tmux` binary.
    pub tmux: ToolProbe,
    /// Whether the working directory is inside a git repository.
    pub in_repo: bool,
}

/// One AI CLI that resolved on `PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliProbe {
    /// The CLI's binary name.
    pub name: String,
    /// `true` when the entry came from a `[clis.*]` config block rather than
    /// the known roster.
    pub custom: bool,
}

/// Configuration facts read from `.git-paw/config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigProbe {
    /// Display path of the repo-level config file.
    pub path: String,
    /// Whether that file exists.
    pub present: bool,
    /// The load error, when the config could not be parsed.
    pub parse_error: Option<String>,
    /// The resolved worktree placement (defaults apply when the file is
    /// absent).
    pub placement: WorktreePlacement,
    /// Dotted key paths present in the file that git-paw does not recognise.
    pub unknown_keys: Vec<String>,
}

/// Spec-system resolution facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecSystemProbe {
    /// The explicitly configured spec format, or `None` when `[specs]` is
    /// absent (there is no filesystem auto-detection).
    pub resolved_type: Option<String>,
    /// Number of specs the backend discovered.
    pub spec_count: Option<usize>,
    /// The scan error, when a configured spec system could not be scanned.
    pub scan_error: Option<String>,
}

/// State of one bundled helper script on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptProbe {
    /// File name, e.g. `sweep.sh`.
    pub name: &'static str,
    /// Whether the file exists under `.git-paw/scripts/`.
    pub present: bool,
    /// Whether it carries the executable bit (always `true` off Unix).
    pub executable: bool,
    /// Whether its content matches the running binary's embedded copy.
    pub matches_embedded: bool,
}

/// Bundled-script facts, plus the interpreter every bundled script needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundledScriptsProbe {
    /// One entry per script `git paw init` installs.
    pub scripts: Vec<ScriptProbe>,
    /// Description of the Python 3 interpreter found on `PATH`, or `None`.
    pub python3: Option<String>,
}

/// What is listening on the broker's configured address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Nothing is listening — the broker can bind.
    Free,
    /// A git-paw broker is already reachable there.
    LiveBroker,
    /// Some other service holds the port.
    Foreign,
    /// The probe could not reach a verdict (it timed out).
    Unknown,
}

/// Broker facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerProbe {
    /// Whether `[broker] enabled = true`.
    pub enabled: bool,
    /// The configured bind address.
    pub bind: String,
    /// The configured port.
    pub port: u16,
    /// What holds the address. Meaningless (and not probed) when disabled.
    pub port_state: PortState,
}

impl Default for BrokerProbe {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: String::new(),
            port: 0,
            port_state: PortState::Free,
        }
    }
}

/// One configured supervisor gate command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCommandProbe {
    /// The `[supervisor]` field the command came from, e.g. `test_command`.
    pub label: &'static str,
    /// The configured command string, verbatim.
    pub command: String,
    /// Its leading token — the binary the command invokes.
    pub binary: String,
    /// Whether that binary resolves on `PATH`.
    pub on_path: bool,
}

/// Supervisor facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupervisorProbe {
    /// Whether `[supervisor] enabled = true`.
    pub enabled: bool,
    /// The gate commands the operator configured, in config-field order. The
    /// verbs come from the resolved stack preset, never from a hard-coded
    /// git-paw toolchain (design D7).
    pub gates: Vec<GateCommandProbe>,
    /// Whether `.git-paw/scripts/sweep.sh` is installed.
    pub sweep_installed: bool,
}

/// Repository-hygiene facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HygieneProbe {
    /// Required `.gitignore` entries that are absent.
    pub missing_gitignore_entries: Vec<String>,
    /// Session names whose receipt claims active but whose tmux session is
    /// gone.
    pub stale_sessions: Vec<String>,
    /// Worktree paths a session receipt registers that no longer exist.
    pub orphaned_worktrees: Vec<String>,
}

/// Every probed input the check functions consume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probes {
    /// Environment facts.
    pub environment: EnvironmentProbe,
    /// Detected AI CLIs.
    pub clis: Vec<CliProbe>,
    /// Configuration facts.
    pub config: ConfigProbe,
    /// Spec-system facts.
    pub spec_system: SpecSystemProbe,
    /// Bundled-script facts.
    pub bundled_scripts: BundledScriptsProbe,
    /// Broker facts.
    pub broker: BrokerProbe,
    /// Supervisor facts.
    pub supervisor: SupervisorProbe,
    /// Hygiene facts.
    pub hygiene: HygieneProbe,
}

// ---------------------------------------------------------------------------
// Pure checks
// ---------------------------------------------------------------------------

/// Extracts the leading `major.minor` pair from a version banner.
///
/// Scans for the first digit run immediately followed by `.` and another digit
/// run, so it copes with the prefixes and suffixes real tools emit —
/// `git version 2.39.3 (Apple Git-146)`, `tmux 3.4`, `tmux next-3.5`,
/// `tmux 3.3a`. Returns `None` when no such pair is present, which callers
/// treat as "version unknown" (⚠) rather than "too old" (✗).
#[must_use]
pub fn parse_version(raw: &str) -> Option<(u32, u32)> {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let major_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let major_end = i;
        if i >= bytes.len() || bytes[i] != b'.' {
            continue;
        }
        i += 1;
        let minor_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if minor_start == i {
            continue;
        }
        let major = raw[major_start..major_end].parse().ok()?;
        let minor = raw[minor_start..i].parse().ok()?;
        return Some((major, minor));
    }
    None
}

/// Judges one required external tool against its minimum version.
fn check_tool(
    name: &'static str,
    probe: &ToolProbe,
    minimum: (u32, u32),
    install_hint: &str,
) -> CheckResult {
    let (min_major, min_minor) = minimum;
    let Some(path) = probe.path.as_deref() else {
        return CheckResult::fail(
            GROUP_ENVIRONMENT,
            name,
            format!("{name} is not on PATH"),
            install_hint.to_string(),
        );
    };
    let banner = probe.version.as_deref().unwrap_or("");
    match parse_version(banner) {
        Some(version) if version >= minimum => {
            CheckResult::ok(GROUP_ENVIRONMENT, name, format!("{banner} ({path})"))
        }
        Some((major, minor)) => CheckResult::fail(
            GROUP_ENVIRONMENT,
            name,
            format!("{name} {major}.{minor} is older than the required {min_major}.{min_minor}"),
            format!("upgrade {name} to {min_major}.{min_minor} or newer — {install_hint}"),
        ),
        None => CheckResult::warn(
            GROUP_ENVIRONMENT,
            name,
            format!("found at {path} but its version could not be determined"),
            format!(
                "check that `{name} --version` prints a version — git-paw needs {name} {min_major}.{min_minor} or newer"
            ),
        ),
    }
}

/// Environment checks: `git` and `tmux` present and recent enough, and the
/// working directory inside a git repository.
#[must_use]
pub fn check_environment(probe: &EnvironmentProbe) -> Vec<CheckResult> {
    vec![
        check_tool(
            "git",
            &probe.git,
            MIN_GIT_VERSION,
            "install git from https://git-scm.com/downloads",
        ),
        check_tool(
            "tmux",
            &probe.tmux,
            MIN_TMUX_VERSION,
            "install tmux: `brew install tmux` (macOS) or `apt install tmux` (Linux)",
        ),
        if probe.in_repo {
            CheckResult::ok(
                GROUP_ENVIRONMENT,
                "git repository",
                "the working directory is inside a git repository",
            )
        } else {
            CheckResult::fail(
                GROUP_ENVIRONMENT,
                "git repository",
                "the working directory is not inside a git repository",
                "run git-paw from inside a git project, or `git init` here first",
            )
        },
    ]
}

/// CLI-availability check: the detected roster, or the `NoCLIsFound` launch
/// condition surfaced early as a ⚠.
#[must_use]
pub fn check_clis(clis: &[CliProbe]) -> Vec<CheckResult> {
    if clis.is_empty() {
        return vec![CheckResult::warn(
            GROUP_CLIS,
            "detected CLIs",
            "no AI CLI resolved on PATH — a launch would fail with `No AI CLIs found`",
            "install an AI CLI (claude, codex, agy, …), or register one with \
             `git paw add-cli <name> <command>` / a `[clis.<name>]` config entry",
        )];
    }
    let listed = clis
        .iter()
        .map(|c| {
            if c.custom {
                format!("{} (custom)", c.name)
            } else {
                c.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    vec![CheckResult::ok(
        GROUP_CLIS,
        "detected CLIs",
        format!("{} available: {listed}", clis.len()),
    )]
}

/// Config checks: the file parses, the resolved worktree placement, and any
/// key git-paw does not recognise.
#[must_use]
pub fn check_config(probe: &ConfigProbe) -> Vec<CheckResult> {
    let mut checks = Vec::new();

    if let Some(error) = &probe.parse_error {
        checks.push(CheckResult::fail(
            GROUP_CONFIG,
            "config.toml",
            format!("could not be loaded: {error}"),
            format!(
                "fix the TOML syntax in {} (or move it aside to fall back to built-in defaults)",
                probe.path
            ),
        ));
    } else if probe.present {
        checks.push(CheckResult::ok(
            GROUP_CONFIG,
            "config.toml",
            format!("{} parses", probe.path),
        ));
    } else {
        checks.push(CheckResult::warn(
            GROUP_CONFIG,
            "config.toml",
            format!(
                "{} does not exist — running on built-in defaults",
                probe.path
            ),
            "run `git paw init` to create it",
        ));
    }

    checks.push(CheckResult::ok(
        GROUP_CONFIG,
        "worktree placement",
        match probe.placement {
            WorktreePlacement::Child => {
                "child — worktrees live in <repo>/.git-paw/worktrees/".to_string()
            }
            WorktreePlacement::Sibling => {
                "sibling — worktrees live beside the repository".to_string()
            }
        },
    ));

    if !probe.unknown_keys.is_empty() {
        checks.push(CheckResult::warn(
            GROUP_CONFIG,
            "unknown fields",
            format!(
                "{} ignored by this version: {}",
                probe.unknown_keys.len(),
                probe.unknown_keys.join(", ")
            ),
            "remove the keys, correct the spelling, or upgrade git-paw if they \
             belong to a newer version",
        ));
    }

    checks
}

/// Spec-system check: the explicitly resolved format and how many specs it
/// discovered, or actionable guidance when nothing is configured.
#[must_use]
pub fn check_spec_system(probe: &SpecSystemProbe) -> Vec<CheckResult> {
    let Some(spec_type) = probe.resolved_type.as_deref() else {
        return vec![CheckResult::warn(
            GROUP_SPEC_SYSTEM,
            "spec system",
            "no spec system configured — spec-driven launch is unavailable",
            "add a [specs] section to .git-paw/config.toml \
             (type = \"openspec\" | \"markdown\" | \"speckit\" | \"superpowers\") \
             or pass --specs-format",
        )];
    };

    if let Some(error) = &probe.scan_error {
        return vec![CheckResult::warn(
            GROUP_SPEC_SYSTEM,
            "spec system",
            format!("{spec_type} is configured but could not be scanned: {error}"),
            "correct the [specs] `dir`/`type` in .git-paw/config.toml so the \
             spec directory resolves",
        )];
    }

    let count = probe.spec_count.unwrap_or(0);
    vec![CheckResult::ok(
        GROUP_SPEC_SYSTEM,
        "spec system",
        format!("{spec_type} — {count} spec(s) discovered"),
    )]
}

/// Bundled-script checks: each helper present, executable, and matching the
/// running binary's embedded copy, plus the Python 3 interpreter they need.
#[must_use]
pub fn check_bundled_scripts(probe: &BundledScriptsProbe) -> Vec<CheckResult> {
    const REINSTALL: &str = "run `git paw init` to (re)install the bundled helper scripts";

    let mut checks: Vec<CheckResult> = probe
        .scripts
        .iter()
        .map(|script| {
            if !script.present {
                CheckResult::fail(
                    GROUP_BUNDLED_SCRIPTS,
                    script.name,
                    format!(".git-paw/scripts/{} is missing", script.name),
                    REINSTALL,
                )
            } else if !script.executable {
                CheckResult::fail(
                    GROUP_BUNDLED_SCRIPTS,
                    script.name,
                    format!(".git-paw/scripts/{} is not executable", script.name),
                    REINSTALL,
                )
            } else if script.matches_embedded {
                CheckResult::ok(
                    GROUP_BUNDLED_SCRIPTS,
                    script.name,
                    "installed, executable, matches this binary",
                )
            } else {
                CheckResult::warn(
                    GROUP_BUNDLED_SCRIPTS,
                    script.name,
                    format!(
                        ".git-paw/scripts/{} differs from this binary's embedded version",
                        script.name
                    ),
                    REINSTALL,
                )
            }
        })
        .collect();

    checks.push(match &probe.python3 {
        Some(found) => CheckResult::ok(GROUP_BUNDLED_SCRIPTS, "python3", found.clone()),
        None => CheckResult::warn(
            GROUP_BUNDLED_SCRIPTS,
            "python3",
            "no Python 3 interpreter on PATH — the bundled helper scripts need one",
            "install Python 3 (`brew install python` on macOS, `apt install python3` \
             on Linux); core start/add/remove keeps working without it",
        ),
    });

    checks
}

/// Broker check: the configured address is free or already serving a broker,
/// or the informational pure-manual baseline when the broker is disabled.
#[must_use]
pub fn check_broker(probe: &BrokerProbe) -> Vec<CheckResult> {
    if !probe.enabled {
        return vec![CheckResult::ok(
            GROUP_BROKER,
            "broker",
            "disabled — the pure-manual baseline (no agent coordination, no dashboard feed)",
        )];
    }

    let address = format!("{}:{}", probe.bind, probe.port);
    vec![match probe.port_state {
        PortState::Free => CheckResult::ok(
            GROUP_BROKER,
            "broker",
            format!("enabled — {address} is free to bind"),
        ),
        PortState::LiveBroker => CheckResult::ok(
            GROUP_BROKER,
            "broker",
            format!("enabled — a git-paw broker is already reachable at {address}"),
        ),
        PortState::Foreign => CheckResult::warn(
            GROUP_BROKER,
            "broker",
            format!("enabled — {address} is held by another service"),
            "stop that service, or set a free `[broker] port` in .git-paw/config.toml",
        ),
        PortState::Unknown => CheckResult::warn(
            GROUP_BROKER,
            "broker",
            format!("enabled — the probe of {address} timed out"),
            "re-run doctor; if it keeps timing out, set a different `[broker] port`",
        ),
    }]
}

/// Supervisor checks: every configured gate command resolves on `PATH`, and
/// `sweep.sh` is installed.
///
/// The verbs probed come from the operator's `[supervisor]` gate-command
/// configuration — the resolved stack preset — so the check stays
/// project-agnostic and never assumes git-paw's own toolchain (design D7).
#[must_use]
pub fn check_supervisor(probe: &SupervisorProbe) -> Vec<CheckResult> {
    if !probe.enabled {
        return vec![CheckResult::ok(
            GROUP_SUPERVISOR,
            "supervisor",
            "disabled — agents run without a supervising pane",
        )];
    }

    let mut checks = Vec::new();

    if probe.gates.is_empty() {
        checks.push(CheckResult::ok(
            GROUP_SUPERVISOR,
            "gate commands",
            "none configured — the five-gate verification runs its manual steps only",
        ));
    } else {
        for gate in &probe.gates {
            if gate.on_path {
                checks.push(CheckResult::ok(
                    GROUP_SUPERVISOR,
                    gate.label,
                    format!("`{}` — `{}` is on PATH", gate.command, gate.binary),
                ));
            } else {
                checks.push(CheckResult::fail(
                    GROUP_SUPERVISOR,
                    gate.label,
                    format!("`{}` — `{}` is not on PATH", gate.command, gate.binary),
                    format!(
                        "install `{}`, or point `[supervisor] {}` at a command this \
                         machine has",
                        gate.binary, gate.label
                    ),
                ));
            }
        }
    }

    checks.push(if probe.sweep_installed {
        CheckResult::ok(
            GROUP_SUPERVISOR,
            "sweep.sh",
            ".git-paw/scripts/sweep.sh is installed",
        )
    } else {
        CheckResult::fail(
            GROUP_SUPERVISOR,
            "sweep.sh",
            ".git-paw/scripts/sweep.sh is missing — the supervisor helper cannot run",
            "run `git paw init` to install it",
        )
    });

    checks
}

/// Hygiene checks: required `.gitignore` entries, stale session receipts, and
/// worktree registrations pointing at directories that no longer exist.
#[must_use]
pub fn check_hygiene(probe: &HygieneProbe) -> Vec<CheckResult> {
    const PURGE_STALE: &str = "run `git paw purge --stale`";

    let mut checks = Vec::new();

    checks.push(if probe.missing_gitignore_entries.is_empty() {
        CheckResult::ok(
            GROUP_HYGIENE,
            ".gitignore",
            "every git-paw entry is ignored",
        )
    } else {
        CheckResult::warn(
            GROUP_HYGIENE,
            ".gitignore",
            format!(
                "missing {}: {}",
                probe.missing_gitignore_entries.len(),
                probe.missing_gitignore_entries.join(", ")
            ),
            "run `git paw init` to add the missing entries",
        )
    });

    checks.push(if probe.stale_sessions.is_empty() {
        CheckResult::ok(GROUP_HYGIENE, "session state", "no stale session receipt")
    } else {
        CheckResult::warn(
            GROUP_HYGIENE,
            "session state",
            format!(
                "{} receipt(s) claim active but their tmux session is gone: {}",
                probe.stale_sessions.len(),
                probe.stale_sessions.join(", ")
            ),
            PURGE_STALE,
        )
    });

    checks.push(if probe.orphaned_worktrees.is_empty() {
        CheckResult::ok(
            GROUP_HYGIENE,
            "worktree registrations",
            "every registered worktree exists on disk",
        )
    } else {
        CheckResult::warn(
            GROUP_HYGIENE,
            "worktree registrations",
            format!(
                "{} registered worktree(s) no longer exist: {}",
                probe.orphaned_worktrees.len(),
                probe.orphaned_worktrees.join(", ")
            ),
            PURGE_STALE,
        )
    });

    checks
}

/// Runs every check function over `probes`, in report order.
#[must_use]
pub fn run_checks(probes: &Probes) -> Vec<CheckResult> {
    let mut checks = check_environment(&probes.environment);
    checks.extend(check_clis(&probes.clis));
    checks.extend(check_config(&probes.config));
    checks.extend(check_spec_system(&probes.spec_system));
    checks.extend(check_bundled_scripts(&probes.bundled_scripts));
    checks.extend(check_broker(&probes.broker));
    checks.extend(check_supervisor(&probes.supervisor));
    checks.extend(check_hygiene(&probes.hygiene));
    checks
}

// ---------------------------------------------------------------------------
// Verdict + rendering
// ---------------------------------------------------------------------------

/// Returns the worst status across `checks` — the report's overall verdict.
#[must_use]
pub fn worst_status(checks: &[CheckResult]) -> CheckStatus {
    checks
        .iter()
        .map(|c| c.status)
        .max()
        .unwrap_or(CheckStatus::Ok)
}

/// Returns the process exit code for `checks`: non-zero when any check failed,
/// zero when the worst status is a warning or better.
#[must_use]
pub fn exit_code(checks: &[CheckResult]) -> i32 {
    match worst_status(checks) {
        CheckStatus::Fail => crate::error::exit_code::ERROR,
        CheckStatus::Ok | CheckStatus::Warn => 0,
    }
}

/// The JSON document `--json` emits.
#[derive(Debug, Serialize)]
struct Report<'a> {
    /// The worst status across every check.
    status: CheckStatus,
    /// Every check, in report order.
    checks: &'a [CheckResult],
}

/// Renders the grouped human-readable report.
#[must_use]
pub fn render_human(checks: &[CheckResult]) -> String {
    let mut out = String::new();
    let width = checks
        .iter()
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(0);
    let mut current_group = "";

    for check in checks {
        if check.group != current_group {
            if !current_group.is_empty() {
                out.push('\n');
            }
            out.push_str(check.group);
            out.push('\n');
            current_group = check.group;
        }
        let _ = writeln!(
            out,
            "  {} {:width$}  {}",
            check.status.glyph(),
            check.name,
            check.detail
        );
        if let Some(remedy) = &check.remedy {
            let _ = writeln!(out, "      \u{21b3} {remedy}");
        }
    }

    let mut counts: BTreeMap<CheckStatus, usize> = BTreeMap::new();
    for check in checks {
        *counts.entry(check.status).or_default() += 1;
    }
    let _ = writeln!(
        out,
        "\n{} {} \u{00b7} {} {} \u{00b7} {} {}",
        counts.get(&CheckStatus::Ok).copied().unwrap_or(0),
        CheckStatus::Ok.glyph(),
        counts.get(&CheckStatus::Warn).copied().unwrap_or(0),
        CheckStatus::Warn.glyph(),
        counts.get(&CheckStatus::Fail).copied().unwrap_or(0),
        CheckStatus::Fail.glyph(),
    );

    out
}

/// Renders the machine-readable `--json` document.
///
/// # Errors
///
/// Returns [`PawError::SessionError`] if the report cannot be serialised.
pub fn render_json(checks: &[CheckResult]) -> Result<String, PawError> {
    let report = Report {
        status: worst_status(checks),
        checks,
    };
    serde_json::to_string_pretty(&report)
        .map_err(|e| PawError::SessionError(format!("could not serialise the doctor report: {e}")))
}

// ---------------------------------------------------------------------------
// Probe layer (all I/O lives here)
// ---------------------------------------------------------------------------

/// Looks a tool up on `PATH` and reads the first line of its version banner.
fn probe_tool(binary: &str, version_arg: &str) -> ToolProbe {
    let Ok(path) = which::which(binary) else {
        return ToolProbe::default();
    };
    let version = Command::new(binary)
        .arg(version_arg)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| first_line(&out.stdout).or_else(|| first_line(&out.stderr)));
    ToolProbe {
        path: Some(path.display().to_string()),
        version,
    }
}

/// Returns the first non-empty line of a command's output.
fn first_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

/// Finds a Python 3 interpreter: `python3`, or a `python` that reports major
/// version 3. Returns a human-readable description of the one it found.
fn probe_python3() -> Option<String> {
    for binary in ["python3", "python"] {
        if which::which(binary).is_err() {
            continue;
        }
        // Python 2 wrote its banner to stderr; probe_tool already falls back.
        let probe = probe_tool(binary, "--version");
        let Some(banner) = probe.version.as_deref() else {
            continue;
        };
        if let Some((3, _)) = parse_version(banner) {
            return Some(format!("{banner} ({binary})"));
        }
    }
    None
}

/// Returns the dotted key paths in `raw_toml` that [`PawConfig`] does not
/// recognise.
///
/// Works by round-tripping: a key serde understood survives
/// `TOML → PawConfig → TOML`, while a key it ignored does not. That keeps the
/// check self-maintaining — a config field added later needs no update here.
/// Any parse or serialisation failure yields an empty list, so the check never
/// invents a warning it cannot substantiate.
#[must_use]
pub fn unknown_config_keys(raw_toml: &str) -> Vec<String> {
    let Ok(raw) = toml::from_str::<toml::Value>(raw_toml) else {
        return Vec::new();
    };
    let Ok(parsed) = toml::from_str::<PawConfig>(raw_toml) else {
        return Vec::new();
    };
    let Ok(recognised) = toml::Value::try_from(&parsed) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_unknown_keys("", &raw, &recognised, &mut out);
    out
}

/// Walks `raw` against `recognised`, recording every key path missing from the
/// latter.
fn collect_unknown_keys(
    prefix: &str,
    raw: &toml::Value,
    recognised: &toml::Value,
    out: &mut Vec<String>,
) {
    let (Some(raw_table), Some(recognised_table)) = (raw.as_table(), recognised.as_table()) else {
        return;
    };
    for (key, value) in raw_table {
        let path = format!("{prefix}{key}");
        match recognised_table.get(key) {
            None => out.push(path),
            Some(known) => collect_unknown_keys(&format!("{path}."), value, known, out),
        }
    }
}

/// Gathers every probed input for the repository rooted at `repo_root`.
///
/// Read-only: every operation here is a lookup, a read, or a connect-probe.
fn collect_probes(repo_root: &Path, environment: EnvironmentProbe) -> Probes {
    let config_path = crate::config::repo_config_path(repo_root);
    let raw_config = std::fs::read_to_string(&config_path).ok();
    let (config, parse_error) = match crate::config::load_config(repo_root, None) {
        Ok(config) => (config, None),
        Err(e) => (PawConfig::default(), Some(e.to_string())),
    };

    let config_probe = ConfigProbe {
        path: display_relative(repo_root, &config_path),
        present: raw_config.is_some(),
        parse_error,
        placement: config.worktree_placement(),
        unknown_keys: raw_config
            .as_deref()
            .map(unknown_config_keys)
            .unwrap_or_default(),
    };

    Probes {
        environment,
        clis: probe_clis(&config),
        config: config_probe,
        spec_system: probe_spec_system(&config, repo_root),
        bundled_scripts: probe_bundled_scripts(repo_root),
        broker: probe_broker(&config),
        supervisor: probe_supervisor(&config, repo_root),
        hygiene: probe_hygiene(repo_root),
    }
}

/// Renders `path` relative to `repo_root` when possible, for compact output.
fn display_relative(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Resolves the AI CLIs available from the known roster plus `[clis.*]`.
fn probe_clis(config: &PawConfig) -> Vec<CliProbe> {
    let custom: Vec<crate::detect::CustomCliDef> = config
        .clis
        .iter()
        .map(|(name, cli)| crate::detect::CustomCliDef {
            name: name.clone(),
            command: cli.command.clone(),
            display_name: cli.display_name.clone(),
        })
        .collect();
    crate::detect::detect_clis(&custom)
        .into_iter()
        .map(|cli| CliProbe {
            name: cli.binary_name,
            custom: cli.source == crate::detect::CliSource::Custom,
        })
        .collect()
}

/// Resolves the configured spec system and counts what it discovers.
fn probe_spec_system(config: &PawConfig, repo_root: &Path) -> SpecSystemProbe {
    let Some(resolved_type) = crate::specs::resolved_spec_type(config, repo_root) else {
        return SpecSystemProbe::default();
    };
    match crate::specs::scan_specs(config, repo_root) {
        Ok(entries) => SpecSystemProbe {
            resolved_type: Some(resolved_type),
            spec_count: Some(entries.len()),
            scan_error: None,
        },
        Err(e) => SpecSystemProbe {
            resolved_type: Some(resolved_type),
            spec_count: None,
            scan_error: Some(e.to_string()),
        },
    }
}

/// Inspects `.git-paw/scripts/` against the binary's embedded helper scripts.
fn probe_bundled_scripts(repo_root: &Path) -> BundledScriptsProbe {
    let scripts_dir = repo_root.join(".git-paw").join("scripts");
    let scripts = crate::init::bundled_scripts()
        .into_iter()
        .map(|(name, embedded)| {
            let path = scripts_dir.join(name);
            let content = std::fs::read_to_string(&path).ok();
            ScriptProbe {
                name,
                present: content.is_some(),
                executable: is_executable(&path),
                matches_embedded: content.is_some_and(|c| c == embedded),
            }
        })
        .collect();
    BundledScriptsProbe {
        scripts,
        python3: probe_python3(),
    }
}

/// Returns whether `path` is a file carrying an owner-execute bit. Off Unix
/// there is no executable bit to read, so an existing file counts.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o100 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Probes the configured broker address, but only when the broker is enabled —
/// a disabled broker is reported without touching the network.
fn probe_broker(config: &PawConfig) -> BrokerProbe {
    let broker = &config.broker;
    let port_state = if broker.enabled {
        match crate::broker::probe_broker(&broker.url()) {
            crate::broker::ProbeResult::NoListener => PortState::Free,
            crate::broker::ProbeResult::LiveBroker => PortState::LiveBroker,
            crate::broker::ProbeResult::ForeignServer => PortState::Foreign,
            crate::broker::ProbeResult::Timeout => PortState::Unknown,
        }
    } else {
        PortState::Free
    };
    BrokerProbe {
        enabled: broker.enabled,
        bind: broker.bind.clone(),
        port: broker.port,
        port_state,
    }
}

/// Resolves the supervisor's configured gate commands to the binaries they
/// invoke and looks each one up on `PATH`.
fn probe_supervisor(config: &PawConfig, repo_root: &Path) -> SupervisorProbe {
    let Some(supervisor) = config.supervisor.as_ref() else {
        return SupervisorProbe::default();
    };
    let gates = supervisor.gate_commands();
    let configured = [
        ("test_command", gates.test_command),
        ("lint_command", gates.lint_command),
        ("build_command", gates.build_command),
        ("fmt_check_command", gates.fmt_check_command),
        ("doc_build_command", gates.doc_build_command),
        ("doc_tool_command", gates.doc_tool_command),
        ("spec_validate_command", gates.spec_validate_command),
        ("security_audit_command", gates.security_audit_command),
    ];
    SupervisorProbe {
        enabled: supervisor.enabled,
        gates: configured
            .into_iter()
            .filter_map(|(label, command)| {
                let command = command?.trim();
                let binary = command.split_whitespace().next()?.to_string();
                Some(GateCommandProbe {
                    label,
                    command: command.to_string(),
                    on_path: which::which(&binary).is_ok(),
                    binary,
                })
            })
            .collect(),
        sweep_installed: is_executable(
            &repo_root.join(".git-paw").join("scripts").join("sweep.sh"),
        ),
    }
}

/// Collects `.gitignore` gaps, stale session receipts, and worktree
/// registrations whose directory is gone.
fn probe_hygiene(repo_root: &Path) -> HygieneProbe {
    let ignored: Vec<String> = std::fs::read_to_string(repo_root.join(".gitignore"))
        .map(|content| content.lines().map(|l| l.trim().to_string()).collect())
        .unwrap_or_default();
    let missing_gitignore_entries = crate::init::required_gitignore_entries()
        .iter()
        .filter(|entry| !ignored.iter().any(|line| line == *entry))
        .map(ToString::to_string)
        .collect();

    let (stale_sessions, orphaned_worktrees) =
        match crate::session::find_session_for_repo(repo_root) {
            Ok(Some(session)) => {
                let liveness = crate::tmux::session_liveness(&session.session_name);
                let stale =
                    if crate::session::DisplayStatus::from_receipt(&session.status, liveness)
                        == crate::session::DisplayStatus::Stale
                    {
                        vec![session.session_name.clone()]
                    } else {
                        Vec::new()
                    };
                let orphans = session
                    .worktrees
                    .iter()
                    .filter(|entry| !entry.worktree_path.exists())
                    .map(|entry| entry.worktree_path.display().to_string())
                    .collect();
                (stale, orphans)
            }
            _ => (Vec::new(), Vec::new()),
        };

    HygieneProbe {
        missing_gitignore_entries,
        stale_sessions,
        orphaned_worktrees,
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Runs `git paw doctor`.
///
/// Prints the grouped report (or the `--json` document when `json` is set) and
/// returns an error only when a check hard-failed, so the process exit code is
/// the worst check's severity. Nothing is written to disk.
///
/// When the working directory is not inside a git repository, the repository
/// -scoped groups cannot be probed; the report then carries the Environment
/// and CLIs groups only, with the Environment group's ✗ naming the cause.
///
/// # Errors
///
/// Returns [`PawError::DoctorFailed`] when any check is ✗, or
/// [`PawError::SessionError`] when the current directory or the JSON document
/// cannot be read/serialised.
pub fn run(json: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = crate::git::validate_repo(&cwd).ok();

    let environment = EnvironmentProbe {
        git: probe_tool("git", "--version"),
        tmux: probe_tool("tmux", "-V"),
        in_repo: repo_root.is_some(),
    };

    let checks = if let Some(root) = &repo_root {
        run_checks(&collect_probes(root, environment))
    } else {
        let mut checks = check_environment(&environment);
        checks.extend(check_clis(&probe_clis(&PawConfig::default())));
        checks
    };

    if json {
        println!("{}", render_json(&checks)?);
    } else {
        print!("{}", render_human(&checks));
    }

    let failures = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();
    if failures > 0 {
        return Err(PawError::DoctorFailed(failures));
    }
    Ok(())
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
