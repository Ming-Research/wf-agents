# wf-agents

wf-agents runs coding agents through a defined flow. Each piece of work goes through research, design,
implementation, testing, review and merge, and the flow fixes what every step is given, what it hands back and where
the work goes next. wf-agents belongs to no project: everything about a project lives in that project's
configuration, outside this repository. [constitution.md](constitution.md) holds the values its decisions come from.

## Layout

- `crates/wf-agents`: the `wf-agents` command.
- `crates/flow`: the flow's compiler; `wf-agents flow check` runs it.
- `crates/db`: creating, migrating and opening the database; every change to its state is kept in `changes`.
- `crates/config`: the configuration's types, read from the data directory (`--data`), which lives outside this
  repository.
- `crates/sessions`: the session manager: the session each agent's round runs in, what the session holds and costs,
  the command line it runs in, keeping a waiting session's prompt cache warm, and telling a round that missed it.
- `crates/api`: the owner's interface apart from serving it: its types (the page's TypeScript types are generated from
  them), the reads that fill them, the bus its live events go through, and what the owner asks of the running engine.
- `crates/web`: the interface as `wf-agents run` serves it on the loopback address: HTTP and JSON under `/api/v1`, the
  event stream, and the page built into the binary.
- `client/`: the owner's page, in TypeScript with React and Vite; `client/src/api.ts` is generated, not written, and
  `client/ui-spec.md` is its UI spec.
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `constitution.md`: the lasting values designs are judged by, which the owner sets.
- `decisions/`: the decisions in force for wf-agents' design, one file each, which the owner chose; its README.md says
  how one is written.
- `delegate/AGENTS.md`: the role of the session the owner starts there to think with and watch over the system.
- `.github/workflows/ci.yml`: the full CI, every check, which each push to main runs and a work's head runs once, at
  the merge; `.github/workflows/fast.yml`: the fast CI the engine runs by hand on a work's branch, with the tests to
  run and the checks named; `.github/actions/lint`: the lint steps both run; `.config/nextest.toml`: how they run the tests; `.config/english-exempt.txt`: the files the check that the
  repository is in English leaves out.
