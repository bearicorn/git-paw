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
#   approve <pane>                      — re-confirm live prompt on a fresh capture,
#                                         then send the resolved option digit + Enter
#                                         (option-index selection); refuses pane 0
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
# ---------------------------------------------------------------------------

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null
}

PROJECT_ROOT=$(repo_root)
if [[ -z "${PROJECT_ROOT}" ]]; then
  echo "sweep.sh: not inside a git repository" >&2
  exit 2
fi

PAW_DIR="${PROJECT_ROOT}/.git-paw"
SESSIONS_DIR="${PAW_DIR}/sessions"
CONFIG_TOML="${PAW_DIR}/config.toml"

# Locate a Python 3 interpreter for JSON / TOML parsing.
if command -v python3 >/dev/null 2>&1; then
  PY=python3
elif command -v python >/dev/null 2>&1 && \
     [[ "$(python -c 'import sys;print(sys.version_info[0])' 2>/dev/null)" == "3" ]]; then
  PY=python
else
  echo "sweep.sh: requires Python 3 on PATH (python3 or python)" >&2
  exit 4
fi

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

# Parse [broker] port + bind from config.toml. Defaults to 127.0.0.1:9119.
discover_broker_url() {
  if [[ ! -f "${CONFIG_TOML}" ]]; then
    echo "http://127.0.0.1:9119"
    return
  fi
  "${PY}" - "${CONFIG_TOML}" <<'PY'
import sys

try:
    import tomllib  # py311+
    mode = "rb"
except ModuleNotFoundError:
    try:
        import tomli as tomllib  # py<311
        mode = "rb"
    except ModuleNotFoundError:
        tomllib = None

if tomllib is None:
    # Fall back to a tiny regex parser for the two fields we care about —
    # avoids requiring tomli on minimal Python installs.
    import re
    text = open(sys.argv[1]).read()
    in_broker = False
    port = 9119
    bind = "127.0.0.1"
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_broker = stripped == "[broker]"
            continue
        if not in_broker:
            continue
        m = re.match(r"^\s*port\s*=\s*(\d+)", line)
        if m:
            port = int(m.group(1))
            continue
        m = re.match(r"^\s*bind\s*=\s*\"([^\"]+)\"", line)
        if m:
            bind = m.group(1)
            continue
    print(f"http://{bind}:{port}")
else:
    with open(sys.argv[1], mode) as f:
        data = tomllib.load(f)
    broker = data.get("broker", {})
    port = broker.get("port", 9119)
    bind = broker.get("bind", "127.0.0.1")
    print(f"http://{bind}:{port}")
PY
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
STUCK_MARKERS_REGEX='Do you want to proceed|Do you want to allow|requires approval|Allow this command|\(y/n\)|\[y/N\]'
STUCK_DEDUP_FILE="${PAW_DIR}/.sweep-stuck-dedup"

# Live-prompt structural markers (approve-send gate). Textual markers of a
# LIVE permission prompt, matched case-insensitively; numbered option lines
# (`1.` / `2)`) anchor a multi-option prompt whose question sits above the
# 4-line tail (block window: last 15 non-blank lines). Kept in lockstep with
# the Rust gate in src/supervisor/auto_approve.rs (LIVE_PROMPT_FOOTER /
# LIVE_PROMPT_PROCEED / LIVE_PROMPT_TAIL / LIVE_PROMPT_BLOCK) so `approve`
# and `classify` agree with the in-tool auto-approver on what "live" means.
LIVE_PROMPT_MARKERS_REGEX='do you want to|esc to cancel'

# --- Additional stuck shapes: stream-timeout, context-bloat, no-progress -----
# A coding agent's CLI API call can fail mid-stream (transport error / timeout)
# and sit dead with no permission marker present; and its context can bloat past
# the point where the CLI surfaces a `/clear to save <N>k tokens` hint. Both are
# pane-text markers kept in named regexes next to STUCK_MARKERS_REGEX so a
# CLI-string tweak is a one-line edit. The patterns are deliberately generic
# (multiple symptom phrasings) rather than one CLI's exact string.
STREAM_TIMEOUT_MARKERS_REGEX='Request timed out|Request timeout|stream error|stream timed out|stream disconnected|transport error|Connection error|error streaming'
# Group 2 captures N (thousands of tokens) from a `/clear to save <N>k tokens`
# (or `/compact ...`) hint; the leading slash is optional for CLI variants.
CONTEXT_BLOAT_MARKER_REGEX='/?(clear|compact) to save ([0-9]+)k tokens'
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
  approve <pane>                      re-confirm live prompt on a fresh capture, then send
                                      the resolved option digit + Enter; refuses pane 0
  detect-stuck                        flag stuck agents (prompt/stream-timeout/bloat/no-progress/blocked)
  stuck-eval <agent> <last_seen> [checkbox_count] [commit_count] [blocked_age]
                                      (stdin: capture) run the stuck decision for one agent
  classify [worktree-root]            (stdin: capture) auto-approve classification (escalate/approve/no-op)
  status [--all]                      broker /status, one line per agent (phantoms filtered)
  worktrees-status                    uncommitted-file count per agent worktree
  inbox                               supervisor inbox payloads (question/feedback/blocked)
  feedback-gate <agent> <gate> <msg…> publish agent.feedback with bracketed gate prefix
  verified <agent> <msg…>             publish agent.verified
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

require_session() {
  if [[ -z "${SESSION}" ]]; then
    echo "sweep.sh: no session found under ${SESSIONS_DIR}" >&2
    exit 3
  fi
}

# Publish a JSON BrokerMessage payload to /publish.
publish() {
  local payload=$1
  curl -s -X POST "${BROKER}/publish" \
    -H "Content-Type: application/json" \
    -d "${payload}"
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

cmd_approve() {
  require_session
  local pane=${1:-}
  if [[ -z "${pane}" ]]; then
    echo "usage: $0 approve <pane>" >&2
    exit 2
  fi
  # Pane 0 is the supervisor's own pane; never blind-send keys into it
  # (broker-mediated-approvals). Clearing pane 0's own prompt is a non-blind
  # concern owned by the drive loop.
  if [[ "${pane}" == "0" ]]; then
    echo "pane 0 excluded from blind send-keys (supervisor pane)"
    return 0
  fi
  # Re-confirm a live permission prompt on a FRESH capture taken immediately
  # before sending keys (structural markers at the tail), and resolve the
  # option index from that same capture: the classifier mirror
  # (run_classifier RESOLVE_OPTION mode) parses the prompt and applies the
  # same shape detection and broad-grant rule as the in-tool auto-approver,
  # so both resolve the same index. If the prompt cleared between the
  # decision and now, send NOTHING (no stray digits into the agent's CLI).
  local cap opt
  cap=$(tmux capture-pane -t "${SESSION}:0.${pane}" -p -S -50 2>/dev/null | tail -50)
  if ! opt=$(printf '%s' "${cap}" | run_classifier "${PROJECT_ROOT}" resolve); then
    echo "cleared before send, no keys sent (pane ${pane})"
    return 0
  fi
  # Dispatch the resolved option digit + Enter as two separate keystrokes —
  # never a blind cursor-movement sequence, whose landing option depends on
  # the prompt shape (Down+Enter selects "No" on a 2-option prompt and the
  # permanent broad grant on a 3-option one).
  tmux send-keys -t "${SESSION}:0.${pane}" "${opt}" >/dev/null 2>&1
  sleep 0.05
  tmux send-keys -t "${SESSION}:0.${pane}" Enter >/dev/null 2>&1
  echo "approved pane ${pane} (option ${opt})"
}

cmd_status() {
  local show_all=0
  if [[ "${1:-}" == "--all" ]]; then
    show_all=1
  fi
  # Pipe curl into python via `-c` so the script body doesn't compete
  # with the curl body for stdin (bash heredocs win over pipes, so
  # `python3 - <<'PY'` swallowed the pipe content and read its own body).
  curl -s "${BROKER}/status" | SHOW_ALL=${show_all} "${PY}" -c "$(cat <<'PY'
import json, os, re, sys
try:
    data = json.load(sys.stdin)
except Exception as exc:  # noqa: BLE001
    print(f"sweep.sh: failed to parse broker /status: {exc}", file=sys.stderr)
    sys.exit(5)
show_all = os.environ.get("SHOW_ALL") == "1"
valid = re.compile(r"^(supervisor|feat[-/].+)$")
phantoms = []
for a in data.get("agents", []):
    aid = a.get("agent_id", "?")
    if not show_all and not valid.match(aid):
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
  publish "${payload}" >/dev/null
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
  publish "${payload}" >/dev/null
  echo "verified: ${target}"
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
  publish "${payload}" >/dev/null
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
# is rejected (non-zero exit, stderr diagnostic) and nothing is published.
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
import hashlib, json, sys
from datetime import datetime, timezone

category, title, body_raw = sys.argv[1], sys.argv[2], sys.argv[3]
try:
    body = json.loads(body_raw)
except ValueError as exc:
    print(f"sweep.sh: learn <body-json> is not valid JSON: {exc}", file=sys.stderr)
    sys.exit(6)
if not isinstance(body, dict):
    print("sweep.sh: learn <body-json> must be a JSON object", file=sys.stderr)
    sys.exit(6)
ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
canon = category + "|" + title + "|" + json.dumps(body, sort_keys=True)
rec_id = hashlib.sha1(canon.encode("utf-8")).hexdigest()[:16]
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
  publish "${payload}" >/dev/null
  echo "learning published (${category})"
}

# Resolve the agent_id owning a tmux pane via its current path. Pane indices
# are NOT alphabetical or CLI-arg order, so we match pane_current_path against
# the session JSON's worktree paths and slugify the matching branch.
resolve_agent_for_path() {
  local path=$1
  [[ -z "${SESSION_FILE}" || -z "${path}" ]] && return
  SESSION_FILE="${SESSION_FILE}" PANE_PATH="${path}" "${PY}" -c "$(cat <<'PY'
import json, os, re
def slug(b):
    s = b.lower()
    s = re.sub(r'[^a-z0-9_]', '-', s)
    s = re.sub(r'-+', '-', s).strip('-')
    return s or 'agent'
sf = os.environ["SESSION_FILE"]
path = os.path.realpath(os.environ["PANE_PATH"])
try:
    d = json.load(open(sf))
except Exception:  # noqa: BLE001
    raise SystemExit(0)
best, best_len = None, -1
for wt in d.get("worktrees", []):
    wp = wt.get("worktree_path", "")
    if not wp:
        continue
    rp = os.path.realpath(wp)
    if path == rp or path.startswith(rp + os.sep):
        if len(rp) > best_len:
            best, best_len = wt.get("branch"), len(rp)
if best:
    print(slug(best))
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
#    Paste-buffer is checked first so its more specific marker wins.
variant = None
if re.search(r"Pasted text #[0-9]", cap):
    variant = "paste-buffer"
elif re.search(markers, cap):
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

# Auto-approve classification, mirroring the Rust classifier
# (src/supervisor/auto_approve.rs) so the bundled helper decides identically:
# command-slice extraction, the curated danger-list (+ per-OS addendum) with
# the rm -rf scratch exception, the stack-neutral whitelist composed with the
# [supervisor.common_dev_allowlist] stack presets and safe_commands from
# .git-paw/config.toml (fail-safe: built-ins only when unreadable), worktree-
# confined git add/commit and dev-test shapes, the live-prompt gate, and
# option-index selection.
#
# Reads a pane capture on stdin; takes an optional worktree root (defaults to
# the project root) used by the worktree-confined rules. Prints one of:
#   no-op (not live)
#   escalate (danger|unknown)
#   approve option=<N> (scratch-rm|worktree-git|worktree-dev-test|whitelist)
cmd_classify() {
  local root=${1:-${PROJECT_ROOT}}
  run_classifier "${root}" ""
}

# Shared classifier invocation (stdin: pane capture). $1 = worktree root,
# $2 = non-empty to run the approve-path RESOLVE_OPTION mode: liveness on
# the fresh capture plus option-index resolution only — prints `cleared`
# (exit 1) or the resolved option digit (exit 0). Both `classify` and
# `approve` run this one Python body, so the live-prompt gate, shape
# detection, and broad-grant rule cannot drift between them.
run_classifier() {
  RESOLVE_OPTION="${2:-}" WORKTREE_ROOT="${1}" CONFIG_TOML="${CONFIG_TOML}" \
    LIVE_MARKERS="${LIVE_PROMPT_MARKERS_REGEX}" "${PY}" -c "$(cat <<'PY'
import os, platform, re, sys

cap = sys.stdin.read()
worktree_root = os.environ.get("WORKTREE_ROOT", "")
config_toml = os.environ.get("CONFIG_TOML", "")

# --- command-slice extraction --------------------------------------------
DECOR = " \t│─╭╮╰╯├┤┐└┘┌⎿❯●•*·"

def strip_decoration(line):
    return line.lstrip(DECOR).rstrip()

def is_option_line(line):
    s = line.lstrip()
    return len(s) >= 2 and s[0].isdigit() and s[1] in ".)"

def is_boundary(line):
    low = line.lower()
    return (low.startswith("do you want to") or "requires approval" in low
            or "[y/n]" in low or "(y/n)" in low or is_option_line(line))

def extract_slice(cap):
    lines = cap.splitlines()
    for i in range(len(lines) - 1, -1, -1):
        line = strip_decoration(lines[i])
        idx = line.find("Bash(")
        if idx != -1:
            after = line[idx + 5:]
            end = after.rfind(")")
            if end != -1 and after[:end].strip():
                return after[:end].strip()
        if line.lower().startswith("bash command"):
            for nxt in lines[i + 1:]:
                c = strip_decoration(nxt)
                if not c:
                    continue
                if is_boundary(c):
                    break
                return c
    return None

# --- curated danger-list --------------------------------------------------
DANGER_BASE = ["rm -rf", "rm -fr", "git push", "--force", "force-push",
    "reset --hard", "git rebase", "git checkout ", "branch -D",
    "git worktree remove", "clean -fd", "clean -fdx", "sudo", "mkfs",
    "dd if=", "> /dev/", "chmod -R", "chown -R", "pkill", "kill"]
OS_ADDENDUM = ["diskutil", "/dev/disk"] if platform.system() == "Darwin" \
    else ["/dev/sd", "/dev/nvme", "mkfs"]

def contains_word(hay, word):
    return re.search(r"(?<![A-Za-z0-9_])" + re.escape(word) + r"(?![A-Za-z0-9_])", hay) is not None

def danger_match(s, pat):
    if pat in ("sudo", "kill", "pkill"):
        return contains_word(s, pat)
    return pat in s

# --- rm -rf scratch-path exception ---------------------------------------
def is_scratch_path(p):
    p = p.strip().strip('"').strip("'")
    if p.startswith("/tmp/paw-") or p.startswith("/private/tmp/paw-"):
        return True
    if p.startswith(".git-paw/tmp/") or "/.git-paw/tmp/" in p:
        return True
    tmp = os.environ.get("TMPDIR", "")
    base = tmp.rstrip("/")
    if base and p.startswith(base + "/paw-"):
        return True
    return False

def parse_var_ref(t):
    if not t.startswith("$"):
        return None
    rest = t[1:]
    if rest.startswith("{"):
        rest = rest[1:].rstrip("}")
    if rest and all(c.isalnum() or c == "_" for c in rest):
        return rest
    return None

def resolve_target(tok, assigns):
    t = tok.strip('"').strip("'")
    name = parse_var_ref(t)
    if name is not None:
        if name in assigns:
            return assigns[name]
        return os.environ.get(name)
    if "$TMPDIR" in t:
        tmp = os.environ.get("TMPDIR")
        return None if tmp is None else t.replace("$TMPDIR", tmp.rstrip("/"))
    return t

def rm_targets(s):
    assigns, targets, seen_rm = {}, [], False
    for tok in s.split():
        if tok in ("&&", "||", ";", "|"):
            break
        if not seen_rm:
            if "=" in tok:
                k, v = tok.split("=", 1)
                if k and all(c.isalnum() or c == "_" for c in k):
                    assigns[k] = v
                    continue
            if tok == "rm":
                seen_rm = True
            continue
        if tok.startswith("-"):
            continue
        r = resolve_target(tok, assigns)
        if r is None:
            return None
        targets.append(r)
    return targets

def rm_all_scratch(s):
    t = rm_targets(s)
    return bool(t) and all(is_scratch_path(x) for x in t)

def is_dangerous(s):
    for pat in DANGER_BASE + OS_ADDENDUM:
        if danger_match(s, pat):
            if pat in ("rm -rf", "rm -fr") and rm_all_scratch(s):
                continue
            return True
    return False

def is_scratch_rm(s):
    if "rm -rf" not in s and "rm -fr" not in s:
        return False
    return rm_all_scratch(s) and not is_dangerous(s)

# --- composed whitelist + worktree git -----------------------------------
# Stack-neutral built-ins, kept in lockstep with the Rust constants
# (READ_MOSTLY_VERBS / default_safe_commands in src/supervisor/auto_approve.rs
# and the presets in src/supervisor/dev_allowlist.rs) — a list-parity test in
# tests/sweep_sh_classify.rs asserts byte-for-byte equality.
READ_MOSTLY = ["curl", "cat", "ls", "grep", "rg", "git", "echo", "sed", "awk",
    "find", "wc", "head", "tail", "jq", "mkdir", "touch", "export", "tmux",
    "env"]
EXPLICIT_SAFE = ["git commit", "git push", "curl http://127.0.0.1:"]
DEV_UNIVERSAL = ["git status", "git log", "git diff", "git show", "git fetch",
    "git commit", "git push", "git pull", "git merge", "git stash", "git add",
    "git restore", "git rm", "find", "grep", "sed -n"]
STACK_RUST = ["cargo build", "cargo test", "cargo clippy", "cargo fmt",
    "cargo check", "cargo tree", "cargo deny", "cargo update"]
STACK_NODE = ["npm install", "npm ci", "npm test", "npm run", "pnpm install",
    "pnpm test", "pnpm run", "yarn install", "yarn test"]
STACK_PYTHON = ["pytest", "pip install", "ruff", "black", "mypy", "flake8",
    "uv pip", "uv sync"]
STACK_GO = ["go build", "go test", "go vet", "go fmt", "gofmt", "go mod",
    "golangci-lint"]
STACKS = {"rust": STACK_RUST, "node": STACK_NODE, "python": STACK_PYTHON,
    "go": STACK_GO}

def read_allowlist_config(path):
    # Resolved stacks / extra / safe_commands from .git-paw/config.toml.
    # Fail-safe: any read or parse problem (missing file, malformed TOML,
    # pre-3.11 Python without tomllib) composes built-ins only — fewer
    # auto-approvals, never more.
    try:
        import tomllib
        with open(path, "rb") as f:
            data = tomllib.load(f)
        sup = data.get("supervisor", {})
        dev = sup.get("common_dev_allowlist", {})
        stacks = [s for s in dev.get("stacks", []) if isinstance(s, str)]
        extra = [s for s in dev.get("extra", []) if isinstance(s, str)]
        aa = sup.get("auto_approve", {})
        safe_cmds = [s for s in aa.get("safe_commands", []) if isinstance(s, str)]
        return stacks, extra, safe_cmds
    except Exception:
        return [], [], []

def compose_whitelist(stacks, extra, safe_cmds):
    # Composition order mirrors AutoApproveConfig::effective_whitelist:
    # built-ins, then universal + stack + extra dev patterns, then
    # safe_commands, de-duplicated.
    out = []
    def push(p):
        if p not in out:
            out.append(p)
    for p in EXPLICIT_SAFE + READ_MOSTLY:
        push(p)
    for p in DEV_UNIVERSAL:
        push(p)
    for name in stacks:
        for p in STACKS.get(name, []):
            push(p)
    for p in extra:
        push(p)
    for p in safe_cmds:
        push(p)
    return out

CFG_STACKS, CFG_EXTRA, CFG_SAFE = read_allowlist_config(config_toml)
WHITELIST = compose_whitelist(CFG_STACKS, CFG_EXTRA, CFG_SAFE)

def leading_verb(s):
    for tok in s.split():
        if "=" in tok:
            k = tok.split("=", 1)[0]
            if k and all(c.isalnum() or c == "_" for c in k):
                continue
        return tok.rsplit("/", 1)[-1]
    return None

def starts_with_boundary(s, entry):
    s = s.lstrip()
    if not s.startswith(entry):
        return False
    nxt = s[len(entry):len(entry) + 1]
    return nxt == "" or nxt.isspace()

def is_safe_command(s):
    return any(starts_with_boundary(s, e) for e in WHITELIST)

def is_worktree_git_op(s, root):
    if not (starts_with_boundary(s, "git add") or starts_with_boundary(s, "git commit")):
        return False
    return bool(root) and os.path.isdir(root)

# --- worktree-confined dev-test shapes (rider rules) ----------------------
# Mirrors is_worktree_dev_test_op in src/supervisor/auto_approve.rs: bash -n
# on a worktree script, non-recursive chmod on worktree paths, mktemp, and
# interpreter runs of worktree-resident scripts. Inline -c code strings,
# shell metacharacters, and out-of-worktree paths never match (fail-closed).
INTERPRETERS = ("bash", "sh", "python3", "python", "node")
# chr(96) is the backtick — spelled numerically so the bash heredoc that
# carries this Python body never sees a literal backtick (quote-tracking).
METACHARS = (";", "|", "&", "$", chr(96), ">", "<")

def inside_worktree(p, root):
    rp = os.path.realpath(os.path.join(root, p))
    rr = os.path.realpath(root)
    return rp == rr or rp.startswith(rr + os.sep)

def command_args(s):
    # Tokens with leading VAR=value assignments skipped.
    toks = s.split()
    while toks:
        head = toks[0]
        if "=" in head:
            k = head.split("=", 1)[0]
            if k and all(c.isalnum() or c == "_" for c in k):
                toks = toks[1:]
                continue
        break
    return toks

def is_worktree_dev_test(s, root):
    s = s.strip()
    if any(m in s for m in METACHARS):
        return False
    if not (root and os.path.isdir(root)):
        return False
    toks = command_args(s)
    if not toks:
        return False
    verb = toks[0].rsplit("/", 1)[-1]
    args = toks[1:]
    if "-c" in args:
        return False
    if verb == "mktemp":
        return all(t.startswith("-") for t in args)
    if verb == "chmod":
        if any(t.startswith("-") for t in args):
            return False
        paths = args[1:]
        return bool(paths) and all(inside_worktree(p, root) for p in paths)
    if verb in INTERPRETERS:
        paths = [t for t in args if not t.startswith("-")]
        return bool(paths) and all(inside_worktree(p, root) for p in paths)
    return False

# --- live-prompt gate -----------------------------------------------------
# Structural mirror of is_live_prompt in src/supervisor/auto_approve.rs: a
# textual marker (LIVE_MARKERS, case-insensitive) within the last 4 non-blank
# lines, or a numbered option line anchoring that tail with a textual marker
# within the last 15 non-blank lines (a multi-option prompt bottoms out in
# its numbered option list, with the question above it).
LIVE_MARKERS = os.environ.get("LIVE_MARKERS", "do you want to|esc to cancel")

def live_textual(lines):
    return any(re.search(LIVE_MARKERS, l, re.IGNORECASE) for l in lines)

def is_live(cap):
    nonblank = [l for l in cap.splitlines() if l.strip()]
    tail, block = nonblank[-4:], nonblank[-15:]
    if live_textual(tail):
        return True
    return any(is_option_line(strip_decoration(l)) for l in tail) and live_textual(block)

# --- prompt shape + option-index selection -------------------------------
def detect_shape(cap):
    low = cap.lower()
    return "three" if ("don't ask again" in low or "don’t ask again" in low) else "two"

def is_arbitrary(s):
    if leading_verb(s) in ("python", "python3", "node", "eval"):
        return True
    return "bash -c" in s or "sh -c" in s or " -c " in s

def select_option(shape, s):
    if shape == "two":
        return 1
    if leading_verb(s) in READ_MOSTLY and not is_arbitrary(s):
        return 2
    return 1

# --- approve-path option resolution ----------------------------------------
# `sweep.sh approve` reuses this classifier body (RESOLVE_OPTION non-empty)
# for its pre-send gate: liveness is re-checked on the FRESH capture, then
# the option index is resolved with the same shape detection and broad-grant
# rule (READ_MOSTLY + arbitrary-code check) as the full classification below,
# so the helper and the in-tool auto-approver resolve the same index.
if os.environ.get("RESOLVE_OPTION"):
    if not is_live(cap):
        print("cleared")
        raise SystemExit(1)
    s = extract_slice(cap)
    if s is None:
        s = cap
    print(select_option(detect_shape(cap), s))
    raise SystemExit(0)

# --- decision -------------------------------------------------------------
if not is_live(cap):
    print("no-op (not live)")
    raise SystemExit(0)

s = extract_slice(cap)
if s is None:
    s = cap
opt = select_option(detect_shape(cap), s)

if is_dangerous(s):
    print("escalate (danger)")
elif is_scratch_rm(s):
    print(f"approve option={opt} (scratch-rm)")
elif is_worktree_git_op(s, worktree_root):
    print(f"approve option={opt} (worktree-git)")
elif is_worktree_dev_test(s, worktree_root):
    print(f"approve option={opt} (worktree-dev-test)")
elif is_safe_command(s):
    print(f"approve option={opt} (whitelist)")
else:
    print("escalate (unknown)")
PY
)"
}

main() {
  local sub=${1:-}
  shift || true
  case "${sub}" in
    snapshot) cmd_snapshot "$@" ;;
    capture) cmd_capture "$@" ;;
    approve) cmd_approve "$@" ;;
    status) cmd_status "$@" ;;
    detect-stuck) cmd_detect_stuck "$@" ;;
    stuck-eval) cmd_stuck_eval "$@" ;;
    classify) cmd_classify "$@" ;;
    worktrees-status) cmd_worktrees_status "$@" ;;
    inbox) cmd_inbox "$@" ;;
    feedback-gate) cmd_feedback_gate "$@" ;;
    verified) cmd_verified "$@" ;;
    status-publish) cmd_status_publish "$@" ;;
    learn) cmd_learn "$@" ;;
    -h|--help|help|"") usage; [[ -z "${sub}" ]] && exit 2 || exit 0 ;;
    *) echo "sweep.sh: unknown subcommand '${sub}'" >&2; usage; exit 2 ;;
  esac
}

main "$@"
