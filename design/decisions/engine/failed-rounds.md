# Failed rounds

How the engine tells why a round failed, and what it does about each kind of failure.

### decision a-failure-is-handled-by-its-kind
A failed round is handled by the kind of its failure, and sent back only if what came back could not be read: some kinds are tried again, some make every work wait, and the rest pause its work.
- replaced: Counting a failed round as a return sent back was on main; the owner changed it, because every error of the agents' command line, not only the usage limit, should be handled correctly and shown.
- rejected: an agent proposed one way for every failure, trying the round twice and then pausing its work for the owner; the owner rejected it, because when a limit is spent or the network is lost, every work would then use up its tries and ask the owner on its own.

### decision the-fields-tell-the-kind-before-the-words
The kind of a failed round, and when a usage limit resets, are read from the fields in which the command line reports its failure; its words decide only what the fields leave open.
- rejected: an agent proposed adding the weekly limit's words to those the engine looks for, and reading the day from "resets Fri 5pm"; the owner rejected it, because what had been lost was the ability to tell the errors apart, not the weekly limit.

### decision a-shared-cause-is-one-wait
When a round fails for a cause outside all works and the same for all of them, the agents' rounds of every work are held in one wait; no work is paused for it, and every work goes on once it ends.
- replaced: Each work trying its failed round again on its own, less and less often, and pausing at the ninth failure in a row, was on main; the owner changed it, because each try put the round's whole message into its session once more, a dozen copies for two works in an hour's outage.
- rejected: an agent proposed trying no round once the agents' program could not run, until the owner pressed a button to go on; the owner rejected it, because the engine can find out by itself that the outside works again, and every work then goes on without anyone pressing anything.

### decision the-engine-checks-the-outside-itself
While the agents' wait lasts, the engine checks the outside with no round and in no work's session: half a minute after the round that met it, then twice as long each time, up to five minutes.
- replaced: Starting the agents' command line again and again while the network was down was on main; the owner changed it, because each try wrote the round's whole input into its session again, 27 copies in one outage.
- rejected: an agent proposed trying a round again every half minute, or less and less often up to every half hour; the owner rejected it, because every try writes the round's whole message into its session again, three tries making three copies, whether signed out or out of usage.
- rejected: an agent proposed checking only the sign-in, with the program's own command for it; the owner rejected it, because the sign-in is not all that fails: the service may go down, and the network may be lost.
- rejected: an agent proposed trying a service's error again after half a minute, then twice as long each time, pausing after two tries; the owner rejected it, because an overload should be tried again soon, then less and less often, never more than five minutes apart.

### decision the-acts-host-is-tried-by-a-look
While the host the acts use cannot be reached, the engine checks it every half minute, and every act goes on once a check reaches it; no act is sent to test it.
- rejected: an agent proposed checking a lost network after a minute, then twice as long each time, up to fifteen minutes; the owner rejected it, because the engine can keep watching the network itself, checking at least every half minute.

### decision an-unread-return-is-sent-back
A return that could not be read is sent back to the agent in its session, like any return that does not fit the form, and counts against the run's limit of returns sent back.
- replaced: Closing the brackets of a return that is not JSON and having the cheapest model fix its syntax was on main; the owner changed it, because the agent in its session knows what it meant to write, while a repair only guesses.

### decision a-flagged-round-begins-a-new-session
A round that the service's safety check flagged is given again at once in a new session of its line; if it is flagged again in the same run, with only failures in between, its work is paused.
- replaced: Treating a flagged round as a passing failure and trying it again after a wait was on main; the owner changed it, because a round flagged again and again would then be tried again forever.