- `.github/workflows/measure.yml`: the measurement workflow the engine's tool `measure` runs for a project that lists it
  (`[[ci.measure]]` in the project's configuration): it times a command on the code to compare with, a twin of it and
  the code measured, in rotating pairs, and uploads the raw times. A project's workflows that run on push must ignore
  the branches under `wf-agents-measure/`, where the tool runs it, as this repository's do.

## Checks

The full CI, `ci`, runs on each push to main, and on a work's branch when the engine runs it by hand: rustfmt, clippy
with warnings as errors, and the tests, on the toolchain
`rust-toolchain.toml` pins, the tests run by cargo-nextest split across three jobs, each test stopped at 60 s; the page's type check, lint, check against its UI spec (`npm run check:ui`), tests and
build, which come before any cargo step that compiles `crates/web`; and that `client/src/api.ts` is what the Rust types generate
(`WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` writes it, and with it `client/src/stored-flow.json`, the
flow as the engine stores it, which the page's tests read and git leaves out). Its job `check` is red whenever another
of its jobs is. A red format check leaves the files as rustfmt makes them, and a stale `client/src/api.ts` the file as
generated, in the artifact `page-types`, for the engine to put in the work's worktree. The workflow `fast` runs only
when asked, by hand, and only what it is asked for: it compiles and runs the tests named (its input `tests`), and runs
the lint only when that is named (its input `checks`), in a few minutes; it has no job `check`, so that a head only
it has run cannot be merged. Where a project's configuration sets `ci.on_demand`, the engine's tool `ci` runs it so,
with the tests listed and the checks named among those the configuration runs by name (`ci.named_checks`, given by
`ci.checks_input`), and runs nothing when it names neither; the full CI runs by hand once, at the merge, on a head that
has no run of it yet, and what it finds red goes back to the work, which fixes it with the fast CI, the lint named
while it fixes the lint, until it hands a head in to the merge again; else a push runs the full CI and the engine only
waits for it. The tool `ci_before` runs the tests it is given on the
code before the change by hand with the same input; a project with no such input (`ci.tests_input`) has it push a
temporary branch and wait for the whole run, unless the project's configuration sets `ci.runs_on_before_branches` to
false, which says its CI does not run on those branches and has the tool refuse at once. GitHub runs a workflow by hand
on a branch by the file of it the branch holds, and refuses to on a branch made before main's took the request; `ci`
and `ci_before` then run it with main's CI files (`.github/`), `ci` on a temporary branch `wf-agents-main-ci/...`
holding the head with them, whose run it gives as the head's. Both tools read what became of each test named from the
JUnit results the run uploads, in the artifact the project's configuration names (`ci.results`, whose documentation in
`crates/config/src/lib.rs` gives the conventions and how a name names a test). One of the tests,
`crates/wf-agents/tests/english.rs`, holds the repository to English: it fails on a
tracked file that holds a letter that is not ASCII, written as itself or as an escape, but for the files
`.config/english-exempt.txt` names, such as a document made as a translation.

## Installing

The program is installed on its own, never in the data directory: build the page (`npm ci` and `npm run build` in
`client/`), then `cargo build --release`, and copy `target/release/wf-agents` to a directory on the `PATH`, such as
`~/.local/bin`; the `build` workflow's artifact is the same binary for macOS. The data directory, named by `--data` or
`WF_AGENTS_DATA`, holds the configuration, the database, the files the database refers to, and the works' and the
architects' worktrees under `worktrees/`, which its git leaves out. Once the configuration is committed,
`wf-agents init` makes the database; it is the only command that does, and every command that works on the database,
the engine included, fails when there is none, so a wrong `--data` is an error and not an empty view.

## A project's records

A project that keeps records by the reasoning system names them in its file, `records = { root = "reasoning", ascii =
true }` (`ascii` for records in English only), and its works derive their designs from them, check them and are given
the system's rules by role; one that names none skips every step on them. The installation keeps one clone of
github.com/Ming-Research/reasoning-system for every project, `reasoning-system` in the data directory unless
`install.toml` names another as `[reasoning] system`: the engine runs its `check.py` on a work's records and reads its
rules from there. The records' directory is the project's design paths: a work that revises them puts each question
to the owner as a decision card, lists a change that changes no decision as an edit, and merges once the owner has
ruled every card (the project file no longer names `design_paths`). The flow is wf-agents' own, built into the
program: only in wf-agents' own repository does a card carry the flow drawn from its work's branch.

## A project's decisions

A project keeps the decisions in force for its design in its own repository, one file for each, in the directory its
file names as `decisions` (`decisions` unless it names another). The engine's input `decisions` gives a cell every file
of that directory but a README.md, each by its path with its text, as the project's main has them at the commit the
round is pinned to, as it gives the constitution; none when the directory is not there or holds nothing else. The
engine's tool `decision_history` gives the history of one on main: each version its file had, newest first, with the
commit that made it, its date and message and its text, a withdrawn decision's included, and for a merge the commits it
brought. wf-agents' own are in `decisions/`, whose README.md says how one is written.

## A project's dependency sources

An agent that researches, designs, implements, reviews or tests a change can read the source and the types of the
dependencies a project's lock files lock, without compiling anything: the engine's tool `dependency_sources` runs the
command the project's file gives for a lock file, in the work's worktree, and the agent reads what it wrote. The engine
knows no language and no lock file format, so a project of any language lists a command of its own, one
`[[dependency_sources]]` table for each lock file (`lock`, the directory the command fills as `into`, the `command`, and
`timeout` in seconds, 600 unless set). For wf-agents itself:

```toml
[[dependency_sources]]
lock = "Cargo.lock"
into = "target/dependency-sources"
command = "cargo vendor --locked --versioned-dirs target/dependency-sources"

[[dependency_sources]]
lock = "client/package-lock.json"
into = "client/node_modules"
command = "npm ci --ignore-scripts --no-audit --no-fund --prefix client"
```

What a command must do: fetch each package at the version the lock file locks, as it was published, with its types;
compile nothing and run no code of a package (`--locked` keeps cargo from changing the lock file, `--ignore-scripts`
runs no script of a package); and write only into its `into`, a directory of its own that it may empty first. `into`
must be ignored by the project's git: the tool checks that before it runs anything, taking the path as a directory, and
refuses when it is not, so that what a command writes never shows as a change of the work. The command is split into
words as a shell splits a line and run without a shell, in the worktree's root; a command that needs a shell is written
`sh -c '...'`. A command that runs past its time limit is stopped with everything it started. `wf-agents config check`
tells every fault of the tables at once.

A project's file that has these tables is refused by an engine that does not include the tool (a field it does not know
is an error): install the engine that includes it first, then add the tables. The tables are the owner's, in the data
directory; the decision behind the tool is `decisions/engine-runs-outside-commands.md`.

