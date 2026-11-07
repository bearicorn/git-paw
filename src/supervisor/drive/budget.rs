//! Usage attribution and the budget guardrail of the unattended drive loop
//! (`worker-usage-budget`).
//!
//! The loop reads attributed usage — every worker's session plus each headless
//! orchestrator invocation it started — through the [`UsageReader`] port, on a
//! cadence slower than its sweep. [`BudgetGuard`] keeps the last known figures
//! of every session seen this run, so the total never goes down when a worker
//! leaves, swaps CLI or turns unknown, compares it with the `[supervisor]`
//! caps and reports the first breach once; the loop then stops every spend it
//! initiates. Unknown usage is never counted as zero, and the operator's own
//! session is outside git-paw's view.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::Escalation;
use crate::agent_cli::{CliProfile, UsageCache, UsageReport, UsageSummary, UsageTotals};
use crate::config::{CustomCli, OnExhausted, SupervisorConfig};
use crate::supervisor::escalation::HeadlessSession;

/// Verdict label of the escalation recorded on the first budget breach.
pub const BUDGET_EXHAUSTED_VERDICT: &str = "budget-exhausted";

/// Verdict label of the escalation recorded at start when `budget_max_cost`
/// is set but not every worker CLI reports cost.
pub const COST_CAP_UNENFORCEABLE_VERDICT: &str = "budget-cost-unenforceable";

/// Whether `escalation` makes a finished wave `escalated-for-review`. The
/// unevaluable-cost-cap notice does not: it describes the config, not the
/// wave, so a clean run with that cap still ends `completed`.
#[must_use]
pub fn awaits_review(escalation: &Escalation) -> bool {
    escalation.verdict != COST_CAP_UNENFORCEABLE_VERDICT
}

/// Sweeps between two usage reads, unless `GIT_PAW_DRIVE_USAGE_EVERY_SWEEPS`
/// overrides it.
pub const DEFAULT_USAGE_EVERY_SWEEPS: u32 = 4;

/// The summary line naming what the usage excludes.
pub const EXCLUDES_OWN_SESSION: &str = "excludes your own session";

/// The `[supervisor]` budget caps and what a breach does.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BudgetCaps {
    /// `budget_max_tokens`: input, output, cache-read and cache-write tokens.
    pub max_tokens: Option<u64>,
    /// `budget_max_cost`, in the CLI's own currency.
    pub max_cost: Option<f64>,
    /// `budget_on_exhausted`: whether a breach ends the run.
    pub on_exhausted: OnExhausted,
}

impl BudgetCaps {
    /// The caps configured in `[supervisor]`.
    #[must_use]
    pub fn from_config(supervisor: &SupervisorConfig) -> Self {
        Self {
            max_tokens: supervisor.budget_max_tokens,
            max_cost: supervisor
                .budget_max_cost
                .map(crate::config::BudgetCost::get),
            on_exhausted: supervisor.budget_on_exhausted.unwrap_or_default(),
        }
    }

    /// Whether any cap is configured.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.max_tokens.is_some() || self.max_cost.is_some()
    }
}

/// Whose spend a session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UsageRole {
    /// A coding agent's session.
    Worker,
    /// A headless orchestrator invocation the loop started.
    Orchestrator,
}

/// One attributed session's report, as read now.
#[derive(Debug, Clone, PartialEq)]
pub struct Attributed {
    /// Whose spend it is.
    pub role: UsageRole,
    /// The CLI id it ran on.
    pub cli: String,
    /// The branch, or the invocation's label.
    pub label: String,
    /// The session id git-paw assigned, when any.
    pub session_id: Option<String>,
    /// The report.
    pub report: UsageReport,
}

/// One read of the run's attributed usage, by role.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSnapshot {
    /// Each coding agent's session, grouped by CLI id.
    pub workers: UsageSummary,
    /// Each headless orchestrator invocation the loop started.
    pub orchestrator: UsageSummary,
}

impl UsageSnapshot {
    /// The sum of every known figure across both roles.
    #[must_use]
    pub fn total(&self) -> UsageTotals {
        UsageTotals::sum(
            self.workers
                .by_cli
                .values()
                .chain(self.orchestrator.by_cli.values()),
        )
    }

    /// A cost cap can only be checked when every known source reports one.
    #[must_use]
    pub fn reports_cost(&self) -> bool {
        self.total().cost.is_some()
    }
}

