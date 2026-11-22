# What a round of the implementation cell (I1) takes and why, measured on 2026-11-22

Item #29. Whom it serves: the owner, who decides whether I1's rounds should be made shorter or cheaper, and the
architect, who decides what to change if so. It is kept until the numbers are measured again after a change to the
cell's effort, model or compaction number. It measures and changes no engine or flow file.

Files, in `research/`: this report, and `i1-round-cost-measured.sql` with the queries on the engine's records. The tool
counts, error counts, timings and compaction records below were read from the round streams in the data directory
(`rounds/<work>/<run>-<round>.stream.jsonl`), which the store does not hold. Each claim says how it is known:
(measured) on the data of the date above, (read) from a record, (inferred) from measurements, not tried.

## 1. The answer

The big rounds the item names are the agent using its tools in the ordinary way, in proportion to a change that was
itself large. Nothing in them is a loop, a repeat or a wrong turn that a rule would remove. What they cost is set by
four things: how many calls the round makes, how large the context is at each call, how many tokens the model writes,
and which model writes them. Each of the four is a choice already made elsewhere (the level, the effort, the
compaction number, the size of the item), not a fault of the round.

| Question | Answer |
|---|---|
| Where is the money | I1 is 59% of all rounds' cost: 262 rounds, $259 of $440 (measured, Q2). Of the $234 in rounds that have all three records, cache reads are 56%, cache writes 23%, output 17%, the rest 5% (measured, Q1 and Q4). 14 rounds of 40 calls or more cost $149 (64%); the 98 rounds of one or two calls (a verdict read, a point answered) cost $10.7 in all (4.6%). |
| Where is the time | In W11 run 17 (18.8 min) the tools took 19 s; in W21 run 40 (69.8 min) 31 s; in W21 run 57 (39.2 min) 78 s. The rest is the model writing: 151,205 output tokens in 1,127 s is 134 tokens a second on Sonnet 5.5, and 393,124 in 4,189 s is 94 a second on Opus 5.5 (measured, from the stream's timestamps). A round's time is its output tokens over the model's speed. |
| What the model writes | Tool inputs, thinking and a few lines of text. In run 17 the 237 Edit calls carry 188 KB of `old_string` and `new_string`; each is one hunk, so about four changed lines to an edit. The thinking is 20% of run 17's output tokens (29,845) and 44% of run 40's (173,027). |
| Why the context is large | Run 17 starts at 189,464 tokens, which R0, R1 and R2 put in the session before I1 runs, and adds 358,950 over 163 calls, 2.2 K a call: 484 KB of tool results, 188 KB of edits, thinking. Each of the 163 calls reads the whole context from the cache. The 189 K it started with alone is $3.09 of the round's $9.01 (34%). Run 40 starts at 181,083 and was compacted twice. |
| Is it a bad use of tools | No exact repeat of a read of an unchanged file in runs 17, 40, 43, 57, 78, 113 or 131. Error results are 14 of 410 calls in run 17 (10 Bash, 3 Edit, 1 Read) and 9 of 705 in run 40 (all Bash, mostly a command the permission rules refuse); each costs one call. |
| The size of the changes | Run 17's commit changed 39 files, +256 and −744 lines, with 410 tool calls: $9.01 for about 1,000 changed lines. W21's whole change is 100 files, +5,840 and −645, and its 43 I1 rounds cost $146.52 (Opus): $2.26 for 100 changed lines, against $0.90 for run 17's (Sonnet), about the ratio of the two models' prices. |

## 2. How it was measured

- **Prices** (measured, Q4): the program's cost for a round, fitted on the round's tokens, is exact to under 1%
  (median error 0.1% on 301 Sonnet rounds, 0.65% on 246 Opus rounds) at these prices per million tokens: Sonnet 5.5 reads
  $0.10, writes $4.01, outputs $10.02; Opus 5.5 reads $0.20, writes $8.02, outputs $20.81. Opus is twice Sonnet. 77 of
  I1's rounds ran on Opus and cost $191.6; 154 ran on Sonnet and cost $52.3. W20 and W21 are items at level 3.
- **Round cost is the session's total cost less its total at the round before** (read: the differences of the result
  events' `total_cost_usd` in W21's session `d9f9eacd` equal the recorded costs). A compaction's own call is not in the
  stream's usage. For run 40, with two compactions, the recorded cost exceeds the cost of its tokens by about $1.2, so a
  compaction costs at most about $0.6 on Opus (inferred). For run 57, with one, it exceeds it by $10.4, which is not
  explained (section 5).
- **A round's time** is the last stream timestamp less the first; **tool time** is the sum of the gaps from an assistant
  message to the next result.

## 3. The compactions

The program compacts a session by itself at the installation's `sessions.compact_at`, 500,000 in `install.toml` today
(read). The three compactions read in the streams (two in W21 run 40 and one in run 57 begin at 465,760, 467,516 and
468,009 tokens) took 87, 71 and 85 s and left 16.9 K, 12.4 K and 13.2 K tokens (measured). The engine records 10 compactions
in I1's rounds, 8 of them in W21. After one, the agent reads again the files it had seen: in run 40, 48 reads
(296 KB) after the first and 48 (195 KB) after the second, W21 runs 43, 57, 78 and 113 4 to 12 reads (15 to 49 KB),
at most about 90 K tokens a time, far fewer than the 466 K a
call carried before.

Whether a lower number would pay is a model, not a measurement (inferred): with a call's context growing 2.2 K, a
compaction costing about $0.5 and the cache read at $0.10 per million, the cost of a long round is least where the
session is compacted every 67 calls or so (a context near 230 K), about 30% cheaper than compacting at 500 K; but each
compaction adds about 85 s, 18% of the time of 67 calls at run 17's 6.9 s a call, and the summary loses what the
agent had in view. It was not tried.

## 4. What would bring the numbers down, and what each costs

None of these is done here. Each is a choice the design tree or the owner holds, and the report gives the figure each
turns on.

| Lever | What it would change | What it costs |
|---|---|---|
| Effort of I1 from xhigh to high (decision the-research-and-the-code-reviews-think-high) | Thinking is 20 to 44% of the output tokens, so of the time and the output cost. | Not measured: whether the changes are as good. An experiment on the same items at both efforts is needed (principle measure-speed-and-cost). |
| A lower `sessions.compact_at` | Section 3. | 85 s a compaction; what the summary drops; the owner's number (decision a-session-is-compacted-at-the-installations-number). |
| A smaller item | The context a call carries is the session's, and grows with the round; two items of half the size read a smaller context at each call. | The architect's choice of how to cut the work. |
| Sonnet for what is now Opus | Half the price per token. | The level is the architect's judgment of the item's difficulty (decision a-level-is-a-difficulty). |

## 5. Not explained

W21 run 57's recorded cost, $24.66, is $10.4 above what its tokens cost ($14.3 at Opus prices); the two other compacted rounds
it was compared with (runs 40 and 43) are within $1.2. Its stream shows nothing else: one compaction, no model but Opus and one
Haiku search. The cause was not found; the engine's records say how much a round cost, not what the calls outside the
round's own usage were.
