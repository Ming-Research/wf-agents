#!/usr/bin/env python3
"""Tests of `lint.py`. Each test builds a small design tree in a temporary directory, runs the script on it as CI runs
it on the real tree, and checks the faults it reports, each by its file and line. What a test expects is taken from the
rules of `README.md` beside this file, not from the script's code. Run it with `python3 design/lint_test.py` from any
directory.
"""

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

LINT = Path(__file__).resolve().parent / "lint.py"
FAULT = re.compile(r"^(\S+):(\d+): .+$")

# The tree every test starts from: in the forms of the README, with principles in one file, decisions in a concept's
# file and in the file of a concept under it, a line of each kind under a decision, names of entries in other files, and
# a file of data in the repository.
TREE = {
    "design/principles/common.md": """\
# Common

What holds for every part of the system.

### principle one-thing-one-place
Each thing is said in one place, and every other place points to it.
Why: Two copies of one thing drift apart, and a reader cannot tell which one holds.

### principle state-from-records
The engine works out its state from its records, never from what it holds in memory.
Why: Records outlive a stopped engine, and what was only in memory is lost with it.
""",
    "design/decisions/engine.md": """\
# Engine

What is true of the engine as a whole.

### decision the-engine-reads-every-record-at-start
The engine reads every record when it starts, as principle state-from-records asks.
- rejected: an agent proposed keeping the state in a cache file; the owner rejected it, because the cache drifted from the records (data: `measurements/cache-drift.csv`).
- replaced: Reading only the last record was on main; the owner changed it, because a stop between two writes lost a step.
""",
    "design/decisions/engine/loop.md": """\
# Loop

What is true of the engine's loop.

### decision one-round-at-a-time
The loop runs one round of a work at a time, after decision the-engine-reads-every-record-at-start.
- rejected: the owner proposed running two rounds side by side; the owner rejected it, because both wrote the same record.
- rejected: a reviewer agent proposed a lock on each record; the owner rejected it, because the lock added a state no test reached.
""",
    "measurements/cache-drift.csv": "run,drift\n1,3\n",
}

# The lines a test's own file opens with: its first entry is on line 5.
OPENING = ["# Extra", "", "What the tests add to the tree.", ""]
RULE = "Each part is built so that it can be tested alone."
WHY = "Why: A part tested only with the others hides the fault of each."
FIRST = "The engine keeps one log for each work."
REJECTED = (
    "- rejected: an agent proposed one log for all works; the owner rejected it, because a work's log could not be read"
    " alone."
)
REPLACED = (
    "- replaced: A log for each round was on main; the owner changed it, because the logs of one work were read"
    " together."
)


def principle(name, rule=RULE, why=WHY):
    return [f"### principle {name}", rule, why]


def decision(name, first=FIRST, under=(REJECTED,)):
    return [f"### decision {name}", first, *under]


def entries(*found):
    """The lines of a file that opens as the README says, with these entries, one blank line between each two."""
    lines = list(OPENING)
    for number, entry in enumerate(found):
        if number:
            lines.append("")
        lines += entry
    return lines


def sized(n, start, end, fill="a"):
    """A line of exactly `n` characters: `start`, then `fill` repeated, then `end`."""
    line = start + fill * (n - len(start) - len(end)) + end
    assert len(line) == n
    return line


class TreeTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        # The repository, and a directory beside it that the script is run from, so that a path read from the directory
        # it is run from, rather than from the repository's root, is never found.
        self.root = Path(temporary.name) / "repository"
        self.elsewhere = Path(temporary.name) / "elsewhere"
        self.elsewhere.mkdir()
        for name, text in TREE.items():
            self.put(name, text)

    def put(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write(self, name, lines):
        """Writes a file of the tree, `name` taken from the design directory, with each line ended by a newline."""
        self.put(f"design/{name}", "".join(line + "\n" for line in lines))

    def run_lint(self, design=None):
        return subprocess.run(
            [sys.executable, str(LINT), str(design or self.root / "design")],
            cwd=str(self.elsewhere),
            capture_output=True,
            text=True,
        )

    def lint(self):
        """The faults the script reports for the tree, each as its file and line. On the way it checks that each fault
        is printed as `<file>:<line>: <what is wrong>`, that a line of what was found ends the report, and that the
        script exits with 1 when there is a fault and 0 when there is none."""
        run = self.run_lint()
        self.assertEqual(run.stderr, "")
        lines = run.stdout.splitlines()
        self.assertTrue(lines, "the script printed nothing")
        *reported, summary = lines
        faults = []
        for line in reported:
            found = FAULT.match(line)
            self.assertIsNotNone(found, f"not a fault in the form `<file>:<line>: <what is wrong>`: {line}")
            faults.append((found.group(1), int(found.group(2))))
        self.assertIsNone(FAULT.match(summary), "the report does not end with a line of what was found")
        self.assertEqual(run.returncode, 1 if faults else 0, run.stdout)
        return faults

    def assertFaults(self, *expected):
        """The tree has exactly these faults, each as a file named from the design directory and a line."""
        self.assertEqual(sorted(self.lint()), sorted((f"design/{name}", line) for name, line in expected))

    def assertFaultsAmong(self, *allowed):
        """The tree has a fault, and each of its faults is at one of these places."""
        faults = self.lint()
        self.assertTrue(faults, "no fault was reported")
        allowed = {(f"design/{name}", line) for name, line in allowed}
        for fault in faults:
            self.assertIn(fault, allowed)

    def assertFaultsWithin(self, name, first, last):
        """The tree has a fault, and each of its faults is in this file, from line `first` to line `last`."""
        self.assertFaultsAmong(*((name, line) for line in range(first, last + 1)))


class TheWholeTree(TreeTest):
    def test_a_tree_in_the_readmes_forms_has_no_fault_and_exits_0(self):
        run = self.run_lint()
        self.assertEqual(run.returncode, 0, run.stdout)
        self.assertEqual(len(run.stdout.splitlines()), 1, run.stdout)
        self.assertFaults()

    def test_a_tree_with_a_fault_exits_1(self):
        self.write("principles/extra.md", entries(principle("spoken-words", why="Because it is so.")))
        run = self.run_lint()
        self.assertEqual(run.returncode, 1, run.stdout)

    def test_every_fault_is_reported_not_only_the_first(self):
        self.write("principles/extra.md", entries(principle("spoken-words", rule=sized(301, "Each part ", "."))))
        line = REJECTED.replace("one log", "one log, against principle no-such-rule,")
        self.write("decisions/extra.md", entries(decision("Spoken-Log", under=[line])))
        self.assertFaults(("principles/extra.md", 6), ("decisions/extra.md", 5), ("decisions/extra.md", 7))

    def test_with_no_argument_it_checks_the_tree_beside_it_from_any_directory(self):
        copy = self.root / "design" / "lint.py"
        shutil.copy(LINT, copy)
        self.write("principles/extra.md", entries(principle("Spoken-Words")))
        run = subprocess.run([sys.executable, str(copy)], cwd=str(self.elsewhere), capture_output=True, text=True)
        self.assertEqual(run.returncode, 1, run.stdout)
        self.assertIn("design/principles/extra.md:5:", run.stdout)

    def test_a_directory_that_is_not_there_exits_2(self):
        run = self.run_lint(self.root / "no-such-directory")
        self.assertEqual(run.returncode, 2, run.stdout)


class AFilesOpening(TreeTest):
    """Each file has a title and one line saying what its concept covers, before its entries."""

    def test_a_file_without_a_title_is_a_fault(self):
        self.write("principles/extra.md", ["What the tests add to the tree.", "", *principle("spoken-words")])
        self.assertFaultsWithin("principles/extra.md", 1, 2)

    def test_a_first_line_that_is_not_a_title_is_a_fault(self):
        lines = ["Extra", "", "What the tests add to the tree.", "", *principle("spoken-words")]
        self.write("principles/extra.md", lines)
        self.assertFaultsWithin("principles/extra.md", 1, 4)

    def test_a_file_without_a_line_of_what_it_covers_is_a_fault(self):
        self.write("principles/extra.md", ["# Extra", "", *principle("spoken-words")])
        self.assertFaultsWithin("principles/extra.md", 1, 3)

    def test_a_line_of_what_it_covers_written_as_two_lines_is_a_fault(self):
        lines = ["# Extra", "", "What the tests add", "to the tree.", "", *decision("spoken-log")]
        self.write("decisions/extra.md", lines)
        self.assertFaultsWithin("decisions/extra.md", 3, 4)


class BlankLines(TreeTest):
    """One blank line between entries."""

    def test_two_blank_lines_between_entries_are_a_fault(self):
        self.write("principles/extra.md", [*OPENING, *principle("spoken-words"), "", "", *principle("written-words")])
        self.assertFaultsWithin("principles/extra.md", 8, 9)

    def test_principles_with_no_blank_line_between_them_are_a_fault(self):
        self.write("principles/extra.md", [*OPENING, *principle("spoken-words"), *principle("written-words")])
        self.assertFaultsWithin("principles/extra.md", 5, 10)

    def test_decisions_with_no_blank_line_between_them_are_a_fault(self):
        self.write("decisions/extra.md", [*OPENING, *decision("spoken-log"), *decision("written-log")])
        self.assertFaultsWithin("decisions/extra.md", 5, 10)

    def test_a_blank_line_ending_a_file_is_a_fault(self):
        lines = [*OPENING, *principle("spoken-words"), ""]
        self.write("principles/extra.md", lines)
        self.assertFaults(("principles/extra.md", len(lines)))


class Headings(TreeTest):
    """An entry opens with `### principle <name>` in principles/ and `### decision <name>` in decisions/."""

    def test_a_decision_among_the_principles_is_a_fault(self):
        self.write("principles/extra.md", entries(decision("spoken-log")))
        self.assertFaults(("principles/extra.md", 5))

    def test_a_principle_among_the_decisions_is_a_fault(self):
        self.write("decisions/extra.md", entries(principle("spoken-words")))
        self.assertFaults(("decisions/extra.md", 5))

    def test_a_heading_of_another_level_is_a_fault(self):
        self.write("principles/extra.md", entries(["## principle spoken-words", RULE, WHY]))
        self.assertFaults(("principles/extra.md", 5))

    def test_a_heading_without_a_name_is_a_fault(self):
        self.write("principles/extra.md", entries(["### principle", RULE, WHY]))
        self.assertFaults(("principles/extra.md", 5))


class Names(TreeTest):
    """A name is lowercase words joined by hyphens, and no two entries in the tree have the same name."""

    def test_lowercase_words_joined_by_hyphens_are_a_name(self):
        self.write("principles/extra.md", entries(principle("spoken-words"), principle("a-cell-knows-only-its-data")))
        self.assertFaults()

    def test_a_name_not_of_lowercase_words_joined_by_hyphens_is_a_fault(self):
        for name in ["Spoken-Words", "spoken_words", "spoken words", "spoken--words", "-spoken-words", "spoken-words-"]:
            with self.subTest(name=name):
                self.write("principles/extra.md", entries(principle(name)))
                self.assertFaults(("principles/extra.md", 5))

    def test_two_principles_of_one_name_in_two_files_are_a_fault(self):
        self.write("principles/extra.md", entries(principle("state-from-records")))
        self.assertFaultsAmong(("principles/common.md", 9), ("principles/extra.md", 5))

    def test_two_entries_of_one_name_in_one_file_are_a_fault(self):
        self.write("principles/extra.md", entries(principle("spoken-words"), principle("spoken-words")))
        self.assertFaultsAmong(("principles/extra.md", 5), ("principles/extra.md", 9))

    def test_two_decisions_of_one_name_in_two_files_are_a_fault(self):
        self.write("decisions/extra.md", entries(decision("one-round-at-a-time")))
        self.assertFaultsAmong(("decisions/engine/loop.md", 5), ("decisions/extra.md", 5))

    def test_a_decision_with_the_name_of_a_principle_is_a_fault(self):
        self.write("decisions/extra.md", entries(decision("one-thing-one-place")))
        self.assertFaultsAmong(("principles/common.md", 5), ("decisions/extra.md", 5))


class Principles(TreeTest):
    """A principle is its rule on one line of at most 300 characters, then `Why: ` and its reason on one line of at most
    300 characters."""

    def test_a_principle_not_of_a_rule_line_and_a_why_line_is_a_fault(self):
        cases = {
            "no reason": [RULE],
            "the rule on two lines": ["Each part is built so that", "it can be tested alone.", WHY],
            "the reason first": [WHY, RULE],
            "a reason without `Why: `": [RULE, "Because a part tested only with the others hides the fault of each."],
            "the reason on two lines": [RULE, "Why: A part tested only with the others", "hides the fault of each."],
        }
        for case, body in cases.items():
            with self.subTest(case=case):
                self.write("principles/extra.md", entries(["### principle spoken-words", *body]))
                self.assertFaultsWithin("principles/extra.md", 5, 4 + 1 + len(body))

    def test_an_empty_reason_is_a_fault(self):
        self.write("principles/extra.md", entries(["### principle spoken-words", RULE, "Why: "]))
        self.assertFaults(("principles/extra.md", 7))

    def test_a_rule_of_300_characters_holds(self):
        for fill in ["a", "é"]:
            with self.subTest(fill=fill):
                rule = sized(300, "Each part ", ".", fill)
                self.write("principles/extra.md", entries(principle("spoken-words", rule=rule)))
                self.assertFaults()

    def test_a_rule_of_301_characters_is_a_fault(self):
        self.write("principles/extra.md", entries(principle("spoken-words", rule=sized(301, "Each part ", "."))))
        self.assertFaults(("principles/extra.md", 6))

    def test_a_reason_of_300_characters_holds(self):
        self.write("principles/extra.md", entries(principle("spoken-words", why="Why: " + sized(300, "A part ", "."))))
        self.assertFaults()

    def test_a_reason_of_301_characters_is_a_fault(self):
        self.write("principles/extra.md", entries(principle("spoken-words", why="Why: " + sized(301, "A part ", "."))))
        self.assertFaults(("principles/extra.md", 7))


class ADecisionsFirstLine(TreeTest):
    """A decision's first line says what is true now, in at most 200 characters."""

    def test_a_first_line_of_200_characters_holds(self):
        self.write("decisions/extra.md", entries(decision("spoken-log", first=sized(200, "The engine ", "."))))
        self.assertFaults()

    def test_a_first_line_of_201_characters_is_a_fault(self):
        self.write("decisions/extra.md", entries(decision("spoken-log", first=sized(201, "The engine ", "."))))
        self.assertFaults(("decisions/extra.md", 6))

    def test_a_first_line_that_is_a_line_of_a_list_is_a_fault(self):
        self.write("decisions/extra.md", entries(["### decision spoken-log", REJECTED, REPLACED]))
        self.assertFaultsWithin("decisions/extra.md", 5, 7)


class TheLinesUnderADecision(TreeTest):
    """Under the first line come one or more lines, each at most 300 characters, each in the exact form of a `rejected`
    or a `replaced` line. The first `<who>` of a `rejected` line is the owner, an agent or a reviewer agent; the one who
    rejected or changed an approach is the owner."""

    def test_a_decision_with_no_line_under_its_first_is_a_fault(self):
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[])))
        self.assertFaultsWithin("decisions/extra.md", 5, 6)

    def test_the_owner_an_agent_and_a_reviewer_agent_may_propose(self):
        for who in ["the owner", "an agent", "a reviewer agent"]:
            with self.subTest(who=who):
                line = REJECTED.replace("an agent proposed", f"{who} proposed")
                self.write("decisions/extra.md", entries(decision("spoken-log", under=[line, REPLACED])))
                self.assertFaults()

    def test_a_line_not_in_the_exact_form_is_a_fault(self):
        approach = "one log for all works"
        because = "because a work's log could not be read alone."
        lines = [
            f"- rejected: a user proposed {approach}; the owner rejected it, {because}",
            f"- rejected: an agent proposed {approach}; an agent rejected it, {because}",
            f"- rejected: an agent proposed {approach}; the owner rejected it.",
            f"- rejected: an agent proposed {approach}; the owner rejected it, {because[:-1]}",
            f"- rejected: an agent suggested {approach}; the owner rejected it, {because}",
            f"- Rejected: an agent proposed {approach}; the owner rejected it, {because}",
            f"rejected: an agent proposed {approach}; the owner rejected it, {because}",
            f"* rejected: an agent proposed {approach}; the owner rejected it, {because}",
            f"- replaced: {approach} was on main; a reviewer agent changed it, {because}",
            f"- replaced: {approach} was tried; the owner changed it, {because}",
            f"- replaced: {approach} was on main; the owner changed it.",
            f"- deferred: an agent proposed {approach}; the owner put it off, {because}",
            "and the engine reads the log when the work ends.",
        ]
        for line in lines:
            with self.subTest(line=line):
                self.write("decisions/extra.md", entries(decision("spoken-log", under=[REJECTED, line])))
                self.assertFaults(("decisions/extra.md", 8))

    def test_the_first_line_carried_onto_a_second_line_is_a_fault(self):
        under = ["and reads it when the work ends.", REJECTED]
        self.write("decisions/extra.md", entries(decision("spoken-log", under=under)))
        self.assertFaults(("decisions/extra.md", 7))

    def test_a_line_of_300_characters_holds(self):
        lines = [
            sized(300, "- rejected: an agent proposed ", "; the owner rejected it, because it was slower."),
            sized(300, "- replaced: ", " was on main; the owner changed it, because it was slower."),
        ]
        for line in lines:
            with self.subTest(line=line[:12]):
                self.write("decisions/extra.md", entries(decision("spoken-log", under=[line])))
                self.assertFaults()

    def test_a_line_of_301_characters_is_a_fault(self):
        lines = [
            sized(301, "- rejected: an agent proposed ", "; the owner rejected it, because it was slower."),
            sized(301, "- replaced: ", " was on main; the owner changed it, because it was slower."),
        ]
        for line in lines:
            with self.subTest(line=line[:12]):
                self.write("decisions/extra.md", entries(decision("spoken-log", under=[line])))
                self.assertFaults(("decisions/extra.md", 7))