/// Reads the run's attributed usage (production: the session receipt and
/// each CLI's own usage report).
pub trait UsageReader: std::fmt::Debug {
    /// The current report of every worker and of each of `headless`.
    fn read(&self, headless: &[HeadlessSession]) -> Vec<Attributed>;

    /// Whether every worker CLI's usage source reports cost.
    fn every_worker_reports_cost(&self) -> bool;
}

/// The cap breached by `snapshot`, described for the escalation, or `None`.
/// Only known figures are compared; a cost cap with no cost figure is never
/// breached.
#[must_use]
pub fn breach(caps: BudgetCaps, snapshot: &UsageSnapshot) -> Option<String> {
    let total = snapshot.total();
    if let Some(cap) = caps.max_tokens
        && total.tokens() >= cap
    {
        return Some(format!(
            "budget_max_tokens {cap} reached: {} tokens of known usage",
            total.tokens()
        ));
    }
    if let (Some(cap), Some(cost)) = (caps.max_cost, total.cost)
        && cost >= cap
    {
        return Some(format!(
            "budget_max_cost {cap} reached: {cost} of known usage"
        ));
    }
    None
}

/// What the loop prints at start when a cap is set: the workers whose usage
/// is unknown, and whether a cost cap can be evaluated at all.
#[must_use]
pub fn start_report(
    caps: BudgetCaps,
    snapshot: &UsageSnapshot,
    every_worker_reports_cost: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    if !caps.is_set() {
        return lines;
    }
    for (worker, reason) in &snapshot.workers.unknown {
        lines.push(format!(
            "budget: {worker} has unknown usage ({reason}); the cap covers known usage only"
        ));
    }
    if caps.max_cost.is_some() && !every_worker_reports_cost {
        lines.push(
            "budget: budget_max_cost cannot be evaluated — not every worker CLI reports cost"
                .to_string(),
        );
    }
    lines
}

/// Renders the exit summary's usage section; `warnings` are the workers whose
/// usage turned unknown during the run.
#[must_use]
pub fn render_section(snapshot: &UsageSnapshot, caps: BudgetCaps, warnings: &[String]) -> String {
    let mut out = format!("Usage (known usage only; {EXCLUDES_OWN_SESSION}):\n");
    for (cli, totals) in &snapshot.workers.by_cli {
        let _ = writeln!(out, "  - workers on {cli}: {}", figures(totals));
    }
    if snapshot.orchestrator.any_known() {
        let _ = writeln!(
            out,
            "  - orchestrator invocations: {}",
            figures(&snapshot.orchestrator.total)
        );
    }
    let unknown_orchestrator = snapshot.orchestrator.unknown.len();
    if unknown_orchestrator > 0 {
        let _ = writeln!(
            out,
            "  - {unknown_orchestrator} orchestrator invocation(s) with unknown usage"
        );
    }
    let _ = writeln!(
        out,
        "  - {} worker(s) with unknown usage",
        snapshot.workers.unknown.len()
    );
    for warning in warnings {
        let _ = writeln!(out, "  - {warning}");
    }
    let total = snapshot.total();
    let _ = writeln!(out, "  - total: {}", figures(&total));
    if let Some(cap) = caps.max_tokens {
        let _ = writeln!(out, "  - budget_max_tokens: {} of {cap}", total.tokens());
    }
    if let Some(cap) = caps.max_cost {
        let known = total
            .cost
            .map_or_else(|| "no cost reported".to_string(), |c| c.to_string());
        let _ = writeln!(out, "  - budget_max_cost: {known} of {cap}");
    }
    out
}

fn figures(t: &UsageTotals) -> String {
    let cost = t.cost.map(|c| format!(", cost {c}")).unwrap_or_default();
    format!(
        "{} tokens ({} in, {} out; cache {} read, {} write){cost}",
        t.tokens(),
        t.input,
        t.output,
        t.cache_read,
        t.cache_write
    )
}

type SessionKey = (UsageRole, String, String);

/// The last known figures of every session seen this run. A session that
/// leaves, swaps CLI (a new session id) or turns unknown keeps its last known
/// figures, so the total only grows.
#[derive(Debug, Clone, Default)]
pub struct UsageLedger {
    known: BTreeMap<SessionKey, (String, UsageTotals)>,
    warned: HashSet<SessionKey>,
}

impl UsageLedger {
    fn key(r: &Attributed) -> SessionKey {
        (
            r.role,
            r.label.clone(),
            r.session_id.clone().unwrap_or_default(),
        )
    }

