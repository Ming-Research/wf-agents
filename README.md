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
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `.github/workflows/ci.yml`: the checks every pushed commit runs.

## Checks

Every commit pushed to any branch runs `ci`: rustfmt, clippy with warnings as errors, and the tests, on the toolchain
`rust-toolchain.toml` pins.
