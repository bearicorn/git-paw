#!/usr/bin/env bash
# Supervisor-sweep helpers for a paw-* tmux session.
#
# Generalized helper installed by `git paw init` into
# `<repo>/.git-paw/scripts/sweep.sh`. The supervisor pane invokes this
# script via relative path; it discovers everything it needs at runtime:
#
#   - session name from <repo>/.git-paw/sessions/*.json (most recently
#     modified file's session_name field),
#   - broker URL from <repo>/.git-paw/config.toml [broker] port (default
#     9119) and bind (default 127.0.0.1),
#   - test command from <repo>/.git-paw/config.toml [supervisor]
#     test_command (commands that need it no-op with a message when unset).
#
# Subcommands implemented (per supervisor-bugfixes-v0-5-x §3.2):
#   snapshot                            — capture-pane tail of every coding pane
#   capture <pane>                      — single-pane full tail-50 capture
#   approve <pane>                      — take the pane's exclusive approval claim,
#                                         re-confirm live prompt on a fresh capture,
#                                         then answer it ONCE with option 1 + Enter;
#                                         judges no command, never selects a durable
#                                         grant; refuses any pane the discovery roster does not list as a coding agent (pane 0 without a roster) and skips a pane
#                                         another approver has claimed. Run only by a
#                                         human, or by the orchestrator on the human's
#                                         explicit instruction
#   status [--all]                      — broker /status, phantoms filtered by default
#   worktrees-status                    — uncommitted-file count per agent worktree
#   inbox                               — agent.question/feedback/blocked from supervisor inbox
#   feedback-gate <agent> <gate> <msg…> — publish agent.feedback with bracketed gate prefix
#   verified <agent> <msg…>             — publish agent.verified
#   status-publish [--phase <p>] [--detail '<obj>'] <msg…>
#                                       — publish agent.status as supervisor
#                                         (--phase / --detail are optional; the
#                                         plain <msg…> form is unchanged)
#   learn <category> <title> <body-json>
#                                       — publish agent.learning as supervisor
#                                         (least-privilege publish path for
#                                         qualitative learnings; body passes
#                                         through verbatim)

set -u

# ---------------------------------------------------------------------------
# Discovery: project root, paw dir, broker URL, session name, test command.
#
# The project root, `.git-paw` paths, Python 3 interpreter and
# `discover_broker_url` all come from the shared preamble `_paw_common.sh`,
# installed as a sibling of this script by `git paw init`. Resolving it relative
# to this file's own location works from both the bundled `assets/scripts/` tree
# and the deployed `.git-paw/scripts/` directory.
# ---------------------------------------------------------------------------

PAW_SCRIPT_NAME="sweep.sh"
_PAW_COMMON="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)/_paw_common.sh"
# shellcheck source=_paw_common.sh
if [[ ! -r "${_PAW_COMMON}" ]] || ! . "${_PAW_COMMON}"; then
  echo "sweep.sh: cannot load shared preamble '${_PAW_COMMON}' — re-run \`git paw init\`" >&2
  exit 3
fi

SESSIONS_DIR="${PAW_DIR}/sessions"

# Discover the active session name.
#
# Prefers the per-repo discovery JSON in .git-paw/sessions/ (the richer
# surface — `git paw start` writes session_name + the agent roster there).
# When no JSON is present (e.g. the supervisor attached to a pre-existing
# paw-* session created outside the normal start flow), falls back to the
# live tmux session via $TMUX / `tmux display-message -p '#S'`, so the
# session file never needs to be hand-authored.
discover_session_name() {
  local name=""
  if [[ -d "${SESSIONS_DIR}" ]]; then
    local newest
    # Pick the most recently modified *.json. Use Python so we don't depend
    # on GNU vs BSD `stat`/`find` flag differences.
    newest=$(
      "${PY}" - "${SESSIONS_DIR}" <<'PY'
import os, sys
d = sys.argv[1]
try:
    files = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".json")]
except FileNotFoundError:
    sys.exit(0)
if not files:
    sys.exit(0)
files.sort(key=lambda p: os.path.getmtime(p), reverse=True)
print(files[0])
PY
    )
    if [[ -n "${newest}" ]]; then
      name=$(
        "${PY}" - "${newest}" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
name = data.get("session_name")
if name:
    print(name)
PY
      )
    fi
  fi
  # Fall back to the live tmux session when no per-repo JSON yielded a name.
  if [[ -z "${name}" && -n "${TMUX:-}" ]]; then
    name=$(tmux display-message -p '#S' 2>/dev/null)
  fi
  if [[ -z "${name}" ]]; then
    return 1
  fi
  printf '%s\n' "${name}"
}

# Parse [supervisor].test_command from config.toml. Prints empty string if
# unset — callers SHALL check for non-empty before using.
discover_test_command() {
  if [[ ! -f "${CONFIG_TOML}" ]]; then
    return
  fi
  "${PY}" - "${CONFIG_TOML}" <<'PY'
import sys

try:
    import tomllib
    mode = "rb"
except ModuleNotFoundError:
    try:
        import tomli as tomllib
        mode = "rb"
    except ModuleNotFoundError:
        tomllib = None

if tomllib is None:
    import re
    text = open(sys.argv[1]).read()
    in_sup = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_sup = stripped == "[supervisor]"
            continue
        if not in_sup:
            continue
        m = re.match(r"^\s*test_command\s*=\s*\"([^\"]*)\"", line)
        if m:
            print(m.group(1))
            break
else:
    with open(sys.argv[1], mode) as f:
        data = tomllib.load(f)
    sup = data.get("supervisor", {})
    tc = sup.get("test_command")
    if tc:
        print(tc)
PY
}

# Parse an integer [supervisor].<field> from config.toml. Prints <default>
# when the file, section, or field is absent (or not an integer). Mirrors
# discover_test_command's tomllib-with-regex-fallback shape.
discover_supervisor_int() {
  local field=$1 default=$2
  if [[ ! -f "${CONFIG_TOML}" ]]; then
    printf '%s\n' "${default}"
    return
  fi
  FIELD="${field}" DEFAULT="${default}" "${PY}" - "${CONFIG_TOML}" <<'PY'
import os, sys

field = os.environ["FIELD"]
default = os.environ["DEFAULT"]

try:
    import tomllib
    mode = "rb"
except ModuleNotFoundError:
    try:
        import tomli as tomllib
        mode = "rb"
    except ModuleNotFoundError:
        tomllib = None

val = None
if tomllib is None:
    import re
    text = open(sys.argv[1]).read()
    in_sup = False
    pat = re.compile(r"^\s*" + re.escape(field) + r"\s*=\s*(\d+)")
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_sup = stripped == "[supervisor]"
            continue
        if not in_sup:
            continue
        m = pat.match(line)
        if m:
            val = m.group(1)
            break
else:
    with open(sys.argv[1], mode) as f:
        data = tomllib.load(f)
    sup = data.get("supervisor", {})
    v = sup.get(field)
    if isinstance(v, bool):
        v = None
    if isinstance(v, int):
        val = str(v)

print(val if val is not None else default)
PY
}

# Most-recently-modified session JSON path (for pane->agent resolution).
discover_session_file() {
  if [[ ! -d "${SESSIONS_DIR}" ]]; then
    return 1
  fi
  "${PY}" - "${SESSIONS_DIR}" <<'PY'
import os, sys
d = sys.argv[1]
try:
    files = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".json")]
except FileNotFoundError:
    sys.exit(0)
if not files:
    sys.exit(0)
files.sort(key=lambda p: os.path.getmtime(p), reverse=True)
print(files[0])
PY
}

SESSION=$(discover_session_name || true)
SESSION_FILE=$(discover_session_file || true)
BROKER=$(discover_broker_url)
TEST_COMMAND=$(discover_test_command)

# --- Bug 4: stuck-on-prompt detection tuning ---------------------------------
# A pane is "stuck on prompt" when its capture matches a permission/paste-buffer
# marker AND the agent's broker last_seen has not advanced for more than
# STUCK_THRESHOLD_SECONDS. The helper dedups repeat detections of the same
# (agent_id, prompt-shape) within STUCK_DEDUP_WINDOW_SECONDS so a persistently
# stuck agent produces exactly one synthetic publish per detection window.
STUCK_THRESHOLD_SECONDS=30
STUCK_DEDUP_WINDOW_SECONDS=300
# Sourced from the resolved CLI prompt-shape profile (cli-prompt-profiles
# capability, D3) via `git paw __prompt-profile`, so this and the compiled
# Rust detector cannot drift; the literal here is only the fallback used when
# the binary is not resolvable. See discover_prompt_profile_field in
# _paw_common.sh.
STUCK_MARKERS_REGEX=$(discover_prompt_profile_field approval_markers \
  'Do you want to proceed|Do you want to allow|requires approval|Allow this command|\(y/n\)|\[y/N\]')
STUCK_DEDUP_FILE="${PAW_DIR}/.sweep-stuck-dedup"

# Live-prompt structural markers (approve-send gate). Textual markers of a
# LIVE permission prompt, matched case-insensitively; numbered option lines
# (`1.` / `2)`) anchor a multi-option prompt whose question sits above the
# 4-line tail (block window: last 15 non-blank lines) at the capture's true
# end. Detection also searches upward over a bounded trailing region above
# that end -- see is_live_capture below for the full anchored-search and
# footer-shape discipline. Sourced from the resolved profile so this helper
# and the compiled detector (src/supervisor/permission_prompt.rs) agree on
# what "live" means.
LIVE_PROMPT_MARKERS_REGEX=$(discover_prompt_profile_field live_prompt_markers \
  'do you want to|esc to cancel')
