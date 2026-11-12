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
#     worktree branch (mirrors sweep.sh's resolve_agent_for_path slug rules);
#     in the repository's main working tree no id is derived at all, because
#     an agent is always in a linked worktree -- pass `--agent <id>` there.
#
# Wrapping every agent->broker curl behind one stable script path means a
# permission rule scoped to that exact path (`.git-paw/scripts/broker.sh`)
# can allow it, instead of requiring a broad `curl *` rule. That is what
# removed the boot-publish dead-stall (v0.7.0 dogfood) where the first
# register curl raised an unanswerable permission prompt. It is the agent
# CLI's own allow rule to grant: git-paw writes no such rule automatically.
# `git paw setup-cli <CLI> --allowlist` can write a path-scoped rule for this
# helper into the main checkout's local settings file, but only because the
# user ran it -- running it is the consent, it prints its plan first, and
# `--undo` removes exactly what it added.
#
# Subcommands implemented (per agent-broker-helper §D1):
#   status [--activity <p>] <message>      — publish agent.status (working)
#   artifact [--exports a,b] [--files a,b] — publish agent.artifact (done)
#   blocked <needs> <from>                 — publish agent.blocked
#   question <text>                        — publish agent.question
#   intent <summary> <files> [ttl] [--region <path>#<kind>=<value>]...
#                                          — publish agent.intent (ttl default 900s)
#   poll [since]                           — GET this agent's broker inbox, then
#                                            print a closing `last_seq: <n>` line

set -u

# A delegate run (GIT_PAW_DELEGATE set, any value) returns its result on its
# standard output, never as a broker message or a keystroke into a pane. Inside
# one, every subcommand is refused here, before any broker or pane is reached.
if [[ -n "${GIT_PAW_DELEGATE+set}" ]]; then
  echo "broker.sh: refused inside a delegate run (GIT_PAW_DELEGATE=${GIT_PAW_DELEGATE}): a delegate publishes nothing and types into no pane; its result is its standard output" >&2
  exit 3
fi

# ---------------------------------------------------------------------------
# Discovery: project root, paw dir, broker URL, agent id.
#
# The project root, `.git-paw` paths, Python 3 interpreter, `discover_broker_url`
# and `slugify` all come from the shared preamble `_paw_common.sh`, installed
# beside this script — in its package's `.git-paw/skills/<package>/scripts/` and
# in the compatibility copy `.git-paw/scripts/` — by `git paw init` and
# `git paw start`. Resolving it relative to this file's own location works from
# either installed directory.
# ---------------------------------------------------------------------------

PAW_SCRIPT_NAME="broker.sh"
_PAW_COMMON="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)/_paw_common.sh"
# shellcheck source=_paw_common.sh
if [[ ! -r "${_PAW_COMMON}" ]] || ! . "${_PAW_COMMON}"; then
  echo "broker.sh: cannot load shared preamble '${_PAW_COMMON}' — re-run \`git paw init\`" >&2
  exit 3
fi

