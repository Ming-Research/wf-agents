# wf-agents

wf-agents runs coding agents through a defined flow.

- `design/`: the design tree. [design/README.md](design/README.md) says how it is read and changed; `constitution.md`
  holds what wf-agents is for and the values its decisions come from; `principles/` holds the principles each kind of
  decision follows; `decisions/` holds the decisions of each ability or component, by area; `pending.md` holds designs
  not yet true of the code, each entering the tree with the change that makes it true.
- `research/`: what a work's research made for its problem beside the tests: data, code, probes and notes, with what
  was measured and how to measure it again.
- `db/`: the engine's database schema (`schema.sql`) and its migrations; a change of the schema adds a migration to
  `migrations/`, named by the minute it was written and what it does.
- `crates/`: the engine's Rust crates. `crates/wf-agents` is the `wf-agents` command; `wf-agents flow page <dir>` serves
  the owner's flow page for the definition in a directory, and `wf-agents run` runs the engine with the owner's pages.
  Its agents run with the `claude` command line and its acts with git, as the installation's settings say
  (`<data>/install.toml`, of which `crates/carry/install.sample.toml` is a sample); given `--script`, they follow the
  script instead. To try it on the trial flow with a script, in `crates/service/tests/trial`:
  `wf-agents run --data <an empty directory> --flow flow --script script.toml`; then
  `wf-agents add --title "Greet the user" --flow trial --write "request=Greet the user."` and `wf-agents resume 1`,
  and `wf-agents list` or the pages to follow it;
  `wf-agents pause --all` and `wf-agents resume --all` pause everything and lift it. Whoever plans the work waits
  with `wf-agents wait` for what needs them, each answer ending with the mark to give the next.
- `client/`: the owner's page, which the `wf-agents` binary carries: build it with `npm ci` and `npm run build` in
  `client/` before building the Rust.
