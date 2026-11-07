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
  as CI does. Do not pass a glob such as `crates/**/*.rs`, so that no file is left out.
- Builds and test suites run through the `ci` tool, not on the machine the agents run on: it makes no difference to the
  work, and locally it only fights the owner for the processor.
- A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- A change to the database changes `db/schema.sql` and adds the migration in the same pull request, a file of
  `db/migrations/` named by the minute it was written and what it does.
- A change to the owner's page passes its check, `npm run check:ui` in `client/`. [client/ui-spec.md](client/ui-spec.md)
  holds the page's details; where it differs from the design tree, the tree holds.
- The page's tests are three Vitest projects (`client/vite.config.ts`): `unit` (Node, or happy-dom for a file that says
  so), `ui` and `browser`. What depends on a real browser's layout, pointer, focus or keys (the graph's viewer in
  `client/src/graph/` above all) is tested in a file named `*.browser.test.tsx`, which draws the page in Chromium;
  the rest stays in `unit`, since the browser costs its own start. A work that changes such code names the browser tests
  in `ci` (`client/src/graph/Viewer.browser.test.tsx`), because the fast CI starts Chromium only for a run that names
  one; the full CI runs them once, at the merge, whenever the page's files changed.
- `flow/flow.json` is the only definition of the flow. A change to it, to the views it reads or to the compiler
  passes `wf-agents flow check`; the flow crate's tests run the same check. Such a change merges once the owner
  approved it at the merge (M1), with a link to the flow page's comparison; it is no reason by itself to go to deep
  research. The pull request says which works going on the change sends back, and to which cell: the comparison at the branch lists them (Works
  going on), as `wf-agents flow cost <the branch's flow/flow.json> --data <dir>` does. Where the change can leave
  earlier returns passing (a new field an earlier return may lack, optional), it is made so, since every tightening of an
  early cell's output sends each work that went past it back to do the cell again.
- A change to a cell's inputs or outputs, in the flow or in the code that fills them, waits for the owner's approval; the
  proposal gives each one's change in characters and why it is needed.
- Before changing a cell's prompt or the flow, replay that cell on a few different past cases and read what it returns;
  judge a written standard the same way. This finds the problem before the flow changes, and whoever writes a standard
  saying what they would do shows nothing of what the cell will do.
- Debugging the engine or trying a changed flow goes one round at a time: run a round, read its input and output, then
  the next. A hundred and one rounds in a row once missed a problem that was there in the first.
- A test holds no total of a whole graph of the flow (its cells, rows, states, places or samples): every work that
  changes the flow would rewrite it, and works that do so side by side cannot be merged, in git or in meaning. What
  the flow must keep is held by the flow's own source (`each_graph_compiles_to_the_cells_of_its_nodes`) or by a named
  check; what the compiler's walk must keep is held by the small graph whose states are counted by hand, in
  `crates/flow/src/walk.rs`. A test that must keep a snapshot for each cell keeps one file for each cell
  (`crates/flow/tests/moves/`), never a line of one file: git's merge takes changes to neighbouring lines for a conflict.
- Two works that add at one place conflict at the merge: add a test to the module of what it tests or a new one (as in
  `crates/engine/tests/e2e/`), never at the end of a file or a list that every work appends to.

## The design tree

`design/` holds wf-agents' design decisions by category: `x.md` holds x's decisions, `x/` its sub-decisions, level by
level. Before changing something, walk down to it from `design/wf-agents.md`, reading each file on the way.

The tree goes ahead of the code: where they differ, the code is behind. Fix it if it is at hand, or tell the architect;
neither holds up the work in hand, and a review holds no merge for an old gap. Code that looks deliberate goes to the
architect unchanged.

The tree changes only by entries the owner confirmed: the assistant writes those confirmed in the owner's chat, and an
engineer, in its work, those the owner answered yes to on its ask; what needs recording elsewhere comes back to that
chat. It records only what is implemented or going to be. For what is going to be, it first walks the tree for
contradictions; if that is too much work, it opens a deep research and writes nothing yet. Changing means rewriting and
not wanting means deleting, the commit saying why; the past is in git.

A decision is a short paragraph: what, then why, failed paths included. "The owner said" is no reason; with no reason it
does not enter, and none is invented. No code names, cell ids, progress, or words bound to when it was written. Longer
background stays in the item.
