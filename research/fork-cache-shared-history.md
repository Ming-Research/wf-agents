# Why a second fork of a round reads none of the history from the cache, measured on 2026-11-22

Item #33. Whom it serves: the owner and the engineer of the item, who decide how forks are asked. It is kept until a
measurement of the engine's own forks after the change replaces it. The script is `fork-cache-shared-history.py`, run
once for each of its three phases on the agents' program 2.1.327 (`claude`), models `haiku` and `sonnet`, in an empty
directory, with no tools. This is an exploration: no result that would reject the proposal was written before the run.
Each claim says how it is known: (read) from a record or a file, (measured) by the script, (inferred) not tried.

## 1. The question

The item says that the second to fourth fork of one round read only the system prompt from the cache (21%) and each
cost $0.36–0.48. It does not know whether the fork's request begins unlike the one before, or whether the cache lives
only minutes. Which is it, and what makes a later fork read what the first wrote?

## 2. What the engine's own forks show (read)

The first request of each of the 26 forks kept in `rounds/11` and `rounds/21` of the data directory (the stream files
beside the rounds), read from each stream's first assistant message:

- Every fork after the first of its round (17 of them, rounds 3, 13, 21, 23 and 26 of work 11) wrote 83,812 to
  601,608 tokens; 16 read only what the system prompt and tools hold (22,087 or 23,720 tokens, 21% in the item's
  example) and one, twelve hours after the one before, read nothing. The gaps between the others were from the same
  minute to hours: those a minute or two apart (work 11 run 13, forks 2 to 4) read the same as those hours apart, so
  the cache's life is not the cause for them.
- The first fork of a round read nothing (6 of 9 rounds) or only the system part, and wrote the rest. Whether the
  cache had let the session go is not read here (inferred from the hours between the round's end and the fork).
- Within one fork, the second call of the fork read all that the first wrote (fork 4 of run 26: 106,408 read of 106,408
  written before), so the cache works inside a fork.

## 3. What the script measures

Phase 1 (measured): a session of about 21,000 tokens begun on `haiku`.

| Call | Model | Read | Written |
|---|---|---|---|
| W1, W2: a fork of the session, its cache alive | haiku | 21,584 | 497 |
| C1: a fork, cold (the other model has no cache of it) | sonnet | 658 | 21,456 |
| C2: a second fork, right after, cold | sonnet | 3,182 | 18,932 |
| P: a message in the session itself, cold | sonnet | 3,182 | 18,922 |
| A1, A2: a fork of the session after P | sonnet | 22,046 | 75 |

A fork of a session whose cache is alive reads the whole history (W1, W2). A cold session's second fork writes the
history again (C2, 88% of C1's write), as the engine's forks do. A session that was sent one message of its own
beforehand is read whole by every fork after (A1, A2).

Phase 2 (measured): the same with a session that went on past the point the fork begins at, a fork being begun at that
point with `--resume-session-at`: B1 read 658 and wrote 21,462; B2 read 3,184 and wrote 18,936 (the same miss). A first
fork of the point that sends only `ping` (F0) wrote 18,926, and two forks of F0 each read 22,052 and wrote 75.

Phase 3 (measured): a fork given two messages on its input, a fixed first message and then its question, on a cold
session (stream-json input, both lines sent at once):

| Fork | Request | Read | Written |
|---|---|---|---|
| X1 | its first message | 3,483 | 22,276 |
| X1 | its question | 25,701 | 75 |
| X2 | its first message | 25,759 | 0 |
| X2 | its question | 25,701 | 75 |
| X3 | its first message | 25,701 | 130 |
| X3 | its question | 25,701 | 895 |

The first message of every fork is the same request, so what X1 wrote the next forks read whole; the question, which
differs, is the only part each writes. Total cost of the call (the program's `total_cost_usd`, which counts all
requests of the call): X1 $0.0970, X2 $0.0101, X3 $0.0139; the cold forks of phase 1 on the same model cost $0.0903
and $0.0813.

Also measured, on `haiku` with no history: the program's result for each message of a call with two messages gives the
tokens of that message's requests only, and its `total_cost_usd` grows over the call (0.00064 after the first
message, 0.00153 after the second).

## 4. The cause, and what it leaves open

The program puts its cache breakpoint at the end of the last message of a request, and on the system prompt. A fork's
request ends with its question, which differs from fork to fork, so the history before the question is nowhere a
breakpoint that an earlier fork wrote; only what the first fork wrote up to its own question stays unread by the next
(measured, phase 1 and phase 3 against the engine's own forks, section 2). The cache's life is no cause for forks
minutes apart (read, section 2), though it is why the first fork of a round long over writes at all.

Not tried: the engine's real command line with its form of return, its hook that refuses tools, and a session of
100,000 tokens or more on the models the works run on; whether the first message's answer, given under the form of
return, leaves the answer to the question unchanged; and the extra time of the first message.
