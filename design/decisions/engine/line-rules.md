### decision line-kind-in-the-flow
Whether the agents of a line only read is said by that line in the flow's definition. A line says nothing and writes.
Why: A list of line names kept by the engine or by the installation would name parts of the flow outside it, and would go wrong without anyone seeing when a line is added or renamed. The flow is what the owner reads and approves, and its line's file is where the owner sees the line's kind. This rests on judgment, not on a case.

### decision rules-are-the-engines
The engine's heavy commands are refused on every installation. An installation's settings may add heavy commands, and lock commands that may run under a heavy command, but no lock command that is a heavy command whole or that pushes. Settings that would otherwise undo the engine's rules are refused when they are read.
Why: The engine pushes and has the project checked, and a project on another toolchain names its own commands in its configuration (Article 1). How wide a lock command under a heavy one may be is the installation's judgment. What stays open: rules in the settings files of the program's own configuration directory, which the engine does not read, and a heavy command the shell reaches by another name.

### decision lock-commands-under-heavy-ones
On a line that writes, the lock commands the settings name, which update a lock file and build nothing, are allowed, even though the heavy commands they fall under are refused. Where the installation allows some other use of such a heavy command, the engine refuses that use again. Where the installation allows the whole heavy command, or every command, the heavy command is refused whole and its lock commands are not allowed.
Why: Agents that could not regenerate a lock file wrote it by hand, or stopped and waited for someone. But the command line refuses whatever any rule refuses, so refusing the heavy command whole would refuse its lock commands as well, and leaving out the refusal without refusing the installation's rules again would let a build through.
