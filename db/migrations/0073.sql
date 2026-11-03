-- Takes a version 72 database to version 73: a question a work asks an architect from a line other than the engineer's
-- is no longer withdrawn for words waiting for the engineer's line, which the cell it goes back to would not hear. Only
-- the comments of `cancelled_item` and `informs` in the interjections' table change, so the table is made afresh with
-- its rows. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE interjections_version_72 AS SELECT * FROM interjections;
DROP TABLE interjections;

CREATE TABLE interjections (
  -- What the owner or the architect says to a running work. The engine hands each one to the line it is for: during a
  -- round after the agent's next tool use, or at the front of the next round's input. Rulings on design cards arrive
  -- this way too. Replies are in the output of the round that answered.
  -- Read by: every agent cell; cells `T7`, `M1` and `E1` when the work stands at them.
  id INTEGER PRIMARY KEY,
  work INTEGER NOT NULL REFERENCES works(id),
  -- The line it is for.
  line TEXT NOT NULL DEFAULT 'engineer',
  -- Who said it. When the owner and the architect disagree, the owner's word holds.
  speaker TEXT NOT NULL CHECK (speaker IN ('owner', 'architect')),
  -- For what the owner said to the engineer alone: who the caller declared to speak, owner, or delegate for a session
  -- that speaks for the owner; NULL for what the architect said, for what the engine wrote in the owner's name (a
  -- ruling on a design card) and for words kept before it was declared. Read by: the engine, which tells who decided a
  -- decision that quotes the words.
  declared TEXT CHECK (declared IN ('owner', 'delegate')),
  -- For what a delegate said to the engineer alone and declared to hold the owner's own words: those parts, a JSON array
  -- of strings, each found letter for letter in the words (text); NULL when none was declared. Read by: the engine,
  -- which tells a decision that quotes one of the parts as the owner's, relayed by the delegate, not checked.
  relayed TEXT CHECK (relayed IS NULL OR (json_valid(relayed) AND declared = 'delegate')),
  -- Their words as said.
  text TEXT NOT NULL,
  -- The round of the work it was handed in, by its number in the work; NULL until then. Read by: the engine, to find
  -- what is still to hand in.
  delivered_round INTEGER,
  -- For the notice that an item this work's item depended on was cancelled: that item; NULL for anything else said.
  -- The cells where the work waits (`A2`, `A6`, `M1`) take such a notice to choose the way on by a row of their own,
  -- as an item they waited for that will never deliver, when it is for the line the work stands on: a question asked
  -- from another line is not withdrawn for it, since the cell the work goes back to would not hear it. Read by: the
  -- engine's values `cancelled` and `held`.
  cancelled_item INTEGER REFERENCES items(id),
  -- 1 for a word that needs no answer and changes no way on, as the owner confirming every decision of a design card:
  -- the line's next round is given it, and a work standing at an engine step is not taken by it to choose the way on.
  -- Read by: the engine's values `interjection`, `cancelled` and `held`, which leave such words out.
  informs INTEGER NOT NULL DEFAULT 0 CHECK (informs IN (0, 1)),
  -- For what the owner said in the work's group to its project's architect: that architect's matter, which holds the
  -- words. Said to the architect alone, the engineer is given it as a word that needs no answer (informs); said to
  -- both, as said to it. NULL for anything said to the engineer alone. Read by: the owner's interface, which shows the
  -- words once, to whom they were said.
  matter INTEGER REFERENCES architect_matters(id),
  -- For the owner's question about a decision card, asked on the card in the work's group: that card, which the
  -- engineer is given with the words, and the group shows them as a reply to. NULL for anything else said.
  design_card INTEGER REFERENCES design_cards(id),
  FOREIGN KEY (work, delivered_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO interjections (id, work, line, speaker, declared, relayed, text, delivered_round, cancelled_item, informs, matter, design_card)
SELECT id, work, line, speaker, declared, relayed, text, delivered_round, cancelled_item, informs, matter, design_card
FROM interjections_version_72;
DROP TABLE interjections_version_72;

-- Interjections still to hand in to a work.
CREATE INDEX interjections_waiting ON interjections (work) WHERE delivered_round IS NULL;
