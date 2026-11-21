# Settings

The settings of the installation and of each project, as the texts sent to the agents name them.

### decision a-text-names-a-setting-by-its-key
A text sent to an agent names a setting by its key in backticks and is given its value beside the name; when it starts, the engine refuses a flow whose texts name a setting with no value.
- rejected: an agent proposed a separate slot for each setting in the texts, filled in by code one at a time; the owner rejected it, because the old system named `owner.language` and gave its value beside the name, which is one way for every setting.

### decision how-tests-are-named-is-a-project-setting
The project's setting `project.test_names`, which the tool's words name, says how an agent names a test to `run_tests`; the project's guide says nothing about the engine's tools.
- replaced: The project's guide telling the engine's agents how to name tests to the engine's checks was on main; the owner changed it, because the guide holds what belongs to the project, and is not to be coupled to the engine.
