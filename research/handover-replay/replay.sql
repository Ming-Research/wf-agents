-- Replays every CLI session the engine recorded through the hand-over rule (crates/sessions/src/handover.rs), from what
-- each round's usage record keeps: the prompt of its first call and of its last, how many calls it made and their
-- prompts in total. The calls between are put on the quadratic through the first and the last prompt whose sum is the
-- total. At each call the replay adds the rent; once the rent comes to the buy, the session hands over after that call,
-- and from then on each of its calls holds what it added past the hand-over on top of a new opening: the session's
-- first prompt and the hand-over's tokens. A session due at its last call never hands over, as nothing follows.
--
-- Net saving = the reads saved - for each hand-over, writing its whole context to the cache again and the call that
-- writes the hand-over (reading that context, and writing the hand-over as output).
--
-- One row for each session that hands over, as `session`, and the totals of the sessions replayed, as `total`, and of
-- those left out, as `left_out`. Read-only; run on the engine's database. The README beside it gives the result.
WITH RECURSIVE
-- The tokens the call that hands over writes, which the new session opens with on top of the session's first prompt.
given(handover) AS (VALUES (3000)),
-- What a million tokens cost each model, in US dollars, as crates/sessions/src/cost.rs has it: a read from the cache,
-- a write to it kept five minutes or an hour, and output; a call of Haiku 5.5 over 100,000 tokens at its higher prices.
prices(model, above, read, w5, w1, output) AS (VALUES
  ('claude-fable-5-1', 0, 0.25, 12.5, 20.0, 50.0),
  ('claude-opus-5-5', 0, 0.2, 5.0, 8.0, 20.0),
  ('claude-sonnet-5-5', 0, 0.1, 2.5, 4.0, 10.0),
  ('claude-haiku-5-5', 0, 0.01, 0.125, 0.2, 0.5),
  ('claude-haiku-5-5', 100000, 0.05, 0.625, 1.0, 2.5)),
