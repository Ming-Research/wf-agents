# The rules of a line

What the agents of a line may run and change, as the flow, the project's declaration and the installation say.

### decision the-review-lines-only-read
The lines that review the plan and the code only read, as their files in the flow say, and are refused the tools that edit files and the git commands that change the repository.
- replaced: The review lines writing as any line does stood on main; the owner changed it, because the old engine's review lines only read.

### decision a-line-of-tests-is-held-to-what-its-run-changed
As a round of a line that changes only the tests ends, it is sent back for each file its run changed that `[tests] paths` of the project's declaration does not take in.
- replaced: The tester changing any file, the implementation's code included, stood on main; the owner changed it, because the old system checked that it changed only the tests.
- rejected: an agent proposed the paths of the tests alone, without the page's test helpers; the owner rejected it, because `testing.ts` and `fixtures.ts` serve only tests, while the page's mocks serve its design mode, and a Rust source file holding tests is code.

### decision the-tests-line-says-where-another-test-goes
A line that changes only the tests is told in its system prompt the paths it may change, and that where a test must go into another file, it says so in what it hands in.
- rejected: an agent proposed telling it only to change the files those paths match; the owner rejected it, because the line was then not told what to do with a test that must go into a source file.

### decision heavy-commands-are-the-projects
Every agent is refused pushing, which the files beside the flows refuse every agent, and the heavy commands its project's declaration names.
- replaced: A list of heavy commands written in the engine, other projects' among them, with pushing refused in the code, stood on main; the owner changed it, because what is a project's belongs in its own declaration, and only the engine pushes.
- rejected: an agent proposed dropping the agents' rule against heavy work on this machine once their permissions refuse it; the owner rejected it, because an agent may still try to run it locally for convenience, so the rule says it briefly and the permissions refuse it too.

### decision lock-commands-under-heavy-ones
On a line that writes, the commands that update a lock file, which the installation's settings name, may run though a heavy command covers them.
- replaced: The engine's tool `lock_update`, run for an agent under settings of its own, stood on main; the owner changed it, because an agent works in its own worktree, where updating its own dependencies harms nothing, and only the host's processor needs guarding.
