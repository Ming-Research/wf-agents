### decision line-kind-in-the-flow
Whether the agents of a line only read is said by that line in the flow's definition. A line says nothing and writes.
Why: A list of line names kept by the engine or by the installation would name parts of the flow outside it, and would go wrong without anyone seeing when a line is added or renamed. The flow is what the owner reads and approves, and where the comparison of two flows shows a line changing kind.

### decision rules-are-the-engines
The engine's lists of heavy commands and lock commands apply on every installation. An installation's settings may add a heavy command, a lock command or a use of one to refuse, but cannot remove an entry. The engine refuses, when it reads the settings, program arguments that would undo or swallow its own: a settings file given there, an end of the flags, and a last flag left without its value. An allow given in the arguments that could match part of a heavy command, even without a word boundary, is refused again, or makes that heavy command refused whole.
Why: The engine pushes and has the project checked. A project on another toolchain names its own heavy and lock commands in its configuration (Article 1). What stays open is the rules in the settings files of the program's own configuration directory, which the engine does not read, and a heavy command the shell reaches by another name.

### decision lock-commands-under-heavy-ones
On a line that writes, the commands that update a lock file and build nothing are allowed, even though the heavy commands they fall under are refused. Where the installation allows some other use of such a heavy command, the engine refuses that use again. Where the installation allows the whole heavy command, or every command, the heavy command is refused whole and its lock commands are not allowed.
Why: Agents that could not regenerate a lock file wrote it by hand, or stopped and waited for someone. But the command line refuses whatever any rule refuses, so refusing the heavy command whole would refuse its lock commands as well, and leaving out the refusal without refusing the installation's rules again would let a build through.
