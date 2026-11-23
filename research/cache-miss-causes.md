# What causes a round to miss its session's prompt cache

Measured on the installation's store and the kept streams of work 23 (item 23) and work 35 (item 35), at the time of
item #94. It backs decision `a-miss-is-told-from-what-the-program-gave-the-model` and the decision on compaction.

All pairs below are two consecutive calls of one work with the same system prompt, form of return and model, which
approximates one line's session, less than an hour apart (the cache's life), the earlier one's `spent.context_tokens`
being what the session held. `shrunk` means the later first prompt (`first_context`) is smaller than what the session
held; `low read` means its first read was under half of what the session held.

## Question 1: does a change of effort alone make the next call miss the cache?

Comparison: pairs whose `before` parts differ by effort only, against pairs whose parts do not differ. Rejecting
result: effort-only pairs with a low read that nothing else explains.

| effort | pairs | low read | low read, not shrunk | shrunk | shrunk with a compaction recorded in the round |
|---|---|---|---|---|---|
| changed (high, xhigh) | 207 | 5 | 1 | 5 | 2 |
| same | 1291 | 7 | 1 | 6 | 5 |

The one effort-only low read that is not shrunk is work 23 run 15, explained below by the program's tool list (its
`before.tools`, a digest of the engine's own arguments, was equal). The four shrunk are compactions (below). So no
effort-only pair misses for effort: effort is not a cause of a miss, for the efforts the flows use (high, xhigh).
Example: work 5 run 34 changed effort and read 145125 of the 145129 tokens held.

## Question 2: what did the program change in work 23 run 15?

The round read 0 of 89372 tokens at its first call, so even the start of the prompt was not read: what comes first in
the prompt, the tools, changed. The engine named `effort`, the only part of `before` that differed, because it
compares only what it gave the program. The init line of each round's stream (`rounds/23/<run>-1.stream.jsonl`) says
what the program gave the model:

| run | tools | agents |
|---|---|---|
| 13 | Task, Bash, Edit, Glob, Grep, Read, StructuredOutput, WebFetch, WebSearch, Write | claude, Explore, general-purpose, Plan, statusline-setup |
| 14 | the same | the same |
| 15 | the same without WebFetch | the same and web-fetch |

Across the 65 kept init lines of work 23, the keys that differ between consecutive ones are `tools` (22 times, between
lines with different permissions), `model` (12), `agents` (1, this change), and `uuid`, `session_id` and
`messaging_socket_path`, which name the process.

## Question 3: what is a round whose first prompt is smaller than what its session held?

Work 35 run 36: its stream has a `compact_boundary` event (auto, 475145 tokens before, 16231 after) before its first
call to the model; the first call's prompt was 49462 tokens, against the 461234 the session held after run 34, and it
read 24668, the system prompt and tools, which no compaction changes. The round's `spent.compactions` is 1.

All eleven shrunk pairs of the same line within the cache's life (of 1541 pairs):

| work, run | held | first prompt | first read | compactions recorded in the round |
|---|---|---|---|---|
| 11, 24 | 614294 | 31676 | 23720 | 0 |
| 11, 30 | 103764 | 45429 | 22087 | 0 |
| 20, 57 | 808765 | 50260 | 23879 | 0 |
| 21, 33 | 465323 | 35033 | 23360 | 1 |
| 21, 43 | 442995 | 71510 | 23850 | 1 |
| 21, 46 | 444997 | 78595 | 23850 | 1 |
| 31, 48 | 465423 | 44799 | 24524 | 1 |
| 35, 36 | 461234 | 49462 | 24668 | 1 |
| 37, 21 | 465082 | 50948 | 24524 | 1 |
| 74, 29 | 453144 | 48164 | 24668 | 1 |
| 46, 11 | 266219 | 212220 | 207524 | 0 (the round before compacted; read nearly all, no miss) |

Seven of the eleven have a compaction recorded in the round. The three with none (works 11 and 20) have no recorded
cause: the first two held 614k and 808k, probably compactions the engine did not yet mark, and run 30 of work 11 is not
known. A round that compacted but whose prompt was not smaller than what was held is judged as before: the compaction
came after its first call. Calls that look like shrinks in a loose grouping (different lines sharing a system prompt, a call ten hours after the
one before, as work 4 run 17) are another session or past the cache's life and are not judged.