# Widest number of trailing non-blank lines of non-prompt content (a task
# list, a status strip, a progress footer) tolerated between a modal's own
# footer and the end of the capture. Mirrors LIVE_PROMPT_TRAILING_BOUND in
# src/supervisor/permission_prompt.rs -- keep both in lockstep.
LIVE_PROMPT_TRAILING_BOUND=10
# Marker drawn only while a modal is actively live -- never left behind by an
# already-answered prompt -- so it is the only marker allowed to anchor a
# match at a bounded offset above the capture's end. Mirrors
# LIVE_PROMPT_FOOTER_MARKERS in src/supervisor/permission_prompt.rs: a bare
# confirmation question on its own must NOT extend past the narrow 4-line
# tail, or an answered prompt followed by ordinary command output would
# misread as live.
LIVE_PROMPT_FOOTER_REGEX='esc to cancel'
# Anchored form of the marker above: must START the line (once leading TUI
# decoration and an optional open-paren are skipped), not merely appear
# ANYWHERE in it. Mirrors is_footer_line in src/supervisor/permission_prompt.rs.
# Gate-review safety finding: a bare substring check at a bounded offset
# above the capture's end also fires on narration ("the footer reads \"Esc to
# cancel\" below it") or grep-style output that happens to echo the marker
# text -- neither is a live modal. This anchored form, combined with the
# option-line check in LIVE_PROMPT_OPTION_REGEX, requires the modal's actual
# shape instead of a lone string.
LIVE_PROMPT_FOOTER_LINE_REGEX="^[[:space:]│─╭╮╰╯├┤┐└┘┌⎿❯●•*·]*\(?(${LIVE_PROMPT_FOOTER_REGEX})"
# A numbered option line once its leading TUI decoration (box-drawing glyphs,
# bullets, the selection caret) is skipped. The decoration class is a
# terminal-rendering detail shared by every CLI, not vendor prompt shape, so
# it stays here; the digit/punctuation shape itself comes from the resolved
# profile's option_line_pattern (mirrors strip_decoration + is_option_line in
# src/supervisor/permission_prompt.rs).
_option_line_digit=$(discover_prompt_profile_field option_line_pattern '^[0-9][.)]')
LIVE_PROMPT_OPTION_REGEX="^[[:space:]│─╭╮╰╯├┤┐└┘┌⎿❯●•*·]*${_option_line_digit#^}"
# The same shape anchored on option ONE specifically. cmd_approve presses `1`,
# so `1` is the only line whose wording it may act on -- matching any numbered
# line would let it clear a prompt after inspecting option 2 or 3. Derived by
# narrowing the profile pattern's digit class; if the override has no `[0-9]`
# to narrow, fall back to a compiled literal rather than silently reusing the
# any-digit pattern.
_option_one_digit=${_option_line_digit//\[0-9\]/1}
if [[ "${_option_one_digit}" == "${_option_line_digit}" ]]; then
  _option_one_digit='^1[.)]'
fi
LIVE_PROMPT_OPTION_ONE_REGEX="^[[:space:]│─╭╮╰╯├┤┐└┘┌⎿❯●•*·]*${_option_one_digit#^}"
unset _option_line_digit
# The CLI's composer/input-line sigils (e.g. `>`, `❯`), sourced from the
# resolved profile's input_sigils. A line carrying one of these with no
# option-line text following it is a CANDIDATE idle input box -- mirrors
# is_idle_input_box_line in src/supervisor/permission_prompt.rs. Gate-review safety
# finding: a real live modal REPLACES the input box while it is up, so an idle
# input-box line appearing below a shape-gated footer match (e.g. the
# supervisor's own pane echoing another pane's modal via `sweep.sh capture`,
# with its own idle composer below) means the footer is quoted/historical
# content, not an active modal.
#
# The `│` border is OPTIONAL in INPUT_BOX_LINE_REGEX: ground-truthed against a
# live `tmux capture-pane -p`, current Claude Code's idle input box has NO
# side borders at all -- a rule line (RULE_LINE_REGEX), the bare sigil line,
# another rule line, then the mode strip. A borderless sigil line only counts
# as the input box when the line immediately above it is a pure rule line
# (see is_idle_input_box_line); a bordered line always counts.
# Durable-grant markers (the "and do not ask again" option wording).
# cmd_approve refuses to press an option 1 that carries one, because that
# answer writes a permanent permission rule the human did not ask for.
#
# TWO regexes, checked as OR, and that is deliberate. The profile-sourced one
# carries the resolved CLI wording; `git paw __prompt-profile` already renders
# its apostrophe as a both-forms character class, so the terminal may draw
# either an ASCII apostrophe or the Unicode right single quote. But a profile
# is operator-supplied data: an override NARROWING the marker to something
# that never matches would disarm the guard entirely. So a compiled fallback
# is ALWAYS checked alongside it -- a profile can widen what counts as a
# durable grant, never shrink it. That is the fail-closed direction.
#
# The compiled fallback is punctuation-free on purpose: this file must stay
# free of apostrophes, which desync quoting inside its `$(cat <<'PY' ... )`
# heredocs.
BROAD_GRANT_MARKER_REGEX=$(discover_prompt_profile_field broad_grant_marker 'ask again')
BROAD_GRANT_COMPILED_REGEX='ask again'

INPUT_SIGILS_REGEX=$(discover_prompt_profile_field input_sigils '>|❯')
INPUT_BOX_LINE_REGEX="^[[:space:]]*│?[[:space:]]*(${INPUT_SIGILS_REGEX})"
RULE_LINE_REGEX='^[─━═-]+$'

# --- Additional stuck shapes: stream-timeout, context-bloat, no-progress -----
# A coding agent's CLI API call can fail mid-stream (transport error / timeout)
# and sit dead with no permission marker present; and its context can bloat past
# the point where the CLI surfaces a `/clear to save <N>k tokens` hint. Both are
# pane-text markers sourced from the resolved profile, same as
# STUCK_MARKERS_REGEX above.
STREAM_TIMEOUT_MARKERS_REGEX=$(discover_prompt_profile_field stream_error_markers \
  'Request timed out|Request timeout|stream error|stream timed out|stream disconnected|transport error|Connection error|error streaming')
# Group 2 captures N (thousands of tokens) from a `/clear to save <N>k tokens`
# (or `/compact ...`) hint; the leading slash is optional for CLI variants.
CONTEXT_BLOAT_MARKER_REGEX=$(discover_prompt_profile_field context_bloat_pattern \
  '/?(clear|compact) to save ([0-9]+)k tokens')
# A paste-aware CLI's own "N lines pasted" acknowledgement, e.g. `Pasted text
# #1`; also sourced from the resolved profile.
PASTE_BUFFER_MARKER_REGEX=$(discover_prompt_profile_field paste_buffer_pattern \
  'Pasted text #[0-9]')
# No-progress heartbeat snapshot: one line per agent,
# `agent<TAB>checkbox_count<TAB>commit_count<TAB>timestamp`.
SWEEP_PROGRESS_FILE="${PAW_DIR}/.sweep-progress"

# Detection thresholds/windows, read from [supervisor] config with documented
# defaults (no-progress ~25 min, context-bloat 250k tokens, blocked-on-
# supervisor ~15 min). See src/config.rs SupervisorConfig for the field docs.
NO_PROGRESS_WINDOW_SECONDS=$(discover_supervisor_int no_progress_window_seconds 1500)
CONTEXT_BLOAT_THRESHOLD_K=$(discover_supervisor_int context_bloat_threshold_k 250)
BLOCKED_ON_SUPERVISOR_WINDOW_SECONDS=$(discover_supervisor_int blocked_on_supervisor_window_seconds 900)

if [[ -z "${SESSION}" ]]; then
  # `status`, `verified`, `feedback-gate`, `status-publish`, and `inbox`
  # don't need a tmux session — only broker access. Tmux-touching commands
  # below fail loudly if SESSION is empty.
  SESSION=""
fi

# Discover coding-agent pane indices for the session. Excludes:
#   - dashboard pane (pane_current_command == "git-paw", or title "dashboard")
#   - supervisor pane (title "supervisor")
discover_coding_panes() {
  [[ -z "${SESSION}" ]] && return
  tmux list-panes -t "${SESSION}" -F "#{pane_index} #{pane_current_command} #{pane_title}" 2>/dev/null \
    | while read -r idx cmd title; do
        case "${cmd}" in
          git-paw|git-paw-dashboard) continue ;;
        esac
        case "${title}" in
          dashboard|supervisor) continue ;;
        esac
        echo "${idx}"
      done
}

PANES=()
while IFS= read -r idx; do
  [[ -n "${idx}" ]] && PANES+=("${idx}")
