# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given. The layout is in
[README.md](README.md).

- [constitution.md](constitution.md) decides between designs; read it before designing.
- Everything in the repository is written in English.
- The code is Rust, edition 2024, on the toolchain `rust-toolchain.toml` pins. It is formatted by rustfmt
  (`rustfmt.toml`), passes clippy with warnings as errors, and uses no `unsafe`; the root `Cargo.toml` sets these
  for every crate.
- Before you call `ci` on a change to Rust files, run `rustfmt` on each Rust file you changed or added, naming each file
  (for example `rustfmt crates/engine/src/inputs.rs`), and commit what it rewrites: CI's lint checks the format first,
  and a push that fails it costs a whole CI call. `rustfmt --check <files>` shows the differences without rewriting
  them. Use `rustfmt` itself, since `cargo fmt` is refused on the engineer's command line. `rustfmt` reads
  `rustfmt.toml`, and rustup runs the toolchain that `rust-toolchain.toml` pins, so the `rustfmt` it provides formats
  as CI does. Do not pass a glob such as `crates/**/*.rs`, so that no file is left out. Where the project lists a push
  check (`ci.push_checks`, such as `cargo fmt --all --check`), the engine runs it before `ci` pushes and pushes nothing
  when it does not pass; that costs a round, which running `rustfmt` first saves.
- A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- `db/schema.sql` is the only definition of the database: change the database by changing it, with the migration in
  the same pull request, a file of `db/migrations/` named by the minute it was written and what it does.
- A change to the owner's page follows its UI spec, [client/ui-spec.md](client/ui-spec.md), and passes its check,
  `npm run check:ui` in `client/`.
- `flow/flow.json` is the only definition of the flow. A change to it, to the views it reads or to the compiler
  passes `wf-agents flow check`; the flow crate's tests run the same check. Such a change merges once the owner
  approved it at the merge (M1), with a link to the flow page's comparison; it is no reason by itself to go to deep
  research. The pull request says which works going on the change sends back, and to which cell: the comparison at the branch lists them (Works
  going on), as `wf-agents flow cost <the branch's flow/flow.json> --data <dir>` does. Where the change can leave
  earlier returns passing (a new field an earlier return may lack, optional), it is made so, since every tightening of an
  early cell's output sends each work that went past it back to do the cell again.
- A test holds no total of a whole graph of the flow (its cells, rows, states, places or samples): every work that
  changes the flow would rewrite it, and works that do so side by side cannot be merged, in git or in meaning. What
  the flow must keep is held by the flow's own source (`each_graph_compiles_to_the_cells_of_its_nodes`) or by a named
  check; what the compiler's walk must keep is held by the small graph whose states are counted by hand, in
  `crates/flow/src/walk.rs`. A test that must keep a snapshot for each cell keeps one file for each cell
  (`crates/flow/tests/moves/`), never a line of one file: git's merge takes changes to neighbouring lines for a conflict.
- Two works that add at one place conflict at the merge: add a test to the module of what it tests or a new one (as in
  `crates/engine/tests/e2e/`), never at the end of a file or a list that every work appends to.
- A work that changes one of the engine's checks of returns (`engine.checks` in the flow) is judged until it is
  installed by the check as it was, and would be sent back by the fault it mends: it names each check it changes in
  its implementation's return (`checks_changed`), or a fix's when the fix changes one more. What only those checks
  find in its returns is then set aside, not sent back ([`crates/engine/src/set_aside.rs`](crates/engine/src/set_aside.rs)),
  and the review checks the declaration against the diff and judges each finding set aside.
- A check of an agent's return reports every problem it can find in that return at once, because each send-back costs
  a round. It collects the problems in `Problems` (`crates/engine/src/checks.rs`, used by `architect::check` too), and
  returns early with `fail` only when a later test presupposes that an earlier one holds. Several ids compared with a
  list the engine gave go through `id_misses`. The one exception is `cards_match_records` with `cards::check`: no
  cell of the flow calls it, so it sends nothing back, and it is removed with `cards.rs` in #203.
