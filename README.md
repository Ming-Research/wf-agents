# wf-agents

wf-agents runs coding agents through a defined flow.

- `design/`: the design tree. [design/README.md](design/README.md) says how it is read and changed; `constitution.md`
  holds what wf-agents is for and its lasting values; `principles/` holds the rules whatever is made later must
  follow; `decisions/` holds what one thing does and why, where the code does not show the why; `docs/` holds the
  designs that hold only as a whole, which may be ahead of the code.
- `research/`: what a work's research made for its problem beside the tests: data, code, probes and notes, with what
  was measured and how to measure it again.
- `db/`: the engine's database schema (`schema.sql`) and its migrations; a change of the schema adds a migration to
  `migrations/`, named by the minute it was written and what it does.
- `crates/`: the engine's Rust crates. `crates/wf-agents` is the `wf-agents` command; `wf-agents flow page <dir>` serves
  the owner's flow page for the definition in a directory, and `wf-agents run` runs the engine with the owner's pages.
  Its agents run with the `claude` command line and its acts with git, as the installation's settings say
  (`<data>/install.toml`, of which `crates/carry/install.sample.toml` is a sample); given `--script`, they follow the
  script instead, reading what is written beside the flows as the real ones do. To try it on the trial flow with its
  script, in `crates/flow/tests/laid_out`, beside what the flows of the tests share:
  `wf-agents run --data <an empty directory> --flow trial --script scripts/trial.toml`; then
  `wf-agents add --title "Greet the user" --flow trial --project trial --write "request=Greet the user."` and
  `wf-agents resume 1`, `wf-agents answer 1 named 9 1` once its plan asks the owner,
  and `wf-agents list` or the pages to follow it;
  `wf-agents pause --all` and `wf-agents resume --all` pause everything and lift it. `wf-agents play` with the same
  `--flow` and `--script` and an empty `--data` plays instead the case the script states, on a clock of its own, and
  says whether the work took the path it states; `run` with the script then serves what it left. Whoever plans the
  work waits with `wf-agents wait` for what needs them, each answer ending with the mark to give the next. They read
  the plan with `wf-agents changes --since <mark>`, `candidates`, `search <text>`, `graph <item>` (up to what it
  depends on and down to what depends on it; `--up` or `--down` for one), `tags` and `text <item> [<block>]`, and
  `list`, which leaves out what merged or was cancelled unless given `--all`; they change items with
  `wf-agents change --why <sentence> <item>:<kind>[=<value>]...`, all or none, or one at a time with `set-title`,
  `set-level`, `add-dependency`, `remove-dependency`, `tag`, `untag`, `cancel` and `reopen`, each with `--why`.
  `wf-agents system` says how the system ran over the last days: what went past the lines of its history, and what is
  under way against its own.
- `client/`: the owner's page, which the `wf-agents` binary carries: build it with `npm ci` and `npm run build` in
  `client/` before building the Rust.
