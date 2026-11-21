# Failed rounds

How the engine tells why a round failed, and what it does about each kind.

### decision a-failure-is-handled-by-its-kind
A round that failed is handled by the kind of its failure, sent back only where what came back could not be read: some kinds are tried again, some wait with every work, the rest pause it.
- replaced: A round that failed counted as a return sent back stood on main; the owner changed it, because every error of the agents' command line, not the usage limit alone, is to be handled rightly and shown.
- rejected: an agent proposed one way for every failure, trying the round twice and then pausing its work for the owner; the owner rejected it, because at a spent limit or a lost network every work would then spend its tries and ask the owner on its own.

### decision the-fields-tell-the-kind-before-the-words
A failed round's kind, and when a usage limit resets, are told by the fields the command line names its failure in, its words deciding only what the fields leave open.
- rejected: an agent proposed adding the weekly limit's words to those the engine looks for, and reading the day in "resets Fri 5pm"; the owner rejected it, because what was lost is the ability to tell the errors apart, not the weekly limit.

### decision a-shared-cause-is-one-wait
A round that fails for a cause outside every work and the same for all of them holds the agents' rounds of every work as one wait; no work is paused for it, and every work goes on once it is over.
- replaced: Each work trying its failed round again on its own, ever later, and pausing at the ninth failure in a row, stood on main; the owner changed it, because each try kept the round's whole message in its session once more, a dozen copies for two works in an hour's outage.
- rejected: an agent proposed trying no round once the agents' program could not run, until the owner pressed a button to go on; the owner rejected it, because the engine can find by itself that the outside is well again, and every work then goes on with nobody pressing.

### decision the-engine-checks-the-outside-itself
While the agents' wait stands, the engine checks the outside with no round and in no work's session: half a minute after the round that met it, then twice as long each time, up to five minutes.
- replaced: Starting the agents' command line again and again while the network was down stood on main; the owner changed it, because each try wrote the round's whole input into its session again, 27 copies in one outage.
- rejected: an agent proposed trying a round again every half minute, or ever less often up to half an hour; the owner rejected it, because every try writes the round's whole message into its session again, three tries three copies, signed out or out of usage alike.
- rejected: an agent proposed checking only the sign-in, with the program's own command for it; the owner rejected it, because the sign-in is not all that fails: the service may go down, and the network may be lost.
- rejected: an agent proposed trying a service's error again after half a minute, twice as long each time, pausing after two tries; the owner rejected it, because an overload is to be tried again soon, then ever less often, never more than five minutes apart.

### decision the-acts-host-is-tried-by-a-look
While the host the acts reach cannot be reached, the engine looks at it every half minute, and every act goes on once a look reaches it; no act is sent to try it.
- rejected: an agent proposed checking a lost network after a minute, then twice as long each time up to fifteen minutes; the owner rejected it, because the engine can keep watching the network itself, looking every half minute at the longest.

### decision an-unread-return-is-sent-back
A return that could not be read is sent back to the agent in its session, as any return that does not meet the form, and counts against the run's limit of returns sent back.
- replaced: Closing the brackets of a return that is no JSON and having the cheapest model mend its syntax stood on main; the owner changed it, because the agent in its session knows what it meant to write, while a mending only guesses.

### decision a-flagged-round-begins-a-new-session
A round the service's safety check flagged is given again at once in a new session of its line; flagged again in the same run, with only failures between, its work is paused.
- replaced: A flagged round taken as a failure in passing and tried again after a wait stood on main; the owner changed it, because a round flagged again and again would then be tried again for ever.
