# wf-agents

wf-agents runs coding agents through a defined flow. Each piece of work goes through research, design,
implementation, testing, review and merge, and the flow fixes what every step is given, what it hands back and where
the work goes next. wf-agents belongs to no project: everything about a project lives in that project's
configuration, outside this repository. [CONSTITUTION.md](CONSTITUTION.md) holds the values its decisions come from.

## Layout

- `crates/wf-agents`: the `wf-agents` command: starts the engine, and gives the owner's commands.
- `crates/engine`: takes each work through the flow, running the agents' rounds, the tools and the actions;
  `wf-agents run` starts it. A work is one run of the flow on one item (a piece of work to do), and a round is one
  run of an agent.
- `crates/flow`: the flow's compiler; `wf-agents flow check` runs it.
- `crates/db`: creating, migrating and opening the database; every change to its state is kept in `changes`.
- `crates/config`: the configuration's types, read from the data directory (`--data`), which lives outside this
  repository.
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `.github/workflows/ci.yml`: the checks every pushed commit runs.
- `.github/workflows/build.yml`: builds the command for the Mac the engine runs on.

## Running

### What it needs

- `git`, and `gh` logged in: the engine opens pull requests, waits for CI and merges through them.
- The agent CLI that `install.toml` names under `agents.clis`; the only kind there is now is `claude`.
- A clone of each project at the `checkout` its project file gives; each work's worktree is made from it.
- A data directory that is a git repository with at least one commit, which holds the configuration (see below).
  `wf-agents run`, `wf-agents item add` and `wf-agents anomalies` read the configuration from that commit, and fail in
  a repository that has none; `wf-agents config check` reads the files and does not.
- The command: `cargo build --release --locked -p wf-agents` on the toolchain `rust-toolchain.toml` pins. SQLite is
  built into the binary.

### The data directory

Every command except `wf-agents flow check` works on a data directory, given as `--data DIR` or in the environment
variable `WF_AGENTS_DATA`. It holds the owner's configuration and what the command and the engine write beside it. It
is a git repository, and the engine reads the configuration from its last commit, so a change counts once it is
committed; `wf-agents config check` reads the files as they are.

The owner keeps and commits:

- `install.toml`: the owner, the machines, the agent CLIs and levels, the gh allow list and the limits of a query.
- `shared/`: files that project files start from.
- `projects/`: one file for each project.
- `rules/`: the rules agents are given.

Their fields are the types in `crates/config`. The rest is not configuration and is not meant to be committed. Nothing
keeps it out of git, so list it in the data directory's `.gitignore`. The command and the engine write all of it but
`stop`:

- `db/`: the SQLite database, `wf-agents.sqlite`, with its `-wal` and `-shm` files while it is open.
- `logs/`: the logs of CI checks that were red.
- `tool-output/`: tool results too long to give an agent whole.
- `notifications.log`: what the owner is told, one line each, with the time in milliseconds.
- `worktrees/`: only the temporary worktree that reverts a merge. A work's own worktree is made under
  `machines.hosts.<name>.worktrees` in `install.toml`.
- `stop`: a file the owner creates to stop `wf-agents run`; the command removes it when it starts and when it has
  stopped.

### Using it, in order

Each command's arguments are in `wf-agents --help` and `wf-agents <command> --help`; this is the order they are used
in.

1. Write the configuration, check it with `wf-agents config check --data DIR` before committing it, and commit it. The
   engine reads it again on every pass and keeps the last good one if a new one fails its check, so a change needs no
   restart.
2. Create items with `wf-agents item add --project P --title T --problem ... --why ... --level L --queue`, and look at
   them with `wf-agents item list`. `--level` names a level in `install.toml`, which decides the CLI, model and effort
   the item's agents run with. `wf-agents item depend` makes one item depend on another and `wf-agents item cancel`
   cancels one and ends its work. An item starts only when it is in the queue. `--queue` puts it last when it is
   created. `wf-agents queue put N` puts any item in the queue, last or with `--first`, `--before M` or `--after M`,
   or moves one that is there, and `wf-agents queue list` shows the items still waiting to start.
3. Start the engine with `wf-agents run`; `wf-agents run --once` takes one pass and exits. It starts works on the
   queue's items while a slot is free (`slots` of each host in `install.toml`) and takes every work one step at a
   time, so a long round holds up the others. Run only one engine per data directory: nothing prevents a second one
   from starting. To stop the engine, create the file `stop` in the data directory. The engine looks for it between
   two passes, ends when it finds it, removes it and says so on its standard error; it also removes one left from
   before when it starts. Ending the process works too: the round that was running is then closed as not done and
   runs again when the engine starts, and a question waiting for the owner stays open.
4. Answer cards. A card is a question for the owner or a design to rule on. The engine tells the owner of a new card
   in `notifications.log` and on its standard error, and in no other way. `wf-agents cards` lists the cards waiting,
   `wf-agents cards --all` lists every card, and `wf-agents card N` shows one in full with its rulings. Answer a
   question with `wf-agents answer N --text ...`. Rule on a design card with
   `wf-agents answer N --verdict confirm --text ...` or `wf-agents answer N --verdict disagree --text ...`; a
   disagreement reaches the engineer as a message, and a design card that is ruled can be ruled again, but a question
   cannot. In this version the owner answers for the architect too: its questions are cards as well, answered with
   `wf-agents answer N --text ... --why ...`, and `--reply` gives the rest of the reply as JSON, in the shape of
   `architect_reply` in `flow/flow.json`.
5. Follow the works. `wf-agents status` shows where each stands and which are paused; the engine pauses a work itself
   when it cannot take a step or when a cell's returns are sent back too many times in a row.
   `wf-agents history WORK` lists a work's rounds, `wf-agents counters WORK` shows how often it took the rows that
   count each counter, `wf-agents anomalies` gathers what went wrong or ran long, and `wf-agents cost` gives the time
   and cost of each work.
6. Step in when needed. `wf-agents say WORK TEXT` says something to a running work, which gets it in its next
   round; `wf-agents pause WORK --why ...` stops taking a work further, and `wf-agents resume WORK` lets it go on.

## Checks

Every commit pushed to any branch runs `ci`: rustfmt, clippy with warnings as errors, and the tests, on the toolchain
`rust-toolchain.toml` pins.