## The agents' own configuration

The engine runs every agent's command line with a configuration directory of its own (`CLAUDE_CONFIG_DIR` for
`claude`), so the owner's own settings, memory, plugins and tool servers never reach an agent. It is `agents/<cli>` in
the data directory unless the CLI's `config_dir` in `install.toml` names another, and the engine keeps it out of git.
Log in to the command line there once: `CLAUDE_CONFIG_DIR=<data>/agents/claude claude`, then `/login`.

Each login account the agents may run under is such a directory beside the one in use, named for the CLI and the
account (`agents/claude-work`), signed in once the same way; boards whose `config_dir` names a directory in another
data directory share its accounts, each board choosing its own. The Settings view lists them with the account each is
signed in as, shows the command that signs a new one in, and switches the agents to another by writing its directory as
`config_dir` and committing it, after which every line starts a new session at its next round, since a session is kept
in the directory it ran in.

## The run guard

While an agent's round runs, the engine reads each of its calls to the model from the command line's stream, keeps it in
the database, and judges the round against lines computed from the rounds before it, so that no line is a setting. The
rule is the flow's (`guard` in the rounds section of `flow/flow.json`), and a round it stops takes the row `guarded` that
every agent's cell has. The System view shows each running round against its lines under Now, and what the guard saw and
did under Anomalies. Only the command line's adapter (`crates/sessions/src/cli.rs`) reads the stream's format: should a
new version of the command line stop telling its calls there, rounds run unjudged, and a running round shows no calls.

## The architect

Each project has an architect of its own. It runs at the level `install.toml` names under `[agents]` as `architect`
(such as `architect = "4"`), the top level when it names none; its search of observations runs at the level named as
`observer`, `1` when none is named, and the reading of a work's history at the level named as `librarian`, the lowest
when none is named. Each project's file names its constitution, a path in the project's repository, such
as `constitution = "constitution.md"`, which agents read as the project's main has it (fetched for each round, spelled
exactly as on main, never read from the checkout's files) and its architect checks the project against article by article (the
level-two headings that start with "Article"), and the tags its observations may carry, each with what it covers:
`observation_tags = [{ name = "ui", desc = "What the owner sees on the page" }]`.

The architect works from a base: a session that read the project and planned what to do next, built anew when a fact
calls for it (no base yet, a new policy, slots free, enough matters gathered for a planning, a matter that said the plan
changes or the base is wrong, a merge that deleted or moved files or changed the guide, a decision log and changes since
past `[architect] carried_most` characters, or another model). The engine takes what comes next in a fixed order, by
fact: the owner's words and replies, main's CI red, a matter a work waits on, the planning, what the plan queued, and
what waited for the planning, which it gathers until `[architect] gathered_most` matters wait. Each matter is handled
alone in a session forked from the base (where the agent's command line cannot fork one, a new session given whole what
it reads), which ends with one line of the decision log; a change that can lose something (a cancel, a merge, a split, a
rewritten or removed acceptance, a change to an item a work runs on) is made only once a second eye, forked from the
same base on the same model, finds it sound. The engine watches main's CI for each project once a minute: when the
full CI of main's latest commit is red, it stops the merge queue, runs the red checks again, reads the known red
checks and the run before the merge, and only when those do not clear it gives the architect the matter of which merge
made it red, and whether to have it fixed or to revert it.

