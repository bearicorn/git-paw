#!/usr/bin/env bash
# Agent-side docs-fetch helper for a paw-* coding agent.
#
# Generalized helper installed by `git paw init` into
# `<repo>/.git-paw/scripts/docs-fetch.sh` (the analogue of `broker.sh`, but for
# *documentation retrieval* rather than broker coordination). The coding agent
# invokes this script via its stable relative path; it discovers everything it
# needs at runtime and shapes the requests internally, so callers pass only
# simple positional arguments:
#
#   - project root via `git rev-parse --show-toplevel`,
#   - the docs base URL from <repo>/.git-paw/config.toml top-level
#     `docs_base_url` (default: git-paw's published documentation site).
#
# Wrapping every docs->agent curl behind one stable script path means the
# launch path can seed a single least-privilege allowlist grant
# (`.git-paw/scripts/docs-fetch.sh`) instead of a broad `curl *` rule, matching
# the by-path model of the broker helper.
#
# Documentation is fetched ON DEMAND from the configured site; no doc content
# is shipped inside this script or the binary. Lookup is best-effort: on an
# unreachable site or a missing page the helper exits non-zero with a short
# diagnostic so the agent can continue its task without the docs.
#
# Subcommands implemented (per docs-fetch-skill D2):
#   find <query>            — fetch llms.txt and print the best-matching page
#                             entries (title + absolute URL + summary)
#   get <page-or-url> [anchor]
#                           — fetch a page; with an anchor, print only that
#                             section using the page's published anchors

set -u

# ---------------------------------------------------------------------------
# Discovery: project root, paw dir, docs base URL, Python interpreter.
# ---------------------------------------------------------------------------

# Built-in default docs site. Kept in sync with the `docs_base_url` default
# documented in `git paw init`'s config template and the Rust config accessor.
DEFAULT_DOCS_BASE_URL="https://bearicorn.github.io/git-paw"

repo_root() {
  git rev-parse --show-toplevel 2>/dev/null
}

PROJECT_ROOT=$(repo_root)
if [[ -z "${PROJECT_ROOT}" ]]; then
  echo "docs-fetch.sh: not inside a git repository" >&2
  exit 2
fi

PAW_DIR="${PROJECT_ROOT}/.git-paw"
CONFIG_TOML="${PAW_DIR}/config.toml"

# Locate a Python 3 interpreter for config parsing and HTML/llms.txt shaping.
if command -v python3 >/dev/null 2>&1; then
  PY=python3
elif command -v python >/dev/null 2>&1 && \
     [[ "$(python -c 'import sys;print(sys.version_info[0])' 2>/dev/null)" == "3" ]]; then
  PY=python
else
  echo "docs-fetch.sh: requires Python 3 on PATH (python3 or python)" >&2
  exit 4
fi

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

BASE_URL=$(discover_docs_base_url)

usage() {
  cat >&2 <<USAGE
usage: $0 <subcommand> [args]

  find <query>                 fetch llms.txt and print best-matching page
                               entries (title + absolute URL + summary)
  get <page-or-url> [anchor]   fetch a page; with an anchor, print only that
                               section using the page's published anchors

Documentation is fetched on demand from the configured docs site. Lookups are
best-effort: on an unreachable site or a missing page the helper exits
non-zero with a short diagnostic so you can continue your task without docs.

Discovered configuration:
  docs base url:  ${BASE_URL}
  project root:   ${PROJECT_ROOT}
USAGE
}

# The shared fetch primitive (same tool the broker helper uses): silent, fail
# on HTTP errors, and bounded so a hung site can never block the agent.
fetch() {
  curl -fsS --connect-timeout 5 --max-time 20 "$1"
}

