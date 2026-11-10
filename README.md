# wf-agents

wf-agents runs coding agents through a defined flow. This branch, `next`, builds it anew from nothing, beside the
engine on `main`; [AGENTS.md](AGENTS.md) says how the two stand to each other.

- `design/`: the design tree. [design/README.md](design/README.md) says how it is read and changed; `constitution.md`
  holds what wf-agents is for and the values its decisions come from; `principles/` holds the principles each kind of
  decision follows.
- `pending-design.md`: designs not yet true of the code, each entering the tree with the change that makes it true.
- `crates/`: the engine's Rust crates. `crates/wf-agents` is the `wf-agents` command; `wf-agents flow page <dir>` serves
  the owner's flow page for the definition in a directory, and `wf-agents run` runs the engine with the owner's pages,
  its agents, acts and tools following a script for now. To try it on the trial flow:
  `wf-agents run --data <an empty directory> --flow crates/service/tests/trial --script crates/service/tests/trial/script.toml`,
  then `wf-agents add --title "Greet the user" --flow trial --write "request=Greet the user."`, `wf-agents resume 1`,
  and `wf-agents list` or the pages to follow it.
- `client/`: the owner's page, which the `wf-agents` binary carries: build it with `npm ci` and `npm run build` in
  `client/` before building the Rust.
