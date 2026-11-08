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
  the command line it runs in, keeping a waiting session's prompt cache warm, and an architect's base for its forks,
  and telling a round that missed it.
- `crates/health`: the system's health numbers: the registry of what is measured, each measure defined once, its
  series over any period at any step, computed from the records or read from the samples the engine takes each minute
  of what no record holds.
- `crates/api`: the owner's interface apart from serving it: its types (the page's TypeScript types are generated from
  them), the reads that fill them, the bus its live events go through, and what the owner asks of the running engine.
- `crates/web`: the interface as `wf-agents run` serves it on the loopback address: HTTP and JSON under `/api/v1`, the
  event stream, and the page built into the binary.
- `client/`: the owner's page, in TypeScript with React and Vite; `client/src/api.ts` is generated, not written.
- `flow/flow.json`: the flow every piece of work goes through.
- `db/schema.sql`: the only definition of the database's tables and views.
- `constitution.md`: the lasting values designs are judged by, which the owner sets.
- `design/`: the design tree, wf-agents' design decisions by category, from `design/wf-agents.md` down.
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
`rust-toolchain.toml` pins, the tests run by cargo-nextest split across three jobs, each test stopped at 60 s; the page's type check, lint, check of its styles (`npm run check:ui`), tests and
build, which come before any cargo step that compiles `crates/web`; and that `client/src/api.ts` is what the Rust types generate
(`WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` writes it, and with it `client/src/stored-flow.json`, the
flow as the engine stores it, which the page's tests read and git leaves out). The page's tests in files named
`*.browser.test.tsx` draw the page in a real Chromium (Vitest's browser mode, `.github/actions/browser`), for what
happy-dom has no real version of: layout, the pointer, focus and the keys; the full CI runs them with the page's other
tests, which it skips when the page's files are the same as in a run that passed, and the workflow `fast` only when a
run names one. On the project's own machine the libraries Chromium links against are installed by hand, as a workflow
has no root there. Its job `check` is red whenever another of its jobs is. A red format check leaves the files as rustfmt makes them, and a stale `client/src/api.ts` the file as
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

Besides the CI, the engine checks each return an agent hands in (`engine.checks` in the flow) and sends one that fails
back. A work that changes one of these checks is judged by the installed engine, so by the check as it was until the
change is merged and installed: it names the checks it changes (`checks_changed`), in the project the engine is built
from only, and what only those find in its returns is not sent back but set aside, kept on the round
(`rounds.set_aside`, the view `work_set_aside`) and numbered S1, S2, ... in the work. The details review and the
re-check judge each against the diff, as the old rule's product or a real problem to fix, none judging what its own
return set aside, and the merge gate sends any still not judged to the fix and the re-check before the work may merge.

## Installing

The program is installed on its own, never in the data directory: build the page (`npm ci` and `npm run build` in
`client/`), then `cargo build --release`, and copy `target/release/wf-agents` to a directory on the `PATH`, such as
`~/.local/bin`; the `build` workflow's artifact is the same binary for macOS. The data directory, named by `--data` or
`WF_AGENTS_DATA`, holds the configuration, the database, the files the database refers to, and the works' and the
architects' worktrees under `worktrees/`, which its git leaves out. Once the configuration is committed,
`wf-agents init` makes the database; it is the only command that does, and every command that works on the database,
the engine included, fails when there is none, so a wrong `--data` is an error and not an empty view.

To replace the installed program while an engine runs, stop the engine first, as Running below says, then copy the new
program over the old one and start it with `wf-agents run`.

## Running

Start the engine on a data directory, then open the owner's page:

```
wf-agents run --data DIR
```

`--data DIR` can be left out when `WF_AGENTS_DATA` names the directory. The engine serves the page at
`http://127.0.0.1:7480/`, on the loopback address only; `wf-agents run --port PORT` takes another port. It starts with
every work paused: nothing runs until the owner presses Start all in the page's bar, or starts one work. The owner
works on the page: answering the asks, ruling on the decision cards, pausing, starting and stepping works, starting a
work over, saying something in a work's group, holding a project's new starts. [The owner's page](#the-owners-page) below says
what each view holds. `wf-agents run --once` takes one pass without the page, which is how a script or a test steps the
engine.

One engine runs on a data directory. It takes an operating-system lock on the file `db/engine.lock` before it opens the
database, removes the stop file or writes anything, and keeps it until it exits. A second `wf-agents run`, or a second
`wf-agents run --once`, on the same directory fails at once with the number of the process that holds the lock, so two
engines never take the same work, which would run its round twice, two agents in one worktree and two pushes. The lock
goes with the process however it ends, a crash or a kill included, and a program the engine started does not keep it, so
there is nothing to clear by hand. The text in the file only says who holds the lock; deleting the file frees nothing.

To stop the engine, for instance before installing a new program, create the file `stop` in the data directory, as `touch DIR/stop` does.
The engine dispatches nothing more, waits for the rounds under way, says it stopped, removes the file and
exits, and the lock goes with the process. A new `wf-agents run` started while the old process still holds the lock
fails at once, naming that process; start it again once the old one has exited.

## On the command line

Much of what the owner does on the page can also be done on the command line, which is for a session that works beside
the owner and for scripts, and the commands below are the ones for the owner. Starting, stepping and starting a work over, and the
page's Start all and Pause all, are on the page only. Each command works on the data directory (`--data DIR`, or `WF_AGENTS_DATA`) and prints what it
finds; `--help` of each tells what its arguments mean, and a command line given none of the commands fails with the
usage and their names. The engine itself and the checks of a change come first:

- `wf-agents run --once`: the engine takes one pass without the page, as above; `wf-agents run` is how the engine and
  the page start.
- `wf-agents init`: makes the database, once.
- `wf-agents config check`: checks the configuration as the data directory's files are now, committed or not.
- `wf-agents flow check flow/flow.json`: checks a flow against its format, the database and every state the engine can
  be in.
- `wf-agents flow cost flow/flow.json`: tells what installing a flow would do with each work that goes on.

What the page shows, read:

- `wf-agents status`: where each work stands.
- `wf-agents history WORK --full`: a work's rounds, in full.
- `wf-agents message WORK --json`: the message a work's next agent round would be sent.
- `wf-agents counters WORK`: how many times a work took the rows that count each counter.
- `wf-agents anomalies`: returns sent back, compactions, counters past their limits, and works paused.
- `wf-agents cost WORK`: the time and cost of each work, or of one work's rounds.

Items and what starts next:

- `wf-agents item add --project P --title T --problem X --why Y --level L`: creates an item.
- `wf-agents item add --project P --title T --problem X --why Y --goal`: creates a goal, an item with no work of its
  own, which no work runs on: it is done once every item it depends on is done.
- `wf-agents item list`: every item.
- `wf-agents item depend ITEM ON --needs WHAT --kind blocking`: makes an item depend on another.
- `wf-agents item edit ITEM --title T`: rewrites what was written of an item, here its title; `--problem`, `--why`,
  `--approach` and `--acceptance` rewrite the rest.
- `wf-agents item main ITEM ON`: makes an item's dependency on another the other's main one, which puts the other
  under it in the tree of items.
- `wf-agents item cancel ITEM --why WHY`: cancels an item.
- `wf-agents ready`: the items waiting to start, in the order they start: one whose work was taken over first, then
  each project's items in its architect's start order, which the architect keeps.

Asks and decision cards, which the page shows where they were asked:

- `wf-agents asks --all`: the asks of the owner, those waiting for an answer, or all of them.
- `wf-agents ask ASK`: one ask in full, with its answer.
- `wf-agents answer ASK --text TEXT --speaker owner`: answers an ask.
- `wf-agents design-cards --all`: the current decision cards, open or ruled, or every one.
- `wf-agents design-card CARD`: one decision card in full, with its rulings.
- `wf-agents rule CARD --confirm`: rules on a decision card, here by confirming the option it recommends; `--choose`
  takes another of its options and `--disagree` says why not.

Saying something, and pausing:

- `wf-agents say WORK TEXT --speaker owner --to both`: says something in a work's group, to its engineer, its architect
  or both.
- `wf-agents architect say TEXT --speaker owner`: says something to a project's architect.
- `wf-agents architect withdraw MATTER --why WHY`: takes a matter out of the architect's queue before it is handled.
- `wf-agents pause WORK --why WHY`: pauses a work; `wf-agents resume WORK` asks to lift the pause.
- `wf-agents hold --why WORDS`: holds a project's new starts; `wf-agents release` lifts the hold.

`--speaker owner` is for the owner. A session that speaks for the owner gives `--speaker delegate`, and marks a part of
what it says that is the owner's own words, found in it letter for letter, with `--relayed`.

## The data directory

The data directory is a git repository with at least one commit that the owner keeps outside this repository, named by
`--data` or `WF_AGENTS_DATA`. The engine reads only the configuration that is committed. Its `.gitignore` should leave out
what the engine writes: `db/`, `logs/`, `tool-output/`, `notifications.log` and `worktrees/`. It holds:

- `install.toml`: the owner, the machines and their slots, the CLIs and agent levels, the gh allow list and the limits of
  a query.
- `shared/`: files that a project's file starts from, by `extends`.
- `projects/`: one file for each project.
- `rules/`: the rules agents are given, by the names the projects list them under.
- `agents/`: the configuration directory of each agent CLI, and of each account it may run under; the engine keeps it out
  of git.
- `db/`: the database `wf-agents.sqlite`, made by `wf-agents init`, and `engine.lock`, the lock that keeps a second
  engine off the directory.
- `logs/`: the logs of the runs of the project's CI.
- `tool-output/`: the output of a tool too long to give in a message, saved under the round that asked for it.
- `notifications.log`: what the engine told the owner, one line each.
- `worktrees/`: the worktrees of the works and of the architects.
- `reasoning-system/`: the clone of the reasoning system that projects with records are checked by.
- `stop`: made by the owner to stop the engine; the engine removes it when it starts and when it stops.

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
history of one is its file's in git, a withdrawn decision's included. wf-agents keeps its own in the design tree, `design/`, and has no
directory of decisions.

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
command = "env RUSTUP_AUTO_INSTALL=0 cargo vendor --locked --versioned-dirs target/dependency-sources"

[[dependency_sources]]
lock = "client/package-lock.json"
into = "client/node_modules"
command = "npm ci --ignore-scripts --no-audit --no-fund --prefix client"
```

What a command must do: fetch each package at the version the lock file locks, as it was published, with its types;
compile nothing and run no code of a package (`--locked` keeps cargo from changing the lock file, `--ignore-scripts`
runs no script of a package); and write only into its `into`, a directory of its own that it may empty first.
`RUSTUP_AUTO_INSTALL=0` makes a machine that lacks the toolchain `rust-toolchain.toml` names fail rather than download it,
and the engine sets no environment of its own for a command. `into`
must be ignored by the project's git: the tool checks that before it runs anything, taking the path as a directory, and
refuses when it is not, so that what a command writes never shows as a change of the work. The command is split into
words as a shell splits a line and run without a shell, in the worktree's root; a command that needs a shell is written
`sh -c '...'`. A command that runs past its time limit is stopped with everything it started, and one that ends leaves nothing it
started running (a process it left in its process group is stopped when it ends). `wf-agents config check`
tells every fault of the tables at once.

A project's file that has these tables is refused by an engine that does not include the tool (a field it does not know
is an error): install the engine that includes it first, then add the tables. The tables are the owner's, in the data
directory; the decisions behind the tool are in `design/wf-agents/backend/tools.md`.

## Updating a project's lock files

An agent that implements or fixes a change changes a manifest as it changes code, and has the engine's tool
`lock_update` bring the lock file in line with it: the agent names the lock file and nothing else, and the engine runs the
command the project's file gives for that lock file, in the directory that holds it in the work's worktree, so the command
need not know the project's layout. The engine knows no language and no lock file format, so a project of any language
lists a command of its own, one `[[lock_updates]]` table for each lock file (`lock`, the `command`, and `timeout` in
seconds, 600 unless set). For wf-agents itself:

```toml
[[lock_updates]]
lock = "Cargo.lock"
command = "env RUSTUP_AUTO_INSTALL=0 cargo update --workspace"

[[lock_updates]]
lock = "client/package-lock.json"
command = "npm install --package-lock-only --ignore-scripts"
```

What a command must do: write only the lock file, compile nothing and run no code of a package, and move nothing the
manifest does not ask to move. `--workspace` locks only what the manifests newly ask for, where a bare `cargo update`
upgrades every dependency; `RUSTUP_AUTO_INSTALL=0` makes a machine that lacks the toolchain `rust-toolchain.toml` names
fail rather than download it; `--package-lock-only` writes only the lock file and `--ignore-scripts` runs no script of a
package, and `--prefix` is not needed in `client/`. The command is split into words as a shell splits a line and run
without a shell; a command that needs a shell is written `sh -c '...'`, and one that needs an environment variable
`env NAME=value ...`, since the engine sets none for a command. A command that runs past its time limit is stopped with
everything it started, and one that ends leaves nothing it started running. The tool says whether the command changed the
lock file's content and gives all the command wrote. `wf-agents config check` tells every fault of the tables at once.

A project that lists no lock file runs no command: the tool refuses and says what to do instead. A project's file that has
these tables is refused by an engine that does not include the tool (a field it does not know is an error): install the
engine that includes it first, then add the tables. The tables are the owner's, in the data directory.

A command the engine runs does not go through the agent's permission check, runs on the owner's machine with the engine's
whole environment, and reads a worktree an agent wrote. What such a command can take from the files an agent writes is not
settled: #284. Until its report, wf-agents' own file lists no lock file for this tool.

## How the engine runs git

Every git command the engine runs in a project's checkout, a work's worktree or the data directory is built by
`config::shared_git::command`. All worktrees of a checkout share one git configuration, which an agent's `git config` can
write, and git runs the programs that configuration names, as the engine's user with the engine's credentials. So the
engine's git ignores the configuration for everything that names a program, and does not read what an agent can write
in the machine's global or system files either. A project that wants the engine to push or fetch has to fit these:

- **Remotes.** The engine reaches `origin` over ssh with the `ssh` on its `PATH` and the keys that user's ssh finds (a
  running ssh agent, `~/.ssh/config`), over a path on the machine, or without authentication. `core.sshCommand` is
  ignored, and no credential helper, `core.askPass` or terminal prompt is used (`GIT_TERMINAL_PROMPT=0`): an `https`
  origin that needs a password cannot be reached by the engine's git, and the command fails. Remote helpers of the form
  `ext::<command>` and `remote.<name>.uploadpack` or `receivepack` are ignored too.
- **Attributes.** The engine's git reads no gitattributes: not the project's `.gitattributes`, nor the global or the
  system file. A filter, merge driver or textconv program is run only where an attribute names it, and the names are
  any word, so they cannot be listed and shut one by one. What the engine checks out and merges has no end-of-line
  conversion, no `merge=union`, no large-file filter (git-lfs) of the project's. An agent's own git is not affected.
- **Hooks, signing, file-system monitor.** None is run: not the project's hooks, not `fsmonitor`, and the engine's
  commits, merges and pushes are not signed, nor are signatures checked.
- **Repositories.** The engine names the empty tree of a SHA-1 repository for "no attributes"; in a SHA-256 repository
  the commands that read attributes fail with git's message, they do not run a program.

Not covered: `$GIT_DIR/info/attributes`, which git reads whatever the attribute source, `ssh`'s own configuration
files, and `uploadpack.packObjectsHook` in git's global or system file (it runs when the engine fetches from a remote that
is a path on the machine, and a `-c` of the engine's does not reach it; stopping it would mean the engine's git reads
neither file, which also drops what the owner keeps there, such as `url.*` and `http.proxy`); a write to any of them is a
write outside the agent's worktree. Where each setting is pinned and why, and the tests that
show a program named by each does not run, are in `crates/config/src/shared_git.rs` and
`crates/engine/tests/git_programs.rs`.

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

The architect works from a base: a session that read the project and planned what to do next, which only saves tokens,
built anew when the project has none, the policy changed, or what came since it passed `[architect] carried_most`
characters. The engine takes what comes next in a fixed order, by fact: a new base when one is due, the owner's words and
replies, main's CI red, the matter a work stopped at its question waits on, then the rest of the queue, oldest first. The
matters of one run, from the work leaving its wait until the queue is empty, are handled one at a time in one session
forked from the base (where the agent's command line cannot fork one, a new session given only what its cells read); it
forks anew at the next matter when a new base was built or it passed the limit of a session's context. Each matter leaves one record and any line it adds to the plan or the understanding; a change that can lose something (a cancel, a merge, a split, a
rewritten or removed acceptance, a change to an item a work runs on) is made only once a second eye, a session of that
matter's own forked from the same base on the same model, finds it sound. A fork reads the base from the prompt cache, instead of writing it again,
because it shares with the base all that comes before the base's messages in the prompt: the model, the base's system
prompt, the same tools, and one schema of the return for the base, each matter's session and the second eye, made from
the forms of all their cells (the engine then checks each return against its own cell's schema); only the messages and
the reasoning effort differ, and their cells all think, none running at `low`, or none does. A fork reads the base from
the cache only up to the end of a call the cache keeps under what the fork puts before its messages, and each fork
writes only its own end, so the session manager keeps the base's end in the cache for the forks
(`crates/sessions/src/warm.rs`). Before a fork is made it warms the base with one keep-warm ping that runs the fork's
command line, resuming the base, unless the base's last call is such a ping less than 55 minutes old: once after the
base is built, whose last round's call ends before the return it handed in; once after an install changed what comes
before the messages; and once after the cache may have let it go. While the project's architect has a work running, it
pings the base 55 minutes after its last call, as it does a waiting session; with no work running no fork comes, and
the first fork after the wait warms the base itself. These pings count among the most a wait is worth at the model's
prices (39 on Opus 5.5, whose write for an hour costs forty reads). On the trial's board a ping of a session of that size
on Opus 5.5 cost about $0.03, while a second eye that found the base's end not cached under its command line wrote again
72,713 of the base's 106,594 tokens, $0.57 beyond reading them, and every fork after it did the same. A fork holds the
pings its base was sent before it was made, for its last call and its cost, and a fork that misses the cache after such
a ping is compared with the ping's call, which the ping records. The engine watches main's CI for each project once a minute: when the
full CI of main's latest commit is red, it stops the merge queue, runs the red checks again, reads the known red
checks and the run before the merge, and only when those do not clear it gives the architect the matter of which merge
made it red, and whether to have it fixed or to revert it.

Every agent hands in flow feedback, what was wrong with its round's cell, which stays in the round's return for the
owner and the assistant to go through later and goes to no agent. A project's file turns it off with
`flow_feedback = false`, and its agents are then not asked for it.

## The owner's page

However the engine ends, Ctrl-C and a closed terminal included, the agents it was running are stopped with it. Start all in
the page's bar, which the engine's start with every work paused waits for, is also how the page is looked at against a
copy of the data; Pause all stops every work at the end of the round it is in, and beside it a
switch keeps waiting sessions warm meanwhile. Each work has its own Start, Pause and Step in the head of its
group in the Messages view, which win over the global state when they come later, Start all aside, which lifts no
work's own pause; Step runs one round of a work, after showing what it will be, and leaves it paused; Start and Step
resume the work's session or start a new one, as the owner chooses. Start over, after asking, ends a work on an item
at once, a round of it under way cut off, and a new work begins the item again from the first cell, taking over its
branch, worktree and pull request; the item's architect can start a work over too.

To let the system wind down without stopping what runs, the owner holds one project's new starts: the button Hold new
starts at the right end of the architect's title row, or `wf-agents hold --why WORDS` (`--project` when the
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
