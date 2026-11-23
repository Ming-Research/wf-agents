#!/usr/bin/env python3
"""Tests of `check_citations.py`. Each test builds a small tree in a temporary directory, runs the script on it as CI
runs it on the repository, and checks the faults it reports, each by its file and line. Run it with
`python3 docs/check_citations_test.py` from any directory.
"""

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CHECK = Path(__file__).resolve().parent / "check_citations.py"
FAULT = re.compile(r"^(\S+):(\d+): (.+)$")

# The documents every tree has: `flow-notes` with paragraphs 1 to 3 and `page` with paragraphs 1 and 2.
DOCS = {
    "docs/flow-notes.md": "# Flow notes\n\n1. First.\n\n2. Second,\n   on two lines.\n\n3. Third.\n",
    "docs/page.md": "# Page\n\n1. One.\n\n2. Two.\n",
}


def run(files):
    """The faults the check reports on a tree of DOCS and `files`, as (file, line, text), and its exit code."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for name, text in {**DOCS, **files}.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        done = subprocess.run([sys.executable, "-I", str(CHECK), str(root)], capture_output=True, text=True)
    found = [FAULT.match(line).groups() for line in done.stdout.splitlines() if FAULT.match(line)]
    return [(f, int(n), t) for f, n, t in found], done.returncode


class CheckCitations(unittest.TestCase):
    def test_a_citation_of_a_paragraph_that_exists_is_no_fault(self):
        found, code = run({"a.rs": "// What it does (design document `flow-notes`, 3).\n"})
        self.assertEqual((found, code), ([], 0))

    def test_a_citation_of_a_missing_paragraph_is_reported_at_its_line(self):
        found, code = run({"a.rs": "fn a() {}\n// What it does (design document `flow-notes`, 4).\n"})
        self.assertEqual(code, 1)
        self.assertEqual([(f, n) for f, n, _ in found], [("a.rs", 2)])
        self.assertIn("paragraph 4", found[0][2])

    def test_each_number_of_a_list_and_each_end_of_a_range_is_checked(self):
        found, _ = run({"a.ts": "// One (document `flow-notes`, 1, 2 and 9) and another (document `flow-notes`, 2 to 7).\n"})
        self.assertEqual(sorted(re.search(r"paragraph (\d+)", t).group(1) for _, _, t in found), ["7", "9"])

    def test_a_citation_broken_over_two_comment_lines_is_read_whole(self):
        found, _ = run({"a.rs": "/// A text (design document `flow-notes`, 1,\n/// 8) and more.\n"})
        self.assertEqual([(f, n) for f, n, _ in found], [("a.rs", 1)])

    def test_the_same_names_the_document_cited_last_before_it(self):
        found, _ = run({"a.md": "Built (document `page`, 1), and more (the same, 3).\n"})
        self.assertEqual(len(found), 1)
        self.assertIn("paragraph 3 of document `page`", found[0][2])

    def test_a_citation_of_a_document_that_does_not_exist_is_reported(self):
        found, code = run({"a.toml": "# See document `gone`, 1.\n"})
        self.assertEqual(code, 1)
        self.assertIn("no docs/gone.md", found[0][2])

    def test_a_design_statement_is_reported_even_where_the_line_breaks_inside_its_name(self):
        found, _ = run({"a.toml": "# What it is (design\n# statements 17 to 19).\n"})
        self.assertEqual([(f, n) for f, n, _ in found], [("a.toml", 1)])

    def test_a_file_of_another_kind_is_not_read(self):
        found, code = run({"b.txt": "# A fixture (design document `flow-notes`, 9)\n"})
        self.assertEqual((found, code), ([], 0))

    def test_a_citation_in_prose_is_read_in_a_markdown_file_and_a_paragraph_ends_at_a_blank_line(self):
        found, _ = run({"a.md": "A text (document `flow-notes`,\n\n9) and another (document `page`, 2).\n"})
        self.assertEqual(found, [])

    def test_a_plural_names_several_documents_and_each_is_checked_with_its_own_paragraphs(self):
        found, _ = run({"a.rs": "//! A text (design documents `flow-notes`, 3, and\n//! `page`, 2 to 5): more.\n"})
        self.assertEqual(sorted(re.search(r"paragraph (\d+) of document `(\S+)`", t).groups() for _, _, t in found), [("5", "page")])
        found, _ = run({"a.rs": "// Two (documents `flow-notes`, 9 and `page`, 1).\n"})
        self.assertEqual(len(found), 1)
        self.assertIn("paragraph 9 of document `flow-notes`", found[0][2])

    def test_a_comment_after_code_is_read_and_a_comment_mark_in_a_string_is_not(self):
        found, code = run(
            {
                "a.rs": 'let a = 1; // (design document `flow-notes`, 7)\nlet b = "// (document `page`, 9)"; // (document `page`, 2)\n',
                "b.toml": 'key = "x" # (design document `page`, 8)\nother = "# (document `page`, 9)"\n',
            }
        )
        self.assertEqual(code, 1)
        self.assertEqual(sorted((f, n) for f, n, _ in found), [("a.rs", 1), ("b.toml", 1)])

    def test_what_comes_before_a_line_cannot_put_it_out_of_step(self):
        # A regular expression with a quote, a text with an apostrophe in JSX and a raw byte string, each before a comment
        # on a line of its own that cites a paragraph the document does not have.
        found, code = run(
            {
                "a.ts": 'const a = /["`]+/g;\n// (document `page`, 7)\nconst b = /`/;\n// (document `page`, 8)\n',
                "b.tsx": "const c = <p>it's here</p>;\n// (document `page`, 9)\n",
                "c.rs": 'const D: &[u8] = br#"a " b"#;\n// (document `page`, 6)\nlet e = r#"x"#; // (document `page`, 5)\n',
            }
        )
        self.assertEqual(code, 1)
        self.assertEqual(
            sorted((f, n) for f, n, _ in found), [("a.ts", 2), ("a.ts", 4), ("b.tsx", 2), ("c.rs", 2), ("c.rs", 3)]
        )

    def test_a_mark_inside_a_string_of_one_line_is_not_a_comment_whatever_quotes_it_is_in(self):
        found, code = run(
            {
                "a.py": "A = '# (document `page`, 9)'\nB = \"# (document `page`, 9)\"\n",
                "b.ts": "const a = `x // (document `page`, 9)`;\nconst b = 'y // (document `page`, 9)';\n",
            }
        )
        self.assertEqual((found, code), ([], 0))

    def test_the_design_of_the_owners_page_is_a_citation_of_the_document_the_owners_page(self):
        found, _ = run(
            {
                "docs/the-owners-page.md": "# The page\n\n1. One.\n",
                "a.tsx": "// See the design of the owner's page, 1, and the design of the owner's page, 99.\n",
            }
        )
        self.assertEqual(len(found), 1)
        self.assertIn("paragraph 99 of document `the-owners-page`", found[0][2])

    def test_the_directories_of_dependencies_and_builds_are_not_read(self):
        found, code = run({"client/node_modules/x/a.ts": "// (document `flow-notes`, 9)\n", "target/a.rs": "// (document `gone`, 1)\n"})
        self.assertEqual((found, code), ([], 0))


if __name__ == "__main__":
    unittest.main()
