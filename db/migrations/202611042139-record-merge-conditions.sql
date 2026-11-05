-- Takes a database to one that records what the owner set to be done before an item's work merges, for the merge gate
-- to hold it: the table `merge_conditions`, with its index, holds none yet; the view `item_merge_conditions` works out
-- where each stands from the owner's answers. The asks of the owner are made afresh with their rows, as the description
-- of the asks now says what a merge condition's ask keeps, and that description is part of the table's text, which
-- agents read; a migration runs with the foreign keys off, as SQLite makes a table afresh, and every one is checked
-- before the migration commits. Each statement below that defines something is written exactly as db/schema.sql writes
-- it.

CREATE TABLE owner_asks_before_merge_conditions AS SELECT * FROM owner_asks;
DROP TABLE owner_asks;

CREATE TABLE owner_asks (
  -- An ask of the owner: what an agent asks the owner, or asks the architect while the owner answers for the
  -- architect. The work waits for the answer, which goes back unchanged to the cell that asked. It belongs to the
  -- round of the asking cell that put it to the owner; its item is found through the round's work.
  -- Read by: cell `R1` (the asks of the owner on this item, with their answers); the engine, which waits
  -- for the answer.
  id INTEGER PRIMARY KEY,
  -- The round that put it to the owner: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- What is asked, JSON: whom it asks, the work and item, and what the asking cell handed in; for the merge gate's ask
  -- of a merge condition, that condition's number too (`condition`). Copied from what that round was given, which
  -- never changes.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The owner's answer, JSON: who answered and when, and the text, and who the caller declared to speak (`speaker`,
  -- owner or delegate; none for an answer kept before it was declared); for an ask of the architect, the rest of the
  -- architect's reply too; for the merge gate's ask of a merge condition, the option chosen (`choice`, met or
  -- withdrawn) when one was. NULL while it waits. Read by: the engine; cells `R1` and `L2`.
  answer TEXT CHECK (answer IS NULL OR json_valid(answer)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO owner_asks (id, work, round, body, answer)
  SELECT id, work, round, body, answer FROM owner_asks_before_merge_conditions;
DROP TABLE owner_asks_before_merge_conditions;

CREATE INDEX owner_asks_by_round ON owner_asks (work, round);

CREATE TABLE merge_conditions (
  -- Something the owner set to be done before an item's work merges, one row for each. The merge gate (cell `M1`) holds
  -- the work until each is judged met or withdrawn, by asking the owner one at a time; the row keeps no state: the view
  -- `item_merge_conditions` works it out from the answers. A row is written when the owner ticks it on words said or an
  -- answer given, or by an item change of the project's architect; what neither ticked nor recorded holds nothing.
  -- Read by: the view `item_merge_conditions`.
  id INTEGER PRIMARY KEY,
  item INTEGER NOT NULL REFERENCES items(id),
  -- One sentence that reads alone, written by whoever recorded it, since the words it rests on may be only "OK". The
  -- engineer reports on it, and the owner judges it. Read by: cells `R1`, `D1`, `I1`, `T6` and `M1`; the owner's interface.
  what TEXT NOT NULL CHECK (trim(what) <> ''),
  -- The record the words are in: an ask, by the owner's answer to it; an interjection; or a matter of the architect's
  -- queue, by the owner's words or the owner's reply it holds. Read by: the merge gate's ask, which links to it.
  source TEXT NOT NULL CHECK (source IN ('ask', 'interjection', 'matter')),
  source_id INTEGER NOT NULL,
  -- The words as that record has them. Empty only for an answer that was images alone.
  quote TEXT NOT NULL,
  -- Who the record says said those words: the owner; relayed, the owner's own words as a delegate declared them, checked
  -- by no one; delegate, a session that speaks for the owner, in words of its own; or unrecorded, a record kept before
  -- who speaks was declared. Never written as the owner's when the record does not say so.
  said_by TEXT NOT NULL CHECK (said_by IN ('owner', 'relayed', 'delegate', 'unrecorded')),
  -- Who recorded it: the owner or a delegate, by ticking it as the caller declared; or the architect, by an item change.
  recorded_by TEXT NOT NULL CHECK (recorded_by IN ('owner', 'delegate', 'architect')),
  -- The round that made the architect's item change, by its work and its number in the work; NULL for one ticked.
  recorded_work INTEGER,
  recorded_round INTEGER,
  -- When it was recorded.
  at INTEGER NOT NULL,
  CHECK ((recorded_by = 'architect') = (recorded_work IS NOT NULL)),
  CHECK ((recorded_work IS NULL) = (recorded_round IS NULL)),
  FOREIGN KEY (recorded_work, recorded_round) REFERENCES rounds (work, n)
) STRICT;

CREATE INDEX merge_conditions_by_item ON merge_conditions (item);

CREATE VIEW item_merge_conditions AS
  -- Each merge condition of each item with where it stands: open, or as the merge gate's ask of it was judged, met or
  -- withdrawn, with that ask (`judged`) and who judged it (`judged_by`, owner or delegate). This is the one place that
  -- says whose answer judges a condition: an answer declared as the owner's or as a delegate's that chose an option.
  -- Words with no option leave it open, and go to the engineer as words. Read by: the engine, for the merge gate's value
  -- `gates` and for what cells `R1`, `D1`, `I1`, `T6` and `M1` are given; the owner's interface, on the item's page.
  SELECT c.item, c.id, c.what, c.source, c.source_id, c.quote, c.said_by, c.recorded_by, c.recorded_work,
    c.recorded_round, c.at,
    coalesce(j.choice, 'open') AS status, j.ask AS judged, j.judged_by
  FROM merge_conditions c
  LEFT JOIN (
    SELECT json_extract(a.body, '$.condition') AS condition, max(a.id) AS ask,
      json_extract(a.answer, '$.choice') AS choice, json_extract(a.answer, '$.speaker') AS judged_by
    FROM owner_asks a
    WHERE json_extract(a.body, '$.condition') IS NOT NULL
      AND json_extract(a.answer, '$.speaker') IN ('owner', 'delegate')
      AND json_extract(a.answer, '$.choice') IN ('met', 'withdrawn')
    GROUP BY 1
  ) j ON j.condition = c.id;
