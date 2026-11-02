-- Takes a version 65 database to version 66: the architect records the owner's decisions in the items, each line
-- saying who decided it, so the head they stand under, `Owner's decisions:`, no longer says what is in it: a line may
-- be the architect's decision or a delegate's. The head of every item's approach is renamed `Decisions:`, its lines
-- left as they are. A new view gives the engineer's last cell what the owner said that concerns a work, and the view of
-- a work's open questions says in its description that their answers are the architect's. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

-- The rewrite below is the migration's, not a writer's, so the trigger that records changes to the rows it writes is
-- dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_items_update;

-- Only a line that is exactly the old head is renamed; the text is taken between two line breaks so that a head at the
-- first line or at the last is found too, and cut back to its own length after.
UPDATE items SET approach = substr(
  replace(char(10) || approach || char(10), char(10) || 'Owner''s decisions:' || char(10), char(10) || 'Decisions:' || char(10)),
  2,
  length(replace(char(10) || approach || char(10), char(10) || 'Owner''s decisions:' || char(10), char(10) || 'Decisions:' || char(10))) - 2)
WHERE instr(char(10) || approach || char(10), char(10) || 'Owner''s decisions:' || char(10)) > 0;

DROP VIEW work_open_questions;

CREATE VIEW work_open_questions AS
  -- Each work's open questions as its design handed them in, each a matter of the project's architect, with the answer
  -- that came, which is the architect's reply: its own judgment, or the owner's answer it carried. Read by: cells `D1`,
  -- `D2` and `L2`; the engine, at cell `M1`.
  SELECT m.work, m.id AS matter, m.what AS question, json_extract(m.body, '$.answer') AS answer
  FROM architect_matters m
  WHERE m.kind = 'open_question';

CREATE VIEW work_owner_words AS
  -- What the owner said that concerns a work, one row for each record that holds it: the words said in the work's
  -- group to its project's architect (a matter of the kind owner_words), the words said to its engineer alone (an
  -- interjection no matter holds) and the owner's answers to the work's asks. A decision cites a row by its source and
  -- its id and quotes its words; the engine reads the words from the record again, and tells who said them from what
  -- the record kept. speaker is what the record kept of who was declared to speak, owner or delegate; none for a record
  -- kept before it was declared, and for an interjection, which keeps none. Read by: cell `L2`.
  SELECT json_extract(m.body, '$.work') AS work, 'matter' AS source, m.id AS id,
    json_extract(m.body, '$.speaker') AS speaker,
    coalesce(json_extract(m.body, '$.text'), json_extract(m.body, '$.answer')) AS words, m.at AS at
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.where') = 'work'
  UNION ALL
  SELECT j.work, 'interjection', j.id, NULL, j.text,
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)), 0)
  FROM interjections j
  WHERE j.speaker = 'owner' AND j.matter IS NULL AND j.cancelled_item IS NULL
  UNION ALL
  SELECT a.work, 'ask', a.id, json_extract(a.answer, '$.speaker'), json_extract(a.answer, '$.text'),
    coalesce(json_extract(a.answer, '$.at'), 0)
  FROM owner_asks a
  WHERE json_extract(a.answer, '$.by') = 'owner';
