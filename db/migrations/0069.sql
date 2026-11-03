-- Takes a version 68 database to version 69: the architect picks side reports and engineers' answers together, a group
-- of at most eight that one round of `H4` and one round of `H5` answer for, each matter with a finding and a way of its
-- own. The matter that leads the group is the one picked; the others name it in a new column of the architects'
-- matters, so that table is made afresh with its rows, none of them in a group. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

CREATE TABLE architect_matters_version_68 AS SELECT * FROM architect_matters;
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
  -- one recommended, the link to the design (design_link) and, once it has come, the answer (answer) with who gave it
  -- (answered_by: architect, or owner when the architect pinned the question for the owner and the owner answered) and,
  -- for the owner, the ask that holds their words (owner_ask). A
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
  resume_cell TEXT,
  -- For a matter of two cells that its first cell sent back to the queue, so that the owner's words that came meanwhile
  -- are taken first: its second cell, where it goes on (`H3` after `H2`, `H5` after `H4`, `P2` after `P1`). NULL
  -- otherwise, and again once it is closed or a cell on its path asks the owner about it. Read by: the queue, which
  -- takes it again before any other matter but the owner's words, a reply and the waits; the engine, which routes it.
  goes_on_at TEXT,
  -- For a side report or an engineer's answer the architect picked together with others of its kind in one pick (the
  -- pick's `with`), the matter that leads the group: the one that was picked. NULL for a matter picked alone, and for
  -- the lead itself. A group is its lead and the matters not handled whose group_lead is the lead; the lead's cells
  -- answer for each of them, and one round closes them all. It is cleared for the matters of a group when the lead is
  -- parked for the owner's reply (the group falls apart, each goes its own way) and for any not handled when the next
  -- pick is made (a group left by a work that ended before it was handled); it stays on a matter handled, saying with
  -- which others it was handled. Read by: the engine, which gives the lead's cells each matter of the group, closes
  -- them together and tells each one's reporter what became of it; the owner's interface.
  group_lead INTEGER REFERENCES architect_matters(id),
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
SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, reply, reply_held, handled_work, handled_round, withdrawn FROM architect_matters_version_68;
DROP TABLE architect_matters_version_68;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;