done < <(discover_coding_panes)
PANE_COUNT=${#PANES[@]}

usage() {
  cat >&2 <<USAGE
usage: $0 <subcommand> [args]

  snapshot                            capture-pane tail of every coding-agent pane
  capture <pane>                      single-pane full tail-50 capture
  approve <pane>                      re-confirm live prompt on a fresh capture, then
                                      answer it once with option 1 + Enter; judges no
                                      command and never selects a durable grant;
                                      refuses a pane the roster does not list as a coding agent; skips a pane another approver
                                      has claimed
  detect-stuck                        flag stuck agents (prompt/stream-timeout/bloat/no-progress/blocked)
  stuck-eval <agent> <last_seen> [checkbox_count] [commit_count] [blocked_age]
                                      (stdin: capture) run the stuck decision for one agent
  status [--all]                      broker /status, one line per agent (phantoms filtered)
  events <since> [wait]               long-poll broker /log from the <since> cursor (default
                                      wait 30s) and print one JSON object
                                      {entries, last_seq, epoch, wave}; read-only — sends no
                                      keystroke and publishes nothing. Pass since=0 again
                                      whenever `epoch` changes (the broker restarted and its
                                      sequence numbers began again). With no broker it
                                      prints {"no_session": true} and exits 0
  worktrees-status                    uncommitted-file count per agent worktree
  inbox                               supervisor inbox payloads (question/feedback/blocked)
  feedback-gate <agent> <gate> <msg…> publish agent.feedback with bracketed gate prefix
  verified <agent> <msg…>             publish agent.verified
  conflict-ack <agent-a> <agent-b> <file> [note…]
                                      record that an in-flight conflict has been
                                      adjudicated; stops further escalation
                                      questions for that pair and file while the
                                      detector keeps tracking the overlap. Both
                                      agents are told; a default note explains
                                      there is nothing to fix if none is given
  status-publish [--phase <p>] [--detail '<obj>'] <msg…>
                                      publish an agent.status from agent_id="supervisor";
                                      --phase adds a phase label, --detail a JSON-object detail
  learn <category> <title> <body-json>
                                      publish an agent.learning from agent_id="supervisor"
                                      (least-privilege qualitative-learning publish path)

Discovered configuration:
  session:        ${SESSION:-<none>}
  broker:         ${BROKER}
  project root:   ${PROJECT_ROOT}
  test_command:   ${TEST_COMMAND:-<unset>}
USAGE
}

# Abort a subcommand that needs the session when none could be resolved.
#
# Resolution is primary-then-secondary: the per-repo discovery file under
# `.git-paw/sessions/` is the real surface (it carries the agent roster), and
# an inference from the live multiplexer (`$TMUX`) is the secondary path, used
# when the supervisor attached to a pre-existing session. When neither
# resolves, say so explicitly and exit non-zero rather than letting the
# subcommand report an empty result as though it had done its work.
require_session() {
  if [[ -z "${SESSION}" ]]; then
    echo "sweep.sh: no session found — no discovery file under ${SESSIONS_DIR}" \
         "and no live session to infer from (\$TMUX unset)" >&2
    exit 3
  fi
  if [[ -z "${SESSION_FILE}" ]]; then
    echo "sweep.sh: warning: session '${SESSION}' was inferred from \$TMUX;" \
         "no discovery file under ${SESSIONS_DIR}, so panes cannot be mapped to agents" >&2
  fi
}

# Publish a JSON BrokerMessage payload to /publish via the shared preamble's
# status-checking publisher: a publish the broker rejects prints its error to
# stderr and returns non-zero, so no subcommand reports a rejected publish as
# done.
publish() {
  paw_publish "${BROKER}" "$1"
}

cmd_snapshot() {
  require_session
  if [[ ${PANE_COUNT} -eq 0 ]]; then
    echo "no coding panes discovered in session '${SESSION}'" >&2
    return 0
  fi
  for p in "${PANES[@]}"; do
    echo "=== pane ${p} ==="
    tmux capture-pane -t "${SESSION}:0.${p}" -p -S -20 2>&1 | tail -20
  done
}

cmd_capture() {
  require_session
  local pane=${1:-}
  if [[ -z "${pane}" ]]; then
    echo "usage: $0 capture <pane>" >&2
    exit 2
  fi
  tmux capture-pane -t "${SESSION}:0.${pane}" -p -S -50 2>&1 | tail -50
}

# ---------------------------------------------------------------------------
# Per-pane approval claim.
#
# Approval keystrokes reach a pane from this helper, from git-paw's in-process
# drive loop, and from its dashboard auto-approver — three separate processes.
# Each takes the pane's claim before sending, so two can never both land a
# keystroke on the same pane. The path formula and the TTL below MUST stay
# byte-identical to the ones the binary uses, or the exclusion stops spanning
# processes.
# ---------------------------------------------------------------------------

# Seconds after which an untouched claim is treated as abandoned by a
# hard-killed approver and may be stolen, bounding the wedge to one TTL.
CLAIM_TTL_SECONDS=10

# Claim file for a pane: <repo>/.git-paw/tmp/approve-pane-<N>.claim
claim_path() {
  printf '%s\n' "${PROJECT_ROOT}/.git-paw/tmp/approve-pane-${1}.claim"
}

# Succeeds when the claim file exists and has gone untouched for longer than
# CLAIM_TTL_SECONDS. An unreadable claim is never reported abandoned.
claim_is_abandoned() {
  CLAIM_FILE="$1" CLAIM_TTL="${CLAIM_TTL_SECONDS}" "${PY}" -c "$(cat <<'PY'
import os, sys, time
try:
    age = time.time() - os.path.getmtime(os.environ["CLAIM_FILE"])
except OSError:
    sys.exit(1)
sys.exit(0 if age > float(os.environ["CLAIM_TTL"]) else 1)
PY
)"
}

# Path of the claim this process holds, released by the EXIT trap. Global, not
# local to cmd_approve, so the trap can still read it after that function
# returns.
PAW_CLAIM_FILE=""
release_pane_claim() {
  if [[ -n "${PAW_CLAIM_FILE}" ]]; then
    rm -f "${PAW_CLAIM_FILE}"
  fi
  return 0
}

# Atomically takes the claim at $1: 0 when acquired, 1 when another approver
# holds it. `set -C` (noclobber) makes the create-if-absent atomic, so exactly
# one of two concurrent acquirers wins. Acquisition is NON-BLOCKING — a caller
# that fails skips the pane rather than waiting. On success an EXIT trap is
# armed, so the claim is released on a normal exit, an error, and an interrupt.
acquire_pane_claim() {
  local claim=$1
  mkdir -p "$(dirname "${claim}")" 2>/dev/null || return 1
  if ! ( set -C; : > "${claim}" ) 2>/dev/null; then
    # Held. Steal it only if its owner left it untouched past the TTL, and only
    # once — a racer that wins the re-create keeps it and we skip the pane.
    claim_is_abandoned "${claim}" || return 1
    rm -f "${claim}" 2>/dev/null
    ( set -C; : > "${claim}" ) 2>/dev/null || return 1
  fi
  PAW_CLAIM_FILE="${claim}"
  trap release_pane_claim EXIT
  return 0
}

cmd_approve() {
  require_session
  local pane=${1:-}
  if [[ -z "${pane}" ]]; then
    echo "usage: $0 approve <pane>" >&2
    exit 2
  fi
  # A pane is a numeric index, checked before the roster lookup: tmux resolves
  # a non-numeric target such as `.` to the active pane, so no other spelling
  # may reach the claim or any tmux call.
  if ! [[ "${pane}" =~ ^[0-9]+$ ]]; then
    echo "pane ${pane} refused: approve takes a numeric pane index, no keys sent" >&2
    exit 2
  fi
  # Keys go only to a pane the discovery roster lists as a coding agent. The
  # dashboard, and any pane no roster entry claims, is refused by role, never
  # by index: pane 0 is the dashboard in a broker-enabled session but the
  # first coding agent in a broker-less one. With no readable roster, fall
  # back to refusing pane 0.
  local roster_panes
  roster_panes=$(roster_pane_indices)
  if [[ -n "${roster_panes}" ]]; then
    if ! grep -qxF -- "${pane}" <<<"${roster_panes}"; then
      echo "pane ${pane} excluded from blind send-keys (not a coding agent)"
      return 0
    fi
  elif [[ "${pane}" == "0" ]]; then
    echo "pane 0 excluded from blind send-keys (not a coding agent; no roster to check)"
    return 0
  fi
  # Take the pane's exclusive claim before doing anything else that could send
  # keys. git-paw's drive loop honours the same claim for its nudge and
  # buffered-input sends, so this helper and the loop never type into the same
  # pane at once.
  local claim
  claim=$(claim_path "${pane}")
  if ! acquire_pane_claim "${claim}"; then
    echo "pane ${pane} claimed by another approver, no keys sent (${claim})"
    return 0
  fi
  # Re-confirm a live permission prompt on a FRESH capture taken immediately
  # before sending keys (structural markers at the tail). If the prompt cleared
  # between the human deciding and now, send NOTHING -- a stray digit would
  # land in the agent's input box.
  local cap
  cap=$(tmux capture-pane -t "${SESSION}:0.${pane}" -p -S -50 2>/dev/null | tail -50)
  if ! is_live_capture "${cap}"; then
    echo "cleared before send, no keys sent (pane ${pane})"
    return 0
  fi
  # Answer ONCE, with option 1. This helper judges no command -- the decision
  # to approve came from the human who ran it. Two guards stand between that
  # decision and the keystroke:
  #   1. option 1 must actually be on screen;
  #   2. option 1 must not itself read as a durable grant. On some prompt
  #      shapes the first option is "Yes, and do not ask again ...", and
  #      pressing it would write a permanent permission rule into the agent
  #      CLI's settings. git-paw never creates one of those on its own.
  #
  # Match option ONE specifically, and take the LAST such line in the capture.
  # The live modal is the bottom-most thing on screen, so its option 1 is the
  # last one; an earlier answered modal higher up cannot stand in for it. A
  # fixed-size window would be wrong here -- the live gate tolerates up to 13
  # trailing chrome lines, so a wrapped multi-option modal plus chrome can push
  # option 1 out of any fixed tail, and the guard would then inspect option 2
  # or 3 and clear a durable grant.
  local opt1
  opt1=$(grep -E "${LIVE_PROMPT_OPTION_ONE_REGEX}" <<<"${cap}" | tail -1)
  if [[ -z "${opt1}" ]]; then
    echo "no option 1 on screen, no keys sent (pane ${pane})"
    return 0
  fi
  if grep -Eqi "${BROAD_GRANT_MARKER_REGEX}" <<<"${opt1}" \
    || grep -Eqi "${BROAD_GRANT_COMPILED_REGEX}" <<<"${opt1}"; then
    echo "option 1 reads as a durable grant, no keys sent (pane ${pane})"
    return 0
  fi
  # Dispatch the digit and Enter as two separate keystrokes -- never a blind
  # cursor-movement sequence, whose landing option depends on the prompt shape
  # (Down+Enter selects "No" on a 2-option prompt and the permanent broad
  # grant on a 3-option one).
  tmux send-keys -t "${SESSION}:0.${pane}" 1 >/dev/null 2>&1
  sleep 0.05
  tmux send-keys -t "${SESSION}:0.${pane}" Enter >/dev/null 2>&1
  echo "approved pane ${pane} (option 1)"
}

