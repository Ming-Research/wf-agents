#!/usr/bin/env python3
"""Checks that every citation of a paragraph of a document in `docs/` points at a paragraph that exists.

A comment or a document cites a paragraph as ``document `name`, 9`` (with `design` before `document` where it is a
design, and lists such as `9, 41` and `5 to 12`), and `(the same, 4)` after one; `name` is the file `docs/name.md`, and
a paragraph is a line of it that begins with its number and a full stop. When a document changes, a citation that is
left behind names nothing, and the agent that reads it is led wrong. A "design statement" is no citation any document
can answer, and is reported too.

What is read is what its syntax says is prose and not data: the `//` lines of `.rs`, `.ts` and `.tsx` files, the `#`
lines of `.toml` and `.py` files, and the text of `.md` files. A string a program holds is not read, nor is any file of
another kind. Consecutive comment lines are read as one text, so that a citation broken over two lines is found.

Run it with `python3 docs/check_citations.py` from any directory, or give it the root of a tree as an argument. It
prints each fault as `file:line: what`, and exits with 1 if there is one.
"""

import re
import sys
from pathlib import Path

SKIPPED_DIRS = {".git", "node_modules", "target", "dist"}
COMMENT_MARKS = {".rs": "//", ".ts": "//", ".tsx": "//", ".toml": "#", ".py": "#"}

NUMBER = r"\d+"
LIST = rf"{NUMBER}(?:\s*(?:,|and|to)\s*{NUMBER})*"
CITATION = re.compile(rf"document\s+`([a-z0-9-]+)`,\s*({LIST})")
SAME = re.compile(rf"\(the same,\s*({LIST})")
STATEMENT = re.compile(r"design statement", re.IGNORECASE)


def paragraphs(root: Path, name: str):
    """The numbers of the paragraphs of `docs/<name>.md`, or none where there is no such document."""
    path = root / "docs" / f"{name}.md"
    if not path.is_file():
        return None
    text = path.read_text(encoding="utf-8")
    return {int(m.group(1)) for m in re.finditer(r"^(\d+)\. ", text, re.MULTILINE)}


def texts(path: Path):
    """The prose of a file as (first line number, [(line number, text)]) groups: a run of comment lines, or a
    paragraph of a Markdown file."""
    suffix = path.suffix
    try:
        lines = path.read_text(encoding="utf-8").split("\n")
    except (UnicodeDecodeError, OSError):
        return
    group = []
    if suffix == ".md":
        for number, line in enumerate(lines, 1):
            if line.strip():
                group.append((number, line.strip()))
            elif group:
                yield group
                group = []
    elif suffix in COMMENT_MARKS:
        mark = COMMENT_MARKS[suffix]
        for number, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith(mark):
                group.append((number, stripped.lstrip(mark[0]).strip()))
            elif group:
                yield group
                group = []
    if group:
        yield group


def joined(group):
    """The text of a group on one line, and where each of its lines begins in it."""
    text, starts = "", []
    for number, line in group:
        starts.append((len(text), number))
        text += line + " "
    return text, starts


def line_at(starts, offset):
    number = starts[0][1]
    for start, at in starts:
        if start <= offset:
            number = at
    return number


def cited(text):
    """Each citation in a text as (offset, document name, paragraph number), the `(the same, N)` ones with the name of
    the document cited last before them."""
    found = []
    for m in CITATION.finditer(text):
        found.append((m.start(), m.group(1), m.group(2)))
    for m in SAME.finditer(text):
        before = [f for f in found if f[0] < m.start()]
        if before:
            found.append((m.start(), max(before)[1], m.group(1)))
    for offset, name, numbers in sorted(found):
        for number in re.findall(NUMBER, numbers):
            yield offset, name, int(number)


def faults(root: Path):
    known = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if SKIPPED_DIRS & set(relative.parts) or not path.is_file():
            continue
        if path.suffix not in COMMENT_MARKS and path.suffix != ".md":
            continue
        for group in texts(path):
            text, starts = joined(group)
            for m in STATEMENT.finditer(text):
                yield f"{relative}:{line_at(starts, m.start())}: names a design statement, which no document numbers"
            for offset, name, number in cited(text):
                if name not in known:
                    known[name] = paragraphs(root, name)
                where = f"{relative}:{line_at(starts, offset)}"
                if known[name] is None:
                    yield f"{where}: cites document `{name}`, and there is no docs/{name}.md"
                elif number not in known[name]:
                    yield f"{where}: cites paragraph {number} of document `{name}`, which has none"


def main(argv):
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path(__file__).resolve().parent.parent
    found = list(faults(root))
    for fault in found:
        print(fault)
    print(f"{len(found)} faults")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
