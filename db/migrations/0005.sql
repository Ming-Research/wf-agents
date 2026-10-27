-- Takes a version 4 database to version 5. A design card and an ask of the owner are two different things, kept in
-- tables of their own: the design cards keep their ids, their rulings and what replaced them; each ask keeps its id,
-- and its last ruling becomes its answer. No failed run asks the owner for a written answer any longer, so an ask a
-- failed run raised that is still open is closed, and its work, paused for it, goes on: its cell runs again. Agents
-- tell the owner what needs no answer in messages, kept in a table of their own. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

CREATE TABLE design_cards (
  -- A change to the design tree for the owner to confirm before it is merged: each decision the change adds, changes
  -- or removes. It asks the owner nothing; what the owner is asked is an ask of the owner. A design card is handed in
  -- by one round and belongs to that round; its work and item are found through it. A ruled design card never
  -- changes; a decision that has to change comes in a new design card that replaces the old one.
  -- Read by: cell 1 (design cards ruled on this item), 2 and 2a (rulings that disagreed or chose another option), 2a
  -- (the latest design card of the items that depend on this one), 15 (the design card to look at), 19 (whether the
  -- design card has been confirmed).
  id INTEGER PRIMARY KEY,
  -- The round that handed the design card in.
  round INTEGER NOT NULL REFERENCES rounds(id),
  -- The design card as the round handed it in, JSON, in the flow's design card shape. Copied from that round's
  -- output, which never changes, so the copy cannot drift.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- open: waiting for a ruling. ruled: everyone it waits for has ruled. replaced: a newer design card took its place
  -- and it can no longer be ruled. Read by: the engine; cell 19.
  status TEXT NOT NULL CHECK (status IN ('open', 'ruled', 'replaced')),
  -- The rulings in the order given, JSON list: who ruled (owner or architect), the verdict, the option chosen and
  -- their own words. Read by: cells 1, 2, 2a and 19.
  rulings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(rulings)),
  -- The design card that replaced this one. Read by: people following a decision to its latest design card.
  replaced_by INTEGER REFERENCES design_cards(id),
  -- A replaced design card names its successor, unless its work ended without one.
  CHECK (replaced_by IS NULL OR status = 'replaced')
) STRICT;

CREATE TABLE owner_asks (
  -- An ask of the owner: what an agent asks the owner, or asks the architect while the owner answers for the
  -- architect. The work waits for the answer, which goes back unchanged to the cell that asked. It belongs to the
  -- round of the asking cell that put it to the owner; its work and item are found through it.
  -- Read by: cell 1 (the asks of the owner on this item, with their answers); the engine, which waits for the answer.
  id INTEGER PRIMARY KEY,
  -- The round that put it to the owner.
  round INTEGER NOT NULL REFERENCES rounds(id),
  -- What is asked, JSON: whom it asks, the work and item, and what the asking cell handed in. Copied from what that
  -- round was given, which never changes.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The owner's answer, JSON: who answered and when, and the text; for an ask of the architect, the rest of the
  -- architect's reply too. NULL while it waits. Read by: the engine; cell 1.
  answer TEXT CHECK (answer IS NULL OR json_valid(answer))
) STRICT;

CREATE TABLE owner_messages (
  -- What an agent tells the owner in a return, one row for each message: something the owner should know that needs
  -- no decision and no answer. The work goes on. It belongs to the round whose return holds it; its work and item are
  -- found through it. Read by: the owner's interface, which shows those unread.
  id INTEGER PRIMARY KEY,
  round INTEGER NOT NULL REFERENCES rounds(id),
  -- The message, copied from that round's return, which never changes.
  text TEXT NOT NULL,
  -- When the owner marked it read; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER
) STRICT;

INSERT INTO design_cards (id, round, body, status, rulings, replaced_by)
  SELECT id, round, body, status, rulings, replaced_by FROM cards WHERE kind = 'design';

-- The works a failed run's open ask paused go on. The change is the migration's, not a writer's, so the trigger that
-- records changes to works is dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_works_update;
UPDATE works SET paused = NULL
  WHERE ended IS NULL AND paused IS NOT NULL AND id IN (
    SELECT r.work FROM cards c JOIN rounds r ON r.id = c.round
    WHERE c.kind = 'ask' AND c.status = 'open' AND json_extract(c.body, '$.failed') IS NOT NULL);

INSERT INTO owner_asks (id, round, body, answer)
  SELECT id, round, body,
    CASE
      WHEN status <> 'open' THEN json_extract(rulings, '$[#-1]')
      WHEN json_extract(body, '$.failed') IS NOT NULL THEN json_object(
        'by', 'engine', 'at', CAST(unixepoch('subsec') * 1000 AS INTEGER),
        'text', 'No written answer is needed: the work goes on once its command line can run.')
    END
  FROM cards WHERE kind = 'ask';

DROP VIEW item_cards;
DROP VIEW parent_design_cards;
DROP TABLE cards;

CREATE INDEX design_cards_by_round ON design_cards (round);
CREATE INDEX owner_asks_by_round ON owner_asks (round);
CREATE INDEX owner_messages_unread ON owner_messages (id) WHERE read_at IS NULL;

CREATE VIEW item_design_cards AS
  -- Every design card of the item's works, with its rulings. Read by: cells 1, 2, 2a, 15 and 19.
  SELECT w.item, r.work, c.id AS design_card, c.round, c.body, c.status, c.rulings, c.replaced_by
  FROM design_cards c JOIN rounds r ON r.id = c.round JOIN works w ON w.id = r.work;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell 1.
  SELECT w.item, r.work, a.id AS ask, a.round, a.body, a.answer
  FROM owner_asks a JOIN rounds r ON r.id = a.round JOIN works w ON w.id = r.work;

CREATE VIEW parent_design_cards AS
  -- The design cards of the items that depend on this one, not yet replaced: what a split-off item's parent
  -- decided. Read by: cell 2a.
  SELECT l.depends_on AS item, l.item AS parent, c.id AS design_card, c.body, c.status, c.rulings
  FROM item_links l
  JOIN works w ON w.item = l.item
  JOIN rounds r ON r.work = w.id
  JOIN design_cards c ON c.round = r.id
  WHERE c.status <> 'replaced';
