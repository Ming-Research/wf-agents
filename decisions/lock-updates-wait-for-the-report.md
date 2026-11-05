# wf-agents' own file lists no lock file for `lock_update` until the report of #284

## What is done now

The engine's tool `lock_update` is in the flow and the engine, and merges with #280. wf-agents' own project file does not
list a `[[lock_updates]]` table yet, so on wf-agents the tool runs no command: it only refuses, and an agent keeps to the
lock commands its work's message names. When the report of #284 (what an agent can reach through a command the engine
runs for it) is out, the owner decides whether to add the tables and whether a boundary held by the engine must come
first.

This holds for the two lock files of wf-agents, `Cargo.lock` and `client/package-lock.json`. Another project's owner
decides for that project, by its own file.

## Why

A command the tool runs goes through no permission check of the agent's command line. It runs on the owner's machine,
started directly by the engine with the engine's user and whole environment. The engine pushes and runs `gh` with its own
credentials; whether a command it runs for an agent can use them has not been tried. Those facts are read from the source.

The command reads a worktree an agent wrote. From the documentation of rustup, cargo and npm, without running any of it:
`path` in `rust-toolchain.toml` can name a directory as the toolchain, so that the configured `cargo` is the program in that
directory; `.cargo/config.toml` can change the registry; npm's command-line options outrank `.npmrc`, so `--ignore-scripts`
and `--package-lock-only` cannot be overridden there. That part is inferred. #284 settles it by running it.

Merging the code changes nothing on the machine, because a project that lists no lock file runs no command. Adding the
tables is the step that makes `lock_update` run commands, and it should rest on a report that ran them, not on an engineer's
reading of documentation.

The tool `dependency_sources` shares the code that runs a command, and so the risk. wf-agents' file leaves its tables unset
as well, for the same reason, and the owner decides the two together once #284 reports.

The price: the tool has no real run until then, so a fault in it shows only later. #283, which removes the old prefix
permissions, stays held, and the lock commands keep resting on one agent command line's rules for longer, which breaches
Article 1 for that time.

## Options not chosen

- Merge the tool and add the tables now: the tool gets real runs soon, #283 can be released once each lock file has one
  success, and the breach of Article 1 ends sooner. It was not chosen because it accepts possibilities nobody has run before
  they are checked, and a command that has run cannot be taken back if the report finds a real path.
- Do not merge the tool before the report: the work waits for #284 and the owner's choice of a boundary, and builds on it.
  It was not chosen because waiting lowers no risk, as a project with no table runs no command, and adds no rework later, as
  the code that runs a command is in one module that a boundary would change alone. It would hold #280 and #283 too.

## How it was decided

The owner's choice at decision point P1 of the design of item #280, relayed by the assistant with the owner's words: the rest
as recommended (the owner's ask 68).

## Challenges considered and turned down

None yet.
