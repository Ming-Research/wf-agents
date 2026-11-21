#!/usr/bin/env python3
"""Checks the files of `principles/` and `decisions/` beside this script against the forms of `README.md` beside it.

It checks what a machine can tell: each file's title, its line of what its concept covers and the blank lines between
its entries; each entry's heading, name and lines in the form of its kind, and the characters of each line; that no
line of a decision names a work, an item or a date; that each entry another names is in the tree, of the kind it is
named as; and that each file a reason names for its data is in the repository. Whether a line says what is true is for
the one who reads it.

It prints one line for each fault, as `<file>:<line>: <what is wrong>`, and ends with a line of what it found. It exits
with 1 when there is a fault and 0 when there is none. It may be run from any directory. With no argument it checks the
tree beside it; given a directory, it checks the tree in that directory instead, as its tests do (`lint_test.py`), and
exits with 2 if there is no such directory. The directory above the tree's is taken for the repository's root: each
file is named from it, and so is each file of data.
"""

import re
import sys
from pathlib import Path

DESIGN = Path(__file__).resolve().parent

KINDS = {"principles": "principle", "decisions": "decision"}
WHO = r"(?:the owner|an agent|a reviewer agent)"
REJECTED = re.compile(r"^- rejected: " + WHO + r" proposed .+; the owner rejected it, because .+\.$")
REPLACED = re.compile(r"^- replaced: .+ was on main; the owner changed it, because .+\.$")
HEADING = re.compile(r"^### (\S+) (\S+)$")
NAME = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)+$")
NAMED = re.compile(r"\b(principle|decision) ([a-z0-9]+(?:-[a-z0-9]+)+)\b")
DATA = re.compile(r"\(data: `([^`]+)`\)")
WORK_ITEM_OR_DATE = re.compile(r"\bW\d+\b|\b[Ww]ork \d+\b|#\d{1,5}\b|\b[Ii]tem #?\d+\b|\b\d{4}-\d{2}-\d{2}\b")

RULE_MOST = 300
WHY_MOST = 300
FIRST_LINE_MOST = 200
LINE_MOST = 300


class Entry:
    def __init__(self, kind, name, file, first, body):
        self.kind, self.name, self.file, self.first, self.body = kind, name, file, first, body
        self.faults = []

    def fault(self, number, what):
        self.faults.append((self.file, number, f"{self.kind} {self.name}: {what}"))


def order(file):
    """Where a file of the tree, named from the repository's root, comes as the tree is read: principles before
    decisions, and a concept's file before the files of the concepts under it."""
    parts = Path(file).with_suffix("").parts
    return list(KINDS).index(parts[1]), parts


def blocks(lines, skip):
    """The entries of a file's lines after the first `skip`, each as the number of its first line and its lines; and
    the numbers of the blank lines too many."""
    found, extra, block, start = [], [], [], 0
    for number, line in enumerate(lines[skip:], start=skip + 1):
        if line:
            if not block:
                start = number
            block.append(line)
        elif block:
            found.append((start, block))
            block = []
        else:
            extra.append(number)
    if block:
        found.append((start, block))
    elif len(lines) > skip:
        extra.append(len(lines))
    return found, extra


def check_principle(entry):
    body, first = entry.body, entry.first
    if len(body) != 2 or body[0].startswith("Why: ") or not body[1].startswith("Why: "):
        entry.fault(first + 1, "a principle is its rule on one line and its reason on one line after `Why: `")
        return
    if len(body[0]) > RULE_MOST:
        entry.fault(first + 1, f"the rule is {len(body[0])} characters, over {RULE_MOST}")
    why = body[1][len("Why: "):]
    if not why.strip():
        entry.fault(first + 2, "the reason after `Why: ` is empty")
    if len(why) > WHY_MOST:
        entry.fault(first + 2, f"the reason is {len(why)} characters, over {WHY_MOST}")


