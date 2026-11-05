-- Takes a database to one whose view of the architect's queue says that the project's architect is waited for on the
-- matter of a return sent back (kind `set_straight`, detail `sent_back`) while the engine has paused the work for
-- returns sent back in a row: the engine queues such a matter at each return sent back now, and the work goes on without
-- waiting for it until three in a row pause it. The view of the problems of returns sent back says what reads it now. No
-- table changes and no row is written: a matter queued before this had no such kind, and a work paused for returns sent
-- back before has a reason that does not start as the view reads it.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW sent_back_problems;

CREATE VIEW sent_back_problems AS
  -- Each problem of each return the engine sent back, one row each, in the order found (`position` counts from 0):
  -- the check that found it (`check_name`, which is `format` for a place that does not fit the cell's schema), the
  -- field and the text, as `rounds.rejected` keeps them under `problems`. A return sent back before `problems` was
  -- kept has no row. `field` is the check's `at` in the flow (`columns[pass].evidence`) for a check and the path in the
  -- schema with each list index written `[]` (`columns[].evidence`) for `format`, so group by `check_name` and
  -- `field` together. One return may hold several problems of one check and field: count returns with
  -- `count(DISTINCT work || '-' || round)` and problems with the rows. Read by: people and agents who query it, to
  -- count the returns sent back by check and by field. The matter the engine queues for the architect at each return
  -- sent back (kind `set_straight`, detail `sent_back`) holds the same problems in its body, as `problems`, and the
  -- merging of like ones (#257) will read either.
  SELECT r.work, r.n AS round, r.cell, p.key AS position,
    json_extract(p.value, '$.check') AS check_name,
    json_extract(p.value, '$.field') AS field,
    json_extract(p.value, '$.text') AS text
  FROM rounds r, json_each(r.rejected, '$.problems') p;

DROP VIEW architect_queue;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question, or, for the
  -- matter of a return sent back (kind `set_straight`, detail `sent_back`), an engineer's work the engine paused for
  -- returns sent back in a row, whose reason starts as `SENT_BACK_FOR_ARCHITECT` in crates/db/src/lib.rs does, until
  -- it goes on; the owner, for the owner's words; or no one), when it came and how long it has waited, whether it comes
  -- back with the owner's answer and to which cell, the second cell a flow version before had a matter of two cells go
  -- on at, and whether its reporter marked it as harming now (urgent).
  -- Read by: cells `S0` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no
  -- work, and takes the next matter by it in its fixed order; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'set_straight' AND m.detail = 'sent_back' AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = json_extract(m.body, '$.work') AND w.ended IS NULL
          AND w.paused_by = 'engine' AND w.paused LIKE 'returns were sent back%')
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AND m.goes_on_at IS NULL AS answered,
    m.resume_cell AS back_to,
    m.goes_on_at,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;
