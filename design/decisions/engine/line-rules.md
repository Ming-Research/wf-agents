# The rules of a line

What the agents of a line may run and change, as set by the flow, the project's declaration and the installation.

### decision the-review-lines-only-read
The lines that review the plan and the code only read, as their files in the flow say, and are refused the tools that edit files and the git commands that change the repository.
- replaced: The review lines writing as any other line does was on main; the owner changed it, because the old engine's review lines only read.

### decision a-line-of-tests-is-held-to-what-its-run-changed
When a round of a line that changes only the tests ends, it is sent back for each file its run changed that `[tests] paths` in the project's declaration does not cover.
- replaced: The tester changing any file, including the implementation's code, was on main; the owner changed it, because the old system checked that it changed only the tests.
- rejected: an agent proposed only the paths of the tests, without the page's test helpers; the owner rejected it, because `testing.ts` and `fixtures.ts` serve only tests, while the page's mocks serve its design mode, and a Rust source file that holds tests is code.

### decision the-tests-line-says-where-another-test-goes
A line that changes only the tests is told in its system prompt the paths it may change, and that if a test must go into another file, it says so in what it hands in.
- rejected: an agent proposed telling it only to change the files those paths match; the owner rejected it, because the line was then not told what to do with a test that must go into a source file.

### decision a-return-that-hands-in-leaves-the-worktree-committed
A return asking for no call, from a line that may change the work, is sent back while the worktree holds a tracked file's uncommitted change or an unfinished merge; untracked files do not stop it.
- rejected: an agent proposed leaving the judgment to the check before the merge and only dropping untracked files from what counts as not committed; the owner rejected it, because a tracked file a local command rewrote would still stop the work before the merge, with nothing to run the check again.
- rejected: an agent proposed that the engine delete every untracked file as a round ends; the owner rejected it, because it would delete a new file the agent forgot to commit, losing work already paid for, and cannot tell a cache from a new file.

### decision heavy-commands-are-the-projects
Every agent is refused pushing and the agents' own program, which the files beside the flows refuse to every agent, and the heavy commands its project's declaration names.
- replaced: A list of heavy commands written in the engine, other projects' commands among them, with pushing refused in the code, was on main; the owner changed it, because what belongs to a project goes in its own declaration, and only the engine pushes.
- replaced: Refusing every agent pushing alone was on main; the owner changed it, because an agent that ran the agents' program itself spent where no work's record or budget counts it.
- rejected: an agent proposed dropping the agents' rule against heavy work on this machine once their permissions refuse it; the owner rejected it, because an agent may still try to run it locally for convenience, so the rule says it briefly and the permissions refuse it too.

### decision a-toolchains-variables-are-the-projects
A project's declaration names the variables its toolchain reads (`[environment]`), given every round of its works after the engine's own; naming one the engine or the program owns is refused.
- replaced: The engine setting `RUSTUP_AUTO_INSTALL` to 0 for every round of every project was on main; the owner changed it, because a toolchain's variable written in the engine is a toolchain's word in the engine (Article 1).
- replaced: Leaving the variable unset was on main; the owner changed it, because then a round does not download hundreds of megabytes unseen, and a missing toolchain is an error the agent sees.

### decision lock-commands-under-heavy-ones
On a line that writes, the commands that update a lock file, which the installation's settings name, may run even though a heavy command covers them.
- replaced: The engine's tool `lock_update`, run for an agent under settings of its own, was on main; the owner changed it, because an agent works in its own worktree, where updating its own dependencies harms nothing, and only the host's processor needs guarding.

### decision permissions-are-rules-not-a-mode
The engine refuses a permission mode or a skip-permissions flag in agents.args and permissions.defaultMode in the program's settings file; permissions are --allowedTools rules.
- replaced: Accepting --permission-mode in the arguments, and reading bypassPermissions and auto as allowing every command, was on main; the owner changed it, because the program starts in default mode under CLAUDE_CODE_SUBPROCESS_ENV_SCRUB, which the engine always sets.
