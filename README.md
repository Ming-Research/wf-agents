# wf-agents

wf-agents runs coding agents through a defined flow. Each piece of work goes through research, design,
implementation, testing, review and merge, and the flow fixes what every step is given, what it hands back and where
the work goes next. wf-agents belongs to no project: everything about a project lives in that project's
configuration, outside this repository. [CONSTITUTION.md](CONSTITUTION.md) holds the values its decisions come from.

## Layout

- `crates/wf-agents`: the `wf-agents` command.
- `crates/flow`: the flow's compiler; `wf-agents flow check` runs it.
- `crates/db`: creating, migrating and opening the database; every change to its state is kept in `changes`.
- `crates/config`: the configuration's types, read from the data directory (`--data`), which lives outside this
  repository.
- `crates/sessions`: the session manager: the session each agent's round runs in, what the session holds and costs,
  the command line it runs in, keeping a waiting session's prompt cache warm, and telling a round that missed it.
- `crates/api`: the owner's interface apart from serving it: its types (the page's TypeScript types are generated from
  them), the reads that fill them, the bus its live events go through, and what the owner asks of the running engine.
- `crates/web`: the interface as `wf-agents run` serves it on the loopback address: HTTP and JSON under `/api/v1`, the
  event stream, and the page built into the binary.
- `client/`: the owner's page, in TypeScript with React and Vite; `client/src/api.ts` is generated, not written.
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `design/`: the design tree, the key decisions this project rests on, written as the flow's design tree rules say.
- `.github/workflows/ci.yml`: the checks every pushed commit runs.

## Checks

Every commit pushed to any branch runs `ci`: rustfmt, clippy with warnings as errors, and the tests, on the toolchain
`rust-toolchain.toml` pins; the page's type check, lint, check against its UI spec (`npm run check:ui`), tests and
build, which come before any cargo step that compiles `crates/web`; and that `client/src/api.ts` is what the Rust types generate
(`WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` writes it, and with it `client/src/stored-flow.json`, the
flow as the engine stores it, which the page's tests read and git leaves out).

## Installing

The program is installed on its own, never in the data directory: build the page (`npm ci` and `npm run build` in
`client/`), then `cargo build --release`, and copy `target/release/wf-agents` to a directory on the `PATH`, such as
`~/.local/bin`; the `build` workflow's artifact is the same binary for macOS. The data directory, named by `--data` or
`WF_AGENTS_DATA`, holds the configuration, the database, the files the database refers to, and the works' worktrees
under `worktrees/`, which its git leaves out. Once the configuration is committed, `wf-agents init` makes the
database; it is the only command that does, and every command that works on the database, the engine included, fails
when there is none, so a wrong `--data` is an error and not an empty view.

## The agents' own configuration

The engine runs every agent's command line with a configuration directory of its own (`CLAUDE_CONFIG_DIR` for
`claude`), so the owner's own settings, memory, plugins and tool servers never reach an agent. It is `agents/<cli>` in
the data directory unless the CLI's `config_dir` in `install.toml` names another, and the engine keeps it out of git.
Log in to the command line there once: `CLAUDE_CONFIG_DIR=<data>/agents/claude claude`, then `/login`.

## The architect

Each project has an architect of its own. It runs at the level `install.toml` names under `[agents]` as `architect`
(such as `architect = "4"`), the top level when it names none; its search of observations runs at the level named as
`observer`, `1` when none is named. Each project's file names its constitution, a path in the project's checkout, such
as `constitution = "CONSTITUTION.md"`, which its architect checks the project against article by article (the
level-two headings that start with "Article"), and the tags its observations may carry, each with what it covers:
`observation_tags = [{ name = "ui", desc = "What the owner sees on the page" }]`.

Every agent hands in flow feedback, what was wrong with its round's cell, which goes to its project's architect and
never to the owner; the architect of the project that is wf-agents itself, the one whose repository wf-agents is built
from, takes in what of it belongs to wf-agents. A project's file turns it off with `flow_feedback = false`, and its
agents are then not asked for it.

## The owner's page

`wf-agents run` serves the owner's page at `http://127.0.0.1:7480/` (`--port` takes another). The engine starts with
every work paused, so nothing runs until the owner presses Start all in the page's bar, which is also how the page is
looked at against a copy of the data; Pause all stops every work at the end of the round it is in, and beside it a
switch keeps waiting sessions warm meanwhile. Each work has its own Start, Pause and Step in its row and its window,
which win over the global state when they come later, Start all aside, which lifts no work's own pause; Step runs one
round of a work, after showing what it will be, and leaves it paused; Start and Step resume the work's session or
start a new one, as the owner chooses. `npm run dev` in `client/` serves the page from its sources and passes the
interface's requests to a running `wf-agents run`.

The flow page shows a stored flow version, or `flow/flow.json` as it is at a git ref of a project's checkout (read
with git and compiled, nothing written), its graphs side by side, and compares two of them, so that a change to the
flow is reviewed there.

The Architect view shows a project's architect, with a switch to another project's when there are several: its wakes
on its graph, its asks of the owner with their forms, its queue with what became of each matter it handled, the item changes its batches made, the
project's observations (by tag, searchable, each closed by the owner with why), the policy, and where things stand as
it understands them, each a tab of the column at its right, the first being the owner's conversation with the
architect: the owner's words, what the architect answered and asked, and a box that says more; its line holds the
architect's Start, Pause and Step.
