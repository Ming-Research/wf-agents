# wf-agents

wf-agents runs coding agents through a defined flow. Each piece of work goes through research, design,
implementation, testing, review and merge, and the flow fixes what every step is given, what it hands back and where
the work goes next. wf-agents belongs to no project: everything about a project lives in that project's
configuration, outside this repository. [CONSTITUTION.md](CONSTITUTION.md) holds the values its decisions come from.

## Layout

- `crates/wf-agents`: the `wf-agents` command: starts the engine, and gives the owner's commands.
- `crates/engine`: takes each work through the flow, running the agents' rounds, the tools and the actions;
  `wf-agents run` starts it. A work is one run of the flow on one item (a piece of work to do), and a round is one
  run of an agent.
- `crates/flow`: the flow's compiler; `wf-agents flow check` runs it.
- `crates/db`: creating, migrating and opening the database; every change to its state is kept in `changes`.
- `crates/config`: the configuration's types, read from the data directory (`--data`), which lives outside this
  repository.
- `crates/api`: the owner's interface apart from serving it: its types (the page's TypeScript types are generated from
  them), the reads that fill them, the bus its live events go through, and what the owner asks of the running engine.
- `crates/web`: the interface as `wf-agents run` serves it on the loopback address: HTTP and JSON under `/api/v1`, the
  event stream, and the page built into the binary.
- `client/`: the owner's page, in TypeScript with React and Vite; `client/src/api.ts` is generated, not written.
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `design/`: the design tree, the key decisions this project rests on, written as the flow's design tree rules say.
- `.github/workflows/ci.yml`: the checks every pushed commit runs.
- `.github/workflows/build.yml`: builds the command for the Mac the engine runs on.

## Installing

The program is installed on its own, never in the data directory: build the page (`npm ci` and `npm run build` in
`client/`), then `cargo build --release --locked -p wf-agents` on the toolchain `rust-toolchain.toml` pins, and copy
`target/release/wf-agents` to a directory on the `PATH`, such as `~/.local/bin`; the `build` workflow's artifact is the
same binary for macOS. SQLite is built into the binary.

## Running

### What it needs

- `git`, and `gh` logged in: the engine opens pull requests, waits for CI and merges through them.
- The agent CLI that `install.toml` names under `agents.clis`; the only kind there is now is `claude`.
- A clone of each project at the `checkout` its project file gives; each work's worktree is made from it.
- A data directory that exists and is a git repository with at least one commit; it holds the configuration (see
  below). `wf-agents run`, `wf-agents item add`, `wf-agents anomalies` and `wf-agents message` read the configuration
  from that commit and fail in a repository that has none; `wf-agents config check` reads the files and does not.

### The data directory

Every command except `wf-agents flow check` and `wf-agents tree-check` works on a data directory, given as `--data DIR`
or in the environment variable `WF_AGENTS_DATA`, and it has to be one that exists. It holds the owner's configuration
and what the command and the engine write beside it. It is a git repository, and the engine reads the configuration
from its last commit, so a change counts once it is committed; `wf-agents config check` reads the files as they are.

The owner keeps and commits:

- `install.toml`: the owner, the machines and their slots, the agent CLIs and levels, the gh allow list and the limits
  of a query.
- `shared/`: files that project files start from, named in a project file's `extends`.
- `projects/`: one file for each project.
- `rules/`: the rules agents are given.

Their fields are the types in `crates/config`. The rest is not configuration and is not meant to be committed. Nothing
writes the data directory's own `.gitignore`, so list in it `db/`, `logs/`, `tool-output/`, `notifications.log`,
`worktrees/` and `stop`; `agents/` keeps itself out of git. The command and the engine write all of it but `stop`:

