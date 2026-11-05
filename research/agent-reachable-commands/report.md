# What an agent can reach through the commands the engine runs for it

Item #284. This report holds what the owner needs to decide whether commands the engine runs for an agent get enabled,
and how a boundary around them is held. It changes no engine, CI or flow file. It and `reproduce.sh` are deleted once
the item that puts a boundary around these commands has replaced their conclusions.

Taken on 2026-11-04 and 2026-11-05 UTC. Each conclusion says how it is known:

- **(read)**: from the source or the workflow files of this branch, or from a record the engine or GitHub holds.
- **(run)**: observed on a run, on the machine it names.
- **(docs)**: deduced from a document listed in section 9.

"Inferred" after a label says the conclusion is drawn from that evidence and was not itself observed. "Not run" says no
run stands behind it.

## The problem

The engine has no definition of what an agent can reach on the owner's machines. The limits are written as Claude Code's
tool rules, and the commands the engine runs for an agent do not pass those rules. The example that started this: an agent
writes a `rust-toolchain.toml` whose `path` names another directory, then asks for the tool `dependency_sources`, and the
`cargo` the configuration names may turn out to be a program in that directory. What goes wrong there might not be
undoable. The report answers three questions: can a file in the worktree do that; what can these commands do that the
agent's own shell cannot; and which limits rest only on Claude Code's rules.

## 1. The conclusions

1. **A file in the worktree can make these commands execute a program the configuration does not name.** On a GitHub
   runner, 6 of the 8 cells did (run, section 3.3). Three files do it: `rust-toolchain.toml`, `.cargo/config.toml` and
   `client/.npmrc`. `--ignore-scripts` stopped the `scripts` of `client/package.json` and did not stop npm's `git`
   setting (run). On the engine's machine this is inferred, with one check left for the owner (section 3.4).
2. **Keeping the engine's tools off does not close this.** `dependency_sources` is off, but the agent's own shell may run
   `cargo update` and `npm install --package-lock-only --ignore-scripts` by a prefix rule (read), and those two commands
   were redirected on the runner (run). The same holds for `rustfmt` (read that it is allowed; inferred that it is
   redirected; not run). Not run in an agent's shell, since that would use an allowed command to run a program the
   rules do not allow.
3. **The engine's own `git push` runs a program named in the repository's git configuration.** On the runner a hook
   named by `core.hooksPath` ran on `git push` (run). The engine runs git with no setting that stops this, and the
   agents' rules let them write that setting (read, section 4.3). This needs no tool to be enabled. Nobody has used it:
   the setting is unset today (read).
4. **On the 14900K a branch's code runs by design.** Its `build.rs` and tests run there, what they build stays across
   runs, and a branch can change the workflow it runs by (read, section 5). Redirecting by a file is not needed there,
   so the matrix does not answer for it.
5. **The limits on the agent's shell exist only on Claude Code's command line** and match the text of a command, not what
   it starts (read, docs, section 6). Another coding agent would not have them. What the engine holds itself stays, but
   it bounds what an agent may ask for, not what a command does once it runs.
6. **Three boundaries the engine would hold are in section 7**, for the owner to choose. None of them covers the 14900K
   by itself.

## 2. Where each thing runs

"The owner's machine" now means two machines, so each conclusion below names its place.

| Place | What runs there | What decides what runs |
|---|---|---|
| The engine's machine (`macbook`) | The agent's own shell; the engine's git; local `gh`; the `dependency_sources` archetype and #280's lock commands, if enabled | The configuration names a command; files in the worktree can change what it does (section 3) |
| The 14900K (`[self-hosted, 14900k]`) | `ci`, `ci_before`, and the full CI a work's merge waits for | The branch (section 5) |
| GitHub runners (`ubuntu-24.04`) | `measure`, and the full CI on each push to main | The branch for `measure`; main for the push to main |

How each is known:

- The agent's shell reports `Darwin arm64` (run: `uname -sm`, in this work's round), and the engine starts the agent's
  command line as its own child process (read: `crates/sessions/src/cli.rs`, `Claude::command`). Every work that records a
  machine names `macbook`: 100 works, and 228 record none (read: `works.machine`). The 14900K is an x86-64 processor, so
  it is not the engine's machine (inferred). The architect read the slots setting, which lists only `macbook`; this
  work cannot read that setting.
