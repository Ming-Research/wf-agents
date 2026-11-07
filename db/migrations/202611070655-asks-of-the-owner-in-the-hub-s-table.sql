-- Takes a database to one whose asks of the owner are the hub's messages: each ask (owner_asks before) is a message of
-- the kind ask, put by its work's line in the round that put it, holding what is asked as it did and the answer once
-- given, and the engine's word of it, the matters and the blockers it holds, and the images of its answer name it.
-- Every ask keeps whether and how it was answered, and when the owner was told of it; it takes a number after the hub's
-- other messages, which keep theirs, and the change log names it by that number. The messages, the architects'
-- matters, the blockers and the images are made afresh with their rows, as SQLite cannot change what a column refers
-- to; a migration runs with the foreign keys off, and every one is checked before it commits.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

-- Each ask with the number it takes, after every other message, in the order the asks were put.
CREATE TABLE asks_before_the_hub AS
  SELECT a.id, a.work, a.round, a.body, a.answer,
    (SELECT coalesce(max(m.id), 0) FROM messages m) + row_number() OVER (ORDER BY a.id) AS message,
    CASE WHEN w.graph = 'engineer' THEN 'engineer' ELSE 'architect' END AS sender,
    coalesce((SELECT min(n.at) FROM messages n WHERE n.ask = a.id),
             (SELECT r.started_at FROM rounds r WHERE r.work = a.work AND r.n = a.round), 0) AS at
  FROM owner_asks a JOIN works w ON w.id = a.work;
CREATE TABLE messages_before_the_asks AS SELECT * FROM messages;
CREATE TABLE architect_matters_before_the_asks AS SELECT * FROM architect_matters;
CREATE TABLE blockers_before_the_asks AS SELECT * FROM blockers;
CREATE TABLE attachments_before_the_asks AS SELECT * FROM attachments;
DROP VIEW conversation_shown;
DROP VIEW conversation_entries;
DROP VIEW item_asks;
DROP VIEW architect_queue;
DROP TABLE messages;
DROP TABLE architect_matters;
DROP TABLE blockers;
DROP TABLE attachments;
DROP TABLE owner_asks;