cmd_status() {
  local show_all=0
  if [[ "${1:-}" == "--all" ]]; then
    show_all=1
  fi
  # Pipe curl into python via `-c` so the script body doesn't compete
  # with the curl body for stdin (bash heredocs win over pipes, so
  # `python3 - <<'PY'` swallowed the pipe content and read its own body).
  # Known agents come from the session's own roster (`supervisor` plus every
  # discovery-file `branch_id`), so an agent on any branch prefix — a
  # spec-driven `spec/…` branch, a `fix/…` branch, anything the operator chose
  # — is shown rather than hidden as a phantom. Without a discovery file the
  # roster is empty and the filter falls back to the broker's own agent-id
  # rule (`supervisor`, or a `<prefix>-<name>` / `<prefix>/<name>` slug).
  local roster
  roster=$(roster_agent_ids)
  curl -s "${BROKER}/status" | SHOW_ALL=${show_all} ROSTER="${roster}" "${PY}" -c "$(cat <<'PY'
import json, os, re, sys
try:
    data = json.load(sys.stdin)
except Exception as exc:  # noqa: BLE001
    print(f"sweep.sh: failed to parse broker /status: {exc}", file=sys.stderr)
    sys.exit(5)
show_all = os.environ.get("SHOW_ALL") == "1"
known = {line.strip() for line in os.environ.get("ROSTER", "").splitlines() if line.strip()}
broker_rule = re.compile(r"^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$")
def is_known(aid):
    if aid == "supervisor":
        return True
    if known:
        return aid in known
    return bool(broker_rule.match(aid))
phantoms = []
for a in data.get("agents", []):
    aid = a.get("agent_id", "?")
    if not show_all and not is_known(aid):
        phantoms.append(aid)
        continue
    print("{:40s} {:10s} {:>5}s".format(
        aid, a.get("status", "?"), a.get("last_seen_seconds", 0)
    ))
if phantoms and not show_all:
    print("phantoms (use --all to show):", ", ".join(phantoms))
PY
)"
}

# Long-poll the broker's message log and print one JSON object
# {entries, last_seq, wave} on stdout.
#
# This is the read surface for an orchestrator that works a turn at a time:
# sampling pane state misses anything that happens and un-happens between two
# samples, while an ordered log read by cursor cannot. When the wait elapses
# with nothing new, the wave object still says whether the quiet is healthy
# or a stall.
#
# Read-only by construction: it GETs /log and /status, sends no keystroke and
# publishes nothing. With no reachable broker it prints an empty object with
# no_session true and exits 0, so a caller polling a repository with no live
# session gets an answer rather than an error.
cmd_events() {
  local since=${1:-0}
  local wait=${2:-30}
  if ! [[ "${since}" =~ ^[0-9]+$ ]] || ! [[ "${wait}" =~ ^[0-9]+$ ]]; then
    echo "usage: $0 events <since> [wait-seconds]" >&2
    exit 2
  fi

  local log status
  # --max-time must outlast the broker's own wait, or curl would abandon a
  # request the broker is legitimately still holding.
  log=$(curl -s --max-time "$((wait + 10))" "${BROKER}/log?since=${since}&wait=${wait}" || true)
  status=$(curl -s --max-time 5 "${BROKER}/status" || true)

  SINCE="${since}" LOG_JSON="${log}" STATUS_JSON="${status}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
since = int(os.environ.get("SINCE", "0"))
def load(raw):
    if not raw:
        return None
    try:
        return json.loads(raw)
    except Exception:  # noqa: BLE001
        return None
log = load(os.environ.get("LOG_JSON", ""))
status = load(os.environ.get("STATUS_JSON", ""))
# No body at all (connection refused), an unparseable body, or a broker
# error object: in every case there are no entries to report and the
# caller must not read the empty result as "nothing happened".
if not isinstance(log, dict) or "entries" not in log:
    out = {"entries": [], "last_seq": since, "no_session": True}
    if isinstance(log, dict) and log.get("error"):
        out["error"] = str(log["error"])
    json.dump(out, sys.stdout)
    print()
    sys.exit(0)
out = {
    "entries": log.get("entries", []),
    "last_seq": log.get("last_seq", since),
}
# Identifies the broker instance. Sequence numbers restart at zero on
# every broker start, so a caller whose recorded epoch changes must pass
# since=0 next time or it will sit above everything the new broker
# appends and time out forever.
if "epoch" in log:
    out["epoch"] = log["epoch"]
if isinstance(status, dict) and isinstance(status.get("wave"), dict):
    out["wave"] = status["wave"]
json.dump(out, sys.stdout)
print()
PY
)"
}

cmd_worktrees_status() {
  # Iterate worktrees from `git worktree list --porcelain` so we don't
  # depend on a directory-name convention.
  git -C "${PROJECT_ROOT}" worktree list --porcelain 2>/dev/null \
    | "${PY}" -c "$(cat <<'PY'
import os, subprocess, sys
worktrees = []
current = {}
for line in sys.stdin:
    line = line.rstrip("\n")
    if not line:
        if current:
            worktrees.append(current)
            current = {}
        continue
    if " " in line:
        key, _, value = line.partition(" ")
    else:
        key, value = line, ""
    current[key] = value
if current:
    worktrees.append(current)
for wt in worktrees:
    path = wt.get("worktree")
    branch = wt.get("branch", "").removeprefix("refs/heads/") or "(detached)"
    if not path:
        continue
    try:
        out = subprocess.check_output(
            ["git", "-C", path, "status", "--short"], text=True
        )
    except subprocess.CalledProcessError:
        continue
    count = sum(1 for _ in out.splitlines() if _.strip())
    print(f"{branch}: {count} uncommitted files ({path})")
PY
)"
}

cmd_inbox() {
  curl -s "${BROKER}/messages/supervisor?since=0" | "${PY}" -c "$(cat <<'PY'
import json, sys
try:
    data = json.load(sys.stdin)
except Exception as exc:  # noqa: BLE001
    print(f"sweep.sh: failed to parse broker /messages/supervisor: {exc}", file=sys.stderr)
    sys.exit(5)
for m in data.get("messages", []):
    t = m.get("type")
    if t not in ("agent.question", "agent.feedback", "agent.blocked"):
        continue
    a = m.get("agent_id", "?")
    p = m.get("payload", {})
    q = p.get("question") or p.get("errors") or p.get("needs") or ""
    if isinstance(q, list):
        q = " / ".join(map(str, q))
    print(f"--- {t} from {a} ---")
    print(str(q)[:500])
    print()
PY
)"
}

cmd_feedback_gate() {
  local target=${1:-}
  local gate=${2:-}
  shift 2 || true
  local msg=$*
  if [[ -z "${target}" || -z "${gate}" || -z "${msg}" ]]; then
    echo "usage: $0 feedback-gate <agent-id> <gate> <message>" >&2
    echo "  gates: testing | regression | spec audit | doc audit | security audit | scope" >&2
    exit 2
  fi
  local payload
  payload=$("${PY}" - "${target}" "${gate}" "${msg}" <<'PY'
import json, sys
target, gate, msg = sys.argv[1], sys.argv[2], sys.argv[3]
print(json.dumps({
    "type": "agent.feedback",
    "agent_id": target,
    "payload": {"from": "supervisor", "errors": [f"[{gate}] {msg}"]},
}))
PY
)
  publish "${payload}" >/dev/null || exit 1
  echo "feedback (${gate}) -> ${target}"
}

cmd_verified() {
  local target=${1:-}
  shift || true
  local msg=$*
  if [[ -z "${target}" || -z "${msg}" ]]; then
    echo "usage: $0 verified <agent-id> <message>" >&2
    exit 2
  fi
  local payload
  payload=$("${PY}" - "${target}" "${msg}" <<'PY'
import json, sys
target, msg = sys.argv[1], sys.argv[2]
print(json.dumps({
    "type": "agent.verified",
    "agent_id": target,
    "payload": {"verified_by": "supervisor", "message": msg},
}))
PY
)
  publish "${payload}" >/dev/null || exit 1
  echo "verified: ${target}"
}

