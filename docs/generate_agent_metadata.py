#!/usr/bin/env python3
"""Generate the agent-friendly discovery + retrieval surface for the git-paw docs.

This is a post-build step over the rendered mdBook output. It reads the built
``docs/book/`` tree together with ``docs/src/SUMMARY.md`` and emits four
machine-readable artifacts so an LLM/agent can discover and target the docs:

* ``llms.txt``   — an llmstxt.org-style index (H1 title, a blockquote summary,
                   grouped ``- [Title](absolute-url): summary`` entries in
                   table-of-contents order).
* ``sitemap.xml``— a well-formed sitemap enumerating every page's canonical URL.
* ``robots.txt`` — allows all crawlers and advertises the sitemap.
* per-page HTML  — each built page gets a page-specific
                   ``<meta name="description">`` plus a JSON metadata block
                   (title, canonical URL, description, section anchor ids)
                   injected into its ``<head>``.

Design constraints (see ``openspec/changes/agent-friendly-docs-site``):

* It is *build tooling*, not shipped runtime — hence a standalone script with no
  third-party dependency (Python standard library only).
* Generation is *deterministic*: the build date is supplied as an argument and
  never read from the wall clock, so two runs over unchanged sources produce
  byte-identical ``llms.txt``/``sitemap.xml``/``robots.txt``. HTML injection is
  idempotent, so re-running over an already-processed book is a no-op.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath

# A markdown link: [text](target). Used both for SUMMARY entries and titles.
LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]*)\)")
# A SUMMARY part title / heading line, e.g. "# Reference".
HEADER_RE = re.compile(r"^#\s+(.*\S)\s*$")
# A leading "<!-- summary: ... -->" override at the very start of a source page.
OVERRIDE_RE = re.compile(r"<!--\s*summary:\s*(.+?)\s*-->", re.IGNORECASE | re.DOTALL)
# mdBook's global description meta (emitted on every page from book.toml).
META_DESC_RE = re.compile(
    r'<meta\s+name="description"\s+content="[^"]*"\s*/?>', re.IGNORECASE
)
# The block this script manages, wrapped in stable markers so it is removable
# and re-insertable byte-for-byte (idempotent injection).
BLOCK_START = "<!-- git-paw:agent-metadata:start -->"
BLOCK_END = "<!-- git-paw:agent-metadata:end -->"
BLOCK_RE = re.compile(
    r"[ \t]*" + re.escape(BLOCK_START) + r".*?" + re.escape(BLOCK_END) + r"\n?",
    re.DOTALL,
)
# First sentence terminator: a ., ! or ? followed by whitespace or end of text.
SENTENCE_END_RE = re.compile(r"(?<=[.!?])(\s|$)")


def normalize_ws(text: str) -> str:
    """Collapse all runs of whitespace to a single space and strip the ends."""
    return re.sub(r"\s+", " ", text).strip()


def first_sentence(text: str) -> str:
    """Return the first sentence of ``text`` (up to the first . ! or ?)."""
    text = normalize_ws(text)
    match = SENTENCE_END_RE.search(text)
    return text[: match.start() + 1].strip() if match else text


class PageParser(HTMLParser):
    """Extracts the first ``<main>`` paragraph and the heading anchor ids.

    Only content inside ``<main>`` is considered, so mdBook chrome (the sidebar
    menu title, the keyboard-help popup) is ignored: those headings carry no
    ``id`` and their paragraphs live outside ``<main>``.
    """

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.main_depth = 0
        self.in_para = False
        self.para_done = False
        self.para_parts: list[str] = []
        self.anchors: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "main":
            self.main_depth += 1
            return
        if self.main_depth <= 0:
            return
        if re.fullmatch(r"h[1-6]", tag):
            anchor = dict(attrs).get("id")
            if anchor:
                self.anchors.append(anchor)
        elif tag == "p" and not self.para_done:
            self.in_para = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "main" and self.main_depth > 0:
            self.main_depth -= 1
        elif tag == "p" and self.in_para:
            self.in_para = False
            self.para_done = True

    def handle_data(self, data: str) -> None:
        if self.in_para and not self.para_done:
            self.para_parts.append(data)

    @property
    def lead_paragraph(self) -> str:
        return normalize_ws("".join(self.para_parts))


class SummaryEntry:
    """One documented page discovered from ``SUMMARY.md``."""

    __slots__ = ("title", "src", "depth", "part", "html_path", "url", "summary")

    def __init__(self, title: str, src: str, depth: int, part: str | None) -> None:
        self.title = title
        self.src = src
        self.depth = depth
        self.part = part
        self.html_path = src_to_html(src)
        self.url = ""  # filled once the base URL is known
        self.summary = ""  # filled once the rendered page is parsed


def src_to_html(src: str) -> str:
    """Map a SUMMARY source path to its rendered HTML path (mdBook rules)."""
    path = PurePosixPath(src)
    if path.name.lower() == "readme.md":
        return str(path.with_name("index.html"))
    return str(path.with_suffix(".html"))


def parse_summary(summary_path: Path) -> list[SummaryEntry]:
    """Parse ``SUMMARY.md`` into ordered page entries.

    Bare part titles (``# Reference``) open a new group. The first ``#`` line is
    mdBook's own summary title and is ignored. External links, empty targets and
    non-markdown targets are skipped.
    """
    entries: list[SummaryEntry] = []
    current_part: str | None = None
    seen_title = False
    for raw in summary_path.read_text(encoding="utf-8").splitlines():
        stripped = raw.strip()
        if stripped.startswith("#"):
            header = HEADER_RE.match(stripped)
            if header:
                if seen_title:
                    current_part = header.group(1).strip()
                else:
                    seen_title = True  # the leading "# Summary" title
                continue
        link = LINK_RE.search(raw)
        if not link:
            continue
        title = normalize_ws(link.group(1))
        target = link.group(2).strip().split("#", 1)[0].split("?", 1)[0]
        if not target or target.startswith(("http://", "https://")):
            continue
        if not target.endswith(".md"):
            continue
        depth = (len(raw) - len(raw.lstrip(" "))) // 2
        entries.append(SummaryEntry(title, target, depth, current_part))
    return entries


def page_summary(entry: SummaryEntry, src_dir: Path, rendered: PageParser) -> str:
    """Resolve a page's one-line summary: source override, else first sentence.

    The ``<!-- summary: ... -->`` override is read from the *source* markdown (it
    must lead the file). The fallback is the first sentence of the *rendered*
    lead paragraph, which is robust to mdBook ``{{#include}}`` directives.
    """
    source = (src_dir / entry.src).read_text(encoding="utf-8").lstrip()
    if source.startswith("<!--"):
        override = OVERRIDE_RE.match(source)
        if override:
            return normalize_ws(override.group(1))
    return first_sentence(rendered.lead_paragraph)


def render_llms_txt(title: str, summary: str, entries: list[SummaryEntry]) -> str:
    """Build the llmstxt.org-style index in table-of-contents order."""
    lines = [f"# {title}", "", f"> {summary}", ""]
    current_part = object()  # sentinel distinct from any real part (incl. None)
    wrote_section = False
    for entry in entries:
        if entry.part != current_part:
            current_part = entry.part
            lines.append("")
            lines.append(f"## {entry.part}" if entry.part else "## Documentation")
            lines.append("")
            wrote_section = True
        indent = "  " * entry.depth
        lines.append(f"{indent}- [{entry.title}]({entry.url}): {entry.summary}")
    if not wrote_section:
        lines.append("## Documentation")
    # Collapse the accidental leading blank the first section header inserts.
    text = "\n".join(lines)
    text = re.sub(r"\n{3,}", "\n\n", text).strip("\n")
    return text + "\n"


def render_sitemap(entries: list[SummaryEntry], build_date: str) -> str:
    """Build a well-formed sitemap with one canonical <loc> per page."""
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ]
    for entry in entries:
        lines.append("  <url>")
        lines.append(f"    <loc>{html.escape(entry.url)}</loc>")
        lines.append(f"    <lastmod>{build_date}</lastmod>")
        lines.append("  </url>")
    lines.append("</urlset>")
    return "\n".join(lines) + "\n"


def render_robots(base_url: str) -> str:
    """Build a robots.txt that allows all crawlers and advertises the sitemap."""
    sitemap_url = base_url.rstrip("/") + "/sitemap.xml"
    return "\n".join(
        [
            "User-agent: *",
            "Allow: /",
            "",
            f"Sitemap: {sitemap_url}",
        ]
    ) + "\n"


def inject_metadata(page_html: str, description: str, meta_obj: dict) -> str:
    """Idempotently inject the description meta + JSON metadata block into <head>.

    Order matters: the managed block is removed *first* so the description-meta
    step never sees a stale in-block meta, guaranteeing byte-identical re-runs.
    """
    page_html = BLOCK_RE.sub("", page_html)

    esc = html.escape(description, quote=True)
    new_meta = f'<meta name="description" content="{esc}">'
    if META_DESC_RE.search(page_html):
        page_html = META_DESC_RE.sub(new_meta, page_html, count=1)
        block_body = ""
    else:
        block_body = f"        {new_meta}\n"

    payload = json.dumps(meta_obj, ensure_ascii=False, separators=(", ", ": "))
    payload = payload.replace("</", "<\\/")  # never break out of the <script>
    block = (
        f"        {BLOCK_START}\n"
        f"{block_body}"
        f'        <script type="application/json" '
        f'id="git-paw-page-metadata">{payload}</script>\n'
        f"        {BLOCK_END}\n"
    )
    return re.sub(r"([ \t]*)</head>", block + r"\1</head>", page_html, count=1)


def read_book_metadata(book_toml: Path) -> tuple[str, str]:
    """Read the book title and description from book.toml (regex, no toml dep)."""
    text = book_toml.read_text(encoding="utf-8") if book_toml.exists() else ""
    title = re.search(r'^\s*title\s*=\s*"([^"]*)"', text, re.MULTILINE)
    desc = re.search(r'^\s*description\s*=\s*"([^"]*)"', text, re.MULTILINE)
    return (
        title.group(1) if title else "Documentation",
        desc.group(1) if desc else "",
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--src-dir", required=True, type=Path)
    parser.add_argument("--book-dir", required=True, type=Path)
    parser.add_argument(
        "--base-url", default="https://bearicorn.github.io/git-paw/"
    )
    parser.add_argument(
        "--build-date",
        required=True,
        help="ISO date (YYYY-MM-DD) used for sitemap <lastmod>; never the clock",
    )
    args = parser.parse_args(argv)

    src_dir: Path = args.src_dir
    book_dir: Path = args.book_dir
    base_url = args.base_url if args.base_url.endswith("/") else args.base_url + "/"

    entries = parse_summary(src_dir / "SUMMARY.md")
    if not entries:
        print("error: no documented pages found in SUMMARY.md", file=sys.stderr)
        return 1

    for entry in entries:
        entry.url = base_url + entry.html_path
        html_file = book_dir / entry.html_path
        if not html_file.exists():
            print(
                f"error: SUMMARY page '{entry.src}' has no built page "
                f"'{entry.html_path}' under {book_dir}",
                file=sys.stderr,
            )
            return 1
        page_html = html_file.read_text(encoding="utf-8")
        rendered = PageParser()
        rendered.feed(page_html)
        rendered.close()
        entry.summary = page_summary(entry, src_dir, rendered)

        meta_obj = {
            "title": entry.title,
            "url": entry.url,
            "description": entry.summary,
            "anchors": rendered.anchors,
        }
        html_file.write_text(
            inject_metadata(page_html, entry.summary, meta_obj),
            encoding="utf-8",
        )

    title, site_summary = read_book_metadata(src_dir.parent / "book.toml")
    (book_dir / "llms.txt").write_text(
        render_llms_txt(title, site_summary, entries), encoding="utf-8"
    )
    (book_dir / "sitemap.xml").write_text(
        render_sitemap(entries, args.build_date), encoding="utf-8"
    )
    (book_dir / "robots.txt").write_text(render_robots(base_url), encoding="utf-8")

    print(
        f"generated llms.txt, sitemap.xml, robots.txt and injected metadata "
        f"into {len(entries)} pages under {book_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
