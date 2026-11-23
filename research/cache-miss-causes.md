# What causes a round to miss its session's prompt cache

Measured on the installation's store and the kept streams of work 23 (item 23) and work 35 (item 35), at the time of
item #94. It backs decision `a-miss-is-told-from-what-the-program-gave-the-model` and decision
`a-compacted-session-is-no-miss`.

## Question 1: does a change of effort alone make the next call miss the cache?

Comparison: each pair of consecutive first calls of one work and one system prompt, within five minutes (inside the
cache's life), grouped by which parts of `measured.before` differ. A call missed if its first read was under half of
the earlier call's context and its first prompt was not smaller than that context.

Rejecting result: a group of effort-only pairs in which calls miss.

| parts that differ | pairs | missed | first prompt smaller than the context held |
|---|---|---|---|
| none | 1016 | 1 | 1 |
| effort | 158 | 0 | 21 |
| model, schema | 17 | 0 | 14 |
| effort, model, schema | 4 | 0 | 2 |
| effort, schema | 2 | 0 | 2 |
| schema | 1 | 0 | 1 |

Effort-only changes (high to xhigh and back) read the whole earlier context, for example work 5 run 34 read 145125 of
145129. Result: effort is not a cause of a miss, for the efforts the flows use (high, xhigh). The one miss with no part
differing is work 23 run 15, below.

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

The arguments the engine gave the program were the same in runs 14 and 15 (`before.tools` is equal).

## Question 3: what is a round that read little after a compaction?

Work 35 run 36: its stream has a `compact_boundary` event (auto, 475145 tokens before, 16231 after) before its first
call to the model; the first call's prompt was 49462 tokens, against the 461234 the session held after run 34, and it
read 24668, the system prompt and tools, which no compaction changes. A resumed session's first prompt holds what the
session held plus the answer and the new message, so a first prompt smaller than the context held means the session was
cut down before the call.
