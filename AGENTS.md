# Working on wf-agents

What an agent needs to know for this repository, beyond the rules every agent is given. The layout is in
[README.md](README.md).

- [reasoning/constitution.md](reasoning/constitution.md) decides between designs; read it before designing.
- Everything in the repository is written in English.
- The code is Rust, edition 2024, on the toolchain `rust-toolchain.toml` pins. It is formatted by rustfmt
  (`rustfmt.toml`), passes clippy with warnings as errors, and uses no `unsafe`; the root `Cargo.toml` sets these
  for every crate.
- A new crate goes under `crates/` and takes its version, edition and lints from the workspace.
- `db/schema.sql` is the only definition of the database: change the database by changing it, with the migration in
  the same pull request.
- A change to the owner's page follows its UI spec,
  [reasoning/derivations/harness/owner-interface/ui-spec.md](reasoning/derivations/harness/owner-interface/ui-spec.md),
  and passes its check, `npm run check:ui` in `client/`.
- `flow/flow.json` is the only definition of the flow. A change to it, to the views it reads or to the compiler
  passes `wf-agents flow check`; the flow crate's tests run the same check.
- `reasoning/` holds the project's records, kept by the rules of the reasoning system, the submodule
  `reasoning-system/`. A change to them is logged at the top of `reasoning/log.md` and passes
  `python3 -B reasoning-system/check.py --root reasoning --ascii --base <the commit before the change> --ready`.