Every agent hands in flow feedback, what was wrong with its round's cell, which stays in the round's return for the
owner and the assistant to go through later and goes to no agent. A project's file turns it off with
`flow_feedback = false`, and its agents are then not asked for it.

## The owner's page

`wf-agents run` serves the owner's page at `http://127.0.0.1:7480/` (`--port` takes another). However the engine ends,
Ctrl-C and a closed terminal included, the agents it was running are stopped with it. The engine starts with
every work paused, so nothing runs until the owner presses Start all in the page's bar, which is also how the page is
looked at against a copy of the data; Pause all stops every work at the end of the round it is in, and beside it a
switch keeps waiting sessions warm meanwhile. Each work has its own Start, Pause and Step in the head of its
group in the Messages view, which win over the global state when they come later, Start all aside, which lifts no
work's own pause; Step runs one round of a work, after showing what it will be, and leaves it paused; Start and Step
resume the work's session or start a new one, as the owner chooses. Start over, after asking, ends a work on an item
at once, a round of it under way cut off, and a new work begins the item again from the first cell, taking over its
branch, worktree and pull request; the item's architect can start a work over too.

To let the system wind down without stopping what runs, the owner holds one project's new starts: the button Hold new
starts at the right end of the architect's title row, or `wf-agents hold --why <words>` (`--project` when the
configuration has more than one project). No new work of the project starts; works already running go on to their
merge, and works waiting for a result go on after it. It is kept in the database, so it stands across a restart of the
engine, and it is another thing from Pause all, which stops every work and is not kept. The owner releases it with the
sign's Release or `wf-agents release`, and may release the architect's own hold and its hold to set the project
straight too; the architect cannot release the owner's, and reads the holds in its board. Under Pause all the engine
queues no matter of free slots for any architect, since no new work can start.

The Messages view is the one place the owner acts: the one-to-one with each project's architect, wf-agents' own
conversation (its faults), a group for each work of the owner, its engineer and its project's architect, where
everything said reaches all three, and a group for the architects of connected projects. An ask of the owner waits where
it was asked and is answered in its own bubble, and a decision card a work waits on is pinned at the top of its
conversation and ruled, or asked about, in its stage's bubble, an item's design showing it read only; every other view
only reads, but for the bar's Start all, Pause all and Keep warm, the hold on a project's new starts, the Settings view, and the Observations view, where
the owner closes an observation, saying why. `npm run dev` in `client/` serves the page
from its sources and passes the interface's requests to a running `wf-agents run`.

The flow page shows a stored flow version, `flow/flow.json` as it is at a git ref of a project's checkout (read
with git and compiled, nothing written), or the flow a decision card carries, kept in the database as the card's
branch had it, its graphs side by side, and compares two of them, so that a change to the flow is reviewed there.

The Works view is one table with a group of rows for each project: the project's name and, at the right end of the same
row, the buttons that show or hide its works by state, then its architect, then its works. The architect's row opens its
architect's page, read only, with a switch to another project's when there are several: what it does now, its rounds on
its graph, the item changes its matters made, what to do next and the policy in force, each a tab of the column at its
right. Its conversation with the owner, its queue with a card for each matter, and its Start, Pause and Step are in its
one-to-one in the Messages view. The Observations view shows each project's
observations, what its architect recorded and no item does yet, filtered by their tags, those taken in by an item or
closed shown when asked for; one opened shows whole beside them, where the owner closes it.
