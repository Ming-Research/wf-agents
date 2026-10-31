-- Takes a version 39 database to version 40: a fault of wf-agents can be forwarded to a project's architect, by the
-- owner by hand or by the owner's setting as each new one comes, as a matter of the kind report with the detail fault.
-- The architects' matters keep the notice each such matter came from, which goes to each project's architect once;
-- the descriptions of the engine's notices, the cache misses and the architects' matters say so, so those tables are
-- made afresh with their rows; and the owner's choices, which keep that setting, are new. Each statement below that
-- defines something is written exactly as db/schema.sql writes it.

CREATE TABLE engine_notices_version_39 AS SELECT * FROM engine_notices;
DROP TABLE engine_notices;

CREATE TABLE engine_notices (
  -- What the engine tells the owner of its own accord, one row for each notice: a work stopped by a new flow version,
  -- a design card posted, every work halted until a usage limit resets, a fault of wf-agents, and the like. It needs no
  -- answer. The same text goes to the notifications log.
  -- Hidden from agents.
  -- Read by: the owner's interface, which shows those unread with the messages agents wrote; the engine, for a fault
  -- of wf-agents (fault).
  id INTEGER PRIMARY KEY,
  -- The work it is about; NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- The notice, as the engine wrote it.
  text TEXT NOT NULL,
  -- When the engine wrote it.
  at INTEGER NOT NULL,
  -- When the owner marked it read, or forwarded the fault it reports to an architect, by hand or by the owner's
  -- setting; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER,
  -- When the notice is of a new ask of the owner: that ask, for the owner's interface to link to. Read by: the
  -- owner's interface; the engine, which hands a flow anomaly's ask to the architect it is forwarded to.
  ask INTEGER REFERENCES owner_asks(id),
  -- When the notice reports a fault of wf-agents, its kind: cache_miss, rounds that missed their session's prompt
  -- cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a row counting a flow anomaly,
  -- where the way on an engineer chose did not fit the situation. wf-agents runs other projects too, and such a fault
  -- is its own, to be fixed in wf-agents whichever project it came up in. NULL for another notice. Read by: the
  -- owner's interface, which sets such a notice apart and forwards it to the architects the owner picks, each as a
  -- matter of its queue; the engine, which forwards a new one to the architect the owner's setting names
  -- (owner_choices), and hands an architect the notice of a fault forwarded to it.
  fault TEXT
) STRICT;

INSERT INTO engine_notices (id, work, text, at, read_at, ask, fault)
SELECT id, work, text, at, read_at, ask, fault FROM engine_notices_version_39;
DROP TABLE engine_notices_version_39;

CREATE TABLE cache_misses_version_39 AS SELECT * FROM cache_misses;
DROP TABLE cache_misses;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and its first call still read from the cache less than half
  -- of what the session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents,
  -- whichever project it came up in: the engine tells the owner in a notice of the fault, and a further miss with the
  -- same cause on the same line of the same project, while that notice is unread or a matter forwarded from it waits
  -- in an architect's queue, adds to it instead of making a new one. Written once, never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the notice a miss adds to, and hands the misses of a fault forwarded to an
  -- architect to it; the owner's interface, which shows with each notice how many misses it holds, the tokens they
  -- wrote again and what they cost.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line and each variable of its environment
  -- that differs from the session's round before, with its value there and in this round; an empty list when none
  -- differs.
  differs TEXT NOT NULL CHECK (json_valid(differs)),
  -- The tokens of what the session held that the round's first call did not read from the cache, and so wrote to it
  -- again.
  written INTEGER NOT NULL CHECK (written >= 0),
  -- The tokens the session held: the last prompt of its round before, as recorded.
  held INTEGER NOT NULL CHECK (held > 0),
  -- What writing them again cost beyond reading them, in US dollars, priced from what the round cost; NULL when its
  -- record does not tell.
  cost_usd REAL,
  -- The notice that tells the owner of it.
  notice INTEGER NOT NULL REFERENCES engine_notices(id),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, notice)
SELECT work, round, line, differs, written, held, cost_usd, notice FROM cache_misses_version_39;
DROP TABLE cache_misses_version_39;

CREATE INDEX cache_misses_by_notice ON cache_misses (notice);

CREATE TABLE owner_choices (
  -- The owner's settings on the page that hold across restarts of the engine and pick one of several, which a switch
  -- (owner_switches) cannot hold: one row for each the owner set; a setting with no row is as it is at first. Who set
  -- one, and when, is in changes, a setting put back as it is at first being its row deleted.
  -- Hidden from agents.
  -- Read by: the engine; the owner's interface.
  -- forward_faults: the project whose architect each new fault of wf-agents is forwarded to as it comes, as a matter
  -- of its queue, the notice then read; with no row, nobody's, and the owner forwards a fault by hand or not at all.
  name TEXT PRIMARY KEY CHECK (name IN ('forward_faults')),
  -- What the owner picked: for forward_faults, a project of the configuration.
  value TEXT NOT NULL
) STRICT;

CREATE TABLE architect_matters_version_39 AS SELECT * FROM architect_matters;
DROP TABLE architect_matters;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, or a place its understanding found to set straight. The project's queue is its
  -- matters not handled; its architect picks among them by the constitution and the policy, not by the order they
  -- came. A matter in the queue wakes the project's architect, but for flow feedback, the project's own or forwarded by
  -- another project's architect, which waits for a wake something else started and is picked only when it came before
  -- that wake started.
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
  -- work, flow_feedback for what agents said was wrong with their cells, or fault for a fault of wf-agents forwarded
  -- to it, by the owner or by the owner's setting, which wakes it as a report does); a failure's (tool_failed,
  -- environment_changed); the owner's words' (said, answer, release, approved_changes); a message's (message, reply,
  -- or flow_feedback for flow feedback another project's architect forwarded); the kind of a place to set straight.
  -- NULL when there is nothing more. Read by: the engine, for the matter's material and whether it wakes the
  -- architect; the owner's interface.
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer.
  resume_cell TEXT,
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by). Read by: the engine,
  -- which takes it back unchanged to the work that asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the item changes of its round to be made, JSON, as the round handed it in: it becomes the
  -- reply, with the numbers of the items they made, once the review of their batch lets them be made. NULL otherwise.
  reply_held TEXT CHECK (reply_held IS NULL OR json_valid(reply_held)),
  -- The architect's round that handled it: its wake and the round that closed it; NULL while it is not handled.
  handled_work INTEGER,
  handled_round INTEGER,
  -- Why it left the queue unhandled: the work it waits on, which raised it (a question, a failure, a design card),
  -- ended, saying how; or the owner answered the question in the work's group, the reply saying what. NULL otherwise.
  -- Read by: architect_queue, which leaves it out; the engine, which tells a new work on the item what an earlier
  -- work asked and had withdrawn; the owner's interface, which leaves it out of the queue.
  withdrawn TEXT,
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (
  id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, reply, reply_held, handled_work,
  handled_round, withdrawn)
SELECT
  id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, reply, reply_held, handled_work,
  handled_round, withdrawn
FROM architect_matters_version_39;
DROP TABLE architect_matters_version_39;

CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;
