-- Replays every session the engine recorded through the hand-over rule (crates/sessions/src/handover.rs), from what
-- each round's usage record keeps: the prompt of its first call and of its last, its turns, from which its calls are
-- counted, and the prompts of its calls in total. The calls between are put on the quadratic through the first and the
-- last prompt whose sum is the total. At each call the replay adds the rent; once the rent comes to the buy, the
-- session starts anew after that call, and from then on each of its calls holds what it added past that point on top
-- of a new opening. A session due at its last call never starts anew, as nothing follows.
--
-- Where a session starts anew decides what it costs (`rule` below). Inside a round, or before the next round of the
-- same cell and work, the agent hands its progress over: the new session opens with the session's first prompt and the
-- hand-over's tokens, and the hand-over costs the call that writes it (reading the context, writing the hand-over as
-- output) and writing to the cache again (`rebuild` below). Before a round of another cell or another work, whose
-- input is whole, the new session needs no hand-over: it opens with the session's first prompt, and costs writing to
-- the cache that opening, or, to bound what the new cell reads again of what the old session held, the whole context
-- (`restart` below).
--
-- Net saving = the reads saved - what starting anew cost.
--
-- One row for each session that starts anew, as `session`; the totals of the sessions replayed, as `total`; where
-- their first new sessions came, as `first_handover`; and the totals of those left out, as `left_out`. Read-only; run
-- on the engine's database. The README beside it gives the results.
WITH RECURSIVE
-- The tokens the call that hands over writes, which the new session opens with on top of the session's first prompt;
-- how a round's calls are counted (`sized` below): 'turns' or 'linear'; what a hand-over writes to the cache: 'whole',
-- the whole context again, the most it can; 'opening', only what it opens with, the least; and the rule: 'any', the
-- buy is the whole context at every call, wherever the session starts anew; 'boundary', the buy before a round of
-- another cell or work is the new opening, and elsewhere the whole context; 'between', as 'boundary', but a session
-- starts anew only between rounds; 'round', as 'any', but the rent counts only the calls of the round under way, so that
-- a session comes due only in a round that has itself paid the buy; and what a session started anew without a hand-over writes to the cache: 'opening',
-- its new opening; 'whole', the whole context, as much as a hand-over at the most.
given(handover, counted, rebuild, rule, restart) AS (VALUES (3000, 'turns', 'whole', 'round', 'whole')),
-- What a million tokens cost each model, in US dollars, as crates/sessions/src/cost.rs has it: a read from the cache,
-- a write to it kept five minutes or an hour, and output; a call of Haiku 5.5 over 100,000 tokens at its higher prices.
prices(model, above, read, w5, w1, output) AS (VALUES
  ('claude-fable-5-1', 0, 0.25, 12.5, 20.0, 50.0),
  ('claude-opus-5-5', 0, 0.2, 5.0, 8.0, 20.0),
  ('claude-sonnet-5-5', 0, 0.1, 2.5, 4.0, 10.0),
  ('claude-haiku-5-5', 0, 0.01, 0.125, 0.2, 0.5),
  ('claude-haiku-5-5', 100000, 0.05, 0.625, 1.0, 2.5)),
