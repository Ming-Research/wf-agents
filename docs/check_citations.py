#!/usr/bin/env python3
"""Checks that every citation of a paragraph of a document in `docs/` points at a paragraph that exists.

A comment or a document cites a paragraph as ``document `name`, 9`` (with `design` before `document` where it is a
design, `documents` where it names several, as ``documents `a`, 4, and `b`, 32 to 36``, and lists such as `9, 41` and
`5 to 12`), as `the design of the owner's page, 9` (the document `the-owners-page`), and as `(the same, 4)` after one;
`name` is the file `docs/name.md`, and a paragraph is a line of it that begins with its number and a full stop. When a
document changes, a citation that is left behind names nothing, and the agent that reads it is led wrong. A "design
statement" is no citation any document can answer, and is reported too.

What is read is what its syntax says is prose and not data: the comments of `.rs`, `.ts` and `.tsx` files (from `//`)
and of `.toml` and `.py` files (from `#`), whether they stand on a line of their own or after code, and the text of
`.md` files. A string a program holds, whatever its lines, is not read, nor is any file of another kind. Consecutive
lines that hold a comment are read as one text, so that a citation broken over two lines is found.

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
ENTRY = rf"`([a-z0-9-]+)`,\s*({LIST})"
CITATION = re.compile(rf"documents?\s+{ENTRY}(?:\s*,?\s*and\s+{ENTRY})*")
PAGE = re.compile(rf"the design of the owner's page,\s*({LIST})")
SAME = re.compile(rf"\(the same,\s*({LIST})")
STATEMENT = re.compile(r"design statement", re.IGNORECASE)
PAGE_DOCUMENT = "the-owners-page"


def paragraphs(root: Path, name: str):
    """The numbers of the paragraphs of `docs/<name>.md`, or none where there is no such document."""
    path = root / "docs" / f"{name}.md"
    if not path.is_file():
        return None
    text = path.read_text(encoding="utf-8")
    return {int(m.group(1)) for m in re.finditer(r"^(\d+)\. ", text, re.MULTILINE)}


def comments(text: str, suffix: str):
    """The comments of a program as {line number: text}, each from its mark to the end of its line, where the mark
    stands outside every string: a string may run over many lines, and a raw one holds anything."""
    mark = COMMENT_MARKS[suffix]
    rust = suffix == ".rs"
    found = {}
    line = 1
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == "\n":
            line += 1
            i += 1
        elif text.startswith(mark, i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            found[line] = text[i + len(mark) : end]
            i = end
        elif rust and c == "r" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")) and re.match(r'r#*"', text[i:]):
            hashes = len(re.match(r"r(#*)", text[i:]).group(1))
            close = text.find('"' + "#" * hashes, i + 2 + hashes)
            close = n if close < 0 else close + 1 + hashes
            line += text.count("\n", i, close)
            i = close
        elif rust and c == "'":
            if text.startswith("\\", i + 1):
                end = text.find("'", i + 3)
                i = n if end < 0 else end + 1
            elif text[i + 2 : i + 3] == "'":
                i += 3
            else:
                i += 1
        elif c in "\"'`" and not (rust and c != '"'):
            quote = c * 3 if suffix in (".py", ".toml") and text.startswith(c * 3, i) else c
            i += len(quote)
            while i < n and not text.startswith(quote, i):
                if text[i] == "\n":
                    if len(quote) == 1 and suffix in (".py", ".toml") and c != "`":
                        break
                    line += 1
                elif text[i] == "\\" and not (suffix == ".toml" and c == "'"):
                    i += 1
                    if i < n and text[i] == "\n":
                        line += 1
                i += 1
            i += len(quote) if i < n and text.startswith(quote, i) else 0
        else:
            i += 1
    return found


def texts(path: Path):
    """The prose of a file as groups of (line number, text): a run of lines that hold comments, or a paragraph of a
    Markdown file."""
    suffix = path.suffix
    try:
        source = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return
    group = []
    if suffix == ".md":
        for number, line in enumerate(source.split("\n"), 1):
            if line.strip():
                group.append((number, line.strip()))
            elif group:
                yield group
                group = []
    elif suffix in COMMENT_MARKS:
        mark = COMMENT_MARKS[suffix]
        found = comments(source, suffix)
        for number in range(1, source.count("\n") + 2):
            if number in found:
                group.append((number, found[number].lstrip(mark[0] + "!").strip()))
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
        for entry in re.finditer(ENTRY, m.group(0)):
            found.append((m.start() + entry.start(), entry.group(1), entry.group(2)))
    for m in PAGE.finditer(text):
        found.append((m.start(), PAGE_DOCUMENT, m.group(1)))
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
