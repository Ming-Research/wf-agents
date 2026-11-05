-- Takes a database to the architect whose base only saves tokens and whose matters add to it. A matter's record replaces
-- its line of the decision log and its two answers, so what each matter left is written anew as a record, with no answer
-- kept: the answers fed the rules that called for a new base, and none reads them now. The tables' descriptions speak of
-- the record and of the notebook's two parts, and a description is part of a table's text, which agents read, so the
-- architect's matters and the notebook's versions are made afresh with their rows; a migration runs with the foreign keys
-- off, as SQLite makes a table afresh, and every one is checked before the migration commits. Only the matter a work's
-- question stands for counts as a work waiting on the architect, and a matter's record is what a handled matter said, so
-- the views over them are made afresh too. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

CREATE TABLE architect_matters_before_the_record AS SELECT * FROM architect_matters;
CREATE TABLE notebook_edits_before_the_record AS SELECT * FROM notebook_edits;
DROP VIEW conversation_shown;
DROP VIEW conversation_entries;
DROP VIEW notebook;
DROP VIEW policy_current;
DROP VIEW work_handled;
DROP VIEW architect_queue;
DROP VIEW work_owner_words;
DROP VIEW work_open_questions;
DROP TABLE architect_matters;
DROP TABLE notebook_edits;

