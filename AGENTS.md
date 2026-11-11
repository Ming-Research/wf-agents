# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given.

- [design/README.md](design/README.md) says how the design tree is read and changed, and what to read before designing
  or changing code. [design/constitution.md](design/constitution.md) decides between designs.
- Everything in the repository is written in English.
- Keep keys, credentials, personal paths and machine-local settings out of the repository, its commits and its PR
  descriptions.
- Tests run in CI (`.github/workflows/ci.yml`), not on the machine the agents run on, where they only fight the owner
  for the processor. Before a check, run `rustfmt` on each Rust file you changed or added, naming each file: CI checks
  the format first.
- An agent the engine runs names to its `check` and `check_before` a test by the name its results give it, ending at
  the test's own name: a Rust test by its function's name, or with its module path and its test binary before it
  (`tests::a_row`, `carry tools::tests::a_row`); a page test by its own title, or with the titles of its describes
  before it, joined by ` > `, its file counted from `client/` (`src/x.test.ts > a describe > its title`). A binary, a
  module, a file or a describe alone names no test. The page's browser tests (`*.browser.test.tsx`) start Chromium
  only in a run that names one.
