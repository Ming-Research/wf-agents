-- Takes a version 46 database to version 47: each project's queue says, of a matter that comes back with the
-- owner's answer, to which cell it goes back (back_to), as the router's cell declares what it routes by. Each statement
-- below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW architect_queue;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn, not held by an ask of the owner still waiting and
  -- not held by a talk with the owner that waits for the owner's reply, each with who waits for it (an agent: an
  -- engineer stopped at the architect's cell for its question; the owner, for the owner's words; or no one), when it
  -- came and how long it has waited, whether it comes back with the owner's answer and to which cell, and whether it
  -- is urgent. A talk held is due again once the owner said more to the architect in their one-to-one since, or ended
  -- it. Read by: cells `S0`, `S1` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect
  -- runs no work, and gives the architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AS answered,
    m.resume_cell AS back_to,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
    AND (m.talk_held IS NULL OR m.talk_ended IS NOT NULL OR EXISTS (
      SELECT 1 FROM architect_matters x
      WHERE x.project = m.project AND x.kind = 'owner_words' AND json_extract(x.body, '$.where') = 'page'
        AND x.handled_round IS NULL AND x.withdrawn IS NULL AND x.at > m.talk_held))
  ORDER BY m.id;
