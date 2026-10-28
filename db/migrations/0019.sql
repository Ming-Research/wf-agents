-- Takes a version 18 database to version 19: the architect's plan goes, its segments, gates and budgets, with where an
-- item hung in it and what it touched; in its place each item has its rank among its project's items, the architect's
-- order works start in after the owner's queue, and every item already there is ranked by the order it was made in its
-- project. The items and the owner's queue are made afresh with their rows, since a column a foreign key names cannot
-- be dropped in place and the queue's description changed, and ready_items is made afresh. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

DROP VIEW ready_items;
DROP VIEW plan;

CREATE TABLE items_version_18 AS SELECT * FROM items;
CREATE TABLE dispatch_queue_version_18 AS SELECT * FROM dispatch_queue;
DROP TABLE dispatch_queue;
DROP TABLE items;
DROP TABLE plan_segments;
DROP TABLE plan_gates;
DROP TABLE plan_budgets;

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. Only cancellation is stored as a status; every other
  -- status is derived.
  -- Read by: every cell that reads the item or the items around it; the engine when it starts and stops works.

  -- Item number, one sequence across all projects.
  id INTEGER PRIMARY KEY,
  -- The project the item belongs to. Picks the repository, CI and settings its works run with.
  -- Read by: the engine; cells 2 and 2b, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell 1 for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell 1 has run; these stay as written.
  -- Read by: cell 1, for this item and the items that depend on it; cell 5 reads the problem only.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell 1.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells 1 and 5a; the engine, which runs reports
  -- without full CI on the draft.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell 0.
  level TEXT NOT NULL,
  -- The locked acceptance: a JSON list of acceptance rows in the flow's acceptance shape. NULL until a first design
  -- passes its checks. Every later design must hand in exactly this list; only the advisor can change it.
  -- Read by: cells 2a, 4, 5, 5a and 6; the engine, which compares each design's acceptance with it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round whose acceptance was locked, by its work and its number in the work: a design round, or an advisor
  -- round that changed it. Read by: cell 2a, to see whether the advisor changed it.
  acceptance_work INTEGER,
  acceptance_round INTEGER,
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell 1 for the items around this one.
  cancelled TEXT,
  -- For an item imported from outside this flow, such as an item of an earlier board: where it came from, as the
  -- import wrote it; NULL for an item made here. Read by: cell 1, through what already exists for the item; the
  -- owner's interface.
  imported_from TEXT,
  -- The branch and the pull request that work on the item had outside this flow, as the import recorded them; NULL
  -- when there were none. Read by: cell 0, which takes them over when the item had no work here before; cell 1; the
  -- owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The locked
  -- acceptance, designed by the work, is acceptance. Read by: cell 1, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work,
    acceptance_round, cancelled, imported_from, imported_branch, imported_pr, wanted, held, rank)
  SELECT id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round,
    cancelled, imported_from, imported_branch, imported_pr, wanted, held,
    row_number() OVER (PARTITION BY project ORDER BY id)
  FROM items_version_18
  ORDER BY id;

CREATE TABLE dispatch_queue (
  -- Items waiting for a work to start, in the order the owner sets by hand, ahead of the architect's order. When a
  -- slot is free the engine starts a work on the first one. This table goes away once the architect's order alone
  -- decides what starts.
  -- Read by: the engine at cell 0.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Lower starts first. A real number, so an item can be put between two others without renumbering.
  position REAL NOT NULL UNIQUE
) STRICT;

INSERT INTO dispatch_queue (item, position) SELECT item, position FROM dispatch_queue_version_18;

DROP TABLE items_version_18;
DROP TABLE dispatch_queue_version_18;

CREATE VIEW ready_items AS
  -- Items waiting to start, in the order they start: first those the owner queued, in the owner's order; then the
  -- others in the architect's order, each by its rank among its project's items, lower first, those placed nowhere yet
  -- after, in the order made. An item is ready when it is not cancelled, not held and has no work running, and, unless
  -- the owner queued it, is not done. An item leaves the owner's queue when a work starts on it, and may be queued
  -- again once its works ended. A work runs until it ends or its last round, at the cell it is at, completed without
  -- taking a row, which only the end of its graph does; it is done at the end of the engineer's graph. While
  -- dispatch_holds has a row, the engine starts none. Read by: cell 0; the architect, through what runs and starts
  -- next.
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
    AND (q.item IS NOT NULL OR i.id NOT IN (SELECT item FROM done))
  ORDER BY q.position NULLS LAST, i.rank NULLS LAST, i.id;
