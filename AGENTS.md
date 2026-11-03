# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given. The layout is in
[README.md](README.md).

- [constitution.md](constitution.md) decides between designs; read it before designing.
- Everything in the repository is written in English.
- The code is Rust, edition 2024, on the toolchain `rust-toolchain.toml` pins. It is formatted by rustfmt
  (`rustfmt.toml`), passes clippy with warnings as errors, and uses no `unsafe`; the root `Cargo.toml` sets these
  for every crate.
- A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- `db/schema.sql` is the only definition of the database: change the database by changing it, with the migration in
  the same pull request.
- A change to the owner's page follows its UI spec, [client/ui-spec.md](client/ui-spec.md), and passes its check,
  `npm run check:ui` in `client/`.
- `flow/flow.json` is the only definition of the flow. A change to it, to the views it reads or to the compiler
  passes `wf-agents flow check`; the flow crate's tests run the same check. Such a change is an open question of its
  design, put to the owner with a link to the flow page's comparison, and the merge waits for the answer.
- A check of an agent's return reports every problem it can find in that return at once, because each send-back costs
  a round. It collects the problems in `Problems` (`crates/engine/src/checks.rs`, used by `architect::check` too), and
  returns early with `fail` only when a later test presupposes that an earlier one holds. Several ids compared with a
  list the engine gave go through `id_misses`. The one exception is `cards::check`, which still stops at the first
  problem; it goes with `cards.rs` in #203.
