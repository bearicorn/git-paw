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
#   approve <pane>                      — Down + Enter (sticky-yes)
#   status [--all]                      — broker /status, phantoms filtered by default
#   worktrees-status                    — uncommitted-file count per agent worktree
#   inbox                               — agent.question/feedback/blocked from supervisor inbox
#   feedback-gate <agent> <gate> <msg…> — publish agent.feedback with bracketed gate prefix
#   verified <agent> <msg…>             — publish agent.verified
#   status-publish <msg…>               — publish agent.status as supervisor

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

# Most-recently-modified session JSON in .git-paw/sessions/.
discover_session_name() {
  if [[ ! -d "${SESSIONS_DIR}" ]]; then
    return 1
  fi
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
  if [[ -z "${newest}" ]]; then
    return 1
  fi
  "${PY}" - "${newest}" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
name = data.get("session_name")
if name:
    print(name)
PY
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

SESSION=$(discover_session_name || true)
BROKER=$(discover_broker_url)
TEST_COMMAND=$(discover_test_command)

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
  approve <pane>                      Down + Enter to <pane> (sticky-yes)
  status [--all]                      broker /status, one line per agent (phantoms filtered)
  worktrees-status                    uncommitted-file count per agent worktree
  inbox                               supervisor inbox payloads (question/feedback/blocked)
  feedback-gate <agent> <gate> <msg…> publish agent.feedback with bracketed gate prefix
  verified <agent> <msg…>             publish agent.verified
  status-publish <msg…>               publish an agent.status from agent_id="supervisor"

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
  tmux send-keys -t "${SESSION}:0.${pane}" Down >/dev/null 2>&1
  sleep 0.05
  tmux send-keys -t "${SESSION}:0.${pane}" Enter >/dev/null 2>&1
  echo "approved pane ${pane}"
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
  local msg=$*
  if [[ -z "${msg}" ]]; then
    echo "usage: $0 status-publish <message>" >&2
    exit 2
  fi
  local payload
  payload=$("${PY}" - "${msg}" <<'PY'
import json, sys
msg = sys.argv[1]
print(json.dumps({
    "type": "agent.status",
    "agent_id": "supervisor",
    "payload": {"status": "working", "modified_files": [], "message": msg},
}))
PY
)
  publish "${payload}" >/dev/null
  echo "supervisor status published"
}

main() {
  local sub=${1:-}
  shift || true
  case "${sub}" in
    snapshot) cmd_snapshot "$@" ;;
    capture) cmd_capture "$@" ;;
    approve) cmd_approve "$@" ;;
    status) cmd_status "$@" ;;
    worktrees-status) cmd_worktrees_status "$@" ;;
    inbox) cmd_inbox "$@" ;;
    feedback-gate) cmd_feedback_gate "$@" ;;
    verified) cmd_verified "$@" ;;
    status-publish) cmd_status_publish "$@" ;;
    -h|--help|help|"") usage; [[ -z "${sub}" ]] && exit 2 || exit 0 ;;
    *) echo "sweep.sh: unknown subcommand '${sub}'" >&2; usage; exit 2 ;;
  esac
}

main "$@"
