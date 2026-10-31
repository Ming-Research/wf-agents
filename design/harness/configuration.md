Decision: The configuration is written in TOML and its format is defined by Rust types in this repository, which reject fields they do not know, and `wf-agents config check` checks a configuration, instead of a format described only in prose, because a typo in a setting must fail where it is written, not change what the engine does without a word.

Decision: The configuration is one installation file (the owner, the machine and its slots, the agent command lines and levels, the list of allowed GitHub commands, the limits on database queries), shared files that several projects use, one file per project, and the full text of the rules; a project file lists in `extends` the shared files it builds on, merged in order, a later file replacing a whole value of an earlier one, lists included, because several projects share most of their settings and a merge that replaces whole values is predictable without reading every file.

Decision: The data directory is a git repository and the engine reads only committed configuration, and each round records the commit of the configuration it ran with, because then every setting behind a round, including those that shaped the engine's own decisions and never reached an agent, can be named afterwards.

Decision: A configuration that fails to read is not used: the engine keeps the last good one and records an anomaly, instead of stopping, because a broken edit must not stop the works that are running.

Decision: The configuration is not pinned to a work: the next round uses the latest committed configuration, instead of the version the work started with, because a setting is changed to take effect now, and each round records the inputs it was actually given.

Decision: A name in the configuration (a project, a level, a rule) is the reference the database holds, so renaming one in the configuration renames it in the database too, instead of separate identifiers, because a second identifier would have to be kept in step with the name everywhere it is shown.

Decision: An item's level, its tier, picks the command line and the model its agents run with, each cell at the reasoning effort the flow declares for it whatever the model, a level setting no effort; and the advisor runs at the level one above the item's (the next in the order of the levels' names, by number when they are numbers; the top level stays at the top), each round recording the model it ran on, instead of every line at the item's level, because the owner decided that the advisor is the engineer with a higher model, as the flow says: one level above the engineer.

Decision: The cells of a line run at `low` all or none, which the flow's check refuses otherwise (the observer's one cell runs at `low`; the architect's rethink after a batch at `medium`), because `low` does not think, and turning thinking on or off may leave nothing of the session's prompt cache; rounds of a line between `medium`, `high` and `xhigh` kept reading it.

Decision: The architect runs at a level the installation's configuration names (`[agents] architect`), the top level when it names none, and each project names the file in its checkout that holds its constitution (`constitution`), which every cell of the architect's graph is given, instead of the architect at an item's level, which it has none of, or one constitution for every project, because the architect is one over every project, and each project's decisions come from its own constitution.

Decision: An item may be at no level above the one the installation's configuration names as the highest for items (`[items] highest_level`): the command line and the architect's changes refuse such an item, and the engine pauses a work that would start on one, saying why, while the advisor still runs one level above the item's, instead of any level for any item, because the owner does not want everyday development run on the top model, which only the advisor uses.

Rejected:
- Deep merging of tables and lists across extended files: rejected because what a setting ends up as would then depend on every file in the chain, while replacing whole values makes the last file that sets a value decide it.
