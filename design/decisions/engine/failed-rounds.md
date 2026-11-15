# Failed rounds

### decision the-fields-tell-the-kind-before-the-words
The kind of a failed round, and when a usage limit resets, are told first by the fields the command line names the failure in: the service's typed kind of the main loop's last error, then that error's own kind, then a rate limit event that refused as the command line itself reads it (not where it carries on, on the account's overage or in a grace), with the reset they give, then why the round ended (`terminal_reason`, and the result's `subtype`). Then the status the service answered with (`api_error_status`) tells the class: a server's fault or a timeout (408, 5xx, 529) passes, a refused sign-in (401, 403) cannot run, a rate limit (429) is the usage limit only where the words say so and else passes, and any other refusal (4xx) pauses its work, its words telling the kind of the refusal. The words of the report decide only what those leave open, as when it crashed. A connection that failed after the round spent tokens is the round's own failure, in passing, and not the network's.
Why: The words change between versions of the command line, and a kind told by them misses each new wording: a limit in new words is taken for a failure in passing and tried again, and a reset written with its day, as "resets Fri 5pm", is read wrong by words that know only a time of day. The status alone cannot tell the account's usage limit from a throttle, both 429, nor a context too long from another refused request, both 400, so within its class the words still choose. A cut connection, or a body that ended early, says nothing of the network once the service has served the round.

### decision an-error-nothing-tells-pauses-its-work
A round whose command line printed an error that neither its fields, the status the service answered with, nor its words tell the kind of pauses its work, with what it said; a result that is no error and holds no return does too.
Why: Taken as a failure in passing, a weekly limit in words no list held failed eight rounds in a row over two hours, each meeting the limit again; what nobody has told apart may as well be a new kind of limit every round meets, and the owner is the one to look (principle nothing-passes-for-normal). A round that printed no result is another matter: it crashed or was killed, and is tried again (decision a-crash-is-tried-again).

### decision a-crash-is-tried-again
A round whose command line printed no result fails in passing, unless its words tell another kind, and is tried again within the bound on failures in passing in a row; what it spent is its calls' tokens at their model's prices, a model not known priced as the dearest.
Why: A crash says nothing of its cause, and most pass; paused, a work left to run unattended would wait for the owner for what may pass by itself, a judgment no case has measured. Counted, what each try spent reaches the work's budget, and the bound keeps the tries from going on unseen.

### decision a-shared-cause-is-one-wait
A round that fails at the account's usage limit, for a command line that cannot run, or for want of the network holds the rounds of every work, as one wait worked out from the records of every work; no work is paused for it, and every work goes on by itself once the wait is over. A cause of one session or one model (a context past what the account may send without credits, a model the account may not use) pauses the work that meets it, with why; a model the service does not have pauses, with why, that work and every other whose rounds run on it.
Why: One cause stops every work alike: paused one by one, each would wait for the owner to start it by hand once the cause had passed, and a mark written on each would have to be lifted on each. A cause of one session or one model does not pass with time: held as every work's, it would stop works it does not touch and be tried again for ever. A model the service does not have fails every round on it until the installation names another, so each other work on it would fail its next round for the same cause.

### decision a-reset-not-said-is-tried-by-one-round
At a usage limit whose reset the command line did not say, or said further ahead than a week's limit reaches, one round, of whichever work comes to one first, tries half an hour after the last one ended; at a reset it said, every work goes on at once, a reset just passed being now.
Why: The time is a guess: every work going on at it would each meet the limit again and write its message into its session once more (principle no-unseen-spending-loop).

### decision a-trial-is-over-at-the-first-answer
A wait every work shares that lets one round try is over once the service answers that round with a message that is no error, without waiting for the round's end; what answered is kept only in memory.
Why: What the round tries is whether the service serves again, which its first answer shows, and a round may go on for tens of minutes after it, every other work held meanwhile. Lost in a stop, the round is given again and answers again.

### decision the-wait-holds-what-its-cause-stops
There is a wait for each service the works reach: the agents' service, whose wait holds only the agents' rounds, and the host the acts reach, whose wait holds only the acts. An act the host answered is of the kind the answer's status tells: a fault of the host's own (5xx) or its limit on requests (429, or a refusal whose headers say requests are limited) fails in passing, and any other refusal pauses its work, whatever its words; so does what the act finds that only a person mends, as a pull request a person closed. An act that got no answer and fails in the words git, ssh, `gh` or the host give a failing network begins the acts' wait where a look at that host finds it gone, and is otherwise tried again in passing; an act of a host no network reaches, as the local repository, never begins one; an act that fails for any other cause pauses its work. Nothing that comes of one service's rounds ends the other's wait.
Why: The engine's acts need no agent but reach their host (a push, a pull request, a merge), and failing there one by one they would each wait for the owner; held for the agents' service they would wait for nothing, and an act that comes back says nothing of the agents' service, nor an agent's round of the host. Read for words of the network, a refusal whose message names a timeout would be tried again, and a server's fault in words the list lacks, as GitHub's 500, would wait for the owner. A failure in other words would fail again.

### decision the-acts-host-is-tried-by-a-look
The acts' wait is tried by a look at their host every half minute, and is over once a look reaches it, every act then going on; no act is sent to try it, and no time ends it without a look.
Why: A look costs nothing, while an act may hold its round for long (the check waits for the full CI), every other act held behind it; ended by time alone, every act would meet the host still gone and fail again.

The backup of the records is held by neither this wait nor the owner's pause: see decision backup-is-the-engines-own-tick in backup.md.

### decision no-round-tries-the-network
While the network to the agents' service cannot be reached, the engine looks at the service's host itself, which costs no round, and a round tries only once a look reaches it, never at a time alone; a look's finding is not recorded, and after a stop the engine looks again at once.
Why: The command line writes a round's message into its session before it calls the service, and retrying rounds through an outage filled a session with the same input again and again.

### decision the-retry-wait-is-a-call
The wait before a round that failed in passing is tried again is a call of the run that the engine makes and the clock answers at its time, its end counted from the failed round's end.
Why: Deciding then needs no clock, a wait made is known as made after a stop, and the work shows what it waits for and until when as for any call; deciding with the time as an input was turned down, since a decision could then not be taken again from the records alone.

### decision an-unread-return-is-sent-back
A return that could not be read is sent back to the agent in its session, as any return that does not meet the form, and counts against the run's limit of returns sent back.
Why: The agent's work is done and only its hand-in is wrong, so running the round from its start would pay for it twice. Repairing a text that is no JSON, in the engine or by a cheap model, was turned down: the agent that wrote it holds in its session what it meant, while a repair guesses at it (judgment, not measured).

### decision a-flagged-round-begins-a-new-session
A round the service's safety check flagged, or the run guard stopped, is given again at once in a new session of its line, whose name is worked out from how many of the line's rounds failed so; failed so again in the same run, with only failures between, the work is paused.
Why: The same session would give the check, or the guard, the same context again, more likely the cause than the task's words, though that is a judgment no case has shown; a second time says what the agent is given needs a look.

### decision a-note-run-is-held-only-by-its-own-pause
While a work is paused, a run that answers the owner, a note a person wrote to its cell or the owner turning to it, has its round after a failure held only where the engine paused the work since that round began, for its failure or for the budget it took the work over, which its carrier says before the round's end.
Why: A failure that is tried again by itself, or waits with every work, is no cause the owner must see first, and holding it would leave the note unanswered for as long as the owner's own pause lasts.