# Record an orchestrator adjudication of an in-flight conflict.
#
# Publishes an agent.feedback carrying the [conflict-ack] token to BOTH
# agents. The broker conflict detector reads it as "this pair and file has
# been adjudicated" and stops emitting further agent.question for them,
# while leaving the overlap visible in the in-flight view. A conflict whose
# region declarations later change materially can still escalate again.
#
# The token must be its own entry in errors (the file path runs to the end of
# the string), so the human-readable note is appended as a second entry. The
# note is not optional in the wire sense: agents are taught that
# agent.feedback carries errors to fix, so a bare tag line would read as a
# cryptic error. When no note is given, a default one explains that the
# overlap was reviewed and there is nothing for them to fix.
cmd_conflict_ack() {
  local a=${1:-} b=${2:-} file=${3:-}
  shift 3 2>/dev/null || true
  local note=$*
  if [[ -z "${a}" || -z "${b}" || -z "${file}" ]]; then
    echo "usage: $0 conflict-ack <agent-a> <agent-b> <file> [note]" >&2
    exit 2
  fi
  local target payload
  for target in "${a}" "${b}"; do
    payload=$("${PY}" - "${target}" "${a}" "${b}" "${file}" "${note}" <<'PY'
import json, sys
target, a, b, path, note = sys.argv[1:6]
if not note:
    note = (
        f"conflict on {path} between {a} and {b} was reviewed and ruled "
        "acceptable - nothing for you to fix, carry on with your task"
    )
errors = [f"[conflict-ack] {a} {b} {path}", note]
print(json.dumps({
    "type": "agent.feedback",
    "agent_id": target,
    "payload": {"from": "supervisor", "errors": errors},
}))
PY
)
    # Both acks must land: the pair only stops re-escalating when each agent
    # has been told. A publish the broker rejects exits non-zero here rather
    # than letting the success line claim a delivery that did not happen.
    publish "${payload}" >/dev/null || exit 1
  done
  echo "conflict-ack: ${a} + ${b} on ${file}"
}

cmd_status_publish() {
  # Optional --phase / --detail flags precede the positional message. The
  # plain `status-publish <message…>` form (no flags) is preserved
  # byte-for-byte: the emitted payload then carries neither key (v0.5.0 shape).
  local phase="" detail="" have_phase=0 have_detail=0
  while [[ $# -gt 0 ]]; do
    case "${1:-}" in
      --phase)
        phase="${2:-}"; have_phase=1
        shift 2 || { echo "sweep.sh: --phase requires a value" >&2; exit 2; }
        ;;
      --phase=*)
        phase="${1#--phase=}"; have_phase=1; shift
        ;;
      --detail)
        detail="${2:-}"; have_detail=1
        shift 2 || { echo "sweep.sh: --detail requires a value" >&2; exit 2; }
        ;;
      --detail=*)
        detail="${1#--detail=}"; have_detail=1; shift
        ;;
      --)
        shift; break
        ;;
      *)
        break
        ;;
    esac
  done
  local msg=$*
  if [[ -z "${msg}" ]]; then
    echo "usage: $0 status-publish [--phase <phase>] [--detail '<json-object>'] <message>" >&2
    exit 2
  fi
  # Shape the payload internally via `-c "$(cat <<'EOF' … EOF)"` so stdin stays
  # free (consistent with cmd_feedback_gate / cmd_verified). `phase` is embedded
  # only when --phase was supplied and `detail` only when --detail was supplied;
  # both are omitted otherwise. A --detail that does not parse to a JSON object
  # is rejected (non-zero exit, stderr diagnostic) and nothing is published.
  local payload rc
  payload=$(MSG="${msg}" PHASE="${phase}" DETAIL="${detail}" \
    HAVE_PHASE="${have_phase}" HAVE_DETAIL="${have_detail}" \
    "${PY}" -c "$(cat <<'PY'
import json, os, sys
payload = {"status": "working", "modified_files": [], "message": os.environ["MSG"]}
if os.environ.get("HAVE_PHASE") == "1":
    payload["phase"] = os.environ["PHASE"]
if os.environ.get("HAVE_DETAIL") == "1":
    try:
        parsed = json.loads(os.environ["DETAIL"])
    except ValueError as exc:
        print(f"sweep.sh: --detail is not valid JSON: {exc}", file=sys.stderr)
        sys.exit(6)
    if not isinstance(parsed, dict):
        print("sweep.sh: --detail must be a JSON object", file=sys.stderr)
        sys.exit(6)
    payload["detail"] = parsed
print(json.dumps({
    "type": "agent.status",
    "agent_id": "supervisor",
    "payload": payload,
}))
PY
)")
  rc=$?
  if [[ ${rc} -ne 0 ]]; then
    exit "${rc}"
  fi
  publish "${payload}" >/dev/null || exit 1
  echo "supervisor status published"
}

# Publish an agent.learning from agent_id="supervisor" — the least-privilege
# publish path for qualitative learnings (mirrors cmd_feedback_gate /
# cmd_verified so the supervisor never hand-rolls a raw curl to /publish for a
# learning). <category> and <title> pass through unchanged; <body-json> is
# parsed and passed through verbatim as the payload body (the supervisor skill
# documents the per-category body shape). The id is a stable 16-hex content
# hash and the timestamp is the current UTC time; both are shaped internally so
# the caller supplies only simple args. A <body-json> that is not a JSON object
# is rejected (non-zero exit, stderr diagnostic) and nothing is published, as
# is a body carrying a non-integer number, a negative zero, an integer outside
# the 64-bit range, or a string that is not valid UTF-8 (see the canonical-form
# note in the body below).
cmd_learn() {
  local category=${1:-}
  local title=${2:-}
  local body=${3:-}
  if [[ -z "${category}" || -z "${title}" || -z "${body}" ]]; then
    echo "usage: $0 learn <category> <title> <body-json>" >&2
    exit 2
  fi
  local payload rc
  payload=$("${PY}" - "${category}" "${title}" "${body}" <<'PY'
import json, sys
from datetime import datetime, timezone

category, title, body_raw = sys.argv[1], sys.argv[2], sys.argv[3]


class BadNumberLiteral(Exception):
    """A number whose canonical form the aggregator would not reproduce."""

    def __init__(self, literal, reason):
        super().__init__(literal)
        self.literal = literal
        self.reason = reason


def reject_float_literal(raw):
    raise BadNumberLiteral(raw, "a non-integer number")


def reject_constant_literal(raw):
    raise BadNumberLiteral(raw, "a non-finite number")


def checked_int_literal(raw):
    # The RAW literal is the only place the sign of a negative zero survives:
    # Python folds `-0` to the int 0 and renders it `0`, while the aggregator
    # renders it `-0.0`, so the two canonical forms would differ and the same
    # record would get two ids. Caught here rather than after parsing, where
    # the sign is already gone.
    if raw.lstrip().startswith("-") and int(raw) == 0:
        raise BadNumberLiteral(raw, "a negative zero")
    return int(raw)


try:
    body = json.loads(
        body_raw,
        parse_float=reject_float_literal,
        parse_constant=reject_constant_literal,
        parse_int=checked_int_literal,
    )
except BadNumberLiteral as exc:
    print(
        f"sweep.sh: learn <body-json> carries {exc.reason} ({exc.literal}); "
        "learning bodies carry 64-bit integers, strings, booleans, null, "
        "arrays and objects",
        file=sys.stderr,
    )
    sys.exit(6)
except ValueError as exc:
    print(f"sweep.sh: learn <body-json> is not valid JSON: {exc}", file=sys.stderr)
    sys.exit(6)
if not isinstance(body, dict):
    print("sweep.sh: learn <body-json> must be a JSON object", file=sys.stderr)
    sys.exit(6)

# Numbers are restricted to 64-bit integers because the canonical form must
# render BYTE-IDENTICALLY here and in the aggregator, and only integers do.
# Python and the aggregator disagree on exponent-form floats (1e+16 / 1e-05
# against 1e16 / 1e-5) and on negative zero (0 against -0.0), and an integer
# outside the 64-bit range is a float on the aggregator side and an exact int
# here — in every case the two publishers would hash different strings and
# assign the same record two ids. Learning bodies carry counts and labels, so
# rejecting is the honest answer; a silent mismatch is not.
#
# Floats and negative zero are caught by the parse hooks above, which still
# see the raw literal; the walk below covers the 64-bit range and strings that
# are not encodable as UTF-8, where the parsed value is enough and a path makes
# the message useful.
INT_MIN = -(2 ** 63)
INT_MAX = 2 ** 64 - 1


def reject_unencodable_text(text, what, path):
    # A `\udXXX` escape parses to a LONE SURROGATE, which is not encodable as
    # UTF-8 — so hashing the canonical form (or sending the payload) would die
    # with a UnicodeEncodeError traceback rather than a diagnostic. The
    # aggregator cannot hold such a string at all (Rust `String` is valid
    # UTF-8), so there is no id to agree on either way.
    try:
        text.encode("utf-8")
    except UnicodeEncodeError:
        print(
            f"sweep.sh: learn <body-json> {path} {what} is not valid UTF-8 "
            f"({text!r}); a lone surrogate escape (\\udXXX) cannot be encoded "
            "or hashed",
            file=sys.stderr,
        )
        sys.exit(6)


def reject_unrepresentable_numbers(value, path):
    if isinstance(value, bool):
        return
    if isinstance(value, str):
        reject_unencodable_text(value, "string", path)
        return
    if isinstance(value, float):
        print(
            f"sweep.sh: learn <body-json> {path} is a non-integer number ({value!r}); "
            "learning bodies carry 64-bit integers, strings, booleans, null, "
            "arrays and objects",
            file=sys.stderr,
        )
        sys.exit(6)
    if isinstance(value, int):
        if not INT_MIN <= value <= INT_MAX:
            print(
                f"sweep.sh: learn <body-json> {path} is outside the 64-bit integer "
                f"range ({value})",
                file=sys.stderr,
            )
            sys.exit(6)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            reject_unencodable_text(key, "key", path)
            reject_unrepresentable_numbers(item, f"{path}.{key}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            reject_unrepresentable_numbers(item, f"{path}[{index}]")


reject_unrepresentable_numbers(body, "body")
now = datetime.now(timezone.utc)
ts = now.strftime("%Y-%m-%dT%H:%M:%SZ")


def canonical(value):
    # Mirrors the aggregator canonicaliser exactly (supervisor-learnings,
    # "Deterministic id for idempotent re-emission"): an object renders as
    # {key:value,...} with keys in ascending order and written UNQUOTED, an
    # array as [item,...] in its own order, and every other value as its
    # compact JSON text.
    if isinstance(value, dict):
        inner = ",".join(f"{k}:{canonical(value[k])}" for k in sorted(value))
        return "{" + inner + "}"
    if isinstance(value, list):
        return "[" + ",".join(canonical(v) for v in value) + "]"
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def fnv1a64(text):
    # 64-bit FNV-1a over the UTF-8 bytes; offset basis and prime are pinned by
    # the same requirement, so the helper and the aggregator assign the same
    # id to the same logical record.
    h = 0xCBF29CE484222325
    for b in text.encode("utf-8"):
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


# Supervisor learnings are cross-cutting, so the branch id is empty. The title
# is deliberately NOT part of the canonical form — two phrasings of the same
# record must dedupe to one id.
canon = "|".join(
    [category, "", canonical(body), now.strftime("%Y-%m-%dT%H")]
)
rec_id = format(fnv1a64(canon), "016x")
print(json.dumps({
    "type": "agent.learning",
    "payload": {
        "id": rec_id,
        "agent_id": "supervisor",
        "category": category,
        "title": title,
        "body": body,
        "timestamp": ts,
    },
}))
PY
)
  rc=$?
  if [[ ${rc} -ne 0 ]]; then
    exit "${rc}"
  fi
  publish "${payload}" >/dev/null || exit 1
  echo "learning published (${category})"
}

# Resolve the agent_id owning a tmux pane via its current path. Pane indices
# are NOT alphabetical or CLI-arg order, so we match pane_current_path against
# the per-repo discovery file's `agents` roster — each entry carries the
# broker agent id (`branch_id`, already slugified by git-paw) and the
# `worktree_path` it occupies. The longest matching worktree path wins, so a
# worktree nested inside another resolves to the inner agent.
resolve_agent_for_path() {
  local path=$1
  [[ -z "${SESSION_FILE}" || -z "${path}" ]] && return
  SESSION_FILE="${SESSION_FILE}" PANE_PATH="${path}" "${PY}" -c "$(cat <<'PY'
import json, os
sf = os.environ["SESSION_FILE"]
path = os.path.realpath(os.environ["PANE_PATH"])
try:
    d = json.load(open(sf))
except Exception:  # noqa: BLE001
    raise SystemExit(0)
best, best_len = None, -1
for agent in d.get("agents", []):
    wp = agent.get("worktree_path", "")
    aid = agent.get("branch_id", "")
    if not wp or not aid:
        continue
    rp = os.path.realpath(wp)
    if path == rp or path.startswith(rp + os.sep):
        if len(rp) > best_len:
            best, best_len = aid, len(rp)
if best:
    print(best)
PY
)"
}

