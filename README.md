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

## The owner's page

`wf-agents run` serves the owner's page at `http://127.0.0.1:7480/` (`--port` takes another). `--mode` sets how the
engine starts dispatching, and the page switches it while `run` runs: `run` takes every work as far as it can, `step`
takes one round each time the owner asks for one on the page, and `hold` starts nothing, which is how the page is
looked at against a copy of the data. `npm run dev` in `client/` serves the page from its sources and passes the
interface's requests to a running `wf-agents run`.