-- Each agent's round that ran in a CLI session, as its usage record has it; a model named with its context window, as
-- `[1m]`, is priced as the model.
told AS MATERIALIZED (
  SELECT s.cli_session AS s, r.work, r.n, r.cell, r.started_at,
    CASE WHEN instr(r.model, '[') > 0 THEN substr(r.model, 1, instr(r.model, '[') - 1) ELSE r.model END AS model,
    max(coalesce(json_extract(r.usage, '$.turns'), 1), 1) AS calls,
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
  FROM rounds r JOIN sessions s ON s.id = r.session
  WHERE r.cell IS NOT NULL AND r.usage IS NOT NULL
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
-- The rounds replayed, in the order they ran in their session, each with the number of the session's calls before it.
ran AS MATERIALIZED (
  SELECT t.*, sum(calls) OVER w - calls AS before, max(hour) OVER (PARTITION BY t.s) AS hour_s
  FROM told t JOIN whole USING (s)
  WHERE t.total > 0
  WINDOW w AS (PARTITION BY t.s ORDER BY started_at, work, n ROWS UNBOUNDED PRECEDING)
),
nums(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM nums WHERE i + 1 < (SELECT max(calls) FROM ran)),
-- Each call of each round, numbered across its session from 1 (i) and in its round from 1 (j): its prompt on the
-- quadratic through the round's first and last prompt whose sum over the round's calls is the round's total, never
-- below 0. For t = j-1 over calls-1, the prompt is first + (last - first) t + k t (1 - t), and the sum of t (1 - t) over
-- the calls is calls (calls - 2) / (6 (calls - 1)).
calls AS MATERIALIZED (
  SELECT r.s, r.before + x.i + 1 AS i, r.work, r.n, x.i + 1 AS j, r.model, r.hour_s AS hour,
    max(CASE
      WHEN r.calls = 1 THEN r.closing
      WHEN r.calls = 2 THEN CASE x.i WHEN 0 THEN r.opening ELSE r.closing END
      ELSE r.opening + (r.closing - r.opening) * (x.i * 1.0 / (r.calls - 1))
        + (r.total - r.calls * (r.opening + r.closing) / 2.0) * 6.0 * (r.calls - 1) / (r.calls * (r.calls - 2.0))
          * (x.i * 1.0 / (r.calls - 1)) * (1 - x.i * 1.0 / (r.calls - 1))
    END, 0) AS p
  FROM ran r JOIN nums x ON x.i < r.calls
),
-- The rule, call by call; a row holds the session as it stands after call i, and the next row hands over first when
-- this one came due (its rent past 0 and at least its buy, e times the write price). o0 is the session's first prompt;
-- d how many tokens less each call holds than it did, by the hand-overs so far; o what the replayed session opened
-- with; e what call i holds once replayed, and rd, wr and outp its prices; rent what the replayed session has paid
-- since it opened; saved the reads saved; paid what the hand-overs cost; handed how many there were, and at where each
-- came: R<work>-<n>#<the call of the round>.
step(s, i, work, n, j, o0, d, o, e, rd, wr, outp, rent, saved, paid, handed, at) AS (
  SELECT c.s, c.i, c.work, c.n, c.j, c.p, 0.0, c.p, c.p, pr.read, CASE WHEN c.hour THEN pr.w1 ELSE pr.w5 END,
    pr.output, 0.0, 0.0, 0.0, 0, ''
  FROM calls c
  JOIN prices pr ON pr.model = c.model
    AND pr.above = CASE WHEN c.model = 'claude-haiku-5-5' AND c.p > 100000 THEN 100000 ELSE 0 END
  WHERE c.i = 1
  UNION ALL
  SELECT c.s, c.i, c.work, c.n, c.j, st.o0,
    -- d, after the hand-over when the call before came due.
    st.d + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6) * max(st.e - st.o0 - g.handover, 0),
    -- o, likewise.
    CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6 THEN st.o0 + g.handover ELSE st.o END,
    -- e: what the call added past the hand-overs so far, never more than it held.
    max(c.p - (st.d + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6) * max(st.e - st.o0 - g.handover, 0)),
        min(c.p, CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6 THEN st.o0 + g.handover ELSE st.o END)),
    pr.read, CASE WHEN c.hour THEN pr.w1 ELSE pr.w5 END, pr.output,
    -- rent: what this call reads again past the opening, after what was paid since the session opened.
    (1 - (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6)) * st.rent
      + max(max(c.p - (st.d + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6) * max(st.e - st.o0 - g.handover, 0)),
                min(c.p, CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6 THEN st.o0 + g.handover ELSE st.o END))
            - CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6 THEN st.o0 + g.handover ELSE st.o END, 0)
        * pr.read / 1e6,
    -- saved: what this call no longer reads.
    st.saved + (c.p - max(c.p - (st.d + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6)
                                   * max(st.e - st.o0 - g.handover, 0)),
                          min(c.p, CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6
                                        THEN st.o0 + g.handover ELSE st.o END))) * pr.read / 1e6,
    -- paid: a hand-over writes the whole context again, and its call reads it and writes the hand-over.
    st.paid + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6) * (st.e * (st.wr + st.rd) + g.handover * st.outp) / 1e6,
    st.handed + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6),
    st.at || CASE WHEN st.rent > 0 AND st.rent >= st.e * st.wr / 1e6
                  THEN ' R' || st.work || '-' || st.n || '#' || st.j ELSE '' END
  FROM step st
  JOIN calls c ON c.s = st.s AND c.i = st.i + 1
  JOIN given g
  JOIN prices pr ON pr.model = c.model
    AND pr.above = CASE WHEN c.model = 'claude-haiku-5-5'
      AND c.p - (st.d + (st.rent > 0 AND st.rent >= st.e * st.wr / 1e6) * max(st.e - st.o0 - g.handover, 0)) > 100000
      THEN 100000 ELSE 0 END
),
-- Each session replayed, as it stands after its last call, with its rounds as recorded.
per AS (
  SELECT st.s, st.handed, trim(st.at) AS at, st.saved, st.paid,
    (SELECT 'R' || r.work || '-' || r.n || ' ' || r.cell FROM ran r WHERE r.s = st.s
     ORDER BY r.started_at, r.work, r.n LIMIT 1) AS first,
    (SELECT count(*) FROM ran r WHERE r.s = st.s) AS rounds,
    (SELECT sum(r.calls) FROM ran r WHERE r.s = st.s) AS calls,
    (SELECT sum(r.cost) FROM ran r WHERE r.s = st.s) AS cost
  FROM step st
  JOIN (SELECT s, max(i) AS last FROM calls GROUP BY s) l ON l.s = st.s AND l.last = st.i
)
SELECT * FROM (
  SELECT 'session' AS kind, first, rounds, calls, round(cost, 2) AS cost, handed, at,
    round(saved, 2) AS saved, round(paid, 2) AS paid, round(saved - paid, 2) AS net
  FROM per WHERE handed > 0
  UNION ALL
  SELECT 'total', count(*) || ' sessions, ' || sum(handed > 0) || ' hand over', sum(rounds), sum(calls),
    round(sum(cost), 2), sum(handed), sum(saved - paid < 0) || ' with a net loss',
    round(sum(saved), 2), round(sum(paid), 2), round(sum(saved - paid), 2)
  FROM per
  UNION ALL
  SELECT 'left_out', count(DISTINCT s) || ' sessions', count(*), sum(calls), round(sum(cost), 2), NULL,
    'a round that used tokens has no first or last prompt, or a model with no prices', NULL, NULL, NULL
  FROM told WHERE s NOT IN (SELECT s FROM whole)
)
ORDER BY kind = 'session', kind, net, first;
