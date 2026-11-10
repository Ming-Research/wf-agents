# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given.

- This branch, `next`, holds wf-agents built anew from nothing, beside the engine on `main`, which it replaces once it
  does what that one does. Nothing here depends on the code of `main`: what is taken from it is read, judged and
  brought over, the commit saying from where. Until then, "main" in the rules of the design tree means this branch.
- [design/README.md](design/README.md) says how the design tree is read and changed, and what to read before designing
  or changing code. [design/constitution.md](design/constitution.md) decides between designs.
- Everything in the repository is written in English.
- Keep keys, credentials, personal paths and machine-local settings out of the repository, its commits and its PR
  descriptions.
- The code is Rust, edition 2024, on the toolchain `rust-toolchain.toml` pins. It is formatted by rustfmt
  (`rustfmt.toml`), passes clippy with warnings as errors, and uses no `unsafe`; the root `Cargo.toml` sets these for
  every crate. A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- Tests run in CI (`.github/workflows/ci.yml`), not on the machine the agents run on, where they only fight the owner
  for the processor; `cargo check` and `cargo clippy` of a crate are light enough to run there. Before pushing, run
  `rustfmt` on each Rust file you changed or added, naming each file: CI checks the format first.
- A change reaches `next` through a pull request whose CI is green and which a session other than its writer reviewed.
