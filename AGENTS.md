# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given. The layout is in
[README.md](README.md).

- [CONSTITUTION.md](CONSTITUTION.md) decides between designs; read it before designing.
- Everything in the repository is written in English.
- The code is Rust, edition 2024, on the toolchain `rust-toolchain.toml` pins. It is formatted by rustfmt
  (`rustfmt.toml`), passes clippy with warnings as errors, and uses no `unsafe`; the root `Cargo.toml` sets these
  for every crate.
- A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- `db/schema.sql` is the only definition of the database: change the database by changing it, with the migration in
  the same pull request.
