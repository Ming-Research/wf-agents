# The accounts

The accounts the agents' program makes its calls under, which the owner adds on the page and switches between.

### decision an-account-is-a-name-and-a-token-in-the-keychain
An account is a name the owner gives and a token the agents' program makes to last a year (`claude setup-token`), kept in the macOS login keychain through `/usr/bin/security`, the token on its standard input, under a service keyed to the installation's id. The token is checked by the check of the outside before it is kept, and is in no file, setting, record, log, words of an error or answer of the page.
Why: The keychain keeps it encrypted at rest, where the program keeps the same kind of token; a file under `~/.wf-agents`, as GitHub's token is kept, was turned down for lying open on the disk, the Security framework called from the engine for trusting only the binary that made an item, which every install replaces (a judgment, not a trial), and a service keyed to the data directory's path for breaking when the directory moves and needing escaping in the line `security -i` reads.

### decision the-account-in-use-is-the-engines-own-state
Which accounts there are, and which is in use, are records of the store only added to (an account added, removed, taken), the engine's own state as whether sessions are kept warm is: the account in use is the latest taken and not removed since, and with none the agents run on the sign-in of their directory. A token is kept before its account is recorded as added, and let go of after its account is recorded as removed, again at each removal and as the engine opens until it is.
Why: The page changes the account while the engine runs, and the engine reads the settings' file only as it starts, so an account kept there would wait for the next start; kept in this order, a recorded account never lacks its token, and a token left behind by a removal is let go of later (principle effects-known-as-done).

### decision the-account-is-handed-out-with-the-round
Every round is handed out with the account in use as it is handed out; each try records that account's name, apart from the directory of the program's settings it ran with, and the program is given that same account's token, a round handed out before a switch running under the account it was handed out with. A token that cannot be given fails the round as a command line that cannot run, in the words of what keeps it.
Why: Read again by the carrier as a try begins, the account recorded and the one whose token is given could part across a switch; one field holding a directory or a name would have to be told apart by guessing; and falling back to the directory's sign-in would run a round under another account than the one recorded, unseen (principle nothing-passes-for-normal).