# Absolutise a directory path git reported, resolving symlinks so two answers
# for the same directory always compare equal. `rev-parse` may answer
# relatively (plain `.git`), and relative answers are relative to the -C
# directory, so they resolve against PROJECT_ROOT. Prints nothing when the
# input is empty or does not name a reachable directory.
abs_dir() {
  local p=${1:-}
  if [[ -z "${p}" ]]; then
    return 1
  fi
  if [[ "${p}" != /* ]]; then
    p="${PROJECT_ROOT}/${p}"
  fi
  (cd "${p}" 2>/dev/null && pwd -P)
}

# Resolve the agent id: explicit --agent override wins; otherwise slugify the
# current worktree branch.
#
# An id is never DERIVED in the repository's main working tree. Agents publish
# from linked worktrees; the main working tree is the orchestrator's, and it
# always passes an explicit --agent. Deriving there would name an agent after
# the default branch ("main" -> "agent-main") and mint a phantom the broker
# happily accepts, so the derivation is refused instead. An explicit --agent
# skips the check entirely and publishes as before.
resolve_agent() {
  if [[ -n "${AGENT_OVERRIDE}" ]]; then
    printf '%s\n' "${AGENT_OVERRIDE}"
    return
  fi
  # git's own answer to "is this a linked worktree": a linked worktree's git
  # dir differs from the common one. Holds under --separate-git-dir too, and
  # is the same comparison `git paw setup-cli` makes (RepoLayout).
  #
  # Both flags are resolved the same way, through `abs_dir`, rather than with
  # `--absolute-git-dir` / `--path-format=absolute`: `git rev-parse` ECHOES an
  # unrecognised flag as an output line and still exits 0, so on a git too old
  # for `--path-format` the answer is a two-line string that is neither empty
  # nor a path. A guard written to fall back on an empty answer would never
  # fire, and would compare two strings that can never be equal — silently
  # letting every derivation through. One code path, supported everywhere,
  # cannot rot that way.
  local git_dir common_dir
  git_dir=$(abs_dir "$(git -C "${PROJECT_ROOT}" rev-parse --git-dir 2>/dev/null)")
  common_dir=$(abs_dir "$(git -C "${PROJECT_ROOT}" rev-parse --git-common-dir 2>/dev/null)")
  if [[ -n "${git_dir}" && "${git_dir}" == "${common_dir}" ]]; then
    echo "broker.sh: refusing to derive an agent id in the repository's main working tree (${PROJECT_ROOT})" >&2
    echo "broker.sh: agents publish from their own linked worktree; from here, pass the identity you mean with --agent <id>" >&2
    exit 2
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

# Default TTL for `intent` when the caller gives none. The broker requires
# `valid_for_seconds` on every `agent.intent`, so the helper always sends one;
# 900s (15 minutes) is the TTL the coordination skill's examples use.
DEFAULT_INTENT_TTL_SECONDS=900

usage() {
  cat >&2 <<USAGE
usage: $0 [--agent <id>] <subcommand> [args]

  status [--activity <phrase>] <message> publish agent.status (status="working", message, modified_files=[];
                                         --activity adds a short what-I-am-doing phrase)
  artifact [--exports a,b] [--files a,b] publish agent.artifact (status="done") — code-less DONE fallback
  blocked <needs> <from>                 publish agent.blocked with dependency info
  question <text>                        publish agent.question
  intent <summary> <files> [ttl]         publish agent.intent (files: comma-separated;
    [--region <path>#<kind>=<value>]...  ttl: valid_for_seconds, default ${DEFAULT_INTENT_TTL_SECONDS};
                                         --region scopes a path to a function, class, block or
                                         range (range value: <start>-<end>); repeatable)
  poll [since]                           GET <broker>/messages/<agent-id>?since=<n>, print the inbox,
                                         then a closing "last_seq: <n>" line (the next since)

Run each call bare — no appended exit-status probe and no pipe — so a
permission rule matching this script's path matches every call. A rejected
publish already exits non-zero with the broker's error on stderr.

The agent id comes from --agent <id> (the boot block passes the pre-expanded
branch id) or, absent one, from slugifying the current worktree branch. In the
repository's main working tree no id is derived: pass --agent <id> there.

Discovered configuration:
  broker:         ${BROKER}
  project root:   ${PROJECT_ROOT}
  agent:          ${AGENT_OVERRIDE:-<from current branch>}
USAGE
}

# POST a JSON BrokerMessage payload to /publish via the shared preamble's
# status-checking publisher: a publish the broker rejects prints its error to
# stderr and returns non-zero, so no subcommand reports a rejected publish as
# done.
publish() {
  paw_publish "${BROKER}" "$1"
}

cmd_status() {
  local activity="" has_activity=0
  while [[ $# -gt 0 ]]; do
    case "${1:-}" in
      --activity) activity="${2:-}"; has_activity=1; shift 2 || { echo "broker.sh: --activity requires a phrase" >&2; exit 2; } ;;
      --activity=*) activity="${1#--activity=}"; has_activity=1; shift ;;
      *) break ;;
    esac
  done
  if [[ "${has_activity}" == 1 && -z "${activity}" ]]; then
    echo "broker.sh: --activity requires a phrase" >&2
    exit 2
  fi
  local msg=$*
  if [[ -z "${msg}" ]]; then
    echo "usage: $0 status [--activity <phrase>] <message>" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent) || exit $?
  payload=$(ACTIVITY="${activity}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
agent, msg = sys.argv[1], sys.argv[2]
payload = {"status": "working"}
# Leave the key out when no activity is given, so the plain payload stays as it was.
activity = os.environ.get("ACTIVITY", "")
if activity:
    payload["activity"] = activity
payload["message"] = msg
payload["modified_files"] = []
print(json.dumps({"type": "agent.status", "agent_id": agent, "payload": payload}))
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
  agent=$(resolve_agent) || exit $?
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
  agent=$(resolve_agent) || exit $?
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
  agent=$(resolve_agent) || exit $?
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
  # Regions reach python one per line through the environment.
  local positionals=() regions=""
  local region
  while [[ $# -gt 0 ]]; do
    region=""
    case "${1}" in
      --region) [[ $# -ge 2 && -n "${2}" ]] || { echo "broker.sh: --region requires <path>#<kind>=<value>" >&2; exit 2; }
        region="${2}"; shift 2 ;;
      --region=*) region="${1#--region=}"; shift ;;
      *) positionals+=("${1}"); shift; continue ;;
    esac
    # Regions travel one per line, so a newline inside one would split it.
    if [[ "${region}" == *$'\n'* ]]; then
      echo "broker.sh: --region must not contain a newline" >&2
      exit 2
    fi
    regions+="${region}"$'\n'
  done
  local summary=${positionals[0]:-} files=${positionals[1]:-} ttl=${positionals[2]:-}
  if [[ -z "${summary}" || -z "${files}" ]]; then
    echo "usage: $0 intent <summary> <files> [valid_for_seconds] [--region <path>#<kind>=<value>]..." >&2
    echo "  files: comma-separated list of paths the agent is about to touch" >&2
    echo "  valid_for_seconds: defaults to ${DEFAULT_INTENT_TTL_SECONDS}" >&2
    echo "  --region: kind is function, class, block or range (range value: <start>-<end>)" >&2
    exit 2
  fi
  local agent payload
  agent=$(resolve_agent) || exit $?
  payload=$(TTL="${ttl}" DEFAULT_TTL="${DEFAULT_INTENT_TTL_SECONDS}" REGIONS="${regions}" "${PY}" -c "$(cat <<'PY'
import json, os, sys
agent, summary, files = sys.argv[1], sys.argv[2], sys.argv[3]

def fail(msg):
    print(f"broker.sh: {msg}", file=sys.stderr)
    sys.exit(2)

KINDS = ("function", "class", "block", "range")

def parse_region(spec):
    # A path (or a block anchor) may itself contain "#", so split at the LAST
    # "#" that opens a "<kind>=" marker; with no such marker, at the last "#".
    split = -1
    for i, ch in enumerate(spec):
        if ch == "#" and any(spec.startswith(k + "=", i + 1) for k in KINDS):
            split = i
    if split < 0:
        split = spec.rfind("#")
    path, rest = (spec[:split], spec[split + 1:]) if split >= 0 else (spec, "")
    kind, eq, value = rest.partition("=")
    if split < 0 or not eq or not path.strip() or not value.strip():
        fail(f"malformed --region {spec!r}: expected <path>#<kind>=<value>")
    path, kind, value = path.strip(), kind.strip(), value.strip()
    if kind in ("function", "class"):
        return path, {"kind": kind, "name": value}
    if kind == "block":
        return path, {"kind": kind, "anchor": value}
    if kind == "range":
        start, dash, end = value.partition("-")
        if not dash or not start.isdigit() or not end.isdigit() or int(start) > int(end):
            fail(f"malformed --region {spec!r}: a range value is <start>-<end> with start <= end")
        return path, {"kind": kind, "start_line": int(start), "end_line": int(end)}
    fail(f"malformed --region {spec!r}: kind must be function, class, block or range")

paths = [p.strip() for p in files.split(",") if p.strip()]
regions = {}
for spec in os.environ.get("REGIONS", "").splitlines():
    if spec.strip():
        path, region = parse_region(spec)
        regions.setdefault(path, []).append(region)
        if path not in paths:
            paths.append(path)
payload = {
    "files": [{"path": p, "regions": regions[p]} if p in regions else p for p in paths],
    "summary": summary,
}
# The broker requires valid_for_seconds on every agent.intent, so an omitted
# TTL takes the documented default rather than being left off the wire (which
# the broker answers with 400).
ttl = os.environ.get("TTL", "").strip() or os.environ["DEFAULT_TTL"]
payload["valid_for_seconds"] = int(ttl)
print(json.dumps({"type": "agent.intent", "agent_id": agent, "payload": payload}))
PY
)" "${agent}" "${summary}" "${files}") || exit $?
  publish "${payload}"
}

cmd_poll() {
  local since=${1:-0}
  # The cursor goes into the request URL, so only a plain number is accepted.
  if [[ ! "${since}" =~ ^[0-9]+$ ]]; then
    echo "broker.sh: poll cursor must be a non-negative integer (got '${since}')" >&2
    exit 2
  fi
  local agent
  agent=$(resolve_agent) || exit $?
  # Pipe curl into python via `-c` so the script body does not compete with
  # the curl body for stdin (a heredoc on stdin would swallow the pipe).
  curl -s "${PAW_BROKER_CURL_BOUNDS[@]}" "${BROKER}/messages/${agent}?since=${since}" | "${PY}" -c "$(cat <<'PY'
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
# Cursor for the next `poll <since>`. Double quotes only here: single quotes
# inside this $(...) reached python mangled.
cursor = data.get("last_seq") or 0
print("last_seq: " + str(cursor))
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
