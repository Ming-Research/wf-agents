# Replaying the recorded sessions through the hand-over rule

For #236: does handing a session over once its rent comes to its buy (crates/sessions/src/handover.rs) save money on
the sessions the engine has run, and under which rule? Delete this directory once the hand-over records that rounds
keep (`$.engine.handover`, see db/schema.sql) cover enough real hand-overs to answer the same question from what
happened.

## Question and criterion

Fixed before measuring, as acceptance A9 of #236 states it: replay every session from what each round's usage record
keeps, and sum the net saving, which is the reads saved less, for each hand-over, writing the whole context to the
cache again and the call that writes the hand-over. The replay passes if the sum over all sessions replayed is above
$0; every session with a net loss is listed. The owner judges it on a replay that joins an architect's CLI session
across its works.

## Method

[replay.sql](replay.sql), one read-only query on the engine's database, deterministic. What it assumes:

- A session is a row of the sessions table, by `rounds.session`. An architect's CLI session goes on across its works
  with a row for each work, so this query replays it work by work; the CLI session id that would join them is hidden
  from agents' queries, and the joined replay is run by the owner's delegate.
- Each round's calls lie on the quadratic through its first and last prompt whose sum is its total prompt, kept between
  those two prompts. How many calls a round made is not recorded: `turns` overstates it (all 2,212 rounds of 2 turns
  made one call; rounds of 3 or 4 turns made one call or more). Two counts are run, both bounded by what the tokens
  allow: `turns`, the turns less one; `linear`, the total over the mean of the first and last prompt.
- A hand-over writes 3,000 tokens and the new session opens at the session's first prompt plus those. It costs reading
  the context once and writing the hand-over as output, plus writing to the cache again either the whole context
  (`whole`, the acceptance's cost, the most a new session can write) or only the new opening (`opening`, the least).
- Prices are those of crates/sessions/src/cost.rs; a session that wrote the cache for an hour writes the new one so.
- `rule` says where the rent is counted and what starting anew costs: `any`, the rent counts every call of the session
  (the first design); `round`, the rent counts only the calls of the round under way (the second design);
  `boundary` and `between`, a session starts anew without a hand-over before a round of another cell or work, at the
  price of its new opening (`restart` bounds that price by the whole context instead).

## Results

All runs on the engine's database as it stood during the second design, unjoined. Left out in every run: 24 sessions,
301 rounds, $180.30, whose rounds lack a first or last prompt or ran on a model with no prices.

| Rule | Calls counted | Rewritten | Start anew | Of them without a hand-over | Saved | Paid | Net | Sessions with a loss |
|---|---|---|---|---|---|---|---|---|
| any | turns | whole | 41 | 0 | $57.05 | $59.46 | **−$2.41** | 20 |
| round | turns | whole | 22 | 0 | $42.12 | $26.40 | **+$15.72** | 4 |
| round | linear | whole | 25 | 0 | $48.13 | $32.59 | **+$15.54** | 6 |
| boundary | turns | whole, opening at boundaries | 164 | 129 | $111.26 | $77.73 | +$33.53 | 57 |
| boundary | linear | whole, opening at boundaries | 171 | 135 | $115.71 | $81.91 | +$33.80 | 61 |
| boundary | turns | whole everywhere | 165 | 130 | $111.59 | $196.70 | −$85.11 | 117 |
| between | turns | whole, opening at boundaries | 153 | 127 | $81.62 | $77.16 | +$4.45 | 65 |

The owner's delegate ran the first design joined: −$11.84, 24 sessions with a loss, all the largest of them architect
sessions that came due at about their 70th call and ended soon after.

`any` loses because a session that came due over many rounds is asked at the start of its next round, which is often
short: by the acceptance's cost a hand-over pays back only after about as many calls as the write price is times the
read price (40 on Sonnet 5.5 for an hour's cache). `round` asks only a round that has itself read as much past the
opening as starting anew costs, which leaves out the short rounds and every architect round (none of them comes due:
their rounds make a few calls each). `boundary` gains only if a new cell reads little of what the old session held:
priced at the whole context it loses $85.11, so its gain rests on what no record tells.

Sessions with a net loss under `round` (`turns`, `whole`), by their first round, the call each hand-over came after,
and net:

| Session | Hand-over | Net |
|---|---|---|
| R271-19 I1 | R271-19 call 114 | −$0.76 |
| R242-51 I1 | R242-51 call 120 | −$0.24 |
| R268-25 I1 | R268-25 call 146 | −$0.10 |
| R262-38 T6 | R262-86 call 53 | −$0.09 |

The thirteen that gain gain $16.92 together; the most are R269-28 I1 (+$4.95), R38-20 I1 (+$2.57), R233-19 I1
(+$2.25), R263-32 I1 (+$1.50) and R267-54 I1 (+$1.21). The call of its session at which each first hand-over came:
least 92, median 146, most 188 (17 sessions).