CREATE TABLE messages (
  -- Every message the hub carries, written once and never changed but for an ask's answer, and the mark that it was
  -- relayed: a saying, which needs no answer, or an ask of the owner, which waits for one. An ask is what an agent asks
  -- the owner, or asks the architect while the owner answers for the architect, or what the engine asks the owner to
  -- approve, put by a round of the work (`work`, `round`); the work waits for the answer, which goes back unchanged to
  -- the cell that asked. A saying goes: to the owner, what an agent tells the owner in a return, its answer to what the
  -- owner said to its work, the architect's reply to the owner's words, and what the engine tells of its own accord (a
  -- work stopped, or gone back in a new flow version, every work halted until a usage limit resets, a limit passed, an
  -- ask of the owner put); to the flow's keeper, a fault of wf-agents; to a line of a work, what the owner says to it,
  -- what the architect says to it, and what the engine says in the owner's name (a ruling on a decision card, an answer
  -- of the owner's relayed, the notice that an item depended on was cancelled). Whom each went to, and what became of
  -- it there, is in deliveries. Words to a line that no agent has taken up yet are deleted when the owner takes them
  -- back.
  -- Read by: agents, who look up with the query tool what was told and asked of the owner and what was said to a work;
  -- cell `R1` (the asks of the owner on this item, with their answers, item_asks); the owner's interface, which shows
  -- each in its conversation (conversation_entries) and takes the answer to an ask; the engine, which waits for an
  -- ask's answer, gives each line what is said to it (line_sayings), adds a further cache miss to the message of a
  -- fault still open, and hands a fault passed on to an architect to it.
  id INTEGER PRIMARY KEY,
  -- say, a saying, as a message is unless it asks; or ask, an ask of the owner.
  kind TEXT NOT NULL DEFAULT 'say' CHECK (kind IN ('say', 'ask')),
  -- Who sent it: owner (the owner's words), engineer (a line of an engineer's work), architect (a line of a project's
  -- architect's work), or engine (what the engine tells of its own accord, and what it says to a line in the owner's
  -- name, which agents are given as the owner's).
  sender TEXT NOT NULL CHECK (sender IN ('owner', 'engineer', 'architect', 'engine')),
  -- The work it is about: an agent's to the owner, the work whose round handed it in; one to a line, the work whose
  -- line it is said to; the engine's, the work it tells of, NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- An agent's to the owner: the round whose return holds it; an ask: the round that put it to the owner; by its number
  -- in the work. NULL for any other.
  round INTEGER,
  -- The message, as its sender wrote it; for an ask, the question its body holds, empty when the body asks no question
  -- in words of its own (the decision points of a research, a change to approve).
  text TEXT NOT NULL,
  -- What more it says, JSON; NULL when nothing. For a fault of wf-agents, its kind (`kind`): cache_miss, rounds that
  -- missed their session's prompt cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a
  -- row counting a flow anomaly, where the way on an engineer chose did not fit the situation. For an agent's answer to
  -- the owner's question about a decision card, or the owner's question about one, that card (`design_card`), which the
  -- work's group shows the one as a reply to and the engineer is given with the other. For the owner's words to a line,
  -- and an answer of the owner's the engine relays to one: who the caller declared to speak (`speaker`, owner, or
  -- delegate for a session that speaks for the owner; none for words kept before it was declared), and the parts a
  -- delegate declared to be the owner's own words (`relayed`, each found letter for letter in the text). For a word to
  -- a line that needs no answer and changes no way on, as the owner confirming every decision of a design card or the
  -- words said in a work's group to its architect alone (`informs`, 1): the line's next round is given it, and a work
  -- standing at an engine step is not taken by it to choose the way on. For the notice that an item the work's item
  -- depended on was cancelled, that item (`cancelled_item`): the cells where the work waits (`A2`, `A6`, `M1`) take
  -- such a notice to choose the way on by a row of their own. For an ask, what is asked: whom it asks, the work and
  -- item, and what the asking cell handed in, copied from what that round was given, which never changes. Read by: the
  -- owner's interface; the engine; for an ask, cell `R1`.
  body TEXT CHECK (body IS NULL OR json_valid(body)),
  -- For the engine's word of an ask of the owner put: that ask, which the owner's conversations date the ask by and
  -- link to; NULL for any other message. Read by: the owner's interface; the engine, which hands an architect the ask
  -- of a flow anomaly passed on to it.
  ask INTEGER REFERENCES messages(id),
  -- For an ask, the owner's answer, JSON: who answered and when, and the text, and who the caller declared to speak
  -- (`speaker`, owner or delegate; none for an answer kept before it was declared); for an ask of the architect, the
  -- rest of the architect's reply too. NULL while it waits, and for a saying. Read by: the engine; cell `R1`.
  answer TEXT CHECK (answer IS NULL OR json_valid(answer)),
  -- When it was sent.
  at INTEGER NOT NULL,
  CHECK (sender IN ('engineer', 'architect') OR round IS NULL),
  CHECK (sender <> 'engineer' OR round IS NOT NULL),
  CHECK (kind = 'say' OR round IS NOT NULL),
  CHECK (kind = 'ask' OR answer IS NULL),
  CHECK (round IS NULL OR work IS NOT NULL),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO messages (id, kind, sender, work, round, text, body, ask, answer, at)
  SELECT m.id, 'say', m.sender, m.work, m.round, m.text, m.body,
    (SELECT a.message FROM asks_before_the_hub a WHERE a.id = m.ask), NULL, m.at
  FROM messages_before_the_asks m ORDER BY m.id;
INSERT INTO messages (id, kind, sender, work, round, text, body, ask, answer, at)
  SELECT message, 'ask', sender, work, round,
    CASE WHEN json_type(body, '$.question') = 'text' THEN json_extract(body, '$.question') ELSE '' END, body, NULL,
    answer, at
  FROM asks_before_the_hub ORDER BY message;
DROP TABLE messages_before_the_asks;

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
  asked INTEGER REFERENCES messages(id),
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
  SELECT id, project, kind, detail, what, body, item, work, round, at, (SELECT a.message FROM asks_before_the_hub a WHERE a.id = m.asked), resume_cell, goes_on_at, group_lead, reply, reply_held, handled_work, handled_round, withdrawn, logged, effects_held
  FROM architect_matters_before_the_asks m ORDER BY m.id;
DROP TABLE architect_matters_before_the_asks;

CREATE TABLE blockers (
  -- What a stopped task waits on, one row for each stop: an engineer's question, an engine tool's failure, an ask of the
  -- owner, a wait for items to deliver, or a wait for the owner's ruling on a design card. A task is an engineer's work,
  -- or the setting of a policy in a flow version that ran it as a work of its own; a work of the architect's graph is
  -- never one. A blocker belongs to the task and the cell that raised it, and has exactly one
  -- holder at a time, who must act: the work's advisor, a project's architect, the owner, or the items it waits for,
  -- whose merge resolves it. A task stops on one blocker at a time, and stands at its holder's cell while it waits;
  -- every move of a blocker is a row of the task's graph, which the engine records here.
  -- Read by: the engine, which moves a blocker by the rows its task takes and checks that every chain closes; the
  -- owner's interface, which reads the owner's conversations and what each work waits for from it.
  id INTEGER PRIMARY KEY,
  -- The task that stopped.
  work INTEGER NOT NULL REFERENCES works(id),
  -- The cell that raised it, where its problem arose: the cell that asked, the tool cell whose tool failed, or the cell
  -- where the work waits for items or rulings; and the round of the task that raised it there, NULL for one recorded
  -- before blockers were.
  cell TEXT NOT NULL,
  round INTEGER,
  -- question: an engineer's question; tool_failure: an engine tool that failed, or main's environment that changed;
  -- owner_ask: an ask of the owner; items: a wait for items to deliver; ruling: a wait for the owner's ruling on the
  -- design card before the merge.
  kind TEXT NOT NULL CHECK (kind IN ('question', 'tool_failure', 'owner_ask', 'items', 'ruling')),
  -- What the problem is, in one sentence, as the engine wrote it; the return of the round that raised it holds the
  -- rest.
  what TEXT NOT NULL,
  -- Who holds it now: advisor, the work's advisor; architect, the architect of project; owner; items, the items it
  -- waits for, whose merge resolves it.
  holder TEXT NOT NULL CHECK (holder IN ('advisor', 'architect', 'owner', 'items')),
  -- For an architect that holds it, the project whose architect it is, the work's own or a related one; NULL for any
  -- other holder.
  project TEXT,
  -- For items that hold it, JSON list: each item waited for, with its project. NULL for any other holder.
  items TEXT CHECK (items IS NULL OR json_valid(items)),
  -- The matter in its architect's queue while an architect holds it, its latest one. Read by: the engine, which takes
  -- the architect's reply from it and withdraws it when the blocker ends.
  matter INTEGER REFERENCES architect_matters(id),
  -- The ask put before the owner while the owner holds it, its latest one. Read by: the engine, which takes the owner's
  -- answer from it and closes it when the blocker ends; the owner's interface.
  ask INTEGER REFERENCES messages(id),
  -- When it was raised.
  at INTEGER NOT NULL,
  -- When its holder took it, or when its holder's wait was counted again: an architect's time to act on it is
  -- measured from here. Read by: the engine's value that says it waited past its limit.
  since INTEGER NOT NULL,
  -- Every hand-over, JSON list in order: when, from which holder to which (with the project of an architect), why, and
  -- what the holder that handed it on found.
  moves TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(moves)),
  -- What is said about it without resolving it or moving it, JSON list in order: when, by whom, and what, such as an
  -- architect's reply that waits for its batch's review, or a resolution that came after it was withdrawn, which is
  -- recorded here and moves nothing.
  said TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(said)),
  -- How it ended: resolved, by its holder; withdrawn, by its task's end or by a row that took the task on without it;
  -- NULL while it is open.
  ended TEXT CHECK (ended IN ('resolved', 'withdrawn')),
  -- For one resolved, what was decided and done, JSON, as its holder gave it; for one withdrawn, why. NULL while it is
  -- open.
  outcome TEXT CHECK (outcome IS NULL OR json_valid(outcome)),
  -- When it ended.
  ended_at INTEGER,
  CHECK ((holder = 'architect') = (project IS NOT NULL)),
  CHECK ((holder = 'items') = (items IS NOT NULL)),
  CHECK ((ended IS NULL) = (outcome IS NULL)),
  CHECK ((ended IS NULL) = (ended_at IS NULL)),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO blockers (id, work, cell, round, kind, what, holder, project, items, matter, ask, at, since, moves, said, ended, outcome, ended_at)
  SELECT id, work, cell, round, kind, what, holder, project, items, matter, (SELECT a.message FROM asks_before_the_hub a WHERE a.id = b.ask), at, since, moves, said, ended, outcome, ended_at
  FROM blockers_before_the_asks b ORDER BY b.id;
