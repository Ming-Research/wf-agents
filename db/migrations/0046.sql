-- Takes a version 45 database to version 46: the owner's words to a project's architect are a talk with it, which the
-- architect takes first and answers, waiting for the owner's reply; once the wait runs out the talk is held out of the
-- queue until the owner says more or ends it, and the owner ends it by Done or by deciding on the study or the text of
-- a new policy that waited meanwhile. The architects' matters say when a talk was held so and when the owner ended it,
-- so that table is made afresh with its rows, and the view of each project's queue leaves a talk held out until the
-- owner's next words. Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW architect_queue;

CREATE TABLE architect_matters_version_45 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, or a place its understanding found to set straight. The project's queue is its
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
  -- Its kind, which decides the path it takes: set_straight, engineer_question, report, design_card, failure,
  -- owner_words, other_architect, slots_free, or set_policy for the setting of a new policy, which the check of the
  -- policy in force raises and takes in hand at once, and which may go on over several works of the architect. A
  -- matter handled before these kinds keeps the kind it had then, as policy did for a setting that ended, when the
  -- setting was a work of its own.
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
  -- the commit merged, the design card, the tool that failed, the owner's words, the message, the place to set
  -- straight; for a fault forwarded, who forwarded it (by: owner, or setting for the owner's setting), the fault
  -- itself being read from its notice when the matter is handled. Read by: the engine, for the matter's material.
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the owner's
  -- reading of a study or approval of a policy, the cell that put it, whose rows go on by the owner's choice.
  resume_cell TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by). Read by: the engine,
  -- which takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its work and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; the owner answered the question in the work's group, the reply saying what; or it said that a
  -- setting of the policy, a work of its own then, had ended, when the setting became a stage of the architect's
  -- works. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  -- For the owner's words, which the architect talks with the owner about (cell `H8`): when the architect went on with
  -- its queue, its wait for the owner's reply having run out (cell `H11`), holding the talk out of the queue until the
  -- owner says more to it in their one-to-one or ends the talk; NULL while it was never held so. Read by:
  -- architect_queue; the owner's interface, which shows the talk waiting for the owner's reply.
  talk_held INTEGER,
  -- For the owner's words the architect talks with the owner about: when the owner ended the talk, by Done or by
  -- deciding on the study or the text of a new policy that waited meanwhile; NULL while the owner has not. Read by: the
  -- engine, whose next turn of the talk hands in what it came to; architect_queue.
  talk_ended INTEGER,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, reply, reply_held, handled_work, handled_round, withdrawn)
SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, reply, reply_held, handled_work, handled_round, withdrawn
FROM architect_matters_version_45;
DROP TABLE architect_matters_version_45;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE INDEX architect_matters_settings ON architect_matters (project, id) WHERE kind = 'set_policy';
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn, not held by an ask of the owner still waiting and
  -- not held by a talk with the owner that waits for the owner's reply, each with who waits for it (an agent: an
  -- engineer stopped at the architect's cell for its question; the owner, for the owner's words; or no one), when it
  -- came and how long it has waited, whether it comes back with the owner's answer, and whether it is urgent. A talk
  -- held is due again once the owner said more to the architect in their one-to-one since, or ended it. Read by: cells
  -- `S0`, `S1` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no work, and
  -- gives the architect its queue; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AS answered,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
    AND (m.talk_held IS NULL OR m.talk_ended IS NOT NULL OR EXISTS (
      SELECT 1 FROM architect_matters x
      WHERE x.project = m.project AND x.kind = 'owner_words' AND json_extract(x.body, '$.where') = 'page'
        AND x.handled_round IS NULL AND x.withdrawn IS NULL AND x.at > m.talk_held))
  ORDER BY m.id;