class NoWorkItemOrDate(TreeTest):
    """A decision names no work, item or date: the file's history keeps those."""

    def test_a_decision_naming_a_work_an_item_or_a_date_is_a_fault(self):
        cases = [
            ("a work", [REJECTED.replace("could not", "in W10 could not")], 7),
            ("an item", [REJECTED.replace("could not", "of #367 could not")], 7),
            ("a date", [REJECTED.replace("could not", "could not, on 2026-11-08,")], 7),
            ("a date in a replaced line", [REPLACED.replace("were read", "were read on 2026-11-08")], 7),
            ("a work in the second line under", [REJECTED, REPLACED.replace("one work", "W10")], 8),
            ("a work by its word", [REJECTED.replace("could not", "in work 11 could not")], 7),
            ("an item by its word", [REJECTED.replace("could not", "of item 5 could not")], 7),
        ]
        for case, under, line in cases:
            with self.subTest(case=case):
                self.write("decisions/extra.md", entries(decision("spoken-log", under=under)))
                self.assertFaults(("decisions/extra.md", line))

    def test_a_first_line_naming_a_date_is_a_fault(self):
        first = "The engine keeps one log for each work since 2026-11-08."
        self.write("decisions/extra.md", entries(decision("spoken-log", first=first)))
        self.assertFaults(("decisions/extra.md", 6))

    def test_other_numbers_name_no_work_item_or_date(self):
        line = (
            "- rejected: an agent proposed one log for all works; the owner rejected it, because Article 3 asks for it,"
            " a round cost 300 tokens, and the page's grey is #333333."
        )
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[line])))
        self.assertFaults()