DROP TABLE blockers_before_the_asks;

CREATE TABLE attachments (
  -- An image the owner sent with words: picked or pasted in a box of the owner's page and sent with the words written
  -- beside it, as one message, or alone. A PNG, JPEG, GIF or WebP image of at most 5 MB, at most 10 to a message. It
  -- is kept here, not in a file, so that it reaches an agent in its round's message and no agent has to find a file
  -- or be let read one: the engine gives an image to an agent's session with the first of the owner's words naming
  -- it (`image:<id>`) that the session is given, and refers to it after that. Written once with its words and never
  -- changed: who sent it, and when, is the change that wrote its words. Not tracked in changes, which holds no bytes.
  -- Hidden from agents.
  -- Read by: the engine, which gives each image with its words; the owner's interface, which shows the images under
  -- the owner's words and in the round that was given them.
  id INTEGER PRIMARY KEY,
  -- The words it came with, exactly one of: what the owner said to a work's line (a message); the owner's words an
  -- architect's matter holds (the words of a matter of the kind owner_words, or the owner's reply to an engineer's
  -- question a matter holds); or the owner's answer to an ask.
  message INTEGER REFERENCES messages(id),
  matter INTEGER REFERENCES architect_matters(id),
  ask INTEGER REFERENCES messages(id),
  -- Its place among the images of its words, from 1, in the order the owner added them.
  position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 10),
  -- What kind of image it is.
  media_type TEXT NOT NULL CHECK (media_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp')),
  -- Its size in bytes.
  size INTEGER NOT NULL CHECK (size BETWEEN 1 AND 5242880),
  -- The SHA-256 of its bytes, in hex. Read by: the engine, which tells by it whether a session holds the image.
  hash TEXT NOT NULL,
  -- The image itself.
  bytes BLOB NOT NULL CHECK (length(bytes) = size),
  CHECK ((message IS NOT NULL) + (matter IS NOT NULL) + (ask IS NOT NULL) = 1)
) STRICT;

