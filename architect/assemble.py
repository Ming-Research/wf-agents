#!/usr/bin/env python3
"""Puts together `AGENTS.md` beside this script, which an architect session started in this directory reads: the
architect's rules and what it reports, the installation's settings an architect works by, the project's constitution and the rules of its
design tree, the rules every agent of the engine works by, and the help of the engine's commands.

The architect's own files are read as they stand beside this script. What the engine gives its agents is read as the
project's main branch holds it, the branch the engine takes its flows from, so that the architect is given what the
engine's agents are given whatever this checkout is on. The settings are read from the engine serving on the loopback
address, which must be running. Run it from any directory, again whenever one of these changes."""

import argparse
import json
import shutil
import subprocess
import sys
import tomllib
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "AGENTS.md"
# The architect's own files, in the order they are given.
OWN = [HERE / "rules.md", HERE / "notes.md"]
# The engine's commands an architect has no use for: the engine's own start, the flow's tooling and the help itself.
NOT_THE_ARCHITECTS = {"run", "flow", "help"}
# The settings an architect works by, as prefixes of their names; a project's are added as its own.
SETTINGS = ["owner.", "agents.slots", "agents.work_budget_usd", "agents.levels.", "items."]
PROJECT_SETTINGS = ["main", "github.repository"]


def fail(why: str) -> None:
    sys.exit(f"assemble.py: {why}")


def run(command: list[str], cwd: Path | None = None) -> str:
    done = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    if done.returncode != 0:
        fail(f"`{' '.join(command)}` failed: {done.stderr.strip()}")
    return done.stdout


def settings(port: int) -> dict:
    url = f"http://127.0.0.1:{port}/api/v1/settings"
    try:
        with urllib.request.urlopen(url, timeout=10) as answer:
            page = json.load(answer)
    except OSError as error:
        fail(f"the engine's settings could not be read from {url} ({error}): is the engine running?")
    if page.get("unreadable"):
        fail(f"the engine cannot read its settings file: {page['unreadable']}")
    values = {}
    for table in page["tables"]:
        for key in table["keys"]:
            written = key["written"] if key["written"] is not None else key["running"]
            if written is not None:
                values[f"{table['name']}.{key['key']}"] = tomllib.loads(f"v = {written}")["v"]
    return values


def project_of(values: dict, root: Path) -> str:
    for name, repo in values.items():
        if name.startswith("projects.") and name.endswith(".repo") and Path(repo).resolve() == root:
            return name[len("projects.") : -len(".repo")]
    fail(f"no project of the installation has its repository at {root}")


def on_main(root: Path, ref: str, path: str) -> str:
    return run(["git", "-C", str(root), "show", f"{ref}:{path}"])


def toml_value(value) -> str:
    return json.dumps(value, ensure_ascii=False)


def named(text: str, names: list[str], values: dict) -> str:
    # Each setting the text names between backticks has its value beside its name, as the engine gives it.
    for name in names:
        if name in values:
            text = text.replace(f"`{name}`", f"`{name}` ({values[name]})")
    return text


def installation(values: dict, project: str) -> str:
    keys = [name for name in values if any(name.startswith(prefix) for prefix in SETTINGS)]
    keys += [f"projects.{project}.{key}" for key in PROJECT_SETTINGS if f"projects.{project}.{key}" in values]
    lines = [f"{name} = {toml_value(values[name])}" for name in keys]
    return "# The installation's settings\n\n```toml\n" + "\n".join(lines) + "\n```"


def commands(program: str) -> str:
    top = run([program, "help"])
    names, listing = [], False
    for line in top.splitlines():
        if line.startswith("Commands:"):
            listing = True
        elif listing and not line.strip():
            break
        elif listing and line.startswith("  ") and not line.startswith("   "):
            names.append(line.split()[0])
    parts = ["# The commands of `wf-agents`", f"```text\n{top.strip()}\n```"]
    for name in names:
        if name not in NOT_THE_ARCHITECTS:
            parts.append(f"## `wf-agents {name}`\n\n```text\n{run([program, name, '--help']).strip()}\n```")
    return "\n\n".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--port", type=int, default=7470, help="the port the engine serves on (default 7470)")
    port = parser.parse_args().port

    root = Path(run(["git", "-C", str(HERE), "rev-parse", "--show-toplevel"]).strip()).resolve()
    values = settings(port)
    project = project_of(values, root)
    remote = values.get(f"projects.{project}.github.remote", "origin")
    main_branch = values[f"projects.{project}.main"]
    run(["git", "-C", str(root), "fetch", "-q", remote, main_branch])
    ref = f"{remote}/{main_branch}"

    declaration = tomllib.loads(on_main(root, ref, "flow/project.toml"))
    session = tomllib.loads(on_main(root, ref, "flow/agents/session.toml"))
    rule_sets = tomllib.loads(on_main(root, ref, "flow/agents/rules/rules.toml"))
    names = tomllib.loads(on_main(root, ref, "flow/agents/installation.toml"))["names"]
    heading = session["heading"]

    def part(path: str, text: str) -> str:
        return heading.format(path=path, text=text.strip())

    program = shutil.which("wf-agents") or str(Path.home() / ".local" / "bin" / "wf-agents")
    general = f"flow/agents/rules/{rule_sets['general']['file']}"
    parts = [
        *(part(str(own.relative_to(root)), named(own.read_text(), names, values)) for own in OWN),
        installation(values, project),
        *(
            part(declaration["files"][kind]["path"], on_main(root, ref, declaration["files"][kind]["path"]))
            for kind in ("constitution", "tree_rules")
        ),
        part(general, named(on_main(root, ref, general), names, values)),
        commands(program),
    ]
    head = f"<!-- Made by assemble.py from {ref} and the engine's settings: run it again rather than edit this file. -->"
    OUT.write_text(head + "\n\n" + "\n\n".join(parts) + "\n")
    print(f"wrote {OUT} ({len(OUT.read_text())} characters) from {ref}")


if __name__ == "__main__":
    main()
