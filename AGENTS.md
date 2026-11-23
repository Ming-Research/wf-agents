# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given.

- [flow/agents/tree_rules.md](flow/agents/tree_rules.md) says how the design tree is read and changed, and what to read
  before designing or changing code. [design/constitution.md](design/constitution.md) decides between designs.
- `docs/` holds designs read as a whole, built or still ahead of the code, and guides such as how wf-agents is tested.
  Read the one of the part your work changes. Where a document describes what is built and differs from the code or the
  design tree, the code and the tree hold, and the work that finds it puts the document right.
- Everything in the repository is written in plain English, as an engineer would write it: short sentences, common
  words, and say who does what.
- Keep keys, credentials, personal paths and machine-local settings out of the repository, its commits and its PR
  descriptions.
- Tests run in CI (`.github/workflows/ci.yml`), not on the machine the agents run on, where they only fight the owner
  for the processor. Before a check, run `rustfmt` on each Rust file you changed or added, naming each file: CI checks
  the format first.