# Print this session's ROSTER agent ids, one per line: every `branch_id` in the
# per-repo discovery file's `agents` array. `supervisor` is NOT printed — it is
# always known and the caller treats it separately, so it needs no roster entry.
# Prints nothing when no discovery file is present, which is the caller's
# signal to fall back to the broker's own agent-id rule rather than a
# branch-name pattern.
roster_agent_ids() {
  [[ -z "${SESSION_FILE}" ]] && return
  SESSION_FILE="${SESSION_FILE}" "${PY}" -c "$(cat <<'PY'
import json, os
try:
    d = json.load(open(os.environ["SESSION_FILE"]))
except Exception:  # noqa: BLE001
    raise SystemExit(0)
for agent in d.get("agents", []):
    aid = agent.get("branch_id", "")
    if aid:
        print(aid)
PY
)"
}

# Print the pane_index of every coding agent in the per-repo discovery
# roster, one per line. Prints nothing when no discovery file is present or
# it cannot be read, which tells the caller no roster is available.
roster_pane_indices() {
  [[ -z "${SESSION_FILE}" ]] && return
  SESSION_FILE="${SESSION_FILE}" "${PY}" -c "$(cat <<'PY'
import json, os
try:
    d = json.load(open(os.environ["SESSION_FILE"]))
except Exception:  # noqa: BLE001
    raise SystemExit(0)
for agent in d.get("agents", []):
    idx = agent.get("pane_index")
    if isinstance(idx, int) and not isinstance(idx, bool):
        print(idx)
PY
)"
}

# Print last_seen_seconds for an agent from a broker /status JSON blob.
agent_last_seen() {
  local json=$1 agent=$2
  printf '%s' "${json}" | AGENT="${agent}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
agent = os.environ["AGENT"]
try:
    d = json.load(sys.stdin)
except Exception:  # noqa: BLE001
    raise SystemExit(0)
for a in d.get("agents", []):
    if a.get("agent_id") == agent:
        print(a.get("last_seen_seconds", 0))
        break
PY
)"
}

# Count completed task checkboxes (`- [x]`) across tasks.md files in an agent
# worktree. Used by the no-progress heartbeat. Prints 0 when the worktree has
# no tasks.md (grep -r degrades gracefully to empty output on no match).
agent_checkbox_count() {
  local wt=$1
  if [[ -z "${wt}" || ! -d "${wt}" ]]; then
    printf '0'
    return
  fi
  grep -rhE '^[[:space:]]*- \[[xX]\]' --include='tasks.md' "${wt}" 2>/dev/null \
    | wc -l | tr -d '[:space:]'
}

# Count commits reachable from HEAD in an agent worktree. Used by the
# no-progress heartbeat. Prints empty on failure (treated as unknown).
agent_commit_count() {
  local wt=$1
  [[ -z "${wt}" ]] && return
  git -C "${wt}" rev-list --count HEAD 2>/dev/null
}

# Compute the unanswered age (seconds) of an agent's latest supervisor-targeted
# agent.blocked, from a broker /log JSON blob. Prints nothing when there is no
# such block or when a later agent.feedback/agent.verified to the agent shows
# the supervisor already answered.
agent_blocked_age() {
  local json=$1 agent=$2
  [[ -z "${agent}" ]] && return
  printf '%s' "${json}" | AGENT="${agent}" "${PY}" -c "$(cat <<'PY'
import json, os, sys, time
agent = os.environ["AGENT"]
try:
    data = json.load(sys.stdin)
except Exception:  # noqa: BLE001
    raise SystemExit(0)
entries = data.get("entries", [])
block_seq = None
block_ts = None
for e in entries:
    m = e.get("message", {})
    if m.get("type") == "agent.blocked" and m.get("agent_id") == agent:
        p = m.get("payload", {})
        if str(p.get("from", "")).lower() == "supervisor":
            block_seq = e.get("seq", 0)
            block_ts = e.get("timestamp_unix_secs")
if block_seq is None:
    raise SystemExit(0)
# A later supervisor response addressed to the agent means it was answered.
for e in entries:
    if e.get("seq", 0) <= block_seq:
        continue
    m = e.get("message", {})
    if m.get("type") in ("agent.feedback", "agent.verified") and m.get("agent_id") == agent:
        raise SystemExit(0)
now = int(time.time())
age = now - int(block_ts if block_ts is not None else now)
if age < 0:
    age = 0
print(age)
PY
)"
}