INSERT INTO attachments (id, message, matter, ask, position, media_type, size, hash, bytes)
  SELECT x.id, x.message, x.matter, (SELECT a.message FROM asks_before_the_hub a WHERE a.id = x.ask), x.position,
    x.media_type, x.size, x.hash, x.bytes
  FROM attachments_before_the_asks x ORDER BY x.id;
DROP TABLE attachments_before_the_asks;
-- The change log names each ask by the number it takes: the rows of the messages, the blockers and the images that
-- name an ask, and those of the architects' matters, name it so, and the asks' own rows are rows of the messages.
UPDATE changes SET
  old_row = CASE WHEN json_extract(old_row, '$.ask') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(old_row, '$.ask',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.old_row, '$.ask')))
    ELSE old_row END,
  new_row = CASE WHEN json_extract(new_row, '$.ask') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(new_row, '$.ask',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.new_row, '$.ask')))
    ELSE new_row END
WHERE table_name IN ('messages', 'blockers', 'attachments');
UPDATE changes SET
  old_row = CASE WHEN json_extract(old_row, '$.asked') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(old_row, '$.asked',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.old_row, '$.asked')))
    ELSE old_row END,
  new_row = CASE WHEN json_extract(new_row, '$.asked') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(new_row, '$.asked',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.new_row, '$.asked')))
    ELSE new_row END
