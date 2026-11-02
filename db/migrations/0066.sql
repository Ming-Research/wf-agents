-- Takes a version 65 database to version 66: the architect records the owner's decisions in the items, each line
-- saying who decided it, so the head they stand under, `Owner's decisions:`, no longer says what is in it: a line may
-- be the architect's decision or a delegate's. The head of every item's approach is renamed `Decisions:`, its lines
-- left as they are. The owner's words and the owner's answers say, in their descriptions, who the caller declared to
-- speak, so the architects' matters and the owner's asks are made afresh with their rows; an interjection keeps who was
-- declared to speak when the owner said it to the engineer alone, so that table is made afresh too, and an open
-- question answered keeps the time of its answer. A new view gives the
-- engineer's last cell what the owner said that concerns a work, and the view of a work's open questions says in its
-- description that their answers are the architect's. Each statement below that defines something is written exactly
-- as db/schema.sql writes it.

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

CREATE TABLE owner_asks_version_65 AS SELECT * FROM owner_asks;
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
  -- What is asked, JSON: whom it asks, the work and item, and what the asking cell handed in. Copied from what that
  -- round was given, which never changes.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The owner's answer, JSON: who answered and when, and the text, and who the caller declared to speak (`speaker`,
  -- owner or delegate; none for an answer kept before it was declared); for an ask of the architect, the rest of the
  -- architect's reply too. NULL while it waits. Read by: the engine; cells `R1` and `L2`.
  answer TEXT CHECK (answer IS NULL OR json_valid(answer)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO owner_asks (id, work, round, body, answer)
SELECT id, work, round, body, answer FROM owner_asks_version_65;
DROP TABLE owner_asks_version_65;
CREATE INDEX owner_asks_by_round ON owner_asks (work, round);

CREATE TABLE architect_matters_version_65 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, a place its plan of what to do next found to set straight, or the check of its
  -- policy or the plan of what to do next, which come at the head of its queue. The project's queue is its
  -- matters not handled; its architect picks among them by the constitution and the policy, not by the order they
  -- came. A matter in the queue starts a work of the project's architect when none runs, but for flow feedback, the
  -- project's own or forwarded by another project's architect, which waits for a work something else started and is
  -- picked only when it came before that work started.
  -- Read by: the engine, which starts the architect's works, picks a lone matter, routes each to its path, hands it its
  -- material and takes its replies back; the architect's cells through architect_queue and work_handled; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, open_question, report, design_card,
  -- failure, owner_words, other_architect, slots_free, or check_policy and make_plan, the check of the policy in force
  -- and the plan of what to do next, which the architect's cells queue at the head of the queue, one of each open at a
  -- time. An open question is what an engineer's design could not settle: its body holds the question, the options, the
  -- one recommended and, once it has come, the answer and the time it came (`answered_at`, in milliseconds). A
  -- matter handled before these kinds keeps the kind it had then: set_policy for the setting of a new policy, which a
  -- flow version before ran as a stage of its own, and policy for a setting that ended, when the setting was a work of
  -- its own.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, followup for a report's what to do next, answer for an engineer's answer to what the architect said to its
  -- work, flow_feedback for what agents said was wrong with their cells, or fault for a fault of wf-agents forwarded
  -- to it, by the owner or by the owner's setting, which starts a work of it as a report does); a failure's
  -- (tool_failed, environment_changed); the owner's words' (said, answer, release, approved_changes); a message's
  -- (message, reply, or flow_feedback for flow feedback another project's architect forwarded); the kind of a place to
  -- set straight. NULL when there is nothing more. Read by: the engine, for the matter's material and whether it starts a work of
  -- the architect; the owner's interface.
  detail TEXT,
  -- What it is, in one sentence, as the engine wrote it.
  what TEXT NOT NULL,
  -- Its material, JSON: what the asking cell handed in, the side report, a report's what to do next with its paths and
  -- the commit merged, the design card, the tool that failed, the owner's words (with where they were said, and who
  -- the caller declared to speak: `speaker`, owner or delegate; none for words kept before it was declared), the
  -- message, the place to set straight; for a fault forwarded, who forwarded it (by: owner, or setting for the owner's
  -- setting), the fault itself being read from its notice when the matter is handled. Read by: the engine, for the
  -- matter's material.
  body TEXT NOT NULL CHECK (json_valid(body)),
  -- The item it concerns; NULL when it concerns none.
  item INTEGER REFERENCES items(id),
  -- The round that raised it: an engineer's round at the architect's cell, a round that handed in a side report or a
  -- design card, an architect's round; NULL for one the owner or the engine raised.
  work INTEGER,
  round INTEGER,
  -- For a fault of wf-agents forwarded to it: the engine's notice that reports the fault, which goes to each
  -- project's architect once; NULL for any other matter. Read by: the engine, which hands the architect the notice
  -- and the engine's evidence of the fault as they stand when the matter is handled, and adds a further cache miss to
  -- a notice forwarded while its matter waits; the owner's interface, which shows where each fault went and how it
  -- stands there.
  notice INTEGER REFERENCES engine_notices(id),
  -- When it came. Read by: the queue, for how long it waited.
  at INTEGER NOT NULL,
  -- The ask of the owner a cell on its path put about it, which holds it while the owner has not answered: it is then
  -- out of the queue, and once answered it is back, and goes back to the cell that asked, with the answer. NULL when no
  -- cell asked about it. Read by: the queue; the engine, which routes it and gives the answer.
  asked INTEGER REFERENCES owner_asks(id),
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
  resume_cell TEXT,
  -- For a matter of two cells that its first cell sent back to the queue, so that the owner's words that came meanwhile
  -- are taken first: its second cell, where it goes on (`H3` after `H2`, `H5` after `H4`, `P2` after `P1`). NULL
  -- otherwise, and again once it is closed or a cell on its path asks the owner about it. Read by: the queue, which
  -- takes it again before any other matter but the owner's words, a reply and the waits; the engine, which routes it.
  goes_on_at TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by), with who the caller
  -- declared to speak (speaker, owner or delegate). Read by: the engine, which takes it back unchanged to the work that
  -- asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its work and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; the owner ruled the design card it was to look at, or another card replaced it, saying which;
  -- the owner answered the question in the work's group, the reply saying what; it said that a setting of the
  -- policy, a work of its own then, had ended, when the setting became a stage of the architect's works; or it was a
  -- setting of the policy as a stage of its own, which runs so no longer. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, reply, reply_held, handled_work, handled_round, withdrawn)
SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, reply, reply_held, handled_work, handled_round, withdrawn FROM architect_matters_version_65;
DROP TABLE architect_matters_version_65;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

-- An open question answered before the time of its answer was kept in its body gets the time the change that wrote the
-- answer was made; one the record of changes does not hold keeps none, and a decision cannot quote it.
UPDATE architect_matters SET body = json_set(body, '$.answered_at', t.at)
FROM (
  SELECT json_extract(c.row_key, '$.id') AS id, max(c.at) AS at
  FROM changes c
  WHERE c.table_name = 'architect_matters'
    AND json_extract(c.new_row, '$.body.answer') IS NOT NULL
    AND json_extract(c.old_row, '$.body.answer') IS NOT json_extract(c.new_row, '$.body.answer')
  GROUP BY 1
) t
WHERE architect_matters.id = t.id AND architect_matters.kind = 'open_question'
  AND json_extract(architect_matters.body, '$.answered_at') IS NULL;

CREATE TABLE interjections_version_65 AS SELECT * FROM interjections;
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
  -- Their words as said.
  text TEXT NOT NULL,
  -- The round of the work it was handed in, by its number in the work; NULL until then. Read by: the engine, to find
  -- what is still to hand in.
  delivered_round INTEGER,
  -- For the notice that an item this work's item depended on was cancelled: that item; NULL for anything else said.
  -- The cells where the work waits (`A2`, `A6`, `M1`) take such a notice to choose the way on by a row of their own,
  -- as an item they waited for that will never deliver. Read by: the engine's value `cancelled`.
  cancelled_item INTEGER REFERENCES items(id),
  -- 1 for a word that needs no answer and changes no way on, as the owner confirming every decision of a design card:
  -- the line's next round is given it, and a work standing at an engine step is not taken by it to choose the way on.
  -- Read by: the engine's value `interjection`.
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

INSERT INTO interjections (id, work, line, speaker, text, delivered_round, cancelled_item, informs, matter, design_card)
SELECT id, work, line, speaker, text, delivered_round, cancelled_item, informs, matter, design_card FROM interjections_version_65;
DROP TABLE interjections_version_65;
CREATE INDEX interjections_waiting ON interjections (work) WHERE delivered_round IS NULL;

CREATE VIEW work_open_questions AS
  -- Each work's open questions as its design handed them in, each a matter of the project's architect, with the answer
  -- that came, which is the architect's reply: its own judgment, or the owner's answer it carried. Read by: cells `D1`,
  -- `D2` and `L2`; the engine, at cell `M1`.
  SELECT m.work, m.id AS matter, m.what AS question, json_extract(m.body, '$.answer') AS answer
  FROM architect_matters m
  WHERE m.kind = 'open_question';

CREATE VIEW work_owner_words AS
  -- What the owner said that concerns a work, one row for each record that holds it: the words said in the work's
  -- group to its project's architect (a matter of the kind owner_words), the owner's answer in the group to a question
  -- of its engineer's (the reply of a matter of the kind engineer_question), the words said to its engineer alone (an
  -- interjection no matter holds) and the owner's answers to the work's asks. A decision cites a row by its source and
  -- its id and quotes its words; the engine reads the words from the record again, and tells who said them from what
  -- the record kept. speaker is what the record kept of who was declared to speak, owner or delegate; none for a record
  -- kept before it was declared. Read by: cell `L2`.
  SELECT json_extract(m.body, '$.work') AS work, 'matter' AS source, m.id AS id,
    json_extract(m.body, '$.speaker') AS speaker,
    coalesce(json_extract(m.body, '$.text'), json_extract(m.body, '$.answer')) AS words, m.at AS at
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.where') = 'work'
  UNION ALL
  SELECT m.work, 'matter', m.id, json_extract(m.reply, '$.speaker'), json_extract(m.reply, '$.text'),
    coalesce(json_extract(m.reply, '$.at'), 0)
  FROM architect_matters m
  WHERE m.kind = 'engineer_question' AND json_extract(m.reply, '$.by') = 'owner'
  UNION ALL
  SELECT j.work, 'interjection', j.id, j.declared, j.text,
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)), 0)
  FROM interjections j
  WHERE j.speaker = 'owner' AND j.matter IS NULL AND j.cancelled_item IS NULL
  UNION ALL
  SELECT a.work, 'ask', a.id, json_extract(a.answer, '$.speaker'), json_extract(a.answer, '$.text'),
    coalesce(json_extract(a.answer, '$.at'), 0)
  FROM owner_asks a
  WHERE json_extract(a.answer, '$.by') = 'owner';
