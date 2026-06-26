#!/usr/bin/env bash
# Agent-side broker helper for a paw-* coding agent.
#
# Generalized helper installed by `git paw init` into
# `<repo>/.git-paw/scripts/broker.sh` (the analogue of `sweep.sh`, but for
# the *agent* side rather than the supervisor). The coding agent invokes
# this script via its stable relative path; it discovers everything it
# needs at runtime and shapes the JSON internally, so callers pass only
# simple positional arguments:
#
#   - project root via `git rev-parse --show-toplevel`,
#   - broker URL from <repo>/.git-paw/config.toml [broker] port (default
#     9119) and bind (default 127.0.0.1),
#   - the agent's own id from `--agent <id>` (the pre-expanded branch id
#     the boot block passes) or, absent one, from slugifying the current
#     worktree branch (mirrors sweep.sh's resolve_agent_for_path slug rules).
#
# Wrapping every agent->broker curl behind one stable script path means the
# launch path can seed a single least-privilege allowlist grant
# (`.git-paw/scripts/broker.sh`) instead of a broad `curl *` rule, removing
# the boot-publish dead-stall (v0.7.0 dogfood) where the first register curl
# raised an unanswerable permission prompt.
#
# Subcommands implemented (per agent-broker-helper §D1):
#   status <message>                       — publish agent.status (working)
#   artifact [--exports a,b] [--files a,b] — publish agent.artifact (done)
#   blocked <needs> <from>                 — publish agent.blocked
#   question <text>                        — publish agent.question
#   intent <summary> <files> [ttl]         — publish agent.intent
#   poll [since]                           — GET this agent's broker inbox

set -u

# ---------------------------------------------------------------------------
# Discovery: project root, paw dir, broker URL, agent id.
# ---------------------------------------------------------------------------

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null
}

PROJECT_ROOT=$(repo_root)
if [[ -z "${PROJECT_ROOT}" ]]; then
  echo "broker.sh: not inside a git repository" >&2
  exit 2
fi

PAW_DIR="${PROJECT_ROOT}/.git-paw"
CONFIG_TOML="${PAW_DIR}/config.toml"

# Locate a Python 3 interpreter for JSON / TOML shaping.
if command -v python3 >/dev/null 2>&1; then
  PY=python3
elif command -v python >/dev/null 2>&1 && \
     [[ "$(python -c 'import sys;print(sys.version_info[0])' 2>/dev/null)" == "3" ]]; then
  PY=python
else
  echo "broker.sh: requires Python 3 on PATH (python3 or python)" >&2
  exit 4
fi

# Parse [broker] port + bind from config.toml. Defaults to 127.0.0.1:9119.
discover_broker_url() {
  if [[ ! -f "${CONFIG_TOML}" ]]; then
    echo "http://127.0.0.1:9119"
    return
  fi
  "${PY}" -c "$(cat <<'PY'
import sys

path = sys.argv[1]
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
    text = open(path).read()
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
    with open(path, mode) as f:
        data = tomllib.load(f)
    broker = data.get("broker", {})
    port = broker.get("port", 9119)
    bind = broker.get("bind", "127.0.0.1")
    print(f"http://{bind}:{port}")
PY
)" "${CONFIG_TOML}"
}

# Slugify a branch name the same way the broker does (lowercase, non
# [a-z0-9_] -> '-', collapse runs, strip ends, default 'agent').
slugify() {
  AGENT_BRANCH="$1" "${PY}" -c "$(cat <<'PY'
import os, re
b = os.environ.get("AGENT_BRANCH", "")
s = b.lower()
s = re.sub(r"[^a-z0-9_]", "-", s)
s = re.sub(r"-+", "-", s).strip("-")
print(s or "agent")
PY
)"
}

# Resolve the agent id: explicit --agent override wins; otherwise slugify the
# current worktree branch.
resolve_agent() {
  if [[ -n "${AGENT_OVERRIDE}" ]]; then
    printf '%s\n' "${AGENT_OVERRIDE}"
    return
  fi
  local branch
  branch=$(git -C "${PROJECT_ROOT}" symbolic-ref --short HEAD 2>/dev/null)
  if [[ -z "${branch}" ]]; then
    echo "broker.sh: cannot resolve agent id (no --agent and detached HEAD)" >&2
    exit 2
  fi
  slugify "${branch}"
}

# ---------------------------------------------------------------------------
# Global option parsing: a leading `--agent <id>` before the subcommand.
# ---------------------------------------------------------------------------

AGENT_OVERRIDE=""
while [[ $# -gt 0 ]]; do
  case "${1:-}" in
    --agent)
      AGENT_OVERRIDE="${2:-}"
      shift 2 || { echo "broker.sh: --agent requires an id" >&2; exit 2; }
      ;;
    --agent=*)
      AGENT_OVERRIDE="${1#--agent=}"
      shift
      ;;
    *)
      break
      ;;
  esac
done

BROKER=$(discover_broker_url)

usage() {
  cat >&2 <<USAGE
usage: $0 [--agent <id>] <subcommand> [args]

  status <message>                       publish agent.status (status="working", message, modified_files=[])
  artifact [--exports a,b] [--files a,b] publish agent.artifact (status="done") — code-less DONE fallback
  blocked <needs> <from>                 publish agent.blocked with dependency info
  question <text>                        publish agent.question
  intent <summary> <files> [ttl]         publish agent.intent (files: comma-separated; ttl: valid_for_seconds)
  poll [since]                           GET <broker>/messages/<agent-id>?since=<n> and print the inbox

The agent id comes from --agent <id> (the boot block passes the pre-expanded
branch id) or, absent one, from slugifying the current worktree branch.

Discovered configuration:
  broker:         ${BROKER}
  project root:   ${PROJECT_ROOT}
  agent:          ${AGENT_OVERRIDE:-<from current branch>}
USAGE
}