WHERE table_name = 'architect_matters';
UPDATE changes SET
  table_name = 'messages',
  row_key = CASE WHEN json_extract(row_key, '$.id') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(row_key, '$.id',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.row_key, '$.id')))
    ELSE row_key END,
  old_row = CASE WHEN json_extract(old_row, '$.id') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(old_row, '$.id',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.old_row, '$.id')))
    ELSE old_row END,
  new_row = CASE WHEN json_extract(new_row, '$.id') IN (SELECT id FROM asks_before_the_hub)
    THEN json_set(new_row, '$.id',
      (SELECT a.message FROM asks_before_the_hub a WHERE a.id = json_extract(changes.new_row, '$.id')))
    ELSE new_row END
WHERE table_name = 'owner_asks' AND json_extract(row_key, '$.id') IN (SELECT id FROM asks_before_the_hub);
DROP TABLE asks_before_the_hub;

CREATE INDEX messages_by_work ON messages (work, round);
CREATE INDEX messages_by_ask ON messages (ask);
CREATE INDEX architect_matters_waiting ON architect_matters (project) WHERE handled_round IS NULL;
CREATE INDEX architect_matters_by_kind ON architect_matters (kind, at);
CREATE INDEX architect_matters_by_handling ON architect_matters (handled_work, handled_round);
CREATE INDEX architect_matters_by_work ON architect_matters (work, kind);
CREATE INDEX architect_matters_by_asked ON architect_matters (asked);
CREATE UNIQUE INDEX blockers_open ON blockers (work) WHERE ended IS NULL;
CREATE INDEX blockers_by_time ON blockers (at);
CREATE INDEX blockers_by_end ON blockers (ended_at);
CREATE INDEX blockers_by_ask ON blockers (ask);
CREATE INDEX blockers_by_matter ON blockers (matter);
CREATE UNIQUE INDEX attachments_of_message ON attachments (message, position) WHERE message IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_matter ON attachments (matter, position) WHERE matter IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_ask ON attachments (ask, position) WHERE ask IS NOT NULL;

CREATE VIEW item_asks AS
  -- Every ask of the owner of the item's works, with its answer. Read by: cell `R1`.
  SELECT w.item, a.work, a.id AS ask, a.round, a.body, a.answer
  FROM messages a JOIN works w ON w.id = a.work
  WHERE a.kind = 'ask';

CREATE VIEW work_asks AS
  -- Every ask of the owner each work put, with its answer: the asks among the hub's messages, one definition of them
  -- for whoever reads asks. Read by: agents, with the query tool; the engine, which waits for an ask's answer and gives
  -- it back to the cell that asked; the owner's interface.
  SELECT a.id, a.work, a.round, a.text, a.body, a.answer, a.at
  FROM messages a
  WHERE a.kind = 'ask';

