-- Takes a version 71 database to version 72: an item says the route its works go, normal or the light route of a bug
-- fix or a feature, which only an item that delivers a change goes; every item there is goes the normal route. The
-- items' description says who reads it, and that a plan on the light route locks the acceptance as a design does, so
-- their table is made afresh with its rows; the view of the item as its creator wrote it gives the route too. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE items_version_71 AS SELECT * FROM items;
DROP TABLE items;

CREATE TABLE items (
  -- A piece of work to do, as its creator wrote it. Text that a work's rounds produce later (the problem as the
  -- engineer understood it, the study, the design, progress) is not stored here: views read it from rounds. Items own
  -- nothing; how they depend on each other is in item_links. Only cancellation is stored as a status; every other
  -- status is derived.
  -- Read by: every cell that reads the item or the items around it; the engine when it starts and stops works.

  -- Item number, one sequence across all projects.
  id INTEGER PRIMARY KEY,
  -- The project the item belongs to. Picks the repository, CI and settings its works run with.
  -- Read by: the engine; cells `D1` and `D3`, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell `R1` for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell `R1` has run; these stay as written.
  -- Read by: cell `R1`, for this item and the items that depend on it; cell `T3` reads the
  -- problem only.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell `R1`.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells `R1` and `T4`;
  -- the engine, which runs reports without full CI on the draft.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell `S1`.
  level TEXT NOT NULL,
  -- The locked acceptance: a JSON list of acceptance rows in the flow's acceptance shape. NULL until a first design, or
  -- on the light route a first plan, passes its checks. Every later design or plan must hand in exactly this list;
  -- only the advisor can change it. Read by: cells `D2`, `F1`, `F2`, `I1`, `T3`, `T4` and `T1`; the engine, which
  -- compares each design's and each plan's acceptance with it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round whose acceptance was locked, by its work and its number in the work: a design round, a plan round on the
  -- light route, or an advisor round that changed it. Read by: cell `D2`, to see whether the advisor changed it.
  acceptance_work INTEGER,
  acceptance_round INTEGER,
  -- Why the item was cancelled; NULL while it is not. Read by: the engine; cell `R1` for the items around
  -- this one.
  cancelled TEXT,
  -- For an item imported from outside this flow, such as an item of an earlier board: where it came from, as the import
  -- wrote it; NULL for an item made here. Read by: cell `R1`, through what already exists for the item;
  -- the owner's interface.
  imported_from TEXT,
  -- The branch and the pull request that work on the item had outside this flow, as the import recorded them; NULL when
  -- there were none. Read by: cell `S1`, which takes them over when the item had no work here before; cell
  -- `R1`; the owner's interface.
  imported_branch TEXT,
  imported_pr INTEGER,
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The locked
  -- acceptance, designed by the work, is acceptance. Read by: cell `R1`, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
  -- The route its works go: normal; or bugfix or feature, which go the light route, a plan reviewed before it is
  -- implemented in place of the judgment whether it stands, the design and its checks. Only an item that delivers a
  -- change goes the light route. The architect sets it when it creates or edits the item; a work past its research
  -- keeps the route it took. Read by: the engine's value of the route a work takes, until the work's research
  -- answers; cells `R1`, `G4`, `F1` and `F2`, through item_body; the owner's interface.
  route TEXT NOT NULL DEFAULT 'normal' CHECK (route IN ('normal', 'bugfix', 'feature')),
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  CHECK (route = 'normal' OR deliver = 'change'),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank)
SELECT
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank
FROM items_version_71;
DROP TABLE items_version_71;

DROP VIEW item_body;

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted and the route its works go included. Read by: cells
  -- `R1`, `G4`, `F1`, `F2`, `I1` and `T4`.
  SELECT id AS item, project, title, problem, why, approach, deliver, route, wanted FROM items;