CREATE TABLE architect_matters (
  -- A matter for a project's architect: an engineer's question, a report, a fault of wf-agents forwarded to it, a
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

INSERT INTO architect_matters (id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn, logged, effects_held)
  SELECT id, project, kind, detail, what, body, item, work, round, notice, at, asked, resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn, CASE WHEN logged IS NULL THEN NULL ELSE json_object('record', json_extract(logged, '$.log'), 'round', json_extract(logged, '$.round'), 'cell', json_extract(logged, '$.cell')) END, effects_held
  FROM architect_matters_before_the_record;
DROP TABLE architect_matters_before_the_record;

CREATE TABLE notebook_edits (
  -- Each version of a part of a project's architect's notebook, as a round of its base wrote it whole: its
  -- understanding of the project (project_now) and its plan (plan), in the owner's language; unclear and other are
  -- sections earlier flow versions wrote, kept as they were. A part is what its latest version says; the versions
  -- before are kept for the owner.
  -- A new base writes its understanding from nothing, and the next plan is given the plan before.
  -- Hidden from agents.
  -- Read by: the engine, which gives the next plan the plan before, through notebook; the owner's interface, which
  -- shows each part with its versions.
  id INTEGER PRIMARY KEY,
  -- The project whose architect's notebook it is.
  project TEXT NOT NULL,
  -- Which section.
  section TEXT NOT NULL CHECK (section IN ('project_now', 'plan', 'unclear', 'other')),
  -- The section's whole text in this version.
  text TEXT NOT NULL,
  -- The round of the architect's line that wrote it: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO notebook_edits (id, project, section, text, work, round)
  SELECT id, project, section, text, work, round FROM notebook_edits_before_the_record;
DROP TABLE notebook_edits_before_the_record;

CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE UNIQUE INDEX architect_matters_by_notice ON architect_matters (notice, project) WHERE notice IS NOT NULL;
CREATE INDEX notebook_edits_by_section ON notebook_edits (project, section);

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
  -- kept before it was declared. relayed is, for words a delegate said, the parts of them it declared to be the owner's
  -- own words, a JSON array; none when it declared none. Read by: cell `L2`.
  SELECT json_extract(m.body, '$.work') AS work, 'matter' AS source, m.id AS id,
    json_extract(m.body, '$.speaker') AS speaker,
    coalesce(json_extract(m.body, '$.text'), json_extract(m.body, '$.answer')) AS words, m.at AS at,
    json_extract(m.body, '$.relayed') AS relayed
  FROM architect_matters m
  WHERE m.kind = 'owner_words' AND json_extract(m.body, '$.where') = 'work'
  UNION ALL
  SELECT m.work, 'matter', m.id, json_extract(m.reply, '$.speaker'), json_extract(m.reply, '$.text'),
    coalesce(json_extract(m.reply, '$.at'), 0), json_extract(m.reply, '$.relayed')
  FROM architect_matters m
  WHERE m.kind = 'engineer_question' AND json_extract(m.reply, '$.by') = 'owner'
  UNION ALL
  SELECT j.work, 'interjection', j.id, j.declared, j.text,
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)), 0), j.relayed
  FROM interjections j
  WHERE j.speaker = 'owner' AND j.matter IS NULL AND j.cancelled_item IS NULL
  UNION ALL
  SELECT a.work, 'ask', a.id, json_extract(a.answer, '$.speaker'), json_extract(a.answer, '$.text'),
    coalesce(json_extract(a.answer, '$.at'), 0), json_extract(a.answer, '$.relayed')
  FROM owner_asks a
  WHERE json_extract(a.answer, '$.by') = 'owner';

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell on this very matter; the owner,
  -- for the owner's words; or no one), when it came and how long it has waited, whether it comes back with the owner's
  -- answer and to which cell, the second cell a flow version before had a matter of two cells go on at, and whether
  -- its reporter marked it as harming now (urgent).
  -- Read by: cells `S0` and `Q0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no
  -- work, and takes the next matter by it in its fixed order; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w JOIN blockers b ON b.work = w.id
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL AND b.matter = m.id AND b.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'owner_words' THEN 'owner'
      ELSE 'none' END AS waits,
    m.at,
    CAST(unixepoch('subsec') * 1000 AS INTEGER) - m.at AS waited_ms,
    m.asked IS NOT NULL AND m.goes_on_at IS NULL AS answered,
    m.resume_cell AS back_to,
    m.goes_on_at,
    coalesce(json_extract(m.body, '$.urgent'), 0) AS urgent
  FROM architect_matters m
  WHERE m.handled_round IS NULL AND m.withdrawn IS NULL
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

CREATE VIEW work_handled AS
  -- The matters each work of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and its record. Read by: the owner's interface; the engine, which gives a matter the log
  -- of what was handled on its item.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    coalesce(json_extract(m.logged, '$.record'), json_extract(r.output, '$.progress')) AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the draft the architect wrote last (cell `P2`) before the round in which the
  -- owner's approval took effect (cell `P12`), in whichever of its works, with the round of that approval; none before
  -- the first policy takes effect. A flow version before set a policy in a setting of its own, whose text was written,
  -- or put together from a correction, last before the owner's latest approval in it, in whichever of the architect's
  -- works the setting went on in since its matter was raised; a setting a flow version before that held in a work of
  -- its own, or in one work of the architect, has the text in the work of the approval. Older versions stay readable in
  -- the rounds and are given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      coalesce(
        (SELECT json_extract(t.output, '$.outcome.policy') FROM rounds t JOIN works y ON y.id = t.work
         WHERE y.graph = 'architect' AND y.project = w.project AND t.cell = 'P2' AND t.transition IS NOT NULL
           AND (t.work < a.work OR (t.work = a.work AND t.n < a.n))
           AND json_extract(t.output, '$.outcome.policy') IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM architect_matters m JOIN rounds t ON t.work BETWEEN m.work AND a.work JOIN works y ON y.id = t.work
         WHERE m.id = (SELECT max(x.id) FROM architect_matters x
                       WHERE x.project = w.project AND x.kind = 'set_policy'
                         AND (x.work < a.work OR (x.work = a.work AND x.round < a.n)))
           AND y.graph = 'architect' AND y.project = w.project AND t.cell IN ('P4', 'P5', 'P8')
           AND t.transition IS NOT NULL
           AND (t.work > m.work OR t.n > m.round) AND (t.work < a.work OR t.n < a.n)
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM rounds t
         WHERE t.work = a.work AND t.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
           'correct', 'policy.correct')
           AND t.transition IS NOT NULL AND t.n < a.n
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.n DESC LIMIT 1)) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph IN ('architect', 'policy') AND a.cell IN ('P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW notebook AS
  -- Each project's architect's notebook as it stands: each part's latest version, with the round that wrote it.
  -- A new base writes its understanding from nothing, and the next plan is given the plan before.
  -- Hidden from agents.
  -- Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, section, text, work, round FROM (
    SELECT project, section, text, work, round,
      row_number() OVER (PARTITION BY project, section ORDER BY id DESC) AS k
    FROM notebook_edits)
  WHERE k = 1;

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
    'owner' AS recipient, m.text AS text, coalesce(r.ended_at, r.started_at) AS at, m.work AS work, m.round AS round,
    NULL AS ask, m.design_card AS design_card, NULL AS matter, m.read_at AS read_at, 0 AS waiting, 0 AS pending,
    0 AS undelivered, 0 AS answerable, NULL AS fault
  FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
  LEFT JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
  UNION ALL
  SELECT CASE WHEN n.fault IS NOT NULL OR w.id IS NULL THEN 'wf-agents'
              WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    CASE WHEN n.fault IS NULL THEN coalesce(w.project, i.project) END, 'notice', n.id,
    CASE WHEN n.fault IS NULL THEN 'engine' ELSE 'wf-agents' END, 'owner', n.text, n.at, n.work, NULL, n.ask, NULL,
    NULL, n.read_at, n.fault IS NOT NULL AND n.read_at IS NULL, 0, 0, 0, n.fault
  FROM engine_notices n LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
  WHERE n.ask IS NULL OR n.fault IS NOT NULL
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
             (SELECT n.text FROM engine_notices n WHERE n.ask = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id), r.started_at), a.work, a.round, a.id, NULL,
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
    coalesce((SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0),
    j.work, j.delivered_round, NULL, j.design_card, j.matter, NULL, 0,
    j.speaker = 'owner' AND ((j.informs = 0 AND j.delivered_round IS NULL AND w.ended IS NULL
        AND w.at NOT IN (CASE WHEN w.graph = 'architect' THEN 'E2' ELSE 'E1' END, '20', 'end'))
      OR EXISTS (SELECT 1 FROM architect_matters x
                 WHERE x.id = j.matter AND x.handled_round IS NULL AND x.withdrawn IS NULL)),
    j.speaker = 'owner' AND j.informs = 0 AND j.delivered_round IS NULL
      AND (w.ended IS NOT NULL OR w.at IN (CASE WHEN w.graph = 'architect' THEN 'E2' ELSE 'E1' END, '20', 'end')),
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

-- A work of the architect at the old base goes back to the look at the queue, which finds no base of this flow and reads
-- the project anew in a new session.
UPDATE works SET at = 'Q0', resume_at = NULL, waiting = '[]', recorded = json_remove(recorded, '$.rebuild_for')
  WHERE graph = 'architect' AND ended IS NULL AND at IN ('U1', 'N1');

-- The old reading of the project asked for a check of the policy each time, though nothing about the policy happened:
-- those askers go, and a check only they asked for is withdrawn.
UPDATE architect_matters SET body = json_set(body, '$.asked_by', (
    SELECT json_group_array(json(value)) FROM json_each(architect_matters.body, '$.asked_by')
    WHERE json_extract(value, '$.cell') <> 'U1'))
  WHERE kind = 'check_policy' AND handled_round IS NULL AND withdrawn IS NULL;
UPDATE architect_matters SET withdrawn = 'Reading the project anew no longer asks for a check of the policy'
  WHERE kind = 'check_policy' AND handled_round IS NULL AND withdrawn IS NULL
    AND json_array_length(body, '$.asked_by') = 0;
