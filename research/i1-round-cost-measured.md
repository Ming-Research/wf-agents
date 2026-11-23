# What a round of the implementation cell (I1) takes and why, measured on 2026-11-22

Item #29. Whom it serves: the owner, who decides whether I1's rounds should be made shorter or cheaper, and the
architect, who decides what to change if so. It is kept until the numbers are measured again after a change to the
cell's effort, model or compaction number. It measures and changes no engine or flow file.

Files, in `research/`: this report; `i1-round-cost-measured.sql`, the queries on the engine's records; and
`i1-round-stream.py`, which measures one round's stream. The store grows, so every query takes only the rounds that
began at or before 2026-11-22 11:45:00 UTC (the cutoff the queries state, by each round's own start); the numbers
below are those at the cutoff. The tool counts, error counts, timings and compaction records were read from the round streams in the data
directory (`rounds/<work>/<run>-<round>.stream.jsonl`), which the store does not hold. Each claim says how it is known:
(measured) at the cutoff, (read) from a record, (inferred) from measurements, not tried.

## 1. The answer

The big rounds the item names are the agent using its tools in the ordinary way, in proportion to a change that was
itself large. Nothing in them is a loop, a repeat or a wrong turn that a rule would remove. What they cost is set by
four things: how many calls the round makes, how large the context is at each call, how many tokens the model writes,
and which model writes them. Each of the four is a choice already made elsewhere (the level, the effort, the
compaction number, the size of the item), not a fault of the round.

| Question | Answer |
|---|---|
| Where is the money | I1 is 58% of all rounds' cost: 262 rounds, $259.16 of $443.41 in 850 rounds (measured, Q2). Of the $233.56 in the 202 of its rounds that have a measured record, cache reads are 56%, cache writes 23%, output 17%, the rest 5% not priced by the three counts (Q1, Q4, Q5). By calls (Q5): 14 rounds of 40 calls or more cost $149.39 (64%); 90 of 3 to 39 calls cost $73.61 (32%); the 98 of one or two calls, a verdict read or a point answered, cost $10.56 (4.5%). |
| Where is the time | In W11 run 17 (18.8 min) the tools took 19 s; in W21 run 40 (69.8 min) 31 s; in W21 run 57 (39.2 min) 78 s (measured, from the stream's timestamps). The rest is the model: each call to it, plus the writing. Fitted on the streams of the cell's rounds of three calls or more, a round's time less its compactions is, on Sonnet 5.5, 0.64 s a call plus the output tokens at 142 a second (50 rounds, median error 11%); on Opus 5.5, 6.0 s a call plus the output tokens at 308 a second (55 rounds, median error 23%) (inferred, `i1-round-stream.py --fit`). Run 17 (151,205 output tokens, 163 calls) is 1,127 s, mostly writing; run 40 (393,124, 461 calls) is 4,190 s, of which the calls are about 2,800 s. |
| What the model writes | Tool inputs, thinking and a few lines of text. In run 17 the 237 Edit calls carry 188 KB of `old_string` and `new_string`; each is one hunk, so about four changed lines to an edit. The thinking is 20% of run 17's output tokens (29,845) and 44% of run 40's (173,027). |
| Why the context is large | Run 17 starts at 189,464 tokens, which R0, R1 and R2 put in the session before I1 runs, and adds 358,950 over 163 calls, 2.2 K a call: 484 KB of tool results, 188 KB of edits, thinking. Each of the 163 calls reads the whole context from the cache. The 189 K it started with alone is $3.09 of the round's $9.01 (34%). Run 40 starts at 181,083 and was compacted twice. |
| Is it a bad use of tools | No repeat of a read of an unchanged file in W11 run 17 or W21 runs 40, 43, 57, 78, 113 and 131. Error results are 14 of 410 calls in run 17 (10 Bash, 3 Edit, 1 Read) and 9 of 705 in run 40 (all Bash, mostly a command the permission rules refuse); each costs one call. |
| The size of the changes | The item's 315 tool calls and 182 edits for W11's round are the counts at the round's 315th tool call, at 03:26:42 UTC, 15 min 2 s into its 18.8 minutes: in the session transcript, the 315th call is the 182nd Edit (read). The round ended with 410 calls and 237 Edits, the same 77% of each; the stream and the transcript agree on 410 and 237. Run 17's commit changed 39 files, +256 and −744 lines, with 410 tool calls: $9.01 for about 1,000 changed lines. W21's whole change is 100 files, +5,840 and −645, and its 43 I1 rounds cost $146.52 (Opus): $2.26 for 100 changed lines, against $0.90 for run 17's (Sonnet), about the ratio of the two models' prices. |

## 2. How it was measured

- **Prices** (measured, Q4): the program's cost for a round, fitted on the round's tokens, is exact to under 1%
  (median error 0.13% on 300 Sonnet rounds, 0.65% on 246 Opus rounds, none compacted) at these prices per million
  tokens: Sonnet 5.5 reads $0.10, writes $4.01, outputs $10.02; Opus 5.5 reads $0.20, writes $8.02, outputs $20.81.
  Opus is twice Sonnet. Of I1's 262 rounds, 77 ran on Opus and cost $191.57, 154 on Sonnet and cost $52.34, and 31
  ($15.26) have no record of the model (Q3), so the 262 are not 77 and 154. All 77 Opus rounds are of W20 and W21, which
  are items at level 3; so the $146.52 of W21's I1 rounds in section 1 was spent on Opus (Q6).
- **Round cost is the session's total cost less its total at the round before** (read: the differences of the result
  events' `total_cost_usd` in W21's session `d9f9eacd` equal the recorded costs). This holds where no round or ping
  of the session was cut off or began a session again; the engine now records what each result says it spent since
  the result before it (`model_usage_since_last_result`), which does not depend on that. A compaction's own call is not in the
  stream's usage. For run 40, with two compactions, the recorded cost exceeds the cost of its tokens by about $1.2, so a
  compaction costs at most about $0.6 on Opus (inferred). For run 57, with one, it exceeds it by $10.4, which is not
  explained (section 5).
- **The stream measures** (`i1-round-stream.py`, on one stream file): a round's time is the last event timestamp less
  the first; tool time is, for each user event, its timestamp less that of the assistant event before it; calls to the
  model are the distinct assistant message ids; tool calls are the `tool_use` blocks, each once; an error result is a
  `tool_result` with `is_error`, counted under the tool its `tool_use_id` names. A repeated read is a Read of a file and
  range an earlier Read gave (an earlier Read of the whole file covers every range), with no Edit or Write of that
  file and no compaction in between. A reread after a compaction is a Read, after a `compact_boundary` event, of a path
  the round had already read, edited or written. Its numbers for runs 17, 40 and 57 are the ones in this report.

## 3. The compactions

The program compacts a session by itself at the installation's `sessions.compact_at`, 500,000 in `install.toml` today
(read). The seven compactions read in the streams (two in W21 run 40, one each in runs 43, 57, 78, 113 and 131) began at 461,182
to 470,146 tokens, took 56 to 87 s (71 to 87 s in all but run 43's) and left 8,088 to 29,933 tokens (measured). The
engine records 10 compactions in I1's rounds, 8 of them in W21, 1 in W23 and 1 in W12 (Q6). After one, the agent reads again files it had
already seen: in run 40, 48 reads (296 KB) after the first and 48 (195 KB) after the second; in W21 runs 43, 57, 78, 113
and 131, 12, 5, 6, 4 and 0 reads (49, 39, 15, 17 and 0 KB); at most about 90 K tokens a time, far fewer than the 466 K a
call carried before.

Whether a lower number would pay is a model, not a measurement (inferred). Take a call's context growing 2.2 K (run 17
and run 40 both: 358,950 over 163 calls, 1,024,529 over 461), 75 K left after a compaction (the summary and the files
read again), a compaction costing C and the cache read r a token; the cost a call is r times the average context plus C
over the calls between compactions, least at about 52 calls (a context near 190 K), which is 34% less than compacting at
500 K (178 calls between compactions). C and r are both twice as large on Opus as on Sonnet (C about $0.6 and $0.3, r
$0.20 and $0.10 per million), so the two models give the same 52 calls and 34%. Each compaction adds about 85 s: 18% of
the time of 52 calls on Opus (9.1 s a call in run 40), 24% on Sonnet (6.9 s in run 17); a compaction of a smaller context
would be quicker and cheaper than the 466 K measured, which the model does not take into account. The summary also loses
what the agent had in view. It was not tried.

## 4. What would bring the numbers down, and what each costs

None of these is done here. Each is a choice the design tree or the owner holds, and the report gives the figure each
turns on.

| Lever | What it would change | What it costs |
|---|---|---|
| Effort of I1 from xhigh to high (decision the-research-and-the-code-reviews-think-high) | Thinking is 20 to 44% of the output tokens, so of the time and the output cost. | Not measured: whether the changes are as good. An experiment on the same items at both efforts is needed (principle measure-speed-and-cost). |
| A lower `sessions.compact_at` | Section 3: about 34% of the cost of a long round, by the model. | Time (18% to 24%) and what the summary drops; the owner's number (decision a-session-is-compacted-at-the-installations-number). |
| Fewer, larger calls (several edits to a call) | On Opus each call is about 6 s whatever it holds, and each reads the whole context; run 40 made 705 tool calls in 461 calls to the model. | How an agent batches its tools is its own; the engine's rules do not direct it, and a sentence that does adds to every round's input (principle lean-agent-input). |
| A smaller item | The context a call carries is the session's, and grows with the round; two items of half the size read a smaller context at each call. | The architect's choice of how to cut the work. |
| Sonnet for what is now Opus | Half the price per token. | The level is the architect's judgment of the item's difficulty (decision a-level-is-a-difficulty). |

## 5. Not explained

W21 run 57's recorded cost, $24.66, is $10.4 above what its tokens cost ($14.3 at Opus prices); the two other compacted rounds
it was compared with (runs 40 and 43) are within $1.2. Its stream shows nothing else: one compaction, no model but Opus and one
Haiku search. The cause was not found; the engine's records say how much a round cost, not what the calls outside the
round's own usage were.
