-- Takes a database to one with the view of the items on hand, a short row each, that agents look up instead of being
-- given the list, and with the views that named the list saying who reads them now. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

CREATE VIEW open_items AS
  -- The items not done or cancelled, a short row each, in the order they start. An item's state: `running`, a work is
  -- on it, whose number and cell are in work and at; `paused`, that work is paused; `resuming`, it is paused and asked
  -- to go on, which waits for a free slot; `stopping`, a cancel or a start over was asked, and it reads as it stands
  -- until the work's step has ended; `held`, the architect holds it (items.held says why); `blocked`, an item it waits
  -- for is not merged (item_blocked_by); `ready`, it starts when a slot is free (ready_items). version is what a change
  -- to the item is written on. Read by: the agents, to see what is on hand.
  WITH
  running(item, work, at, paused) AS (
    SELECT w.item, w.id, w.at, w.paused FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.at NOT IN ('E1', '20', 'end') AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.graph = 'engineer' AND w.at IN ('E1', '20', 'end'))
  SELECT i.id AS item, i.project, i.title,
    CASE
      WHEN EXISTS (
        SELECT 1 FROM outbox o
        WHERE (o.kind = 'start_over' AND o.work = r.work)
          OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = i.id)) THEN 'stopping'
      WHEN r.paused IS NOT NULL AND EXISTS (
        SELECT 1 FROM outbox o WHERE o.kind = 'resume_work' AND o.work = r.work) THEN 'resuming'
      WHEN r.paused IS NOT NULL THEN 'paused'
      WHEN r.item IS NOT NULL THEN 'running'
      WHEN i.held IS NOT NULL THEN 'held'
      WHEN i.id IN (SELECT item FROM item_blocked_by) THEN 'blocked'
      ELSE 'ready'
    END AS state,
    i.rank,
    (SELECT coalesce(max(c.id), 0) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
      AS version,
    r.work, r.at
  FROM items i LEFT JOIN running r ON r.item = i.id LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND (r.item IS NOT NULL OR q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

DROP VIEW item_blocked_by;
CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell
  -- (`M2`) records a merge: no work starts on the item until every one is. A cancelled item, which never
  -- delivers, holds back none. Read by: ready_items; the agents; the owner's interface, which
  -- shows an item that would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l JOIN items d ON d.id = l.depends_on
  WHERE l.kind = 'blocking' AND d.cancelled IS NULL AND NOT EXISTS (
    SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = l.depends_on AND r.cell IN ('M2', '9', 'merge_queue', 'merge.queue') AND r.transition = 'merged');

DROP VIEW item_context;
CREATE VIEW item_context AS
  -- Where the item sits in the whole: the items above it (those that depend on it, up to the root), beside it
  -- (the other items its parents depend on) and below it (those it depends on), each with its problem and why and
  -- the kind of the dependency that brings it in: above, how it depends on the item below it on the way; beside, how
  -- their parent depends on it; below, how this item depends on it. Read by: the agents.
  WITH RECURSIVE up(item, other, depth, kind) AS (
    SELECT depends_on, item, 1, kind FROM item_links
    UNION
    SELECT up.item, l.item, up.depth + 1, l.kind FROM up JOIN item_links l ON l.depends_on = up.other
  )
  SELECT up.item, 'above' AS relation, up.depth, up.kind, o.id AS other, o.title, o.problem, o.why, o.cancelled
  FROM up JOIN items o ON o.id = up.other
  UNION ALL
  SELECT a.depends_on, 'beside', 1, b.kind, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links a JOIN item_links b ON b.item = a.item AND b.depends_on <> a.depends_on
  JOIN items o ON o.id = b.depends_on
  UNION ALL
  SELECT l.item, 'below', 1, l.kind, o.id, o.title, o.problem, o.why, o.cancelled
  FROM item_links l JOIN items o ON o.id = l.depends_on;

DROP VIEW work_routes;
CREATE VIEW work_routes AS
  -- The way each work of the engineer went, from the rows it took: `turned`, a work that started to implement on the
  -- research's plan and then took the row counting `shallow_research`, which sent it to the design; `designed`, one
  -- that went to the design (cell `D1`), whether its research sent it there or not; `direct`, one whose research
  -- (cell `R1`) answered with its row `start`, so that it implements on the research's plan and has had no design; none
  -- before the research answers, and for a work of a flow version whose research took other rows. Nothing marks the
  -- item for a way: the research judges it. Read by: the owner's interface, beside the item's head; the agents.
  SELECT w.id AS work, w.item,
    CASE
      WHEN EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.counts = 'shallow_research') THEN 'turned'
      WHEN EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.cell = 'D1') THEN 'designed'
      WHEN (SELECT r.transition FROM rounds r
            WHERE r.work = w.id AND r.cell = 'R1' AND r.transition IN ('start', 'cannot_start')
            ORDER BY r.n DESC LIMIT 1) = 'start' THEN 'direct'
    END AS route
  FROM works w
  WHERE w.graph = 'engineer';

CREATE TABLE dispatch_holds_before AS SELECT * FROM dispatch_holds;
DROP TABLE dispatch_holds;

CREATE TABLE dispatch_holds (
  -- What stops new works of a project from starting, one row for each: the owner's hold, its architect's own hold, its
  -- architect setting it straight. While a project has a row here, the engine starts no new work on its items; works
  -- already running go on, and the other projects' works start as usual.
  -- Read by: the engine at cell `S0`; the architect; the owner's interface.
  project TEXT NOT NULL,
  -- owner: held by the owner. architect: held by the architect's own action. set_straight: its plan of what to do next
  -- found the whole must be set straight. The owner releases any of them; the architect only its own and the one to
  -- set straight.
  reason TEXT NOT NULL CHECK (reason IN ('owner', 'architect', 'set_straight')),
  -- When it began.
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the owner or the architect said it for their own hold; NULL for the one to set
  -- straight, whose reason says why. Read by: the owner's interface.
  why TEXT,
  PRIMARY KEY (project, reason)
) STRICT;

INSERT INTO dispatch_holds (project, reason, since, why) SELECT project, reason, since, why FROM dispatch_holds_before;
DROP TABLE dispatch_holds_before;
