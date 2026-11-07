-- Takes a database to one whose sayings to the owner and to the flow's keeper are the hub's messages, in one table:
-- what an agent told the owner and what the engine told of its own accord (owner_messages and engine_notices before)
-- are each a message, written once, and whom it went to, and what became of it there, is a delivery of it. A fault of
-- wf-agents goes to the flow's keeper, every other message to the owner, and a fault passed on to an architect is a
-- delivery to it, which names the matter it came as; the setting that forwarded each new fault to an architect goes,
-- since a fault is passed on only to the architect of the project wf-agents is built from. The engine's reminders of
-- the asks waiting are no messages: each is kept in reminders, for the next to be timed from.
--
-- What is in flight stays as it was: each message is sent at the time it was (an agent's at the end of the round that
-- handed it in, as the owner's conversations dated it), the ids following that order; each keeps whether and when it
-- was read; a fault forwarded keeps its matter, now named by its delivery to the architect, and its cache misses name
-- its message. The matters of the architects and the cache misses are made afresh with their rows, as SQLite cannot
-- change what a column refers to and a description is part of a table's text, which agents read; a migration runs
-- with the foreign keys off, and every one is checked before it commits.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE messages (
  -- Every message the hub carries to the owner, the flow's keeper or an architect, written once and never changed: a
  -- saying, which needs no answer. An agent's is what it tells the owner in a return, its answer to what the owner said
  -- to its work, or the architect's reply to the owner's words; the engine's is what it tells of its own accord: a work
  -- stopped, or gone back in a new flow version, every work halted until a usage limit resets, a limit passed, an ask of
  -- the owner put, a fault of wf-agents. Whom each went to, and what became of it there, is in deliveries.
  -- Read by: the owner's interface, which shows each in its conversation (conversation_entries); the engine, which adds
  -- a further cache miss to the message of a fault still open, and hands a fault passed on to an architect to it.
  id INTEGER PRIMARY KEY,
  -- Who sent it: engineer (a line of an engineer's work), architect (a line of a project's architect's work), or engine.
  sender TEXT NOT NULL CHECK (sender IN ('engineer', 'architect', 'engine')),
  -- The work it is about: an agent's, the work whose round handed it in; the engine's, the work it tells of, NULL for
  -- one about no work.
  work INTEGER REFERENCES works(id),
  -- An agent's: the round whose return holds it, by its number in the work; NULL for the engine's.
  round INTEGER,
  -- The message, as its sender wrote it.
  text TEXT NOT NULL,
  -- What more it says, JSON; NULL when nothing. For a fault of wf-agents, its kind (`kind`): cache_miss, rounds that
  -- missed their session's prompt cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a row
  -- counting a flow anomaly, where the way on an engineer chose did not fit the situation. For the engine's word of an
  -- ask of the owner put, that ask (`ask`). For an agent's answer to the owner's question about a decision card, that
  -- card (`design_card`), which the work's group shows it as a reply to. Read by: the owner's interface; the engine.
  body TEXT CHECK (body IS NULL OR json_valid(body)),
  -- When it was sent.
  at INTEGER NOT NULL,
  CHECK ((sender = 'engine') = (round IS NULL)),
  CHECK (round IS NULL OR work IS NOT NULL),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE deliveries (
  -- Each recipient a message went to, and what became of it there: the owner; the flow's keeper, a queue for
  -- wf-agents' own faults, which the owner and the assistant work; or a project's architect, to whose queue a fault of
  -- wf-agents was passed on. A fault goes to the keeper, and every other message to the owner.
  -- Read by: the owner's interface, which shows what is unread and where each fault went; the engine, which hands an
  -- architect the fault its matter is.
  id INTEGER PRIMARY KEY,
  -- The message delivered.
  message INTEGER NOT NULL REFERENCES messages(id),
  -- owner, keeper or architect.
  recipient TEXT NOT NULL CHECK (recipient IN ('owner', 'keeper', 'architect')),
  -- For an architect: the matter of its queue the message came as, which names the project; NULL for another
  -- recipient.
  matter INTEGER REFERENCES architect_matters(id),
  -- For the owner and the keeper: when it was marked read, or passed on to an architect; NULL while it is unread, and
  -- for an architect, whose matter says what became of it.
  read_at INTEGER,
  CHECK ((recipient = 'architect') = (matter IS NOT NULL)),
  CHECK (recipient <> 'architect' OR read_at IS NULL)
) STRICT;

CREATE TABLE reminders (
  -- Each reminder the engine gave the owner, through the notifications log, of the asks waiting for the owner's
  -- answer: one for all of them, once an interval of the installation's setting while any waits.
  -- Hidden from agents.
  -- Read by: the engine, which reminds again an interval after the last; people looking back at what the owner was
  -- reminded of.
  -- When it was given.
  at INTEGER NOT NULL,
  -- What it said: how many asks waited, and the oldest with how long it had waited.
  text TEXT NOT NULL
) STRICT;

-- Each saying, the agents' and the engine's but its reminders, with the id it takes: in the order sent.
CREATE TABLE sayings_before_the_hub AS
  SELECT row_number() OVER (ORDER BY at, source, old) AS id, source, old, sender, work, round, text, body, at, read_at,
    recipient
  FROM (
    SELECT 'message' AS source, m.id AS old,
      CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect' ELSE 'engineer' END AS sender, m.work AS work,
      m.round AS round, m.text AS text,
      CASE WHEN m.design_card IS NOT NULL THEN json_object('design_card', m.design_card) END AS body,
      coalesce(r.ended_at, r.started_at) AS at, m.read_at AS read_at, 'owner' AS recipient
    FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
    UNION ALL
    SELECT 'notice', n.id, 'engine', n.work, NULL, n.text,
      CASE WHEN n.fault IS NOT NULL AND n.ask IS NOT NULL THEN json_object('kind', n.fault, 'ask', n.ask)
           WHEN n.fault IS NOT NULL THEN json_object('kind', n.fault)
           WHEN n.ask IS NOT NULL THEN json_object('ask', n.ask) END,
      n.at, n.read_at, CASE WHEN n.fault IS NOT NULL THEN 'keeper' ELSE 'owner' END
    FROM engine_notices n WHERE n.reminder = 0);

INSERT INTO messages (id, sender, work, round, text, body, at)
  SELECT id, sender, work, round, text, body, at FROM sayings_before_the_hub ORDER BY id;
INSERT INTO deliveries (message, recipient, read_at)
  SELECT id, recipient, read_at FROM sayings_before_the_hub ORDER BY id;
INSERT INTO reminders (at, text) SELECT at, text FROM engine_notices WHERE reminder = 1 ORDER BY id;

CREATE TABLE architect_matters_before_the_hub AS SELECT * FROM architect_matters;
CREATE TABLE cache_misses_before_the_hub AS SELECT * FROM cache_misses;
DROP VIEW conversation_shown;
DROP VIEW conversation_entries;
DROP TABLE architect_matters;
DROP TABLE cache_misses;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents passed on to it, a
  -- design card to look at, a failure, the owner's words, a message from another project's architect, slots free with
  -- nothing of the project ready, main's CI red, a place its plan of what to do next found to set straight, or the
  -- check of its policy. The project's queue is its matters not handled; the engine takes them in a fixed order, by
  -- what waits on each: the owner's words and replies first, then main's CI red, then what a work stopped waits on, the
  -- one that came first; then the base when it is due; then what the plan queued; then what waited for the planning.
  -- Each is handled in a session of its own forked from the base. A matter something waits on starts a work of the
  -- project's architect when none runs; one that waits for the planning starts one only once it came before the base
  -- was built, or once as many wait as the installation's amount.
  -- Read by: the engine, which starts the architect's works, takes each matter, routes it to its path, hands it its
  -- material and takes its replies back; the architect's cells through architect_queue and work_handled; the owner's
  -- interface.
  id INTEGER PRIMARY KEY,
  -- The project whose architect it is for.
  project TEXT NOT NULL,
  -- Its kind, which decides the path it takes: set_straight, engineer_question, open_question, report, design_card,
  -- failure, owner_words, other_architect, slots_free, main_red (main's CI red, which the engine's watch of main
  -- queues: its body holds the commit, its red checks and their runs, and while one is open the project's merge queue
  -- is stopped), or check_policy, the check of the policy in force, one open at a time; slots_free and make_plan (the
  -- plan of what to do next, which a flow version before queued) are what a planning answers. An open question is what an engineer's design could not settle: its body holds the question, the options, the
  -- one recommended, the link to the design (design_link) and, once it has come, the answer (answer) with who gave it
  -- (answered_by: architect, or owner when the architect pinned the question for the owner and the owner answered), for
  -- the owner the ask that holds their words (owner_ask), and the time it came (answered_at, in milliseconds). A
  -- matter handled before these kinds keeps the kind it had then: set_policy for the setting of a new policy, which a
  -- flow version before ran as a stage of its own, and policy for a setting that ended, when the setting was a work of
  -- its own.
  kind TEXT NOT NULL,
  -- What more its kind says: an engineer's question's own kind (split, duplicate, ...); a report's (a side report's
  -- kind, followup for a report's what to do next, answer for an engineer's answer to what the architect said to its
  -- work, flow_feedback for what agents said was wrong with their cells, or fault for a fault of wf-agents passed on
  -- to it, by the owner or, before, by the owner's setting, which starts a work of it as a report does, its message
  -- being the one delivered to it as this matter); a failure's
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
  -- message, the place to set straight; for a fault passed on, who passed it on (by: owner, or setting for the setting
  -- that forwarded each new fault before), the fault itself being read from its message when the matter is handled.
  -- Read by: the engine, for the matter's material.
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
  -- The cell that asked, set with asked: where the matter goes back to with the owner's answer; for the approval of
  -- the draft of a new policy, the cell that wrote it, a reply of exactly approved going to where it takes effect
  -- instead.
  resume_cell TEXT,
  -- For a matter of two cells that its first cell sent back to the queue, as a flow version before did so that the
  -- owner's words that came meanwhile were taken first: its second cell, where it goes on (`H3` after `H2`, `H5` after
  -- `H4`, `P2` after `P1`). No cell sets it now, since a matter's cells follow one another in its own session; it is
  -- cleared once the matter is closed or a cell on its path asks the owner about it. Read by: the engine, which routes
  -- a matter that still holds one there.
  goes_on_at TEXT,
  -- For a side report or an engineer's answer a flow version before had the architect pick together with others of
  -- its kind in one pick, the matter that led the group: the one picked. No pick groups matters now, each being handled
  -- in a session of its own; it stays on a matter handled so, saying with which others it was handled. Read by: the
  -- owner's interface.
  group_lead INTEGER REFERENCES architect_matters(id),
  -- The architect's reply to an engineer's question or failure, JSON; NULL until it replied. The owner's answer in the
  -- work's group to a question the architect holds is the reply too, said by the owner (by), with who the caller
  -- declared to speak (speaker, owner or delegate). Read by: the engine, which takes it back unchanged to the work that
  -- asked.
  reply TEXT CHECK (reply IS NULL OR json_valid(reply)),
  -- A reply that waits for the second eye, JSON, as the round handed it in: it becomes the reply, with the numbers of
  -- the items made, once the second eye finds the round's changes sound. NULL otherwise.
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
  -- What the session that handled it left as it ended, JSON: its record (`record`), and the lines it adds to the plan
  -- (`plan`) and to the understanding (`understanding`) when it adds any, with the round that handed them in (`round`).
  -- NULL until then, and for a matter handled before the architect's matters were forked. Read by: the engine, which
  -- gives every matter handled after it what came since the base, and the next plan what came since the plan before;
  -- the owner's interface.
  logged TEXT CHECK (logged IS NULL OR json_valid(logged)),
  -- What the round handed in that waits for the second eye, JSON: its effects and its answer, made once the second eye
  -- finds the changes sound, or dropped when it sends them back. NULL otherwise. Read by: the engine.
  effects_held TEXT CHECK (effects_held IS NULL OR json_valid(effects_held)),
  CHECK ((work IS NULL) = (round IS NULL)),
  CHECK ((handled_work IS NULL) = (handled_round IS NULL)),
  CHECK ((asked IS NULL) = (resume_cell IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n),
  FOREIGN KEY (handled_work, handled_round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn, logged, effects_held)
  SELECT id, project, kind, detail, what, body, item, work, round, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn, logged, effects_held
  FROM architect_matters_before_the_hub;

-- A fault forwarded to an architect: a delivery to it, naming the matter it came as.
INSERT INTO deliveries (message, recipient, matter)
  SELECT s.id, 'architect', m.id
  FROM architect_matters_before_the_hub m JOIN sayings_before_the_hub s ON s.source = 'notice' AND s.old = m.notice
  ORDER BY m.id;
DROP TABLE architect_matters_before_the_hub;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and its first call still read from the cache less than half
  -- of what the session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents,
  -- whichever project it came up in: the engine tells the flow's keeper in a message of the fault, and a further miss
  -- with the same cause on the same line of the same project, while that message is unread or a matter it was passed
  -- on as waits in an architect's queue, adds to it instead of making a new one. A miss after what comes before the session's
  -- messages in the prompt changed since its round before (the model, the tools the envelope's schema is given with,
  -- the command line's own version), as when a work moved to a new flow version, is the command line writing the
  -- session again by design: no fault, kept for what it cost, with no message. Written once, never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the message a miss adds to, and hands the misses of a fault passed on to an
  -- architect to it; the owner's interface, which shows with each message how many misses it holds, the tokens they
  -- wrote again and what they cost, and counts the misses that are faults among what was spent for nothing.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line and each variable of its environment
  -- that differs from the session's round before, or from the keep-warm ping since it that recorded its call (as one
  -- that warmed an architect's base for a fork), with its value there and in this round; an empty list when none
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
  -- The message that tells the flow's keeper of it; NULL for a miss the engine expects, which is no fault.
  message INTEGER REFERENCES messages(id),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, message)
  SELECT c.work, c.round, c.line, c.differs, c.written, c.held, c.cost_usd, s.id
  FROM cache_misses_before_the_hub c LEFT JOIN sayings_before_the_hub s ON s.source = 'notice' AND s.old = c.notice;
DROP TABLE cache_misses_before_the_hub;

DROP TABLE sayings_before_the_hub;
DROP TABLE owner_messages;
DROP TABLE engine_notices;
DROP TABLE owner_choices;

CREATE INDEX cache_misses_by_message ON cache_misses (message);
CREATE INDEX messages_by_work ON messages (work, round);
CREATE INDEX deliveries_by_message ON deliveries (message);
CREATE UNIQUE INDEX deliveries_by_matter ON deliveries (matter) WHERE matter IS NOT NULL;
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE INDEX architect_matters_by_kind ON architect_matters (kind, at);
CREATE INDEX architect_matters_by_handling ON architect_matters (handled_work, handled_round);
CREATE INDEX architect_matters_by_work ON architect_matters (work, kind);
CREATE INDEX architect_matters_by_asked ON architect_matters (asked);
CREATE INDEX messages_by_ask ON messages (json_extract(body, '$.ask'));

CREATE VIEW conversation_entries AS
  -- Every entry of the owner's conversations, gathered from the tables that hold them: a group for each engineer's
  -- work, whose members are the owner, its engineer and its project's architect (what the engineer and the architect
  -- tell and ask the owner and each other about the work, the architect's messages while it handles a matter of the
  -- work, the owner's words to either or both and the architect's reply to its own, the design cards and the rulings
  -- on them, and the engine's notices about it); the owner's one-to-one with each project's architect (the owner's
  -- words to it and its replies, those a flow version before took while a talk with it was open being entries of that
  -- talk, what it tells and asks the owner about no work, in its works and in the settings of its policy a flow
  -- version before ran); one for the architects of projects the configuration connects (what they say
  -- to each other, and the owner's words to them); and one of wf-agents itself (its faults, and what the engine tells
  -- the owner about no work). The owner's words said to the architect's work in hand or to its policy setting while it
  -- runs, and its answers to them, are in the one-to-one, whatever matter it handles. A work's helpers (its advisor,
  -- review and adversarial testing) are in no group: what they and the engineer say to each other is in the work's
  -- rounds.
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations, counts what waits and what is unread in each, and
  -- shows each.
  -- conversation is work:<id>, architect:<project>, architects:<project>[|<project>] (the projects' architects; the
  -- interface takes it to the group of every project the configuration connects them with), or wf-agents. source is
  -- what the entry is (message, notice, ask, interjection, words, talk, question, report, reply, card, ruling or
  -- between) and id its row in the table it comes from; an ask is one entry, the owner's answer to it shown in it, never
  -- an entry of its own beside it. sender and recipient are a line (the architect for its base and for a matter's
  -- session forked from it), the owner, the engine or wf-agents, the recipient of
  -- the owner's words in a work's group the engineer, the architect or both, and in the one-to-one the line of the
  -- architect's work they were said to (architect or policy); for between, the projects. matter: the architect's
  -- matter the entry is, or holds the owner's words of, or whose talk it is an entry of. design_card: the decision
  -- card the entry is, or is a ruling on, or the owner's question asks about, or an answer answers about. waiting: it waits for the
  -- owner (an ask that stands for an open blocker the owner holds, an architect's ask that holds a matter still
  -- open, neither handled nor withdrawn, or an ask the work goes on without while it runs, not answered; a design card not ruled while its work goes on; a fault not
  -- read). pending: the owner's words not taken
  -- up yet. undelivered: the owner's words to an agent that its work ended without taking in. answerable: an
  -- engineer's question the architect holds now, which the owner may answer in the group instead.
  SELECT CASE WHEN w.graph = 'engineer' THEN 'work:' || w.id
              WHEN r.transition = 'answers' THEN 'architect:' || w.project
              ELSE coalesce((SELECT CASE WHEN y.graph = 'engineer' THEN 'work:' || x.work
                                         WHEN x.kind = 'owner_words' AND json_extract(x.body, '$.where') = 'work'
                                           THEN 'work:' || json_extract(x.body, '$.work') END
                             FROM architect_matters x LEFT JOIN works y ON y.id = x.work
                             WHERE x.id = (
                               SELECT coalesce(json_extract(p.output, '$.next.matter'),
                                 json_extract(p.output, '$.outcome.pick.matter'),
                                 json_extract(p.output, '$.queue.matter'))
                               FROM rounds p
                               WHERE p.work = m.work AND p.n < m.round AND p.transition IS NOT NULL
                                 AND coalesce(json_extract(p.output, '$.next.matter'),
                                   json_extract(p.output, '$.outcome.pick.matter'),
                                   json_extract(p.output, '$.queue.matter')) IS NOT NULL
                               ORDER BY p.n DESC LIMIT 1)
                               AND (x.handled_round IS NULL OR x.handled_work <> m.work OR x.handled_round >= m.round)),
                            'architect:' || w.project) END AS conversation,
    coalesce(w.project, i.project) AS project, 'message' AS source, m.id AS id,
    CASE WHEN w.graph NOT IN ('architect', 'policy') THEN coalesce(s.line, 'engineer')
         WHEN s.line IS NULL OR s.line IN ('architect', 'matter') THEN 'architect' ELSE s.line END AS sender,
    'owner' AS recipient, m.text AS text, m.at AS at, m.work AS work, m.round AS round, NULL AS ask,
    json_extract(m.body, '$.design_card') AS design_card, NULL AS matter, d.read_at AS read_at, 0 AS waiting,
    0 AS pending, 0 AS undelivered, 0 AS answerable, NULL AS fault
  FROM messages m JOIN deliveries d ON d.message = m.id AND d.recipient = 'owner'
  JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
  LEFT JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  UNION ALL
  SELECT CASE WHEN d.recipient = 'keeper' OR w.id IS NULL THEN 'wf-agents'
              WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    CASE WHEN d.recipient = 'owner' THEN coalesce(w.project, i.project) END, 'notice', n.id,
    CASE WHEN d.recipient = 'owner' THEN 'engine' ELSE 'wf-agents' END, 'owner', n.text, n.at, n.work, NULL,
    json_extract(n.body, '$.ask'), NULL, NULL, d.read_at, d.recipient = 'keeper' AND d.read_at IS NULL, 0, 0, 0,
    CASE WHEN d.recipient = 'keeper' THEN json_extract(n.body, '$.kind') END
  FROM messages n JOIN deliveries d ON d.message = n.id AND d.recipient IN ('owner', 'keeper')
  LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
  WHERE n.sender = 'engine' AND (json_extract(n.body, '$.ask') IS NULL OR d.recipient = 'keeper')
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'ask', a.id,
    CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = a.work AND x.n < a.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    'owner',
    coalesce(json_extract(a.body, '$.question'),
             (SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             (SELECT n.text FROM messages n WHERE json_extract(n.body, '$.ask') = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM messages n WHERE json_extract(n.body, '$.ask') = a.id), r.started_at), a.work, a.round, a.id, NULL,
    NULL, NULL,
    a.answer IS NULL AND (
      (w.graph = 'architect' AND EXISTS (
        SELECT 1 FROM architect_matters m WHERE m.asked = a.id AND m.handled_round IS NULL AND m.withdrawn IS NULL))
      OR EXISTS (SELECT 1 FROM blockers b WHERE b.ask = a.id AND b.ended IS NULL AND b.holder = 'owner')
      OR (w.ended IS NULL AND json_extract(a.body, '$.kind') IN ('design_review', 'points', 'change'))), 0, 0, 0, NULL
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'interjection', j.id, j.speaker,
    CASE WHEN j.matter IS NULL THEN j.line WHEN j.informs = 1 THEN 'architect' ELSE 'both' END,
    coalesce((SELECT json_extract(x.body, '$.text') FROM architect_matters x WHERE x.id = j.matter), j.text),
    coalesce((SELECT max(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)
                AND c.old_row IS NULL),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0),
    j.work, j.delivered_round, NULL, j.design_card, j.matter, NULL, 0,
    j.speaker = 'owner' AND ((j.informs = 0 AND j.delivered_round IS NULL AND w.ended IS NULL
        AND w.at NOT IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end'))
      OR EXISTS (SELECT 1 FROM architect_matters x
                 WHERE x.id = j.matter AND x.handled_round IS NULL AND x.withdrawn IS NULL)),
    j.speaker = 'owner' AND j.informs = 0 AND j.delivered_round IS NULL
      AND (w.ended IS NOT NULL OR w.at IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end')),
    0, NULL
  FROM interjections j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
  WHERE j.text NOT LIKE 'Ruling on design card %'
    AND j.text NOT LIKE 'The owner confirmed every decision of design card %'
    AND j.text NOT LIKE 'Ruling on decision card %'
    AND j.text NOT LIKE 'The owner confirmed decision card %'
  UNION ALL
  SELECT CASE WHEN json_extract(m.body, '$.where') = 'projects' THEN 'architects:' || m.project
              WHEN y.graph = 'engineer' THEN 'work:' || y.id ELSE 'architect:' || m.project END,
    m.project, 'words', m.id, 'owner',
    CASE WHEN json_array_length(m.body, '$.to') > 1 THEN 'architects' ELSE 'architect' END,
    json_extract(m.body, '$.text'), m.at, y.id, NULL, NULL, NULL, m.id, NULL, 0,
    m.handled_round IS NULL AND m.withdrawn IS NULL, 0, 0, NULL
  FROM architect_matters m LEFT JOIN works y ON y.id = json_extract(m.body, '$.work')
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.text') IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM interjections j WHERE j.matter = m.id)
    AND (json_extract(m.body, '$.where') IS NOT 'projects' OR json_extract(m.body, '$.to[0]') = m.project)
  UNION ALL
  SELECT 'architect:' || m.project, m.project, 'talk', e.id, 'owner', 'architect', e.text, e.at, NULL, NULL, NULL,
    NULL, e.matter, NULL, 0, e.taken_round IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL, 0, 0, NULL
  FROM talk_entries e JOIN architect_matters m ON m.id = e.matter
  UNION ALL
  SELECT 'work:' || m.work, m.project, CASE WHEN m.kind = 'report' THEN 'report' ELSE 'question' END, m.id,
    coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
              WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer'),
    'architect',
    coalesce((SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(m.body, '$.asked') q, json_each(q.value, '$.value.points') p), m.what),
    m.at, m.work, m.round, NULL, NULL, m.id, NULL, 0, 0, 0,
    m.reply IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL AND EXISTS (
      SELECT 1 FROM blockers b WHERE b.matter = m.id AND b.ended IS NULL AND b.holder = 'architect'),
    NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer' AND m.kind IN ('engineer_question', 'failure', 'report')
  UNION ALL
  SELECT 'work:' || m.work, m.project, 'reply', m.id,
    CASE WHEN json_extract(m.reply, '$.by') = 'owner' THEN 'owner' ELSE 'architect' END,
    CASE WHEN json_extract(m.reply, '$.hand_over.to') = 'owner' THEN 'owner'
         ELSE coalesce((SELECT s.line FROM rounds x JOIN sessions s ON s.id = x.session
                        WHERE x.work = m.work AND x.n <= m.round ORDER BY x.n DESC LIMIT 1), 'engineer') END,
    coalesce(json_extract(m.reply, '$.text'), ''),
    coalesce(json_extract(m.reply, '$.at'),
             (SELECT x.ended_at FROM rounds x WHERE x.work = m.handled_work AND x.n = m.handled_round), m.at),
    m.work, m.round, NULL, NULL, m.id, NULL, 0, 0, 0, 0, NULL
  FROM architect_matters m JOIN works w ON w.id = m.work
  WHERE w.graph = 'engineer' AND m.reply IS NOT NULL
  UNION ALL
  SELECT 'work:' || c.work, i.project, 'card', c.id, coalesce(s.line, 'engineer'), 'owner', '',
    coalesce(cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, NULL,
    c.status = 'open' AND w.ended IS NULL, 0, 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round LEFT JOIN sessions s ON s.id = cr.session
  WHERE c.status IN ('open', 'ruled')
  UNION ALL
  SELECT 'work:' || c.work, i.project, 'ruling', c.id, 'owner', 'engineer', '',
    coalesce(json_extract(x.value, '$.at'), cr.ended_at, cr.started_at), c.work, c.round, NULL, c.id, NULL, NULL, 0,
    0, 0, 0, NULL
  FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
  JOIN rounds cr ON cr.work = c.work AND cr.n = c.round, json_each(c.rulings) x
  WHERE c.status IN ('open', 'ruled') AND json_extract(x.value, '$.by') = 'owner'
  UNION ALL
  SELECT 'architects:' || min(json_extract(m.body, '$.from'), m.project) || '|'
      || max(json_extract(m.body, '$.from'), m.project),
    NULL, 'between', m.id, json_extract(m.body, '$.from'), m.project, coalesce(json_extract(m.body, '$.text'), m.what),
    m.at, json_extract(m.body, '$.work'), NULL, NULL, NULL, m.id, NULL, 0, 0, 0, 0, NULL
  FROM architect_matters m
  WHERE m.kind = 'other_architect' AND json_extract(m.body, '$.from') IS NOT NULL;

CREATE VIEW conversation_shown AS
  -- The entries of the owner's conversations that the owner's page shows and the architect is told of. They are
  -- those of conversation_entries but the engine's notices in a work's group and in an architect's one-to-one, which
  -- the bubble of the stage the work was at, the bubble of the matter the architect has in hand and their heads say
  -- instead; the side reports a work's lines handed in, in a work's group, which are the architect's to read in its
  -- queue; and what a helper of the work (the review, the adversarial testing, the advisor) or of the architect (the
  -- review of its batch, the search of its observations) told the owner, whose words go to the one it helps: a work's
  -- round's progress note in the stage's bubble says it, and the architect reads the rest. This is the one
  -- definition, so that what the page shows of a conversation and what the architect is told of its one-to-one are the
  -- same entries.
  -- Hidden from agents.
  -- Read by: the owner's interface, which lists the conversations and shows each; the engine, which gives a new session
  -- of an architect what was said in its one-to-one since its previous session began.
  SELECT * FROM conversation_entries
  WHERE NOT ((conversation LIKE 'work:%' OR conversation LIKE 'architect:%') AND source = 'notice')
    AND NOT (conversation LIKE 'work:%' AND source = 'report')
    AND NOT ((conversation LIKE 'work:%' OR conversation LIKE 'architect:%') AND source = 'message'
      AND sender NOT IN ('engineer', 'architect', 'policy'));
