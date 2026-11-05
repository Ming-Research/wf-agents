-- Takes a database to one whose items carry the decision points a discussion settled, which send a work to deep
-- research and go into the project's decisions/ once it confirms them; the item's acceptance is no longer locked but
-- recorded, which the implementation, a fix or the advisor may change saying why, and the mark of a bug fix or a
-- feature no longer lights a route. Their table is made afresh with its rows, as its description says who reads it now;
-- the view of the item as its creator wrote it gives the decision points, and the view of its acceptance says what it
-- is now. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE items_before_decision_points AS SELECT * FROM items;
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
  -- Read by: the engine; cells `R1` and `D1`, through the project's design goals.
  project TEXT NOT NULL,
  -- What to do, as an action. Read by: cell `R1` for the items around this one; people.
  title TEXT NOT NULL,
  -- What cannot be done or is wrong, and why the work is worth doing, as written at creation. The engineer's own
  -- understanding replaces them in views once cell `R1` has run; these stay as written.
  -- Read by: cell `R1`, for this item and the items that depend on it.
  problem TEXT NOT NULL,
  why TEXT NOT NULL,
  -- How to do it, when the creator already knows; NULL otherwise. Read by: cell `R1`.
  approach TEXT,
  -- What the work hands in: a change to merge, or a report. Read by: cells `R1`, `D1` and `I1`; the engine.
  deliver TEXT NOT NULL CHECK (deliver IN ('change', 'report')),
  -- Agent level; decides which agent and model run the work. Read by: the engine at cell `S1`.
  level TEXT NOT NULL,
  -- The acceptance: a JSON list of acceptance rows in the flow's acceptance shape, as the research's plan or the design
  -- the owner approved gave them when the work went on to implement, and as the implementation, a fix or the advisor
  -- changed them since, each saying why; NULL until then. Read by: cells `I1`, `T2`, `T3`, `T1` and `T6`; the engine,
  -- which checks the conditions met against it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round that gave the acceptance, by its work and its number in the work: the research's, the design's, or the
  -- round of the implementation, a fix or the advisor that changed it. Read by: people following a condition back.
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
  -- The acceptance as the creator wrote it, JSON list of conditions; NULL when the creator wrote none. The acceptance
  -- the work goes by is acceptance. Read by: cell `R1`, through item_body.
  wanted TEXT CHECK (wanted IS NULL OR json_valid(wanted)),
  -- Why the architect holds the item, with the owner's approval: no work starts on it while it is held; NULL while it
  -- is not. Read by: ready_items; the architect.
  held TEXT,
  -- Its place among its project's items in the architect's order, which works start in, lower first: the architect
  -- sets it with its approved changes, moving an item first, last, or before or after another item of the project,
  -- which ranks the project's items afresh. NULL for an item placed nowhere yet, as one just made is, which comes after
  -- those placed, in the order made. Read by: the dispatch order in ready_items; the architect.
  rank INTEGER,
  -- The mark of a bug fix or a feature, which a flow version before took for the light route its works went; normal
  -- otherwise. Only an item that delivers a change carries a mark. The architect sets it when it creates or edits the
  -- item. The flow no longer reads it: every work's research judges whether it can be started directly. Read by: the
  -- views of the routes works of a flow version before went (work_routes); the owner's interface.
  route TEXT NOT NULL DEFAULT 'normal' CHECK (route IN ('normal', 'bugfix', 'feature')),
  -- The decision points the item carries, JSON list in the flow's decision point shape, each with the option the owner
  -- chose and why: what a discussion settled that goes into the project's decisions/ once deep research confirms it.
  -- NULL when it carries none. An item that carries one goes to deep research, whatever its research finds. The
  -- architect writes them as it creates or edits the item. Read by: the engine's value `decision_points`; cells `R1`,
  -- `D1` and `T2`, through item_body.
  decision_points TEXT CHECK (decision_points IS NULL OR json_valid(decision_points)),
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  CHECK (route = 'normal' OR deliver = 'change'),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank, route)
SELECT
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank, route
FROM items_before_decision_points;
DROP TABLE items_before_decision_points;

DROP VIEW item_body;

CREATE VIEW item_body AS
  -- The item as its creator wrote it, the acceptance it wanted and the decision points it carries included. Read by:
  -- cells `R1`, `D1`, `I1`, `T2`, `T3` and `T6`.
  SELECT id AS item, project, title, problem, why, approach, deliver, wanted, decision_points FROM items;

DROP VIEW item_acceptance;

CREATE VIEW item_acceptance AS
  -- The item's acceptance, and the round that gave it, by its work and its number in the work; NULL until the work
  -- goes on to implement. locked says only whether there is one. Read by: cells `I1`, `T2`, `T3`, `T1` and `T6`.
  SELECT id AS item, acceptance IS NOT NULL AS locked, acceptance, acceptance_work, acceptance_round FROM items;