-- Each agent's round that ran in a session, as its usage record has it; a model named with its context window, as
-- `[1m]`, is priced as the model. A session is a row of the sessions table, which agents cannot read: an architect's
-- CLI session, which goes on across its works with a row for each, is replayed work by work.
told AS MATERIALIZED (
  SELECT r.session AS s, r.work, r.n, r.cell, r.started_at,
    CASE WHEN instr(r.model, '[') > 0 THEN substr(r.model, 1, instr(r.model, '[') - 1) ELSE r.model END AS model,
    coalesce(json_extract(r.usage, '$.turns'), 1) AS turns,
    coalesce(json_extract(r.usage, '$.first_call.input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.first_call.cache_read_input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.first_call.cache_creation_input_tokens'), 0) AS opening,
    coalesce(json_extract(r.usage, '$.usage.iterations[#-1].input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.iterations[#-1].cache_read_input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.iterations[#-1].cache_creation_input_tokens'), 0) AS closing,
    coalesce(json_extract(r.usage, '$.usage.input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.cache_read_input_tokens'), 0)
      + coalesce(json_extract(r.usage, '$.usage.cache_creation_input_tokens'), 0) AS total,
    coalesce(json_extract(r.usage, '$.usage.cache_creation.ephemeral_1h_input_tokens'), 0) > 0 AS hour,
    coalesce(json_extract(r.usage, '$.cost_usd'), 0) AS cost
  FROM rounds r
  WHERE r.cell IS NOT NULL AND r.session IS NOT NULL AND r.usage IS NOT NULL
),
-- The sessions replayed: those whose every round that used tokens keeps its first and last prompt, on a model with
-- prices. A session with a round that does not is left out whole, since its rent cannot be told.
whole AS MATERIALIZED (
  SELECT s FROM told GROUP BY s
  HAVING min(CASE WHEN total = 0 THEN 1
                  WHEN opening > 0 AND closing > 0 AND model IN (SELECT model FROM prices) THEN 1
                  ELSE 0 END) = 1
    AND max(total) > 0
),
-- How many calls each round replayed made. The record's turns are not its calls: a round of 2 turns made one call,
-- and a round of 3 or 4 turns one or more. A round whose total is less than its last prompt and its least made one
-- call; else at least two, and no more than its total over the least of its first and last prompt, since a call holds
-- at least that much. Within those bounds, `counted` says: `turns`, its turns less the one the return takes; `linear`,
-- as many as its total over the mean of its first and last prompt.
sized AS MATERIALIZED (
  SELECT t.*,
    CASE WHEN t.total < t.closing + min(t.opening, t.closing) THEN 1
      ELSE max(2, min(CASE g.counted WHEN 'turns' THEN t.turns - 1
                        ELSE CAST(round(2.0 * t.total / (t.opening + t.closing)) AS INTEGER) END,
                      t.total / min(t.opening, t.closing)))
    END AS calls
  FROM told t JOIN whole USING (s) JOIN given g
  WHERE t.total > 0
),
-- The rounds replayed, in the order they ran in their session, each with the number of the session's calls before it,
-- and the cell and work of the round after it in the session, if any.
ran AS MATERIALIZED (
  SELECT t.*, sum(calls) OVER w - calls AS before, max(hour) OVER (PARTITION BY t.s) AS hour_s,
    lead(t.cell) OVER w AS next_cell, lead(t.work) OVER w AS next_work
  FROM sized t
  WINDOW w AS (PARTITION BY t.s ORDER BY started_at, work, n ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
),
nums(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM nums WHERE i + 1 < (SELECT max(calls) FROM ran)),
-- Each call of each round, numbered across its session from 1 (i) and in its round from 1 (j): its prompt on the
-- quadratic through the round's first and last prompt whose sum over the round's calls is the round's total, kept
-- between the least and the most of those two, as a round's context neither falls below where it started or ended nor
-- rises above. For t = j-1 over calls-1, the prompt is first + (last - first) t + k t (1 - t), and the sum of t (1 - t)
-- over the calls is calls (calls - 2) / (6 (calls - 1)). `may`: the session may start anew after this call, by the
-- rule; `cheap`: if it does, it needs no hand-over, as the next round is of another cell or work.
calls AS MATERIALIZED (
  SELECT r.s, r.before + x.i + 1 AS i, r.work, r.n, x.i + 1 AS j, r.model, r.hour_s AS hour,
    max(min(CASE
      WHEN r.calls = 1 THEN r.closing
      WHEN r.calls = 2 THEN CASE x.i WHEN 0 THEN r.opening ELSE r.closing END
      ELSE r.opening + (r.closing - r.opening) * (x.i * 1.0 / (r.calls - 1))
        + (r.total - r.calls * (r.opening + r.closing) / 2.0) * 6.0 * (r.calls - 1) / (r.calls * (r.calls - 2.0))
          * (x.i * 1.0 / (r.calls - 1)) * (1 - x.i * 1.0 / (r.calls - 1))
    END, max(r.opening, r.closing)), min(r.opening, r.closing)) AS p,
    (g.rule != 'between' OR (x.i + 1 = r.calls AND r.next_cell IS NOT NULL)) AS may,
    (g.rule IN ('boundary', 'between') AND x.i + 1 = r.calls AND r.next_cell IS NOT NULL
      AND (r.next_cell != r.cell OR r.next_work != r.work)) AS cheap
  FROM ran r JOIN nums x ON x.i < r.calls JOIN given g
),
-- The rule, call by call; a row holds the session as it stands after call i, and the next row starts anew first when
-- this one came due: its rent past 0 and at least its buy, the write price times the whole context (e), or times the
-- new opening (o0) when `cheap`. o0 is the session's first prompt; d how many tokens less each call holds than it did,
-- by the new sessions so far; o what the replayed session opened with; e what call i holds once replayed, and rd, wr
-- and outp its prices; rent what the replayed session has paid since it opened; saved the reads saved; paid what
-- starting anew cost; handed how many times it started anew, and cheap of those without a hand-over; at where each
-- came: R<work>-<n>#<the call of the round>, with ~ when without a hand-over; due1 the call of the session the first
-- came after.
step(s, i, work, n, j, may, cheap, o0, d, o, e, rd, wr, outp, rent, saved, paid, handed, cheapn, at, due1) AS (
  SELECT c.s, c.i, c.work, c.n, c.j, c.may, c.cheap, c.p, 0.0, c.p, c.p, pr.read,
    CASE WHEN c.hour THEN pr.w1 ELSE pr.w5 END, pr.output, 0.0, 0.0, 0.0, 0, 0, '', NULL
  FROM calls c
  JOIN prices pr ON pr.model = c.model
    AND pr.above = CASE WHEN c.model = 'claude-haiku-5-5' AND c.p > 100000 THEN 100000 ELSE 0 END
  WHERE c.i = 1
  UNION ALL
  SELECT c.s, c.i, c.work, c.n, c.j, c.may, c.cheap, st.o0,
    -- d, after starting anew when the call before came due.
    st.d + (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
      * max(st.e - st.o0 - (1 - st.cheap) * g.handover, 0),
    -- o, likewise.
    CASE WHEN st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
      THEN st.o0 + (1 - st.cheap) * g.handover ELSE st.o END,
    -- e: what the call added past the new sessions so far, never more than it held.
    max(c.p - (st.d + (st.rent > 0 AND st.may
                       AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
                 * max(st.e - st.o0 - (1 - st.cheap) * g.handover, 0)),
        min(c.p, CASE WHEN st.rent > 0 AND st.may
                       AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                      THEN st.o0 + (1 - st.cheap) * g.handover ELSE st.o END)),
    pr.read, CASE WHEN c.hour THEN pr.w1 ELSE pr.w5 END, pr.output,
    -- rent: what this call reads again past the opening, after what was paid since the session opened.
    (1 - (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6))
      * (g.rule != 'round' OR c.j > 1) * st.rent
      + max(max(c.p - (st.d + (st.rent > 0 AND st.may
                               AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
                         * max(st.e - st.o0 - (1 - st.cheap) * g.handover, 0)),
                min(c.p, CASE WHEN st.rent > 0 AND st.may
                               AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                              THEN st.o0 + (1 - st.cheap) * g.handover ELSE st.o END))
            - CASE WHEN st.rent > 0 AND st.may
                    AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                   THEN st.o0 + (1 - st.cheap) * g.handover ELSE st.o END, 0)
        * pr.read / 1e6,
    -- saved: what this call no longer reads.
    st.saved + (c.p - max(c.p - (st.d + (st.rent > 0 AND st.may
                                         AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
                                   * max(st.e - st.o0 - (1 - st.cheap) * g.handover, 0)),
                          min(c.p, CASE WHEN st.rent > 0 AND st.may
                                         AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                                        THEN st.o0 + (1 - st.cheap) * g.handover ELSE st.o END))) * pr.read / 1e6,
    -- paid: a session started without a hand-over writes its new opening; a hand-over writes the context again, the
    -- whole or only the new opening, and its call reads it and writes the hand-over.
    st.paid + (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
      * (CASE WHEN st.cheap THEN (CASE g.restart WHEN 'whole' THEN st.e ELSE st.o0 END) * st.wr
              ELSE (CASE g.rebuild WHEN 'whole' THEN st.e ELSE st.o0 + g.handover END) * st.wr + st.e * st.rd
                   + g.handover * st.outp END) / 1e6,
    st.handed + (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6),
    st.cheapn + st.cheap
      * (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6),
    st.at || CASE WHEN st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                  THEN ' ' || CASE WHEN st.cheap THEN '~' ELSE '' END || 'R' || st.work || '-' || st.n || '#' || st.j
                  ELSE '' END,
    coalesce(st.due1, CASE WHEN st.rent > 0 AND st.may
                            AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6
                           THEN st.i END)
  FROM step st
  JOIN calls c ON c.s = st.s AND c.i = st.i + 1
  JOIN given g
  JOIN prices pr ON pr.model = c.model
    AND pr.above = CASE WHEN c.model = 'claude-haiku-5-5'
      AND c.p - (st.d + (st.rent > 0 AND st.may AND st.rent >= (CASE WHEN st.cheap THEN st.o0 ELSE st.e END) * st.wr / 1e6)
                        * max(st.e - st.o0 - (1 - st.cheap) * g.handover, 0)) > 100000
      THEN 100000 ELSE 0 END
),
-- Each session replayed, as it stands after its last call, with its rounds as recorded.
per AS (
  SELECT st.s, st.handed, st.cheapn, trim(st.at) AS at, st.due1, st.saved, st.paid,
    (SELECT 'R' || r.work || '-' || r.n || ' ' || r.cell FROM ran r WHERE r.s = st.s
     ORDER BY r.started_at, r.work, r.n LIMIT 1) AS first,
    (SELECT count(*) FROM ran r WHERE r.s = st.s) AS rounds,
    (SELECT sum(r.calls) FROM ran r WHERE r.s = st.s) AS calls,
    (SELECT sum(r.cost) FROM ran r WHERE r.s = st.s) AS cost
  FROM step st
  JOIN (SELECT s, max(i) AS last FROM calls GROUP BY s) l ON l.s = st.s AND l.last = st.i
)
SELECT * FROM (
  SELECT 'session' AS kind, first, rounds, calls, round(cost, 2) AS cost, handed, 'call ' || due1 || ': ' || at AS at,
    round(saved, 2) AS saved, round(paid, 2) AS paid, round(saved - paid, 2) AS net
  FROM per WHERE handed > 0
  UNION ALL
  SELECT 'total', count(*) || ' sessions, ' || sum(handed > 0) || ' start anew', sum(rounds), sum(calls),
    round(sum(cost), 2), sum(handed), sum(cheapn) || ' without a hand-over, ' || sum(saved - paid < 0)
      || ' with a net loss',
    round(sum(saved), 2), round(sum(paid), 2), round(sum(saved - paid), 2)
  FROM per
  UNION ALL
  -- The call of its session each first new session came after: the least, the median and the most.
  SELECT 'first_handover', count(*) || ' sessions', min(due1),
    (SELECT due1 FROM per WHERE handed > 0 ORDER BY due1 LIMIT 1 OFFSET (SELECT (count(*) - 1) / 2 FROM per
                                                                         WHERE handed > 0)),
    NULL, max(due1), 'least, median and most of the session call the first new session came after', NULL, NULL, NULL
  FROM per WHERE handed > 0
  UNION ALL
  SELECT 'left_out', count(DISTINCT s) || ' sessions', count(*), sum(turns), round(sum(cost), 2), NULL,
    'a round that used tokens has no first or last prompt, or a model with no prices', NULL, NULL, NULL
  FROM told WHERE s NOT IN (SELECT s FROM whole)
)
ORDER BY kind = 'session', kind, net, first;