CREATE VIEW architect_queue AS
  -- Each project's queue: the matters not handled, not withdrawn and not held by an ask of the owner still waiting,
  -- each with who waits for it (an agent: an engineer stopped at the architect's cell on this very matter, or, for the
  -- matter of a return sent back (kind `report`, detail `sent_back`), an engineer's work the engine paused for
  -- returns sent back in a row, whose reason starts as `SENT_BACK_FOR_ARCHITECT` in crates/db/src/lib.rs does, until
  -- it goes on; the owner, for the owner's words; or no one), when it came and how long it has waited, whether it comes
  -- back with the owner's answer and to which cell, the second cell a flow version before had a matter of two cells go
  -- on at, and whether its reporter marked it as harming now (urgent).
  -- Read by: cells `S0` and `N0`; the engine, which takes `S0`'s rows by it for each project whose architect runs no
  -- work, and takes the next matter by it in its fixed order; the owner's interface.
  SELECT m.project, m.id AS matter, m.kind, m.detail, m.item, m.what,
    CASE
      WHEN m.work IS NOT NULL AND m.reply IS NULL AND EXISTS (
        SELECT 1 FROM works w JOIN blockers b ON b.work = w.id
        WHERE w.id = m.work AND w.graph = 'engineer' AND w.at IN ('A2', '15', 'to_architect', 'ask.architect')
          AND w.ended IS NULL AND b.matter = m.id AND b.ended IS NULL)
        THEN 'agent'
      WHEN m.kind = 'report' AND m.detail = 'sent_back' AND EXISTS (
        SELECT 1 FROM works w
        WHERE w.id = json_extract(m.body, '$.work') AND w.ended IS NULL
          AND w.paused_by = 'engine' AND w.paused LIKE 'returns were sent back%')
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
    AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM work_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))
  ORDER BY m.id;

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
    n.ask, NULL, NULL, d.read_at, d.recipient = 'keeper' AND d.read_at IS NULL, 0, 0, 0,
    CASE WHEN d.recipient = 'keeper' THEN json_extract(n.body, '$.kind') END
  FROM messages n JOIN deliveries d ON d.message = n.id AND d.recipient IN ('owner', 'keeper')
  LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
  WHERE n.sender = 'engine' AND (n.ask IS NULL OR d.recipient = 'keeper')
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
             (SELECT n.text FROM messages n WHERE n.ask = a.id ORDER BY n.id LIMIT 1), ''),
    coalesce((SELECT min(n.at) FROM messages n WHERE n.ask = a.id), r.started_at), a.work, a.round, a.id, NULL,
    NULL, NULL,
    a.answer IS NULL AND (
      (w.graph = 'architect' AND EXISTS (
        SELECT 1 FROM architect_matters m WHERE m.asked = a.id AND m.handled_round IS NULL AND m.withdrawn IS NULL))
      OR EXISTS (SELECT 1 FROM blockers b WHERE b.ask = a.id AND b.ended IS NULL AND b.holder = 'owner')
      OR (w.ended IS NULL AND json_extract(a.body, '$.kind') IN ('design_review', 'points', 'change'))), 0, 0, 0, NULL
  FROM messages a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
  WHERE a.kind = 'ask'
  UNION ALL
  SELECT CASE WHEN w.graph IN ('architect', 'policy') THEN 'architect:' || w.project ELSE 'work:' || w.id END,
    coalesce(w.project, i.project), 'interjection', j.id, j.speaker,
    CASE WHEN j.matter IS NULL THEN j.line WHEN j.informs = 1 THEN 'architect' ELSE 'both' END,
    coalesce((SELECT json_extract(x.body, '$.text') FROM architect_matters x WHERE x.id = j.matter), j.text),
    j.at, j.work, j.taken_round, NULL, j.design_card, j.matter, NULL, 0,
    j.speaker = 'owner' AND ((j.informs = 0 AND j.taken_round IS NULL AND w.ended IS NULL
        AND w.at NOT IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end'))
      OR EXISTS (SELECT 1 FROM architect_matters x
                 WHERE x.id = j.matter AND x.handled_round IS NULL AND x.withdrawn IS NULL)),
    j.speaker = 'owner' AND j.informs = 0 AND j.taken_round IS NULL
      AND (w.ended IS NOT NULL OR w.at IN (CASE WHEN w.graph = 'architect' THEN 'R1' ELSE 'E1' END, 'E2', '20', 'end')),
    0, NULL
  FROM line_sayings j JOIN works w ON w.id = j.work LEFT JOIN items i ON i.id = w.item
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
    AND NOT EXISTS (SELECT 1 FROM deliveries a JOIN deliveries e ON e.message = a.message AND e.recipient = 'line'
                    WHERE a.matter = m.id)
    AND (json_extract(m.body, '$.where') IS NOT 'projects' OR json_extract(m.body, '$.to[0]') = m.project)
  UNION ALL
  SELECT 'architect:' || m.project, m.project, 'talk', e.id, 'owner', 'architect', e.text, e.at, NULL, NULL, NULL,
    NULL, m.id, NULL, 0, d.taken_round IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL, 0, 0, NULL
  FROM deliveries d JOIN messages e ON e.id = d.message JOIN architect_matters m ON m.id = d.matter
  WHERE d.recipient = 'architect' AND e.sender = 'owner'
    AND NOT EXISTS (SELECT 1 FROM deliveries x WHERE x.message = e.id AND x.recipient = 'line')
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