# Core stuck-prompt decision + synthetic publish for a single agent. Reads the
# pane capture from stdin; takes <agent> <last_seen_seconds>. Factored out so
# fixture tests can drive it without tmux via the `stuck-eval` subcommand.
stuck_eval() {
  local agent=${1:-} last_seen=${2:-0} checkbox=${3:-} commit=${4:-} blocked_age=${5:-}
  if [[ -z "${agent}" ]]; then
    echo "usage: $0 stuck-eval <agent-id> <last_seen_seconds> [checkbox_count] [commit_count] [blocked_age_seconds]" >&2
    exit 2
  fi
  AGENT="${agent}" LAST_SEEN="${last_seen}" THRESHOLD="${STUCK_THRESHOLD_SECONDS}" \
    MARKERS="${STUCK_MARKERS_REGEX}" STREAM_MARKERS="${STREAM_TIMEOUT_MARKERS_REGEX}" \
    PASTE_MARKER="${PASTE_BUFFER_MARKER_REGEX}" \
    BLOAT_MARKER="${CONTEXT_BLOAT_MARKER_REGEX}" BLOAT_THRESHOLD_K="${CONTEXT_BLOAT_THRESHOLD_K}" \
    NO_PROGRESS_WINDOW="${NO_PROGRESS_WINDOW_SECONDS}" BLOCKED_WINDOW="${BLOCKED_ON_SUPERVISOR_WINDOW_SECONDS}" \
    PROGRESS="${SWEEP_PROGRESS_FILE}" CHECKBOX="${checkbox}" COMMIT="${commit}" BLOCKED_AGE="${blocked_age}" \
    BROKER="${BROKER}" DEDUP="${STUCK_DEDUP_FILE}" WINDOW="${STUCK_DEDUP_WINDOW_SECONDS}" \
    "${PY}" -c "$(cat <<'PY'
import hashlib, json, os, re, sys, time, urllib.request

cap = sys.stdin.read()
agent = os.environ["AGENT"]
last_seen = int(os.environ.get("LAST_SEEN") or 0)
threshold = int(os.environ["THRESHOLD"])
markers = os.environ["MARKERS"]
stream_markers = os.environ.get("STREAM_MARKERS", "")
paste_marker = os.environ.get("PASTE_MARKER", r"Pasted text #[0-9]")
bloat_marker = os.environ.get("BLOAT_MARKER", "")
bloat_threshold_k = int(os.environ.get("BLOAT_THRESHOLD_K") or 0)
no_progress_window = int(os.environ.get("NO_PROGRESS_WINDOW") or 0)
blocked_window = int(os.environ.get("BLOCKED_WINDOW") or 0)
progress_path = os.environ.get("PROGRESS", "")
checkbox_raw = os.environ.get("CHECKBOX", "").strip()
commit_raw = os.environ.get("COMMIT", "").strip()
blocked_age_raw = os.environ.get("BLOCKED_AGE", "").strip()
broker = os.environ["BROKER"]
dedup_path = os.environ["DEDUP"]
window = int(os.environ["WINDOW"])
now = int(time.time())

prompt = cap.strip()[:200]


def load_entries():
    entries = {}
    try:
        with open(dedup_path) as f:
            for line in f:
                parts = line.rstrip("\n").split("\t")
                if len(parts) == 3:
                    entries[(parts[0], parts[1])] = int(parts[2])
    except FileNotFoundError:
        pass
    return entries


def save_entries(entries):
    os.makedirs(os.path.dirname(dedup_path), exist_ok=True)
    tmp = dedup_path + ".tmp"
    with open(tmp, "w") as f:
        for (a, s), t in entries.items():
            f.write(f"{a}\t{s}\t{t}\n")
    os.replace(tmp, dedup_path)


def clear_dedup_for_agent():
    # Recovery reset: drop dedup entries for this agent so a future stall of any
    # shape re-publishes rather than being suppressed forever.
    entries = load_entries()
    pruned = {k: v for k, v in entries.items() if k[0] != agent}
    if len(pruned) != len(entries):
        save_entries(pruned)


def load_progress():
    prog = {}
    if not progress_path:
        return prog
    try:
        with open(progress_path) as f:
            for line in f:
                parts = line.rstrip("\n").split("\t")
                if len(parts) == 4:
                    try:
                        prog[parts[0]] = (parts[1], parts[2], int(parts[3]))
                    except ValueError:
                        continue
    except FileNotFoundError:
        pass
    return prog


def save_progress(prog):
    if not progress_path:
        return
    os.makedirs(os.path.dirname(progress_path), exist_ok=True)
    tmp = progress_path + ".tmp"
    with open(tmp, "w") as f:
        for a, (cb, ci, ts) in prog.items():
            f.write(f"{a}\t{cb}\t{ci}\t{ts}\n")
    os.replace(tmp, progress_path)


def publish(phase, message, detail, shape_key):
    # Dedup per (agent, shape) within the detection window: one publish per
    # window per shape. A persistently-stuck agent emits exactly once.
    entries = load_entries()
    key = (agent, shape_key)
    last = entries.get(key)
    if last is not None and now - last < window:
        print(f"stuck: {agent} ({phase}, deduped)")
        return
    payload = json.dumps({
        "type": "agent.status",
        "agent_id": agent,
        "payload": {
            "status": "working",
            "modified_files": [],
            "phase": phase,
            "message": message,
            "detail": detail,
        },
    }).encode("utf-8")
    try:
        req = urllib.request.Request(
            broker + "/publish",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        urllib.request.urlopen(req, timeout=2).read()
    except Exception as exc:  # noqa: BLE001
        print(f"stuck: {agent} publish failed: {exc}", file=sys.stderr)
        raise SystemExit(0)
    entries[key] = now
    save_entries(entries)
    print(f"stuck: {agent} published (phase={phase})")


# Classification order (Decision 2): read LIVE pane state first. Pane-marker
# shapes are evaluated BEFORE the no-progress heuristic so an agent sitting on
# a prompt is never mis-classified as idle.

# 1. Stream-timeout / transport-error marker: the CLI API call failed and the
#    agent is stalled. The marker is definitive evidence (no heartbeat gate).
if stream_markers and re.search(stream_markers, cap):
    publish(
        "stuck-stream-timeout",
        "stream timeout / transport error in pane",
        {"captured_prompt": prompt},
        "stuck-stream-timeout",
    )
    raise SystemExit(0)

# 2. Permission / paste-buffer marker → stuck-on-prompt (existing path).
#    Paste-buffer is checked first so its more specific marker wins. The
#    permission match is case-insensitive: the embedded profile stores
#    approval_markers lowercased (the compiled Rust detectors lowercase both
#    sides before comparing), so a case-sensitive match here silently missed
#    every prompt whose on-screen text was not literally lowercase once the
#    markers started coming from the binary instead of this fallback literal
#    (v0.16.0 regression).
variant = None
if re.search(paste_marker, cap):
    variant = "paste-buffer"
elif re.search(markers, cap, re.IGNORECASE):
    variant = "permission"

if variant is not None:
    if last_seen < threshold:
        # Fresh heartbeat — the agent may have caught it pre-stall. A pane
        # marker is present, so the read-pane rule forbids falling through to
        # the no-progress heuristic: hold off, do not prune.
        print(f"not-stuck: {agent} (fresh heartbeat {last_seen}s < {threshold}s)")
        raise SystemExit(0)
    shape = hashlib.sha1(prompt.encode("utf-8", "replace")).hexdigest()[:16]
    publish(
        "stuck-on-prompt",
        f"stuck on prompt ({variant})",
        {"captured_prompt": prompt, "variant": variant},
        shape,
    )
    raise SystemExit(0)

# 3. Context-bloat clear hint at/over the configured threshold → proactive
#    flag. The agent is still responsive; the flag lets the drive loop pre-empt
#    the eventual freeze. Below threshold is NOT flagged and falls through (the
#    /clear hint alone is not a stall).
if bloat_marker:
    m = re.search(bloat_marker, cap)
    if m:
        try:
            tokens_k = int(m.group(2))
        except (IndexError, ValueError):
            tokens_k = None
        if tokens_k is not None and bloat_threshold_k and tokens_k >= bloat_threshold_k:
            publish(
                "context-bloat",
                f"context bloat (~{tokens_k}k tokens >= {bloat_threshold_k}k)",
                {"captured_prompt": prompt, "tokens_k": tokens_k,
                 "threshold_k": bloat_threshold_k},
                "context-bloat",
            )
            raise SystemExit(0)

# 4. Blocked-on-supervisor: an unanswered supervisor-targeted block whose wait
#    exceeds the window. blocked_age is computed by detect-stuck from the
#    broker agent.blocked stream (passed here as an arg for fixture testing).
if blocked_age_raw:
    try:
        blocked_age = int(blocked_age_raw)
    except ValueError:
        blocked_age = None
    if blocked_age is not None and blocked_window and blocked_age >= blocked_window:
        publish(
            "blocked-on-supervisor",
            f"blocked on supervisor for {blocked_age}s (unanswered)",
            {"from": "supervisor", "unanswered_for_seconds": blocked_age},
            "blocked-on-supervisor",
        )
        raise SystemExit(0)

# 5. No-progress heartbeat: BOTH checkbox count AND commit count unchanged over
#    the window (Decision 3). Only reachable when no pane marker is present.
if checkbox_raw != "" and commit_raw != "":
    prog = load_progress()
    prev = prog.get(agent)
    if prev is None:
        # First observation only records state — never no-progress.
        prog[agent] = (checkbox_raw, commit_raw, now)
        save_progress(prog)
        clear_dedup_for_agent()
        print(f"not-stuck: {agent} (no-progress: first observation recorded)")
        raise SystemExit(0)
    prev_cb, prev_ci, prev_ts = prev
    if checkbox_raw != prev_cb or commit_raw != prev_ci:
        # Movement in either counter clears the timer (reset the snapshot).
        prog[agent] = (checkbox_raw, commit_raw, now)
        save_progress(prog)
        clear_dedup_for_agent()
        print(f"not-stuck: {agent} (no-progress: counters moved)")
        raise SystemExit(0)
    unchanged_for = now - prev_ts
    if no_progress_window and unchanged_for >= no_progress_window:
        # Keep prev_ts so unchanged_for keeps growing across sweeps; dedup keeps
        # the publish to one per detection window.
        publish(
            "no-progress",
            f"no progress for {unchanged_for}s (checkbox+commit unchanged)",
            {"unchanged_for_seconds": unchanged_for,
             "checkbox_count": checkbox_raw, "commit_count": commit_raw},
            "no-progress",
        )
        raise SystemExit(0)
    # Unchanged but within the window — not yet no-progress; keep the snapshot.
    print(f"not-stuck: {agent} (no-progress: unchanged {unchanged_for}s < {no_progress_window}s)")
    raise SystemExit(0)

# Nothing detected — clear any prior dedup so a future stall re-publishes.
clear_dedup_for_agent()
print(f"not-stuck: {agent} (no stuck marker)")
PY
)"
}

