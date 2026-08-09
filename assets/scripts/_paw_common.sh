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