def check_decision(entry):
    body, first = entry.body, entry.first
    if not body or body[0].startswith("- "):
        entry.fault(first + 1, "a decision's first line says what holds now")
        return
    if len(body[0]) > FIRST_LINE_MOST:
        entry.fault(first + 1, f"the first line is {len(body[0])} characters, over {FIRST_LINE_MOST}")
    if len(body) == 1:
        entry.fault(first + 1, "a decision has a `rejected` or a `replaced` line, or more, under its first line")
    for number, line in enumerate(body[1:], start=first + 2):
        if not (REJECTED.match(line) or REPLACED.match(line)):
            entry.fault(number, "not a line in the form of a `rejected` or a `replaced` line")
        if len(line) > LINE_MOST:
            entry.fault(number, f"the line is {len(line)} characters, over {LINE_MOST}")
    for number, line in enumerate(body, start=first + 1):
        named = WORK_ITEM_OR_DATE.search(line)
        if named:
            entry.fault(number, f"names a work, an item or a date: {named.group(0)}")


def read(design, directory, kind, faults):
    """The entries of the files of one directory of the tree in `design`, each checked by itself; the faults of the
    files' own lines go to `faults`."""
    root = design.parent
    found = []
    for path in sorted((design / directory).rglob("*.md"), key=lambda path: order(path.relative_to(root))):
        file = path.relative_to(root).as_posix()
        lines = path.read_text(encoding="utf-8").split("\n")
        if lines and lines[-1] == "":
            lines.pop()
        # Where a file does not open as it should, its entries are read from its first heading on.
        skip = 4
        if len(lines) < 3 or not lines[0].startswith("# ") or lines[1] or not lines[2] or lines[2].startswith("#"):
            faults.append((file, 1, "a file opens with `# <title>`, a blank line and one line of what it covers"))
            skip = next((number for number, line in enumerate(lines) if line.startswith("### ")), len(lines))
        elif len(lines) > 3 and lines[3]:
            faults.append((file, 4, "the line of what the file covers is one line, and a blank line follows it"))
            skip = next((number for number, line in enumerate(lines) if line.startswith("### ")), len(lines))
        entries, extra = blocks(lines, skip)
        for number in extra:
            faults.append((file, number, "two entries are parted by one blank line, and no blank line ends the file"))
        for first, block in entries:
            heading = HEADING.match(block[0])
            if not heading or heading.group(1) != kind:
                faults.append((file, first, f"an entry of {directory}/ opens with `### {kind} <name>`"))
                continue
            entry = Entry(kind, heading.group(2), file, first, block[1:])
            if not NAME.match(entry.name):
                entry.fault(first, "the name is not lowercase words joined by hyphens")
            check_principle(entry) if kind == "principle" else check_decision(entry)
            for number, line in enumerate(entry.body, start=first + 1):
                for data in DATA.findall(line):
                    if not (root / data).exists():
                        entry.fault(number, f"the data `{data}` is not in the repository")
            found.append(entry)
    return found


def main(design=DESIGN):
    design = Path(design).resolve()
    if not design.is_dir():
        print(f"{design} is not a directory", file=sys.stderr)
        return 2
    faults = []
    entries = [entry for directory, kind in KINDS.items() for entry in read(design, directory, kind, faults)]
    names = {}
    for entry in entries:
        if entry.name in names:
            other = names[entry.name]
            entry.fault(entry.first, f"the name is taken by {other.file}:{other.first}")
        else:
            names[entry.name] = entry
    for entry in entries:
        for number, line in enumerate(entry.body, start=entry.first + 1):
            for kind, name in NAMED.findall(line):
                if name not in names:
                    entry.fault(number, f"names {kind} {name}, which no file holds")
                elif names[name].kind != kind:
                    entry.fault(number, f"names {kind} {name}, which is a {names[name].kind}")
    for entry in entries:
        faults += entry.faults
    for file, number, what in sorted(faults, key=lambda fault: (order(fault[0]), fault[1])):
        print(f"{file}:{number}: {what}")
    held = []
    for kind in KINDS.values():
        of_kind = [entry for entry in entries if entry.kind == kind]
        held.append(f"{sum(1 for entry in of_kind if not entry.faults)} of {len(of_kind)} {kind}s hold")
    print(f"{len(faults)} faults; " + " and ".join(held))
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else DESIGN))
