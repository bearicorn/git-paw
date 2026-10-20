#!/usr/bin/env bash
# Shared preamble for the bundled git-paw helper scripts — binary-managed
# (installed by `git paw init`) and sourced by the sibling `broker.sh`,
# `sweep.sh`, and `docs-fetch.sh`; edit `assets/scripts/_paw_common.sh`, never a
# deployed copy.
#
# Sourcing this file resolves the project root, the `.git-paw` directory and its
# config path, and a Python 3 interpreter, then defines the discovery helpers the
# scripts share. It exits non-zero — naming the sourcing script via
# `PAW_SCRIPT_NAME` — when the repository or the interpreter cannot be resolved,
# so a helper never runs on half-resolved discovery.
#
# Project-agnostic: every URL is read from the consumer's resolved
# `.git-paw/config.toml` (or a documented built-in default); no consumer
# toolchain is hard-coded here.
#
# Provides:
#   PROJECT_ROOT / PAW_DIR / CONFIG_TOML / PY  resolved at source time
#   repo_root                                  git top-level, empty when none
#   discover_broker_url                        [broker] bind + port
#   discover_docs_base_url                     top-level docs_base_url
#   discover_prompt_profile_field <f> <def>    resolved CLI prompt-shape profile field
#   paw_publish <broker-url> <payload>         POST /publish, non-zero on rejection
#   slugify <branch>                           broker agent_id slug rules

# Name used in this file's diagnostics. Each helper sets it before sourcing;
# the default keeps the preamble self-contained if one ever forgets.
: "${PAW_SCRIPT_NAME:=paw-helper}"

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null
}

PROJECT_ROOT=$(repo_root)
if [[ -z "${PROJECT_ROOT}" ]]; then
  echo "${PAW_SCRIPT_NAME}: not inside a git repository" >&2
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
  echo "${PAW_SCRIPT_NAME}: requires Python 3 on PATH (python3 or python)" >&2
  exit 4
fi

# Built-in default docs site. Kept in sync with the `docs_base_url` default
# documented in `git paw init`'s config template and the Rust config accessor.
DEFAULT_DOCS_BASE_URL="https://bearicorn.github.io/git-paw"

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

# Resolve the docs base URL: top-level `docs_base_url` from config.toml, else
# the built-in default. Trailing slashes are trimmed by callers as needed.
discover_docs_base_url() {
  if [[ ! -f "${CONFIG_TOML}" ]]; then
    printf '%s\n' "${DEFAULT_DOCS_BASE_URL}"
    return
  fi
  DEFAULT_URL="${DEFAULT_DOCS_BASE_URL}" "${PY}" -c "$(cat <<'PY'
import os, sys

path = sys.argv[1]
default = os.environ.get("DEFAULT_URL", "")
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
    # Minimal fallback: read the top-level `docs_base_url` key that appears
    # before any [section] header, avoiding a tomli dependency.
    import re
    text = open(path).read()
    in_root = True
    val = None
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_root = False
            continue
        if not in_root:
            continue
        m = re.match(r"^\s*docs_base_url\s*=\s*\"([^\"]+)\"", line)
        if m:
            val = m.group(1)
    print(val or default)
else:
    with open(path, mode) as f:
        data = tomllib.load(f)
    url = data.get("docs_base_url")
    print(url if url else default)
PY
)" "${CONFIG_TOML}"
}

# Resolves one CLI prompt-shape profile field via `git paw __prompt-profile`
# (cli-prompt-profiles capability, D3) — the single source of prompt-shape
# literals shared with the compiled detectors, so the shell and Rust views of
# a prompt cannot drift. A marker-list field prints as a `grep -E`-ready
# alternation (each marker regex-escaped); a pattern field prints its raw
# regex source.
#
# Falls back to `default` when `git paw` is not resolvable on PATH, exits
# non-zero, or prints nothing (e.g. outside a recognised repo) — a helper
# script must never block on the binary being present.
discover_prompt_profile_field() {
  local field=$1 default=$2 val
  val=$(git paw __prompt-profile --field "${field}" 2>/dev/null)
  if [[ -z "${val}" ]]; then
    printf '%s\n' "${default}"
  else
    printf '%s\n' "${val}"
  fi
}

# POST a JSON BrokerMessage to `<broker-url>/publish` and CHECK the answer.
#
#   paw_publish <broker-url> <payload-json>
#
# Prints the broker's response body on stdout when it accepts the publish
# (any 2xx). On any other status — or no response at all — prints the broker's
# error body to stderr and returns non-zero, so a publish the broker rejected
# is never reported as done. A caller that genuinely wants fire-and-forget
# must say so explicitly (`paw_publish … || true`).
paw_publish() {
  local url=$1 payload=$2
  local body status rc
  body=$(mktemp "${TMPDIR:-/tmp}/paw-publish.XXXXXX") || {
    echo "${PAW_SCRIPT_NAME}: cannot create a temporary file for the publish response" >&2
    return 1
  }
  status=$(curl -sS -o "${body}" -w '%{http_code}' -X POST "${url}/publish" \
    -H "Content-Type: application/json" \
    -d "${payload}" 2>>"${body}")
  rc=$?
  if [[ ${rc} -ne 0 || -z "${status}" || "${status}" == "000" ]]; then
    echo "${PAW_SCRIPT_NAME}: publish to ${url}/publish failed — no response from the broker" >&2
    cat "${body}" >&2
    rm -f "${body}"
    return 1
  fi
  if [[ "${status}" != 2* ]]; then
    echo "${PAW_SCRIPT_NAME}: broker rejected the publish (HTTP ${status})" >&2
    cat "${body}" >&2
    rm -f "${body}"
    return 1
  fi
  cat "${body}"
  rm -f "${body}"
}

# Slugify a branch name into the broker agent_id the Rust side derives
# (lowercase, non [a-z0-9] -> '-' with underscores included, collapse runs,
# strip ends; empty -> 'agent-unnamed'; a single word gets an 'agent-' prefix
# so the two-segment id the broker's /publish validator requires is always
# produced). Byte-for-byte identical to slugify_branch in
# src/broker/messages.rs -- tests/slugify_parity.rs pins the two together.
#
# The substitution runs before lowercasing and matches ASCII [A-Za-z0-9] only,
# so a non-ASCII branch cannot take Python's Unicode-aware str.lower() path and
# diverge from Rust's to_ascii_lowercase.
#
# The branch reaches Python through the environment, never argv, so a branch
# name can never be read as an interpreter option.
slugify() {
  AGENT_BRANCH="$1" "${PY}" -c "$(cat <<'PY'
import os, re
b = os.environ.get("AGENT_BRANCH", "")
s = re.sub(r"[^A-Za-z0-9]", "-", b).lower()
s = re.sub(r"-+", "-", s).strip("-")
if not s:
    print("agent-unnamed")
elif "-" in s:
    print(s)
else:
    print("agent-" + s)
PY
)"
}
