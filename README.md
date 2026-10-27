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
`rust-toolchain.toml` pins; the page's type check, lint, tests and build, which come before any cargo step that
compiles `crates/web`; and that `client/src/api.ts` is what the Rust types generate
(`WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` writes it).

## Installing

The program is installed on its own, never in the data directory: build the page (`npm ci` and `npm run build` in
`client/`), then `cargo build --release`, and copy `target/release/wf-agents` to a directory on the `PATH`, such as
`~/.local/bin`; the `build` workflow's artifact is the same binary for macOS. The data directory, named by `--data` or
`WF_AGENTS_DATA`, holds the configuration, the database, the files the database refers to, and the works' worktrees
under `worktrees/`, which its git leaves out.

## The agents' own configuration

The engine runs every agent's command line with a configuration directory of its own (`CLAUDE_CONFIG_DIR` for
`claude`), so the owner's own settings, memory, plugins and tool servers never reach an agent. It is `agents/<cli>` in
the data directory unless the CLI's `config_dir` in `install.toml` names another, and the engine keeps it out of git.
Log in to the command line there once: `CLAUDE_CONFIG_DIR=<data>/agents/claude claude`, then `/login`.

## The owner's page

`wf-agents run` serves the owner's page at `http://127.0.0.1:7480/` (`--port` takes another). `--mode` sets how the
engine starts dispatching, and the engine's state in the page's bar switches it while `run` runs: `run` takes every
work as far as it can, and `hold` starts nothing by itself, which is how the page is looked at against a copy of the
data. While it holds, a work's page runs that work's next round when the owner asks, after showing what it will
be. `npm run dev` in `client/` serves the page from its sources and passes the
interface's requests to a running `wf-agents run`.

The flow page shows a stored flow version, or `flow/flow.json` as it is at a git ref of a project's checkout (read
with git and compiled, nothing written), and compares two of them, so that a change to the flow is reviewed there.
