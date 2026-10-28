-- Takes a version 26 database to version 27: every way a work ends goes through one ending, which withdraws from the
-- architect's queue the matters that waited on the work (its questions, failures and design cards), saying why; and a
-- cancelled item, which never delivers, holds back no item. Each statement below that defines something is written
-- exactly as db/schema.sql writes it.

ALTER TABLE architect_matters ADD COLUMN withdrawn TEXT;

DROP VIEW architect_queue;
CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell for its question; the owner, for
  -- the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer, and whether it is urgent. Read by: cells W0 and D0; the engine, which wakes each project's architect and
  -- gives the architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w WHERE w.id = m.work AND w.graph = 'engineer' AND w.at = '15' AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AS answered,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

DROP VIEW item_blocked_by;
CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell (9)
  -- records a merge: no work starts on the item until every one is. A cancelled item, which never delivers, holds back
  -- none. Read by: ready_items; the architect, through its board; the owner's interface, which shows an item that
  -- would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l JOIN items d ON d.id = l.depends_on
  WHERE l.kind = 'blocking' AND d.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = l.depends_on AND r.cell = '9' AND r.transition = 'merged');
