-- Takes a version 23 database to version 24: each dependency between items is of a kind, blocking or parallel. A
-- blocking dependency keeps an item from starting until the item it depends on is merged (item_blocked_by, which
-- ready_items leaves out); a parallel one is how every dependency worked before, which every dependency already there
-- becomes. The views that list an item's dependencies give the kind. The links are made afresh with their rows, since
-- a column with no default cannot be added in place, and the views over them are made afresh. Each statement below
-- that defines something is written exactly as db/schema.sql writes it.

DROP VIEW ready_items;
DROP VIEW item_dependencies;
DROP VIEW item_context;
DROP VIEW waited_items;

CREATE TABLE item_links_version_23 AS SELECT * FROM item_links;
DROP TABLE item_links;

CREATE TABLE item_links (
  -- Dependencies between items: item cannot finish until depends_on is done. Together they form a directed acyclic
  -- graph, and the engine refuses a write that would close a cycle. Splitting an item means creating new items and
  -- making it depend on them. A blocking dependency keeps item from starting until depends_on is merged; with a
  -- parallel one, item can start while depends_on is unfinished, and waits before it merges. A work already running
  -- goes on either way.
  -- Read by: cell 1 (the items above and beside this one, what it waits for, who waits for it), 3e (what it has been
  -- split into), 11 and 11b (what the items it waits for handed in), 19 (how far they have got); item_blocked_by.
  item INTEGER NOT NULL REFERENCES items(id),
  depends_on INTEGER NOT NULL REFERENCES items(id),
  -- What item needs depends_on to hand in, in one sentence. Read by: cells 1 and 11b.
  needs TEXT NOT NULL,
  -- blocking: item does not start until depends_on is merged, as when it needs a module or a library depends_on makes.
  -- parallel: item may start before, and waits where it needs depends_on: before it merges, or where the architect's
  -- reply has it stop. Read by: item_blocked_by; cells 1, 3e, 11, 11b and 19; the architect; the owner's interface.
  kind TEXT NOT NULL CHECK (kind IN ('blocking', 'parallel')),
  PRIMARY KEY (item, depends_on),
  CHECK (item <> depends_on)
) STRICT;

INSERT INTO item_links (item, depends_on, needs, kind)
  SELECT item, depends_on, needs, 'parallel' FROM item_links_version_23;

DROP TABLE item_links_version_23;

CREATE INDEX item_links_by_depends_on ON item_links (depends_on);

CREATE VIEW item_blocked_by AS
  -- Each item with each item it depends on by a blocking dependency that is not merged yet, as the merge cell (9)
  -- records a merge: no work starts on the item until every one is. Read by: ready_items; the architect, through its
  -- board; the owner's interface, which shows an item that would start but for them as waiting for them.
  SELECT l.item, l.depends_on, l.needs
  FROM item_links l
  WHERE l.kind = 'blocking' AND NOT EXISTS (
    SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.item = l.depends_on AND r.cell = '9' AND r.transition = 'merged');

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then the
  -- others in the architect's order, each by its rank among its project's items, lower first, those placed nowhere yet
  -- after, in the order made. An item is ready when it is not cancelled, not held, has no work running and no item it
  -- depends on by a blocking dependency unmerged (item_blocked_by), and, unless the owner queued it, is not done. An
  -- item leaves the owner's queue when a work starts on it, and may be queued again once its works ended. A work runs
  -- until it ends or its last round, at the cell it is at, completed without taking a row, which only the end of its
  -- graph does; it is done at the end of the engineer's graph. While dispatch_holds has a row, the engine starts none.
  -- Read by: cell 0; the architect, through what runs and starts next.
  WITH
  running(item) AS (
    SELECT w.item FROM works w
    WHERE w.item IS NOT NULL AND w.ended IS NULL AND NOT EXISTS (
      SELECT 1 FROM rounds r
      WHERE r.work = w.id AND r.n = (SELECT max(x.n) FROM rounds x WHERE x.work = w.id)
        AND r.cell = w.at AND r.ended_at IS NOT NULL AND r.transition IS NULL AND r.rejected IS NULL)),
  done(item) AS (
    SELECT w.item FROM works w WHERE w.item IS NOT NULL AND w.ended IS NULL AND w.graph = 'engineer' AND w.at = '20')
  SELECT i.id AS item, i.project, i.title, i.level, q.position, i.rank
  FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id
  WHERE i.cancelled IS NULL AND i.held IS NULL AND i.id NOT IN (SELECT item FROM running)
    AND i.id NOT IN (SELECT item FROM item_blocked_by)
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;

CREATE VIEW item_dependencies AS
  -- What the item depends on, with the kind of each dependency, what it needs of each and how far each is: the cell
  -- its latest work is at, or why it was cancelled. Read by: cells 3e and 19.
  SELECT l.item, l.depends_on, l.kind, l.needs, d.title, d.cancelled,
    (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at,
    (SELECT w.pr FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS pr
  FROM item_links l JOIN items d ON d.id = l.depends_on;

CREATE VIEW item_context AS
  -- Where the item sits in the whole: the items above it (those that depend on it, up to the root), beside it
  -- (the other items its parents depend on) and below it (those it depends on), each with its problem and why and
  -- the kind of the dependency that brings it in: above, how it depends on the item below it on the way; beside, how
  -- their parent depends on it; below, how this item depends on it. Read by: cell 1.
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

CREATE VIEW waited_items AS
  -- The items each work's item depends on, with the kind of each dependency, what it needs of each, how far each is
  -- and, once merged, the last return of its merge. Read by: cells 11 and 11b.
  SELECT w.id AS work, d.id AS item, l.kind, l.needs, d.title, d.cancelled,
    (SELECT x.at FROM works x WHERE x.item = d.id ORDER BY x.id DESC LIMIT 1) AS at,
    (SELECT r.output FROM rounds r JOIN works x ON x.id = r.work
     WHERE x.item = d.id AND r.cell = '9' AND r.transition = 'merged' ORDER BY r.work DESC, r.n DESC LIMIT 1) AS merged
  FROM works w JOIN item_links l ON l.item = w.item JOIN items d ON d.id = l.depends_on;
