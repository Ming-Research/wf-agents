# Settings

The settings of the installation and of each project: where each is kept, how a project's declaration is read and held, and how the texts sent to the agents name them.

### decision a-text-names-a-setting-by-its-key
A text sent to an agent names a setting by its key in backticks, given its value beside it; a project's own setting its declaration lacks holds that project, any other with no value refuses the start.
- rejected: an agent proposed a separate slot for each setting in the texts, filled in by code one at a time; the owner rejected it, because the old system named `owner.language` and gave its value beside the name, which is one way for every setting.
- replaced: Refusing the start for any setting a text names that has no value, a project's own included, was on main; the owner changed it, because a setting a project's declaration lacks is that project's fault, and it must not stop the other projects.

### decision how-tests-are-named-is-a-project-setting
The project's setting `project.test_names`, which the tool's words name, says how an agent names a test to `run_tests`; the project's guide says nothing about the engine's tools.
- replaced: The project's guide telling the engine's agents how to name tests to the engine's checks was on main; the owner changed it, because the guide holds what belongs to the project, and is not to be coupled to the engine.

### decision a-project-declares-what-is-its-own
What is one project's alone (its files, tests, workflows, heavy and lock commands, check command, how its pull requests merge) is in its declaration; the installation keeps what is of its machine.
- replaced: Keeping the lock commands, the local check command, the merge method, branch deletion and the fast workflow's time limit in the installation's settings was on main; the owner changed it, because each installation had to copy them, and it was not clear whose they were.

### decision a-declaration-is-wf-agents-toml-at-the-root
A project's declaration is `wf-agents.toml` at the root of its repository, read from its main branch; no default declaration stands beside the flows for a project that has none.
- replaced: The declaration was `flow/project.toml`, beside the flows, with a copy of it read there for the page and the commands, was on main; the owner changed it, because the declaration is the project's, not the flows', and a copy beside the flows is a default no project owns.

### decision a-project-whose-declaration-cannot-be-read-is-held
A project whose declaration cannot be used is held: its works pause with the reason, the others go on; a fault of the flow, the settings or two projects in one repository still refuses the start.
- replaced: A declaration that could not be read keeping the whole engine from starting was on main; the owner changed it, because one project's fault stopped the works of every project.

### decision a-declaration-lists-itself-as-protected
A declaration whose `[protected]` paths do not hold `wf-agents.toml` holds its project, since it names commands the engine runs and a work's change to it is the owner's to let merge.
- rejected: an agent proposed the engine always taking the declaration as protected, so that none need list it; the owner rejected it, because the protected paths would then not all be in the declaration, and the list the agents are told would have a second source.

### decision a-held-projects-declaration-is-read-as-the-engine-starts
A held project's declaration is read again only as the engine starts: the owner mends it and starts the engine again.
- rejected: an agent proposed reading a held project's declaration again when its main moves, and carrying the project out at once; the owner rejected it, because the engine would then set up a project's whole carrying while running, a kind of state it has none of, with much more code and tests.
