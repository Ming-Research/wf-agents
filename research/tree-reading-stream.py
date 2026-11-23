"""Measures how much of the design tree one round read, for research/tree-reading-measured.md.

Run as `python3 -I research/tree-reading-stream.py [--files] <rounds/W/RUN-ROUND.stream.jsonl>...`. It reads the
files only and prints, for each round's stream:

- its tool calls (each `tool_use` block once, by its id);
- the calls whose input names a path under `design/principles` or `design/decisions` (a Read of a file of the tree, or
  a Grep, Glob or Bash on it), and the bytes of their results;
- the files of the tree the round read with the Read tool, and the bytes of all its tool results.

With `--files` it also prints the files read, one line after each round.

A script and not `jq` and the shell, because it matches each result to its call by `tool_use_id` and sums the results
of the calls on the tree only. It serves whoever measures the tree's reading again, as the report says when; it is kept
with the report and goes when the report does.
"""
import json
import re
import sys

TREE = re.compile(r"design/(principles|decisions)")


def size(content):
    if isinstance(content, str):
        return len(content)
    if isinstance(content, list):
        return sum(len(x.get("text", "")) if isinstance(x, dict) else size(x) for x in content)
    return len(json.dumps(content))


def scan(path):
    uses, results = {}, {}
    for line in open(path):
        if not line.strip():
            continue
        event = json.loads(line)
        message = event.get("message")
        if not isinstance(message, dict) or not isinstance(message.get("content"), list):
            continue
        for block in message["content"]:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_use":
                uses[block["id"]] = (block["name"], block.get("input", {}))
            if block.get("type") == "tool_result":
                results[block["tool_use_id"]] = size(block.get("content"))
    files, on_tree, tree_bytes, all_bytes = set(), 0, 0, 0
    for id, (name, given) in uses.items():
        got = results.get(id, 0)
        all_bytes += got
        if TREE.search(json.dumps(given)):
            on_tree += 1
            tree_bytes += got
            if name == "Read":
                files.add(re.sub(r".*/design/", "", given.get("file_path", "")))
    return len(uses), on_tree, files, tree_bytes, all_bytes


def main(args):
    listed = "--files" in args
    for path in [a for a in args if a != "--files"]:
        calls, on_tree, files, tree_bytes, all_bytes = scan(path)
        print(f"{path}: {calls} tool calls, {on_tree} on the tree, {len(files)} files of it read, "
              f"{tree_bytes / 1000:.1f} KB of the tree's results of {all_bytes / 1000:.1f} KB")
        if listed:
            print("  " + " ".join(sorted(files)))


if __name__ == "__main__":
    main(sys.argv[1:])