- `fast.yml` has `runs-on: [self-hosted, 14900k]` on every job. `ci.yml` has an expression that gives the same runner to
  a run by hand and `ubuntu-24.04` to a push (read). The engine runs the full CI of a work's merge by hand, on the
  work's branch (read: `crates/engine/src/tools.rs`, `full_checks_on`). So that run is on the 14900K. This moved after
  the design and its review were written (commit f86ef835, 2026-11-04 15:58 -0800).
- `measure.yml` has `runs-on: ubuntu-24.04` (read). A branch can change that line in its own copy (inferred from
  section 5; not run).

## 3. Question 1: can a file in the worktree redirect a command?

### 3.1 The state of the tools

`dependency_sources` is off. At about 21:30 UTC on 2026-11-04 the tool's description listed two commands; by 21:48 UTC it
said `none`; `tool_runs` holds 0 runs of it (the times read by the architect from the engine's records; the count, and
`none` in the tool's description, read again by this work). #280's `lock_update` has 0 runs too (read). So this report
answers whether to enable them later, and how the boundary is held then.

### 3.2 From the engine to the command (read, not run)

The commands of `dependency_sources` run in `crates/engine/src/dependency_sources.rs`, `run_command`, called by `run`. By
its design #280's lock commands use the same code.

- The working directory is the worktree: `.current_dir(dir)`.
- There is no shell: `Command::new(program).args(args)`. The program is the first word of the configured command and is
  found on the engine's `PATH`.
- The environment is the engine's, whole: no `env` or `env_clear` is called. `ENVIRONMENT`, which sets
  `RUSTUP_AUTO_INSTALL=0`, is applied only to the agent's command line (`crates/sessions/src/cli.rs`, `Claude::command`).
- The agent's permission layer is not passed: `crates/engine/src/steps.rs` puts `Permissions::allowed` and
  `Permissions::refused` into the command line's request, and `crates/engine/src/tools.rs`, `run_tools`, reads neither.
- Standard input is `/dev/null`. The command has its own process group and a time limit.

### 3.3 What the commands do with the files (run, on a GitHub runner only)

[The run](https://github.com/Ming-Research/wf-agents/actions/runs/41939701531), through the `measure` tool, on
`ubuntu24` image 20261026.354.1: rustup 1.29.2, cargo and rustc 1.99.1, npm 10.9.9, node 22.23.4, git 2.56.0.
`RUSTUP_TOOLCHAIN` was not set, and `cargo` resolves to rustup ([an earlier probe of this
work](https://github.com/Ming-Research/wf-agents/actions/runs/41930847827)). The workflow ran the matrix three times, and
the three passes gave the same results in every row.

Each cell is built in a temporary directory, with a harmless stand-in that only writes a marker named after itself and
exits 0.

| Command | File in the worktree | Marker | Exit | Result |
|---|---|---|---|---|
| `cargo vendor --locked --versioned-dirs target/dependency-sources` | `rust-toolchain.toml`, `path` | `cargo` | 0 | Redirected |
| same | `.cargo/config.toml`, `rustc` and `rustc-wrapper` | `rustc-wrapper` | 101 | Redirected |
| `cargo update --workspace` | `rust-toolchain.toml`, `path` | `cargo` | 0 | Redirected |
| same | `.cargo/config.toml`, `rustc` and `rustc-wrapper` | `rustc-wrapper` | 101 | Redirected |
| `npm ci --ignore-scripts --no-audit --no-fund --prefix client` | `client/.npmrc`, `git` and `script-shell` | `git` | 254 | Redirected |
| same | `client/package.json`, `scripts` | none | 0 | Not redirected |
| `npm install --package-lock-only --ignore-scripts`, in `client/` | `client/.npmrc`, `git` and `script-shell` | `git` | 254 | Redirected |
| same | `client/package.json`, `scripts` | none | 0 | Not redirected |

The baselines have none of these files. Each of the four commands exited 0 and left no marker.

The controls show that each file takes effect, which is what lets "not redirected" count:

| File | Control | Result |
|---|---|---|
| `rust-toolchain.toml` | `rustup show active-toolchain` | Names the stand-in's directory, "overridden by" the file |
| `.cargo/config.toml` | `cargo check` | Marker `rustc-wrapper`, exit 101 |
| `client/.npmrc` | `npm config get git`, in `client/` and with `--prefix client`; `npm config get script-shell` | Each gives the stand-in |
| `client/package.json`, `scripts` | The two npm commands without `--ignore-scripts` | Markers `preinstall` and `prepare`, exit 0 |

No npm row attempted a request: its debug log counted 0. A control shows the count works: `npm view` of a package with
the registry pointed at a closed port counted 1.

What the rows mean:

- **`--ignore-scripts` stops `scripts`, and not the `git` setting.** The two "not redirected" rows hold for `scripts` on
  npm 10.9.9, not for npm in general.
- **`script-shell` left no marker**, and the control shows npm read it (run). With `--ignore-scripts` no script ran for
  it to start.
- **A non-zero exit does not undo a redirect.** The stand-ins do nothing, so cargo and npm failed after they had run
  them. A program that passes the call on to the real tool would exit 0 (inferred; not run).
- **An exit of 0 reads as success.** `crates/engine/src/dependency_sources.rs`, `run`, gives `fetched` on exit 0 (read),
  so a redirected `cargo vendor` would be reported as fetched (inferred; not run through the engine).

**Not run**, so nothing is claimed either way:

- `[build] rustc` alone: `rustc-wrapper`, in the same file, took the call first, so `rustc` left no marker.
- The other keys of `.cargo/config.toml`: `[source]`, `[net]`, `[registry]`, `[target.*]`.
- A manifest that names `rust-version`, as this project's workspace does. The fixture names none. An agent can also edit
  the manifest.
- `.npmrc` at the worktree's root, and a git dependency with a `prepare` script of its own.
- A relative `path`, and a `channel` in `rust-toolchain.toml`. Since `run_command` does not set `RUSTUP_AUTO_INSTALL=0`, a
  `channel` the machine lacks might download a toolchain (read for the setting; inferred for the effect).
- npm above 10.9.9. npm added `allow-git` in 11.9.0, and its default becomes `none` in npm 12 (docs), so the `git` rows
  do not carry over to npm 12.
- The real `run_command`: the first layer in the table below is read, not run.

**The executable bit.** A stand-in must be executable, and the Write tool leaves a file at `-rw-r--r--` (run, in this work's
round). `chmod +x` in the agent's shell is refused with `This command requires approval` (run, in this work's round; no
rule allows it, and the docs' list for `acceptEdits` does not hold it). `Bash(git *)` is allowed (read). On the runner,
`git update-index --chmod=+x`, a commit and `git checkout` turned a file that was not executable into one that is, with
no `chmod` (run). So the bit is within reach by git alone (inferred; not run in an agent's shell).

### 3.4 What the runner does not say about the engine's machine

| Layer | What | Evidence |
|---|---|---|
| From the engine to the command | Section 3.2 | Read, not run |
| What the command does with the files | Section 3.3 | Run, on that runner and those versions only |
| The engine's machine | The engine's `PATH` and `RUSTUP_TOOLCHAIN`, whether `cargo` is a proxy, the versions | To be checked |

Known so far: the agent's shell finds `cargo`, `rustup` and `rustfmt` in `~/.cargo/bin`, and `npm` in Homebrew's `node@24`
(run: `which`). The engine process's own environment was not read: this shell refuses `env` and `ps` (run, the second in
an earlier round of this work).

The owner can check it in two steps. Neither has been run, for that reason. The meaning of `-E`, which shows the
environment a process started with, is read from the macOS manual.

```sh
# Prints only these four variables as the engine started with them. Keep the grep:
# the whole environment may hold credentials. <pid> is the process of `wf-agents run`.
ps -wwE -o command= -p <pid> | grep -oE ' (PATH|RUSTUP_TOOLCHAIN|RUSTUP_HOME|CARGO_HOME)=[^ ]*'
```

```sh
# With the PATH from above: where the programs resolve. cargo can be a proxy only if it sits beside rustup.
PATH='<that value>' sh -c 'command -v cargo rustup npm; rustup which cargo; npm --version'
```

If `RUSTUP_TOOLCHAIN` is set, it comes before the toolchain file (docs), and `rust-toolchain.toml` may not redirect. If it
is not set and the programs resolve as on the runner, the `rust-toolchain.toml` rows carry over by the same mechanism,
and that is still inferred. For a run, the owner can use that `PATH` on the same script:

```sh
PATH='<that value>' bash research/agent-reachable-commands/reproduce.sh
```

It writes only in a temporary directory, since it points `CARGO_HOME` and npm's cache there. It reads no credential, and
points npm at a closed port. It has not been run on macOS.

## 4. Question 2: what these commands can do that the agent's own shell cannot

### 4.1 What differs

| What | A command the engine runs | The agent's own shell, by its rules | Source and evidence |
|---|---|---|---|
| Heavy work | Runs it, such as `cargo vendor` or `npm ci` | Refused | `crates/engine/src/dependency_sources.rs`, `run_command`; `crates/config/src/lib.rs`, `Permissions::refused` (read) |
| The agent's permission layer | Never passes it | Every command passes it | `crates/engine/src/tools.rs`, `run_tools`; `crates/engine/src/steps.rs` (read) |
| The engine's whole environment and credentials | Has them: nothing is removed, and `gh` uses the engine's stored login | The command line inherits the same variables, as nothing clears them. What it passes on to its shell was not read, since `env` is refused. The shell may not run `gh` | `run_command`; `crates/engine/src/hub.rs`, `gh`; `crates/sessions/src/cli.rs`, `Claude::command` (read); `env` refused (run) |
| Files outside the worktree | No limit: a redirected program runs as the engine's OS user | Needs approval, so it is refused, as `ls` of the data directory was | `run_command` (read); the refusal (run, in this work's round) |
| Network | Whatever the program does | Only what an allowed tool or command reaches: `WebSearch`, `WebFetch`, `git` and the lock commands | `crates/sessions/src/cli.rs`, `command_line`; the recorded command line (read) |
| Installing a toolchain | Not stopped: no `RUSTUP_AUTO_INSTALL=0` | Set to 0 | `crates/sessions/src/cli.rs`, `ENVIRONMENT` (read) |
| Carrying a variable to the network without running a foreign program | A project `.npmrc` expands `${VARIABLE_NAME}` and can set `_authToken` for a registry it names, so npm may send a variable of the engine's environment there. So "not redirected" does not mean safe | The same under the prefix rule for `npm install`, except for what the command line leaves out of the shell's environment | [npm's documents](#9-reproducing-it-and-the-sources) (docs; **not run**) |

The rows are what the rules say. Section 6.2 shows why they do not hold as a boundary even under Claude Code.

### 4.2 By place

All read from the source named.

| Place | Command | Source | Note |
|---|---|---|---|
| The engine's machine | `dependency_sources` archetype | `crates/engine/src/dependency_sources.rs`, `run` | Off |
| The engine's machine | #280's lock commands | #280's design, on the same code | Not merged |
| The engine's machine | git | `crates/engine/src/hub.rs`, `push`; `crates/engine/src/git.rs`, `catch_up`; `crates/sessions/src/worktree.rs`, `is_clean`; `crates/config/src/shared_git.rs`, `run` | Live; section 4.3 |
| The engine's machine | local `gh` | `crates/engine/src/hub.rs`, `gh`; `crates/engine/src/tools.rs`, `gh` | Live; held by `gh.allow` |
| The 14900K | `ci`, `ci_before`, the merge's full CI | `crates/engine/src/tools.rs`, `ci`, `ci_before`, `fast_checks`, `full_checks_on` | Live; section 5 |
| GitHub runners | `measure`, the full CI on a push to main | `crates/engine/src/tools.rs`, `measure`; `.github/workflows/measure.yml`, `ci.yml` | Live |

**`gh` is the contrast.** It too runs with the engine's environment and credentials, but only what starts with an entry
of `gh.allow` runs, and `api` only reads (`crates/engine/src/tools.rs`, `gh_refusal`, `api_write`; read). That boundary is
the engine's own, and holds under any coding agent. Local git and the commands of `dependency_sources` have nothing like
it for what the command does once it has started.

### 4.3 The engine's git

This was not in the design's matrix. Reading the code showed it does not depend on any tool being enabled, so a small
case was added to the run.

- The engine runs `git push` in a work's worktree (`crates/engine/src/hub.rs`, `push`), `git merge`
  (`crates/engine/src/git.rs`, `catch_up`) and `git status` (`crates/sessions/src/worktree.rs`, `is_clean`), with no setting
  that disables hooks (read: `crates/config/src/shared_git.rs`, `run`).
- `Bash(git *)` lets an agent run `git config`. The refusals for lines that only read (`GIT_CHANGES`,
  `crates/config/src/lib.rs`) do not include `config` (read).
- All of a project's worktrees share one git configuration: the worktree's common directory is the checkout's, and
  `extensions.worktreeConfig` is unset (read). So a setting made in one work reaches the engine's git in every work
  (inferred).
- On the runner, `git push` left the marker `pre-push` with `core.hooksPath` set, and none without it. Both exited 0 (run).
- Git's manual names more settings that hold a program: `core.sshCommand` for fetch and push, and a hook-based
  `core.fsmonitor`, which `git status` can use (docs; not run).
- `core.hooksPath`, `core.sshCommand` and `core.fsmonitor` are unset in this repository today (read, on 2026-11-05).

Writing the setting from an agent's shell was not tried: the configuration is shared, so it would reach every work.

## 5. The 14900K

**What the workflows do** (read: `.github/workflows/fast.yml`, `.github/workflows/ci.yml`):

- The tools `ci` and `ci_before`, and a run by hand of the full CI, run there.
- A branch's `build.rs` and tests are compiled and run there (`cargo nextest run`), and so are `rustup toolchain install`,
  `npm ci` without `--ignore-scripts`, and `npm run build`, each reading the branch's files.
- `target`, `client/node_modules` and `client/dist` stay across runs: the checkout has `clean: false`, and the step after
  it is `git clean -ffdxq -e target -e client/node_modules -e client/dist`. So what one branch's run leaves there is there
  for the next run, of any branch (inferred).
- The project's runners share a home (read: a comment in `fast.yml`). What one job leaves in it is there for later jobs,
  and `$HOME/.cargo/bin` is on the jobs' `PATH` (read; inferred for the effect).

**The matrix does not answer for it.** Nothing needs redirecting there: running the branch's code is what the job is for.
This work ran no experiment on the 14900K.

**A branch can change the workflow it runs by.** The engine's push rules do not refuse it:

- GitHub runs a workflow by the file the ref holds (docs). `fast_checks` dispatches on the work's branch, and
  `full_checks_on` does for the merge (read: `crates/engine/src/tools.rs`).
- `push_refusal` checks that the head is the clean worktree's HEAD, follows the last push and carries no other branch.
  Nothing in it looks at `.github/` (read).
- Where the engine falls back to main's CI files, `with_main_ci` keeps the ones the work changed itself (read). The test
  `a_work_s_own_change_to_its_fast_workflow_is_refused_and_never_run_with_main_s` holds this: what refuses there is
  GitHub, when the work's file lacks the trigger or the input.
- `branch_changes_fit_work` limits a report to `research/`, but it judges a return, after the pushes, and sets no such limit
  on a work that delivers a change (read: `crates/engine/src/checks.rs`).
- Nothing reviews this before it runs: `ci` runs during the implementation, before any review (read, the flow's order).

So an agent can choose the steps that run on the 14900K (inferred; not run). One thing could stop it on GitHub's side:
adding or changing a workflow file needs a credential with the `workflow` scope (docs). Which scopes the engine's
credential has only the owner can read.

**What GitHub says** (read, through `gh api`, on 2026-11-05):

- The repository is public. No repository or organization secret is visible to it. The default workflow permission is
  `read`, and a workflow can ask for more with `permissions:` (docs), so a branch's own workflow might get a token that
  can write (inferred; not run). The only rules on main forbid deleting it and pushing to it without a fast-forward, so
  a push to main needs no check to pass.
- The list of the repository's own runners is empty, so the runner is registered above it, for the organization (inferred).
  Other repositories of the organization may then be able to use it (docs).
- No workflow here has a `pull_request` trigger (read), but a fork's pull request can add one. GitHub warns that a public
  repository's forks can run code on a self-hosted runner this way (docs). The repository's policy is
  `all_external_contributors`: such a run waits for someone to approve it. So that approval is what stands between a
  fork and the 14900K (inferred; not run).

**What the runner's account can reach only the owner can read.** A check to follow on the 14900K, as that account. It has
**not been run**. It assumes a Unix shell, since the jobs' steps are bash.

```sh
# Who it is, and whether it can become root without a password. A group `docker` is as good as root.
id
sudo -n true; echo "sudo exit: $?"
# Whether its home holds credentials. Names only: do not print what the files hold.
ls -a ~ ~/.ssh ~/.config/gh ~/.aws ~/.docker 2>&1
# The names of the variables a job starts with, no values.
env | cut -d= -f1 | sort
# Whether it can reach anything else from there: other machines, a home network, a NAS.
ip route 2>/dev/null || netstat -rn
```

```sh
# On the engine's machine: the scopes of the credential the engine pushes with. Look for `workflow`.
gh auth status
```

The owner can also read the organization's runner settings on GitHub: which repositories the runner group admits.

## 6. Question 3: which limits rest only on Claude Code's rules

### 6.1 The two lists

**Only on Claude Code's command line. Another coding agent would not have them** (read: `crates/sessions/src/cli.rs`,
`command_line`, which passes them; `crates/config/src/lib.rs` for the values; the recorded command line of round R317-5):

| Limit | Written as | From |
|---|---|---|
| No heavy work | `--disallowedTools Bash(rustc *)`, `Bash(npm ci *)` and the rest | `Permissions::refused`, `heavy` |
| No `git push` | `--disallowedTools Bash(git push *)` | `heavy` |
| The lock commands let through by prefix (#266) | `--allowedTools Bash(cargo update *)` and the rest | `Permissions::allowed`, `locks` |
| The uses of those refused again | `--disallowedTools Bash(npm install *=*)` and the rest | `locks_except` |
| Lines that only read edit nothing and commit nothing | `--disallowedTools Edit`, `Bash(git commit *)` and the rest | `read_only`, `EDIT_TOOLS`, `GIT_CHANGES` |
| `sqlite3` only with `-safe -readonly`; `find` without `-exec`, `-ok`, `-delete`, `-fprint` | `--allowedTools`, `--disallowedTools` | The CLI's own arguments, read by `Cli::allows` |
| Which commands run at all; edits taken without asking; which tools exist | `--allowedTools`, `--permission-mode acceptEdits`, `--tools` | The CLI's own arguments |
| Directories outside the worktree read and not edited | `--add-dir`, `Edit(//...)` | `command_line`, `readable` |
| No Rust toolchain downloaded | `RUSTUP_AUTO_INSTALL=0` | `ENVIRONMENT`, `Claude::command` |

The command line's settings also load the user's settings directory (`--setting-sources user`), which this work cannot read
(run: the read was refused). So one thing stays open: whether those settings add anything, such as Claude Code's own
sandbox. Nothing on the command line turns that on (read).

**Held by the engine. These stay under any coding agent** (read):

| Boundary | Where |
|---|---|
| `gh` only from an allow list, `api` only reads | `crates/engine/src/tools.rs`, `gh_refusal`, `api_write` |
| A push is the clean head, follows the last push, carries no other branch | `crates/engine/src/tools.rs`, `push_refusal`; `crates/sessions/src/worktree.rs`, `handed_in` |
| `dependency_sources` runs only a listed command, into a directory git ignores | `crates/engine/src/dependency_sources.rs`, `run`, `ignored` |
| A command's process group and time limit | `crates/engine/src/dependency_sources.rs`, `run_command`, `stop` |
| `measure` runs only listed workflows and inputs | `crates/engine/src/tools.rs`, `measure_request` |
| `ci` names only known checks | `crates/engine/src/tools.rs`, `checks_refusal`, `tests_refusal` |
| `query` only reads, with hidden tables and limits | `crates/db/src/lib.rs`, `open_for_agents` |
| A report changes nothing outside `research/` | `crates/engine/src/checks.rs`, `branch_changes_fit_work` |

These hold what an agent may ask the engine for. Except for the time limit, none holds what a command does once it runs.

### 6.2 Why the rules are no boundary even under Claude Code

Claude Code's Bash rules match the text of the command (docs). They do not follow what it starts, and its docs say a deny
rule does not match the same program by path or inside `sh -c`. So an allowed command that reads the worktree can run what
the rules refuse:

- `cargo update` and `npm install --package-lock-only --ignore-scripts` by the files of section 3.3 (run, on the runner).
- `rustfmt` by `rust-toolchain.toml`, since it comes from the same proxy directory (inferred; not run).
- `git` by a hook named in its configuration (run, on the runner), or by an alias starting with `!`, which git's manual
  says runs a shell command (docs; not run).

None of this was run in an agent's shell. The rules, the runner's results and the manual are the evidence; that the
same steps work in that shell is inferred.

This is the part of the constitution's Article 1 not yet done: "nothing rests on a feature of one agent's command line".

## 7. Three boundaries the engine would hold

The owner chooses after reading this. Nothing here is built or run: what each option does and what it costs is deduced
from the documents in section 9 and the source read for this report (docs, read; inferred), but where a line says otherwise.

### Option 1: a sandbox around each command

Each command the engine runs for an agent runs in an OS sandbox that limits files and network: Seatbelt through
`sandbox-exec` on macOS; namespaces, seccomp, landlock or a container on Linux.

- **Good:** it holds whichever file or key does the redirecting, the ones not run here included, and for everything the
  program starts. The engine needs to know no tool.
- **Bad:** each OS needs its own profile, and the macOS manual marks `sandbox-exec` deprecated (read). A command that
  fetches needs the network, so the profile must also keep credentials from being read, or a redirected program can
  send them; the environment has to be filtered as well. A profile that is too tight shows up as a failure of a command
  that did nothing wrong. Claude Code's own sandbox is no substitute: it covers the agent's shell, not what the engine
  runs, and it is one agent's feature (docs).
- **The 14900K:** it does not cover it, since GitHub's runner service starts those jobs, not the engine.

### Option 2: neutralize the files and pin the environment

Before each command, the engine removes or overrides what the worktree can set, and pins the environment, so that what
runs is what the configuration names: `RUSTUP_TOOLCHAIN`, `RUSTC_WRAPPER`, `npm_config_git`, `PATH`. The documents say an
environment setting comes before each of these files (docs; not run).

- **Good:** it needs no OS feature and no setup, works the same on every machine, and the command keeps its caches and
  network.
- **Bad:** it is a list of what is dangerous. A key not on the list passes silently, such as those section 3.3 did not run,
  or those of the next version. The list is per tool, while the engine knows no language and no lock file format
  (`decisions/engine-runs-outside-commands.md`), so each project's configuration would have to hold it. Removing files
  changes the worktree the agent works in. And a command that runs as configured still has the engine's whole reach.
- **The 14900K:** it does not cover it, since a CI job is there to run the branch's code.

### Option 3: a restricted OS user

The commands run as an OS user that has no access to the engine's credentials or the owner's home, and has limited network.

- **Good:** the OS enforces it, whichever program ends up running, with no list of tools. The same user could run the
  agent's whole command line, which would put the agent's own shell inside the boundary too and take the boundary off any
  agent's rules (inferred).
- **Bad:** each machine needs setup by someone with admin rights, and the engine needs a way to switch user. The worktree
  and the package caches must be shared between that user and the engine, which makes file ownership and git's
  `safe.directory` a concern. A user alone does not limit the network: that needs a firewall rule for the user. The
  environment has to be filtered as well.
- **The 14900K:** the engine does not cover it, but the owner can apply the same idea to the runner's account. The check
  in section 5 shows whether that is needed. Jobs in that account would still share what they leave.

### With any of the three

The engine's own git needs the owner's credentials, so it cannot simply sit inside the boundary. Its programs named by
configuration need their own answer (section 4.3): running it with those settings pinned, or taking write access to the
shared configuration away from agents. Option 3 alone could do the second, if the configuration stays out of the
restricted user's reach (inferred).

Moving the lock commands to a GitHub runner, which #280 left for this report to weigh, is not a fourth option. It solves
only the lock commands: `dependency_sources` must put the source on the disk of the machine the agents read from, so a
boundary is still needed there. And a branch can change the workflow it runs by (section 5).

## 8. What this means for the decisions around it

These are judgments. Each says the evidence it rests on.

- **Whether to enable the tools later.** Decide the boundary first, then enable. Until then, enabling adds little reach an
  agent's shell does not already have by the prefix rules, and adds the engine's environment without
  `RUSTUP_AUTO_INSTALL=0`. Keeping them off is not a boundary (read, run on the runner; inferred).
- **Which option.** I would take Option 3, for the whole agent command line and what the engine runs for it, since it is
  the only one that needs no list and takes the boundary off any agent's rules (read, docs; inferred). What I am unsure of
  is how much the shared worktree costs on the engine's machine, which no run here tried. Option 1 is the alternative if a
  second OS user is not wanted.
- **#280 (lock commands as an engine tool).** Its two commands are redirected by the same files (run, on the runner), and
  it adds no guard by its design (read), so it needs the same boundary.
- **#283 (remove the lock commands' rules).** It closes two of the routes in section 6.2. The ones through `rustfmt` and `git`
  stay (read; inferred).
- **The 14900K** is a decision of its own: none of the three options reaches it from the engine, and the owner's check in
  section 5 comes first (read; inferred).

## 9. Reproducing it, and the sources

Three ways of getting the evidence were compared. The owner chose the first:

- **A script through the `measure` tool, on a GitHub runner.** It runs the real commands on real files, leaves a record and a
  link, and changes nothing outside `research/`. What it does not run is the engine's own `run_command`, which section 3.2
  reads instead.
- **An integration test in `crates/engine`.** It would run the same commands on the same kind of machine, and add only the
  few lines of `run_command` already read. A report cannot change a file outside `research/`, so it would need an item of
  its own. A test that expects the marker would hold the behaviour in place, not guard against it; a real guard belongs
  with the boundary.
- **The documents alone.** The item asks for real runs, and a decision that may not be undoable should not rest on deduction.

The run: `measure` with `command=bash research/agent-reachable-commands/reproduce.sh`, `base` the same commit as the head,
`pairs=1`. A pass took 4.55 s, 2.49 s and 2.49 s on that runner (run). A pass counts only if it ends in
`REPRODUCE_COMPLETE`, and all three did.

A cell's verdict was fixed before the run. A marker means redirected. No marker means not redirected only if the
command's baseline and the file's control both passed; otherwise the cell has no usable result. Every cell had a usable
result.

The script's environment differs from the engine's on purpose, in settings the matrix does not test: `CARGO_HOME`,
`CARGO_NET_OFFLINE` and npm's `cache`, `registry`, `fetch-retries`, `audit` and `update-notifier`. They keep what it writes in
the temporary directory and keep it off the network.

Documents:

- rustup: [Overrides](https://rust-lang.github.io/rustup/overrides.html),
  [Environment variables](https://rust-lang.github.io/rustup/environment-variables.html).
- cargo: [Configuration](https://doc.rust-lang.org/cargo/reference/config.html),
  [Environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html).
- npm: [config](https://docs.npmjs.com/cli/v11/using-npm/config/), [.npmrc](https://docs.npmjs.com/cli/v11/configuring-npm/npmrc/),
  [npm link, on `allow-git` in npm 12](https://docs.npmjs.com/cli/v12/commands/npm-link/),
  [private packages in CI](https://docs.npmjs.com/using-private-packages-in-a-ci-cd-workflow/).
- git: the manuals `git-config` and `githooks`, read on the engine's machine.
- macOS: the manual `sandbox-exec`, read on the engine's machine; the manual `ps`.
- GitHub: [Events that trigger workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows),
  [Secure use](https://docs.github.com/en/actions/reference/security/secure-use),
  [Self-hosted runners](https://docs.github.com/en/actions/reference/runners/self-hosted-runners),
  [GITHUB_TOKEN](https://docs.github.com/actions/reference/authentication-in-a-workflow),
  [Scopes for OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps).
- Claude Code: [Permissions](https://code.claude.com/docs/en/permissions),
  [Permission modes](https://code.claude.com/docs/en/permission-modes),
  [Sandboxing](https://code.claude.com/docs/en/sandboxing).
