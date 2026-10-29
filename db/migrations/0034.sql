-- Takes a version 33 database to version 34: the start of a work is cell `S1`, and `S0` the cell an item waits at to
-- start, so the items' description names the start by its code now; and flow feedback, a project's own or forwarded by
-- another project's architect, wakes no architect, which the description of the architects' matters says with every
-- detail a matter's kind has. Agents read these descriptions through the query tool, and only the descriptions change,
-- so the two tables are made afresh with their rows. Each statement below that defines something is written exactly
-- as db/schema.sql writes it.

CREATE TABLE items_version_33 AS SELECT * FROM items;
CREATE TABLE architect_matters_version_33 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;
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
  -- The locked acceptance: a JSON list of acceptance rows in the flow's acceptance shape. NULL until a first design
  -- passes its checks. Every later design must hand in exactly this list; only the advisor can change it.
  -- Read by: cells `D2`, `I1`, `T3`, `T4` and `T1`; the
  -- engine, which compares each design's acceptance with it.
  acceptance TEXT CHECK (acceptance IS NULL OR json_valid(acceptance)),
  -- The round whose acceptance was locked, by its work and its number in the work: a design round, or an advisor
  -- round that changed it. Read by: cell `D2`, to see whether the advisor changed it.
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
  CHECK ((acceptance IS NULL) = (acceptance_round IS NULL)),
  CHECK ((acceptance_work IS NULL) = (acceptance_round IS NULL)),
  FOREIGN KEY (acceptance_work, acceptance_round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a design card to look at, a failure, the
  -- owner's words, a message from another project's architect, slots free with nothing of the project ready, or a
  -- place its understanding found to set straight. The project's queue is its matters not handled; its architect
  -- picks among them by the constitution and the policy, not by the order they came. A matter in the queue wakes the
  -- project's architect, but for flow feedback, the project's own or forwarded by another project's architect, which
  -- waits for a wake something else started and is picked only when it came before that wake started.
  -- Read by: the engine, which wakes the architect, picks a lone matter, routes each to its path, hands it its material
  -- and takes its replies back; the architect's cells through architect_queue and wake_handled; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, report, design_card, failure,
  -- owner_words, other_architect or slots_free. A matter handled before these kinds keeps the kind it had then.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, followup for a report's what to do next, answer for an engineer's answer to what the architect said to its
  -- work, or flow_feedback for what agents said was wrong with their cells); a failure's (tool_failed,
  -- environment_changed); the owner's words' (said, answer, release, approved_changes); a message's (message, reply,
  -- or flow_feedback for flow feedback another project's architect forwarded); the kind of a place to set straight.
  -- NULL when there is nothing more. Read by: the engine, for the matter's material and whether it wakes the
  -- architect; the owner's interface.
  detail TEXT,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the side report, a report's what to do next with its paths and
  -- the commit merged, the design card, the tool that failed, the owner's words, the message, the place to set
  -- straight. Read by: the engine, for the matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report or a
  -- design card, an architect's round; NULL for one the owner or the engine raised.
  work INTEGER,
  round INTEGER,
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- The ask of the owner a cell on its path put about it, which holds it while the owner has not answered: it is then
  -- out of the queue, and once answered it is back, and goes back to the cell that asked, with the answer. NULL when no
  -- cell asked about it. Read by: the queue; the engine, which routes it and gives the answer.
  asked INTEGER REFERENCES owner_asks(id),
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer.
  resume_cell TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. Read by: the engine, which
  -- takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its wake and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; NULL otherwise. Read by: architect_queue, which leaves it out; the engine, which tells a new
  -- work on the item what an earlier work asked and had withdrawn; the owner's interface, which leaves it out of the
  -- queue.
  withdrawn TEXT,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO items (
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank)
SELECT
  id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, acceptance_round, cancelled,
  imported_from, imported_branch, imported_pr, wanted, held, rank
FROM items_version_33;
INSERT INTO architect_matters (
  id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, reply, reply_held, handled_work,
  handled_round, withdrawn)
SELECT
  id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, reply, reply_held, handled_work,
  handled_round, withdrawn
FROM architect_matters_version_33;
DROP TABLE items_version_33;
DROP TABLE architect_matters_version_33;

CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
