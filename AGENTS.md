# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given.

- [design/README.md](design/README.md) says how the design tree is read and changed, and what to read before designing
  or changing code. [design/constitution.md](design/constitution.md) decides between designs.
- Everything in the repository is written in English.
- Keep keys, credentials, personal paths and machine-local settings out of the repository, its commits and its PR
  descriptions.
- Tests run in CI (`.github/workflows/ci.yml`), not on the machine the agents run on, where they only fight the owner
  for the processor. CI checks the format first: run `rustfmt` on each Rust file you changed or added, naming each file.