# Join the base URL with a page path, or pass an absolute URL through.
resolve_page_url() {
  local page=$1
  case "${page}" in
    http://*|https://*|file://*) printf '%s' "${page}" ;;
    /*) printf '%s' "${BASE_URL%/}${page}" ;;
    *) printf '%s' "${BASE_URL%/}/${page}" ;;
  esac
}

cmd_find() {
  local query=$*
  if [[ -z "${query}" ]]; then
    echo "usage: $0 find <query>" >&2
    exit 2
  fi
  local url body
  url="${BASE_URL%/}/llms.txt"
  if ! body=$(fetch "${url}"); then
    echo "docs-fetch.sh: could not fetch ${url} (docs site unreachable?); continue your task without the docs" >&2
    exit 3
  fi
  printf '%s' "${body}" | QUERY="${query}" "${PY}" -c "$(cat <<'PY'
import os, re, sys

query = os.environ.get("QUERY", "").lower()
terms = [t for t in re.split(r"\s+", query) if t]
entry_re = re.compile(r"^\s*-\s+\[([^\]]+)\]\(([^)]+)\):\s*(.*)$")

entries = []
for line in sys.stdin.read().splitlines():
    m = entry_re.match(line)
    if m:
        entries.append((m.group(1).strip(), m.group(2).strip(), m.group(3).strip()))

def score(entry):
    hay = (entry[0] + " " + entry[2] + " " + entry[1]).lower()
    return sum(1 for t in terms if t in hay)

ranked = [e for e in entries if score(e) > 0]
ranked.sort(key=score, reverse=True)

if not ranked:
    sys.stderr.write("docs-fetch.sh: no pages matched the query; listing all pages\n")
    ranked = entries

for title, url, summary in ranked[:10]:
    print(title)
    print("  " + url)
    if summary:
        print("  " + summary)
    print()
PY
)"
}

cmd_get() {
  local page=${1:-} anchor=${2:-}
  if [[ -z "${page}" ]]; then
    echo "usage: $0 get <page-or-url> [anchor]" >&2
    exit 2
  fi
  local url body
  url=$(resolve_page_url "${page}")
  if ! body=$(fetch "${url}"); then
    echo "docs-fetch.sh: could not fetch ${url} (missing page or docs site unreachable?); continue your task without the docs" >&2
    exit 3
  fi
  printf '%s' "${body}" | ANCHOR="${anchor}" "${PY}" -c "$(cat <<'PY'
import json, os, re, sys
from html.parser import HTMLParser

anchor = os.environ.get("ANCHOR", "").strip()
page_html = sys.stdin.read()

# Authoritative anchor list published by agent-friendly-docs-site, if present.
meta_anchors = []
meta_match = re.search(
    r"<script[^>]*id=\"git-paw-page-metadata\"[^>]*>(.*?)</script>",
    page_html,
    re.DOTALL,
)
if meta_match:
    try:
        meta = json.loads(meta_match.group(1).replace("<\\/", "</"))
        meta_anchors = [a for a in meta.get("anchors", []) if a]
    except (ValueError, AttributeError):
        meta_anchors = []


class Extractor(HTMLParser):
    """Collect readable text from <main> and record heading positions."""

    def __init__(self, force_all):
        super().__init__(convert_charrefs=True)
        self.force_all = force_all
        self.main_depth = 0
        self.skip_depth = 0
        self.parts = []
        self.length = 0
        # (anchor_id_or_none, level, offset) in document order
        self.headings = []

    def _capturing(self):
        return self.skip_depth == 0 and (self.force_all or self.main_depth > 0)

    def _emit(self, text):
        self.parts.append(text)
        self.length += len(text)

    def handle_starttag(self, tag, attrs):
        if tag == "main":
            self.main_depth += 1
            return
        if not (self.force_all or self.main_depth > 0):
            return
        if tag in ("script", "style"):
            self.skip_depth += 1
            return
        if re.fullmatch(r"h[1-6]", tag):
            aid = dict(attrs).get("id")
            self._emit("\n\n")
            self.headings.append((aid, int(tag[1]), self.length))
            self._emit("#" * int(tag[1]) + " ")
        elif tag == "li":
            self._emit("\n- ")
        elif tag in ("p", "br", "div", "section", "article", "ul", "ol", "pre", "tr", "table", "blockquote"):
            self._emit("\n")

    def handle_endtag(self, tag):
        if tag == "main" and self.main_depth > 0:
            self.main_depth -= 1
            return
        if tag in ("script", "style") and self.skip_depth > 0:
            self.skip_depth -= 1
            return
        if not (self.force_all or self.main_depth > 0):
            return
        if re.fullmatch(r"h[1-6]", tag):
            self._emit("\n")
        elif tag in ("p", "div", "section", "article", "ul", "ol", "pre", "tr", "table", "blockquote"):
            self._emit("\n")

    def handle_data(self, data):
        if self._capturing():
            self._emit(data)

    @property
    def raw(self):
        return "".join(self.parts)


def clean(text):
    # Collapse runs of blank lines. Avoid comma-brace regex quantifiers here:
    # this body is delivered to python via a quoted heredoc, and macOS bash 3.2
    # brace-expands comma-braces even inside a quoted heredoc, corrupting the
    # pattern. A brace-free form (three literal newlines plus one-or-more) is
    # equivalent and survives delivery intact.
    text = re.sub(r"[ \t]+\n", "\n", text)
    text = re.sub(r"\n\n\n+", "\n\n", text)
    return text.strip() + "\n"


parser = Extractor(force_all=False)
parser.feed(page_html)
parser.close()
if not parser.raw.strip():
    # No <main> found; degrade to reading the whole document.
    parser = Extractor(force_all=True)
    parser.feed(page_html)
    parser.close()

raw = parser.raw

if not anchor:
    sys.stdout.write(clean(raw))
    sys.exit(0)

# Available anchors: prefer the published metadata, else the parsed headings.
available = meta_anchors or [aid for aid, _lvl, _off in parser.headings if aid]
if anchor not in available:
    sys.stderr.write("docs-fetch.sh: anchor %r not found on page\n" % anchor)
    if available:
        sys.stderr.write("available anchors: " + ", ".join(available) + "\n")
    sys.exit(5)

# Slice from the requested heading to the next heading of the same or higher
# level (its full subtree), so "that section" reads as a coherent unit.
start = None
level = None
for aid, lvl, off in parser.headings:
    if aid == anchor:
        start = off
        level = lvl
        break
if start is None:
    # Anchor is in the metadata but not a parsed heading; return whole page.
    sys.stdout.write(clean(raw))
    sys.exit(0)

end = len(raw)
for aid, lvl, off in parser.headings:
    if off > start and lvl <= level:
        end = off
        break

sys.stdout.write(clean(raw[start:end]))
PY
)"
}

main() {
  local sub=${1:-}
  shift || true
  case "${sub}" in
    find) cmd_find "$@" ;;
    get) cmd_get "$@" ;;
    -h|--help|help|"") usage; [[ -z "${sub}" ]] && exit 2 || exit 0 ;;
    *) echo "docs-fetch.sh: unknown subcommand '${sub}'" >&2; usage; exit 2 ;;
  esac
}

main "$@"