    /// Folds one read in. Returns the run's snapshot and a warning for each
    /// worker whose usage turned unknown for the first time.
    pub fn merge(&mut self, records: &[Attributed]) -> (UsageSnapshot, Vec<String>) {
        let mut warnings = Vec::new();
        for r in records {
            let key = Self::key(r);
            match &r.report {
                UsageReport::Known(t) => {
                    let grew = self
                        .known
                        .get(&key)
                        .is_none_or(|(_, old)| t.tokens() >= old.tokens());
                    if grew {
                        self.known.insert(key, (r.cli.clone(), *t));
                    }
                }
                UsageReport::Unknown(reason) => {
                    if r.role == UsageRole::Worker
                        && self.known.contains_key(&key)
                        && self.warned.insert(key)
                    {
                        warnings.push(format!(
                            "budget: {}'s usage turned unknown ({reason}); its last known \
                             figures stay in the total",
                            r.label
                        ));
                    }
                }
            }
        }
        let snapshot = UsageSnapshot {
            workers: self.summary(UsageRole::Worker, records),
            orchestrator: self.summary(UsageRole::Orchestrator, records),
        };
        (snapshot, warnings)
    }

    fn summary(&self, role: UsageRole, records: &[Attributed]) -> UsageSummary {
        let known: Vec<(&str, &str, UsageReport)> = self
            .known
            .iter()
            .filter(|((r, _, _), _)| *r == role)
            .map(|((_, label, _), (cli, t))| (cli.as_str(), label.as_str(), UsageReport::Known(*t)))
            .collect();
        // A session whose last known figures are kept is counted there, not
        // again as unknown.
        let unknown = records.iter().filter(|r| {
            r.role == role && r.report.known().is_none() && !self.known.contains_key(&Self::key(r))
        });
        UsageSummary::from_reports(
            known
                .iter()
                .map(|(cli, label, report)| (*cli, *label, report))
                .chain(unknown.map(|r| (r.cli.as_str(), r.label.as_str(), &r.report))),
        )
    }
}

/// The budget check's state across the run.
#[derive(Debug, Clone)]
pub struct BudgetGuard {
    caps: BudgetCaps,
    every: u32,
    sweeps: u32,
    exhausted: bool,
    ledger: UsageLedger,
    warnings: Vec<String>,
    pending: Vec<String>,
}

