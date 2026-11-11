### decision line-kind-in-the-flow
Whether the agents of a line only read is said by that line in the flow's definition. A line says nothing and writes.
Why: A list of line names kept by the engine or by the installation would name parts of the flow outside it, and would go wrong without anyone seeing when a line is added or renamed. The flow is what the owner reads and approves, and where the comparison of two flows shows a line changing kind.

### decision rules-are-the-engines
The engine refuses builds, test suites and pushing on every line, and an installation's arguments can add rules but cannot take one away. Letting an installation set these lists was turned down.
Why: The engine runs the project's check and does the pushing. An agent that ran builds itself would fight the owner for the processor, and one that pushed would put out a commit that no check passed. No installation is better off without these rules.

### decision lock-commands-under-heavy-ones
On a line that writes, the commands that update a lock file and build nothing are allowed, even though the heavy commands they fall under are refused. Where the installation allows some other use of such a heavy command, the engine refuses that use again. Where the installation allows the whole heavy command, or every command, the heavy command is refused whole and its lock commands are not allowed.
Why: Agents that could not regenerate a lock file wrote it by hand, or stopped and waited for someone. But the command line refuses whatever any rule refuses, so refusing the heavy command whole would refuse its lock commands as well, and leaving out the refusal without refusing the installation's rules again would let a build through.

### decision told-what-is-refused
The agent's system prompt tells it what its line's rules refuse, even though the rules already enforce it: that builds and tests go through the tool that runs the project's check, which lock commands it has, that a line only reads, and how the permission check treats chains, pipes and other directories in git. Only what a permission decides by itself and the agent would not try, such as pushing, is left unsaid.
Why: An agent that learns a rule by being refused spends a call on each refusal, and an agent that pipes git into a refused command spends one every time. That cost is the agent lacking information, which the principle of lean input does not ask it to pay.