- `agents/`: the default home of each agent CLI's own configuration, `agents/<the CLI's name>`, which the CLI's
  `config_dir` setting in `install.toml` can move elsewhere; the engine keeps it out of git with a `.gitignore` holding
  `*`.
- `db/`: the SQLite database, `wf-agents.sqlite`, with its `-wal` and `-shm` files while it is open.
- `logs/`: the logs of CI checks that were red.
- `tool-output/`: tool results too long to give an agent whole.
- `notifications.log`: what the owner is told, one line each, with the time in milliseconds first.
- `worktrees/`: each work's worktree, `<project>-<work number>`, and the temporary worktree, `revert-<commit>`, that
  reverts a merge.
- `stop`: a file the owner creates to stop `wf-agents run`; the command removes it when it starts and when it has
  stopped.

### Using it, in order

The owner works from the page (see below), where an ask is answered, a design card is ruled on and a paused work is let
go on. The commands do the same, and are the only way to add an item, order the queue, say something to a work or pause
one. Each command's arguments are in `wf-agents --help` and `wf-agents <command> --help`; this is the order they are
used in.

1. Write the configuration, check it with `wf-agents config check --data DIR` before committing it, and commit it. The
   engine reads the committed configuration again on every pass and keeps the last good one if a new one fails its
   check, so a change needs no restart.
2. Create items with `wf-agents item add --project P --title T --problem ... --why ... --level L --queue`, and look at
   them with `wf-agents item list`. `--level` names a level in `install.toml`, which decides the CLI, model and effort
   the item's agents run with. `wf-agents item depend` makes one item depend on another, saying with `--needs` what it
   needs of it, and `wf-agents item cancel` cancels one and ends its work. An item starts only when it is in the
   queue: `--queue` puts it last when it is created, and `wf-agents queue put N` puts any item in the queue, last or
   with `--first`, `--before M` or `--after M`, or moves one that is there. `wf-agents queue list` shows the items
   still waiting to start.
3. Start the engine with `wf-agents run`, one engine for each data directory. It starts works on the queue's items
   while a slot is free (`slots` of each host in `install.toml`), takes every work through the flow and serves the
   owner's page; `wf-agents run --once` takes one pass without the page and exits. To stop the engine, create the file
   `stop` in the data directory: the engine looks for it between two passes, ends when it finds it, removes it and says
   so on its standard error; a `stop` left from before is removed when the engine starts. Ending the process works
   too: the round that was running is recorded as cut off, and its work goes on from the same cell when the engine
   starts.
4. Answer asks and rule on design cards. An ask is a question for the owner that holds a work until it is answered; a
   design card is a change to the design tree that the owner confirms before it is merged, and asks nothing. The engine
   tells the owner of a new one on the page, in `notifications.log` and on its standard error. `wf-agents asks` lists
   the asks waiting (`--all` adds the answered ones) and `wf-agents ask N` shows one in full. Answer one with
   `wf-agents answer N --text ...`; an ask is answered once. In this version the owner answers for the architect too:
   its questions are asks as well, answered with `wf-agents answer N --text ... --why ...`, and `--reply` gives the
   rest of the reply as JSON, in the shape of `architect_reply` in `flow/flow.json`. `wf-agents design-cards` lists the
   design cards waiting for a ruling and those ruled but not yet confirmed for the merge, and `wf-agents design-card N`
   shows one in full with its rulings. Rule on one with `wf-agents rule N --verdict confirm --text ...` or
   `wf-agents rule N --verdict disagree --text ...`; a disagreement reaches the engineer as a message, and a design
   card can be ruled again until it is replaced.
5. Follow the works. `wf-agents status` shows where each stands, which are paused and what waits for the owner; the
   engine pauses a work itself when it cannot take a step or when a cell's returns are sent back too many times in a
   row, and tells the owner. `wf-agents history WORK` lists a work's rounds (`--full` gives each in full),
   `wf-agents counters WORK` shows how often it took the rows that count each counter, `wf-agents anomalies` gathers
   what went wrong or ran long, `wf-agents cost` gives the time and cost of each work (given a work, of its rounds by
   cell), and `wf-agents message WORK` prints what the work's next agent round would be sent, built as the engine
   builds it, with nothing run or recorded.
6. Step in when needed. `wf-agents say WORK TEXT` says something to a running work, which gets it in its next round;
   `wf-agents pause WORK --why ...` stops taking a work further, and `wf-agents resume WORK` lets it go on, running its
   cell again.

## The agents' own configuration

The engine runs every agent's command line with a configuration directory of its own (`CLAUDE_CONFIG_DIR` for
`claude`), so the owner's own settings, memory, plugins and tool servers never reach an agent; `agents/` above says
where it is. Log in to the command line there once: `CLAUDE_CONFIG_DIR=<data>/agents/claude claude`, then `/login`.

## The owner's page

`wf-agents run` serves the owner's page at `http://127.0.0.1:7480/` (`--port` takes another). `--mode` sets how the
engine starts dispatching, and the engine's state in the page's bar switches it while `run` runs: `run` takes every
work as far as it can, and `hold` starts nothing by itself, which is how the page is looked at against a copy of the
data (`wf-agents run --mode hold`). While it holds, a work's page runs that work's next round when the owner asks,
after showing what it will be. `npm run dev` in `client/` serves the page from its sources and passes the interface's
requests to a running `wf-agents run`.

The page is where the owner answers an ask, rules on a design card, lets a paused work go on and reads what the engine
and the agents tell the owner. The flow page shows a stored flow version, or `flow/flow.json` as it is at a git ref of
a project's checkout (read with git and compiled, nothing written), and compares two of them, so that a change to the
flow is reviewed there.

## Checks

Every commit pushed to any branch runs `ci`: rustfmt, clippy with warnings as errors, and the tests, on the toolchain
`rust-toolchain.toml` pins; the page's type check, lint, tests and build, which come before any cargo step that
compiles `crates/web`; and that `client/src/api.ts` is what the Rust types generate
(`WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` writes it).

The tests also check this README against the command: every command it has is written here as a command line, and
every command line written here names only commands and options that exist. `wf-agents flow check` runs the flow's
compiler on `flow/flow.json` and prints the digest of the compiled flow. `wf-agents tree-check FORK` is what a project
names as its `design_check`: it counts the design tree's nodes, depth and decisions at `HEAD`, and how many more than at
the fork point `FORK`.