impl BudgetGuard {
    /// A guard reading usage every `every` sweeps (at least one).
    #[must_use]
    pub fn new(caps: BudgetCaps, every: u32) -> Self {
        Self {
            caps,
            every: every.max(1),
            sweeps: 0,
            exhausted: false,
            ledger: UsageLedger::default(),
            warnings: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Whether a cap has been breached this run.
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Reads usage now and folds it into the run's ledger.
    pub fn observe(
        &mut self,
        reader: &dyn UsageReader,
        headless: &[HeadlessSession],
    ) -> UsageSnapshot {
        let (snapshot, warnings) = self.ledger.merge(&reader.read(headless));
        self.warnings.extend(warnings.iter().cloned());
        self.pending.extend(warnings);
        snapshot
    }

    /// Warnings raised since the last call, for the operator's terminal.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }

    /// The start-of-run report when a cap is set: its lines, plus a one-time
    /// escalation when `budget_max_cost` cannot be evaluated.
    pub fn start(
        &mut self,
        reader: &dyn UsageReader,
        headless: &[HeadlessSession],
    ) -> (Vec<String>, Option<Escalation>) {
        if !self.caps.is_set() {
            return (Vec::new(), None);
        }
        let snapshot = self.observe(reader, headless);
        let costed = reader.every_worker_reports_cost();
        let lines = start_report(self.caps, &snapshot, costed);
        let escalation = self
            .caps
            .max_cost
            .filter(|_| !costed)
            .map(|cap| Escalation {
                agent_id: crate::surface::SUPERVISOR_AGENT_ID.to_string(),
                verdict: COST_CAP_UNENFORCEABLE_VERDICT.to_string(),
                command: format!(
                    "budget_max_cost {cap} cannot be evaluated: not every worker CLI reports cost"
                ),
            });
        (lines, escalation)
    }

    /// Counts one sweep and, every `every` sweeps with a cap set, reads usage
    /// and compares it. Returns the escalation for the FIRST breach only.
    pub fn on_sweep(
        &mut self,
        reader: &dyn UsageReader,
        headless: &[HeadlessSession],
    ) -> Option<Escalation> {
        if !self.caps.is_set() || self.exhausted {
            return None;
        }
        self.sweeps += 1;
        if !self.sweeps.is_multiple_of(self.every) {
            return None;
        }
        let snapshot = self.observe(reader, headless);
        let reason = breach(self.caps, &snapshot)?;
        self.exhausted = true;
        Some(Escalation {
            agent_id: crate::surface::SUPERVISOR_AGENT_ID.to_string(),
            verdict: BUDGET_EXHAUSTED_VERDICT.to_string(),
            command: reason,
        })
    }

    /// The exit summary's usage section, from one last read.
    pub fn section(&mut self, reader: &dyn UsageReader, headless: &[HeadlessSession]) -> String {
        let snapshot = self.observe(reader, headless);
        render_section(&snapshot, self.caps, &self.warnings)
    }
}

/// Everything the loop needs to attribute usage and enforce the caps.
#[derive(Debug, Clone)]
pub struct UsageSetup {
    /// The configured caps.
    pub caps: BudgetCaps,
    /// Where usage is read from.
    pub reader: std::sync::Arc<dyn UsageReader + Send + Sync>,
    /// Sweeps between two usage reads.
    pub every_sweeps: u32,
}

/// Sweeps between usage reads: [`DEFAULT_USAGE_EVERY_SWEEPS`] or the
/// `GIT_PAW_DRIVE_USAGE_EVERY_SWEEPS` override.
///
/// The override is a test hook so e2e tests need not wait several sweeps; it
/// is not a documented setting.
#[must_use]
pub fn usage_every_sweeps() -> u32 {
    std::env::var("GIT_PAW_DRIVE_USAGE_EVERY_SWEEPS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_USAGE_EVERY_SWEEPS)
}

/// The production [`UsageSetup`]: `caps` over the repository's session,
/// read on the [`usage_every_sweeps`] cadence.
#[must_use]
pub fn session_usage<S: std::hash::BuildHasher>(
    caps: BudgetCaps,
    repo_root: &Path,
    clis: &HashMap<String, CustomCli, S>,
) -> UsageSetup {
    UsageSetup {
        caps,
        reader: std::sync::Arc::new(SessionUsageReader {
            repo_root: repo_root.to_path_buf(),
            clis: clis.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            cache: UsageCache::new(),
        }),
        every_sweeps: usage_every_sweeps(),
    }
}

/// The production [`UsageReader`]: each worker from the repository's session
/// receipt, resolved through its CLI record, plus the headless invocations.
/// Reports are parsed incrementally across the run's reads.
#[derive(Debug)]
pub struct SessionUsageReader {
    /// The repository whose session is read.
    pub repo_root: PathBuf,
    /// The `[clis]` table.
    pub clis: HashMap<String, CustomCli>,
    /// What earlier reads already parsed.
    pub cache: UsageCache,
}

impl SessionUsageReader {
    fn workers(&self) -> Vec<crate::session::WorktreeEntry> {
        crate::session::find_session_for_repo(&self.repo_root)
            .ok()
            .flatten()
            .map(|s| s.worktrees)
            .unwrap_or_default()
    }

    fn attribute(
        &self,
        role: UsageRole,
        cli: &str,
        label: String,
        session_id: Option<&str>,
        cwd: &Path,
    ) -> Attributed {
        let profile = CliProfile::resolve(cli, &self.clis);
        Attributed {
            role,
            cli: profile.id().to_string(),
            label,
            session_id: session_id.map(str::to_string),
            report: profile.usage_cached(session_id, cwd, Some(&self.cache)),
        }
    }
}

impl UsageReader for SessionUsageReader {
    fn read(&self, headless: &[HeadlessSession]) -> Vec<Attributed> {
        let workers = self.workers().into_iter().map(|e| {
            self.attribute(
                UsageRole::Worker,
                &e.cli,
                e.branch.clone(),
                e.cli_session_id.as_deref(),
                &e.worktree_path,
            )
        });
        let orchestrator = headless.iter().enumerate().map(|(n, h)| {
            self.attribute(
                UsageRole::Orchestrator,
                &h.cli,
                format!("orchestrator invocation {}", n + 1),
                h.session_id.as_deref(),
                &h.cwd,
            )
        });
        workers.chain(orchestrator).collect()
    }

    fn every_worker_reports_cost(&self) -> bool {
        self.workers()
            .iter()
            .all(|e| CliProfile::resolve(&e.cli, &self.clis).reports_cost())
    }
}
