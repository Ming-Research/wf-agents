# Replaying the recorded sessions through the hand-over rule

For #236: does handing a session over once its rent comes to its buy (crates/sessions/src/handover.rs) save money on
the sessions the engine has run? Delete this directory once the hand-over records that rounds keep
(`$.engine.handover`, see db/schema.sql) cover enough real hand-overs to answer the same question from what happened.

## Question and criterion

Fixed before measuring, as acceptance A9 of #236 states it: replay every session from what each round's usage record
keeps, and sum the net saving, which is the reads saved less, for each hand-over, writing the whole context to the
cache again and the call that writes the hand-over. The replay passes if the sum over all sessions replayed is above
$0; every session with a net loss is listed.

## Method

[replay.sql](replay.sql), one read-only query on the engine's database, deterministic. What it assumes:

- A session is a row of the sessions table, by `rounds.session`. An architect's CLI session goes on across its works
  with a row for each work, so it is replayed work by work: a hand-over near the end of a work saves nothing in the
  works after it. The CLI session id that would join them is hidden from agents' queries.
- Each round's calls lie on the quadratic through its first and last prompt whose sum is its total prompt, kept between
  those two prompts. How many calls a round made is not recorded: `turns` overstates it (all 2,212 rounds of 2 turns
  made one call; rounds of 3 or 4 turns made one call or more). Two counts were run, both bounded by what the tokens
  allow: `turns`, the turns less one; `linear`, the total over the mean of the first and last prompt.
- A hand-over writes 3,000 tokens and the new session opens at the session's first prompt plus those. It costs reading
  the context once and writing the hand-over as output, plus writing to the cache again either the whole context
  (`whole`, the acceptance's cost, the most a new session can write) or only the new opening (`opening`, the least).
- Prices are those of crates/sessions/src/cost.rs; a session that wrote the cache for an hour writes the new one so.

## Results

Run on 2026-11-02 on the engine's database. Left out in every run: 24 sessions, 301 rounds, $180.30, whose rounds lack
a first or last prompt or ran on a model with no prices. The third run came a few rounds of this work later, so it
holds two sessions more.

| Calls counted | Rewritten | Sessions replayed | Spend replayed | Hand over | Hand-overs | Saved | Paid | Net | Sessions with a loss |
|---|---|---|---|---|---|---|---|---|---|
| turns | whole | 371 | $1,030.02 | 31 | 34 | $40.45 | $50.76 | **−$10.31** | 19 |
| linear | whole | 371 | $1,030.02 | 32 | 35 | $43.45 | $53.66 | **−$10.21** | 19 |
| turns | opening | 373 | $1,041.66 | 31 | 34 | $41.64 | $11.76 | **+$29.88** | 7 |

By the acceptance's cost the replay does not pass: −$10.31 and −$10.21, about 1% of the spend replayed. With the least
a new session writes it saves $29.88, about 3%. What a new session really writes lies between the two, and the
records of real hand-overs will tell it.

Three architect sessions, each replayed for one work, account for −$11.54 of the first run: they come due at their
work's last calls (76 to 78 of 78 to 83), where the work's row ends. Without them the first run nets +$1.23.

The call of its session at which each first hand-over came: with `turns`, least 76, median 121, most 188 (31
sessions); with `linear`, least 76, median 98, most 111 (32 sessions).

Sessions with a net loss in the first run (`turns`, `whole`), by their first round, the call each hand-over came after,
and net:

| Session | Hand-over | Net |
|---|---|---|
| R245-516 U1 | R245-702 call 1 | −$4.10 |
| R245-1123 U1 | R245-1283 call 1 | −$3.97 |
| R245-11 U1 | R245-172 call 1 | −$3.47 |
| R35-70 A4 | R35-80 call 3 | −$1.87 |
| R263-34 I1 | R263-38 call 11 | −$1.35 |
| R259-26 I1 | R259-36 call 5 | −$0.92 |
| R260-26 I1 | R260-67 call 6 | −$0.92 |
| R238-19 I1 | R238-21 call 8 | −$0.89 |
| R242-57 T1 | R242-114 call 2 | −$0.87 |
| R258-21 I1 | R258-23 call 4 | −$0.87 |
| R251-19 I1 | R251-31 call 6 | −$0.74 |
| R255-48 T6 | R255-116 call 6 | −$0.71 |
| R232-42 T6 | R232-81 call 14 | −$0.51 |
| R265-21 I1 | R265-23 call 8 | −$0.43 |
| R267-27 I1 | R267-29 call 6 | −$0.36 |
| R268-25 I1 | R268-25 call 146 | −$0.36 |
| R242-77 T6 | R242-106 call 26 | −$0.29 |
| R242-51 I1 | R242-51 call 120 | −$0.24 |
| R233-46 T6 | R233-50 call 6 | −$0.18 |

The twelve that gain gain $12.73 together; the most are R38-20 I1 (+$2.85), R233-19 I1 (+$2.25), R254-38 T6 (+$1.59),
R35-90 T6 (+$1.55) and R263-32 I1 (+$1.50). A session loses when it ends within a few dozen calls of its hand-over:
by the acceptance's cost a hand-over pays back only after about as many calls as the write price is times the read
price (40 on Sonnet 5.5 for an hour's cache).