# POST a JSON BrokerMessage payload to /publish.
publish() {
  local payload=$1
  curl -s -X POST "${BROKER}/publish" \
    -H "Content-Type: application/json" \
    -d "${payload}"
}

cmd_status() {
  local msg=$*
  if [[ -z "${msg}" ]]; then
    echo "usage: $0 status <message>" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent)
  payload=$("${PY}" -c "$(cat <<'PY'
import json, sys
agent, msg = sys.argv[1], sys.argv[2]
print(json.dumps({
    "type": "agent.status",
    "agent_id": agent,
    "payload": {"status": "working", "message": msg, "modified_files": []},
}))
PY
)" "${agent}" "${msg}")
  publish "${payload}"
}

cmd_artifact() {
  local exports="" files=""
  while [[ $# -gt 0 ]]; do
    case "${1:-}" in
      --exports) exports="${2:-}"; shift 2 || { echo "broker.sh: --exports requires a value" >&2; exit 2; } ;;
      --exports=*) exports="${1#--exports=}"; shift ;;
      --files) files="${2:-}"; shift 2 || { echo "broker.sh: --files requires a value" >&2; exit 2; } ;;
      --files=*) files="${1#--files=}"; shift ;;
      *) echo "broker.sh: unknown artifact option '${1}'" >&2; exit 2 ;;
    esac
  done
  local agent payload
  agent=$(resolve_agent)
  payload=$(EXPORTS="${exports}" FILES="${files}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
agent = sys.argv[1]
def split(name):
    raw = os.environ.get(name, "")
    return [p.strip() for p in raw.split(",") if p.strip()]
print(json.dumps({
    "type": "agent.artifact",
    "agent_id": agent,
    "payload": {
        "status": "done",
        "exports": split("EXPORTS"),
        "modified_files": split("FILES"),
    },
}))
PY
)" "${agent}")
  publish "${payload}"
}

cmd_blocked() {
  local needs=${1:-} from=${2:-}
  if [[ -z "${needs}" || -z "${from}" ]]; then
    echo "usage: $0 blocked <needs> <from>" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent)
  payload=$("${PY}" -c "$(cat <<'PY'
import json, sys
agent, needs, frm = sys.argv[1], sys.argv[2], sys.argv[3]
print(json.dumps({
    "type": "agent.blocked",
    "agent_id": agent,
    "payload": {"needs": needs, "from": frm},
}))
PY
)" "${agent}" "${needs}" "${from}")
  publish "${payload}"
}

cmd_question() {
  local text=$*
  if [[ -z "${text}" ]]; then
    echo "usage: $0 question <text>" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent)
  payload=$("${PY}" -c "$(cat <<'PY'
import json, sys
agent, text = sys.argv[1], sys.argv[2]
print(json.dumps({
    "type": "agent.question",
    "agent_id": agent,
    "payload": {"question": text},
}))
PY
)" "${agent}" "${text}")
  publish "${payload}"
}

cmd_intent() {
  local summary=${1:-} files=${2:-} ttl=${3:-}
  if [[ -z "${summary}" || -z "${files}" ]]; then
    echo "usage: $0 intent <summary> <files> [valid_for_seconds]" >&2
    echo "  files: comma-separated list of paths the agent is about to touch" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent)
  payload=$(TTL="${ttl}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
agent, summary, files = sys.argv[1], sys.argv[2], sys.argv[3]
payload = {
    "files": [p.strip() for p in files.split(",") if p.strip()],
    "summary": summary,
}
ttl = os.environ.get("TTL", "").strip()
if ttl:
    payload["valid_for_seconds"] = int(ttl)
print(json.dumps({"type": "agent.intent", "agent_id": agent, "payload": payload}))
PY
)" "${agent}" "${summary}" "${files}")
  publish "${payload}"
}

cmd_poll() {
  local since=${1:-0}
  local agent
  agent=$(resolve_agent)
  # Pipe curl into python via `-c` so the script body does not compete with
  # the curl body for stdin (a heredoc on stdin would swallow the pipe).
  curl -s "${BROKER}/messages/${agent}?since=${since}" | "${PY}" -c "$(cat <<'PY'
import json, sys
try:
    data = json.load(sys.stdin)
except Exception as exc:  # noqa: BLE001
    print(f"broker.sh: failed to parse broker /messages: {exc}", file=sys.stderr)
    sys.exit(5)
messages = data.get("messages", [])
if not messages:
    print("(no messages)")
for m in messages:
    t = m.get("type", "?")
    a = m.get("from") or m.get("agent_id", "?")
    print(f"--- {t} from {a} ---")
    print(json.dumps(m.get("payload", {}), indent=2))
    print()
PY
)"
}

main() {
  local sub=${1:-}
  shift || true
  case "${sub}" in
    status) cmd_status "$@" ;;
    artifact) cmd_artifact "$@" ;;
    blocked) cmd_blocked "$@" ;;
    question) cmd_question "$@" ;;
    intent) cmd_intent "$@" ;;
    poll) cmd_poll "$@" ;;
    -h|--help|help|"") usage; [[ -z "${sub}" ]] && exit 2 || exit 0 ;;
    *) echo "broker.sh: unknown subcommand '${sub}'" >&2; usage; exit 2 ;;
  esac
}

main "$@"