class NamedEntries(TreeTest):
    """An entry is named as "principle <name>" or "decision <name>", and the entry named is in the tree, of that
    kind."""

    def test_entries_named_from_other_files_hold(self):
        first = "The engine keeps one log for each work, as principle one-thing-one-place asks."
        line = REJECTED.replace("alone.", "alone, after decision one-round-at-a-time.")
        self.write("decisions/extra.md", entries(decision("spoken-log", first=first, under=[line])))
        self.assertFaults()

    def test_a_named_entry_no_file_holds_is_a_fault(self):
        line = REJECTED.replace("one log", "one log, against principle no-such-rule,")
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[line])))
        self.assertFaults(("decisions/extra.md", 7))

    def test_a_reason_of_a_principle_naming_an_entry_no_file_holds_is_a_fault(self):
        why = "Why: A part tested only with the others hides the fault of each, as decision no-such-choice found."
        self.write("principles/extra.md", entries(principle("spoken-words", why=why)))
        self.assertFaults(("principles/extra.md", 7))

    def test_a_principle_named_as_a_decision_is_a_fault(self):
        line = REJECTED.replace("one log", "one log, against decision state-from-records,")
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[line])))
        self.assertFaults(("decisions/extra.md", 7))

    def test_a_decision_named_as_a_principle_is_a_fault(self):
        first = "The engine keeps one log for each work, as principle one-round-at-a-time asks."
        self.write("decisions/extra.md", entries(decision("spoken-log", first=first)))
        self.assertFaults(("decisions/extra.md", 6))


class Data(TreeTest):
    """A reason that rests on data ends with the repository file that holds it, as "(data: `<path>`)"."""

    def line(self, path):
        return REJECTED.replace("alone.", f"alone (data: `{path}`).")

    def test_data_in_the_repository_holds(self):
        self.put("measurements/rounds.csv", "work,rounds\n1,4\n")
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[self.line("measurements/rounds.csv")])))
        self.assertFaults()

    def test_data_not_in_the_repository_is_a_fault(self):
        self.write("decisions/extra.md", entries(decision("spoken-log", under=[self.line("measurements/rounds.csv")])))
        self.assertFaults(("decisions/extra.md", 7))

    def test_the_path_of_data_is_taken_from_the_repositorys_root(self):
        self.put("design/rounds.csv", "work,rounds\n1,4\n")
        with self.subTest(path="rounds.csv"):
            self.write("decisions/extra.md", entries(decision("spoken-log", under=[self.line("rounds.csv")])))
            self.assertFaults(("decisions/extra.md", 7))
        with self.subTest(path="design/rounds.csv"):
            self.write("decisions/extra.md", entries(decision("spoken-log", under=[self.line("design/rounds.csv")])))
            self.assertFaults()


if __name__ == "__main__":
    unittest.main()