cmd_detect_stuck() {
  require_session
  if [[ ${PANE_COUNT} -eq 0 ]]; then
    echo "no coding panes discovered in session '${SESSION}'" >&2
    return 0
  fi
  local status_json log_json
  status_json=$(curl -s "${BROKER}/status")
  log_json=$(curl -s "${BROKER}/log?since=0")
  for p in "${PANES[@]}"; do
    local cap path agent ls wt cb ci bage
    cap=$(tmux capture-pane -t "${SESSION}:0.${p}" -p -S -50 2>/dev/null | tail -50)
    path=$(tmux display-message -p -t "${SESSION}:0.${p}" '#{pane_current_path}' 2>/dev/null)
    agent=$(resolve_agent_for_path "${path}")
    if [[ -z "${agent}" ]]; then
      continue
    fi
    ls=$(agent_last_seen "${status_json}" "${agent}")
    [[ -z "${ls}" ]] && ls=0
    wt=$(git -C "${path}" rev-parse --show-toplevel 2>/dev/null)
    cb=$(agent_checkbox_count "${wt}")
    ci=$(agent_commit_count "${wt}")
    bage=$(agent_blocked_age "${log_json}" "${agent}")
    printf '%s' "${cap}" | stuck_eval "${agent}" "${ls}" "${cb}" "${ci}" "${bage}"
  done
}

# Read a pane capture from stdin and run the stuck decision for one agent.
# Extra positional args (checkbox_count, commit_count, blocked_age_seconds) let
# the no-progress and blocked-on-supervisor branches be driven from fixtures
# without tmux or a live broker.
cmd_stuck_eval() {
  local agent=${1:-} last_seen=${2:-0} checkbox=${3:-} commit=${4:-} blocked_age=${5:-}
  local cap
  cap=$(cat)
  printf '%s' "${cap}" | stuck_eval "${agent}" "${last_seen}" "${checkbox}" "${commit}" "${blocked_age}"
}

# Live-prompt gate (arg 1: the pane capture). Structural mirror of
# live_prompt_markers_at_tail in src/supervisor/permission_prompt.rs: anchors on
# the modal block by searching upward from the end of the non-blank lines,
# tolerating up to LIVE_PROMPT_TRAILING_BOUND trailing non-prompt lines (a
# task list, a status strip, a progress footer) so a CLI that renders chrome
# below its input area does not hide a live modal underneath it.
#
# At the true end of the capture (offset 0): a textual marker
# (LIVE_PROMPT_MARKERS_REGEX, case-insensitive) within the last 4 non-blank
# lines, or a numbered option line anchoring that tail with a textual marker
# within the last 15 non-blank lines -- a multi-option prompt bottoms out in
# its option list, with the question above it.
#
# At any bounded offset above the true end, a bare substring match is NOT
# enough: a line merely mentioning the footer text (narration, grep-style
# output echoing it) is not a live modal. The gate instead requires the
# modal's actual SHAPE: LIVE_PROMPT_FOOTER_LINE_REGEX must match a line
# STARTING the last-4 window (not just appear anywhere in it) AND
# LIVE_PROMPT_OPTION_REGEX must match a numbered option line within the last
# 15 non-blank lines. Either alone can appear in prose or tool output; both
# together only render while a modal is genuinely live. A bare confirmation
# question never qualifies here either way -- an answered prompt can leave
# its question text in the pane followed by ordinary command output, and
# admitting that would misread a resolved prompt as live.
#
# Gate-review safety finding: even the shape check above can be fooled by a
# FULL modal copied verbatim into tool output (e.g. the supervisor's own pane
# running `sweep.sh capture 2`, echoing another pane's footer + option lines
# quoted behind the tool-output glyph). So beyond the narrow tail, a match is
# rejected outright when an INPUT_BOX_LINE_REGEX line (the CLI's idle
# composer, not an option line) sits anywhere between the matched footer and
# the capture's true end -- a real live modal replaces the input box while it
# is up, so seeing one below a footer means the footer is quoted/historical.
#
# This gate runs BEFORE any keystroke, so a pane with no live prompt can
# never be typed into.

# True when $1 is the CLI's idle input-box line, recognising BOTH shapes: a
# `│`-bordered composer line, or a borderless composer line framed by a
# horizontal rule immediately above it ($2, the preceding non-blank line --
# current Claude Code's real shape, ground-truthed against a live
# `tmux capture-pane -p`). Either way, a numbered option line is never read as
# the input box (the same selection-caret sigil also prefixes option lines).
# Mirrors is_idle_input_box_line in src/supervisor/permission_prompt.rs.
is_idle_input_box_line() {
  local line="${1}" preceding="${2:-}"
  grep -Eq "${INPUT_BOX_LINE_REGEX}" <<<"${line}" || return 1
  grep -Eq "${LIVE_PROMPT_OPTION_REGEX}" <<<"${line}" && return 1
  if [[ "${line}" == *"│"* ]]; then
    return 0
  fi
  [[ -n "${preceding}" ]] || return 1
  local trimmed_preceding
  trimmed_preceding=$(sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' <<<"${preceding}")
  grep -Eq "${RULE_LINE_REGEX}" <<<"${trimmed_preceding}"
}

is_live_capture() {
  local -a nonblank
  local line total limit offset keep tail_start block_start i idx
  while IFS= read -r line; do
    [[ -n "${line//[[:space:]]/}" ]] && nonblank+=("${line}")
  done <<<"${1}"
  total=${#nonblank[@]}
  if ((total == 0)); then
    return 1
  fi
  limit=${LIVE_PROMPT_TRAILING_BOUND}
  if ((limit > total)); then
    limit=${total}
  fi
  for ((offset = 0; offset <= limit; offset++)); do
    keep=$((total - offset))
    if ((keep <= 0)); then
      continue
    fi
    tail_start=$((keep > 4 ? keep - 4 : 0))
    block_start=$((keep > 15 ? keep - 15 : 0))
    if ((offset == 0)); then
      for ((i = tail_start; i < keep; i++)); do
        if grep -Eqi "${LIVE_PROMPT_MARKERS_REGEX}" <<<"${nonblank[i]}"; then
          return 0
        fi
      done
      local tail_has_option=0
      for ((i = tail_start; i < keep; i++)); do
        if grep -Eq "${LIVE_PROMPT_OPTION_REGEX}" <<<"${nonblank[i]}"; then
          tail_has_option=1
          break
        fi
      done
      if ((tail_has_option)); then
        for ((i = block_start; i < keep; i++)); do
          if grep -Eqi "${LIVE_PROMPT_MARKERS_REGEX}" <<<"${nonblank[i]}"; then
            return 0
          fi
        done
      fi
    else
      local block_has_option=0
      for ((idx = block_start; idx < keep; idx++)); do
        if grep -Eq "${LIVE_PROMPT_OPTION_REGEX}" <<<"${nonblank[idx]}"; then
          block_has_option=1
          break
        fi
      done
      if ((block_has_option == 0)); then
        continue
      fi
      for ((i = tail_start; i < keep; i++)); do
        if grep -Eqi "${LIVE_PROMPT_FOOTER_LINE_REGEX}" <<<"${nonblank[i]}"; then
          local blocked=0
          for ((idx = i + 1; idx < total; idx++)); do
            if is_idle_input_box_line "${nonblank[idx]}" "${nonblank[idx - 1]}"; then
              blocked=1
              break
            fi
          done
          if ((blocked == 0)); then
            return 0
          fi
        fi
      done
    fi
  done
  return 1
}

main() {
  local sub=${1:-}
  shift || true
  case "${sub}" in
    snapshot) cmd_snapshot "$@" ;;
    capture) cmd_capture "$@" ;;
    approve) cmd_approve "$@" ;;
    status) cmd_status "$@" ;;
    events) cmd_events "$@" ;;
    detect-stuck) cmd_detect_stuck "$@" ;;
    stuck-eval) cmd_stuck_eval "$@" ;;
    worktrees-status) cmd_worktrees_status "$@" ;;
    inbox) cmd_inbox "$@" ;;
    feedback-gate) cmd_feedback_gate "$@" ;;
    verified) cmd_verified "$@" ;;
    conflict-ack) cmd_conflict_ack "$@" ;;
    status-publish) cmd_status_publish "$@" ;;
    learn) cmd_learn "$@" ;;
    -h|--help|help|"") usage; [[ -z "${sub}" ]] && exit 2 || exit 0 ;;
    *) echo "sweep.sh: unknown subcommand '${sub}'" >&2; usage; exit 2 ;;
  esac
}

main "$@"
