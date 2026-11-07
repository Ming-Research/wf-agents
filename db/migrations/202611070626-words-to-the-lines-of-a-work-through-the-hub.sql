-- Takes a database to one whose words to the lines of a work are the hub's messages: what the owner, the architect and
-- the engine in the owner's name said to a work's line (interjections before) is each a message, delivered to that
-- line, which a round takes; the owner's words said in a work's group to its architect too are delivered to the
-- architect as well, as the matter that holds them. The entries of a talk with the architect a flow version before
-- held (talk_entries), which nothing writes now, are the owner's messages delivered to the architect, naming the talk's
-- matter. An image the owner sent with words to a line belongs to their message.
--
-- What is in flight stays as it was: each word to a line keeps its number, which the rounds that were given it hold
-- (their input's `engine.interjections`), as do the sessions (`ij:<number>`) and the decisions that cite it; the
-- messages before, to the owner and the keeper, take numbers after them, which nothing outside the database names.
-- Words not taken stay to be taken, by the same line; those taken keep the round that took them; each keeps when it was
-- said, as the owner's conversations dated it. The messages, their deliveries and the images are made afresh with their
-- rows, as SQLite cannot change a table's checks or what a column refers to; a migration runs with the foreign keys off,
-- and every one is checked before it commits.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE messages_before_the_lines AS SELECT * FROM messages;
CREATE TABLE deliveries_before_the_lines AS SELECT * FROM deliveries;
CREATE TABLE attachments_before_the_lines AS SELECT * FROM attachments;
-- How far the messages before move up, so that every word to a line keeps its number.
CREATE TABLE shift_before_the_lines AS SELECT coalesce(max(id), 0) AS by FROM interjections;
DROP VIEW conversation_shown;
DROP VIEW conversation_entries;
DROP VIEW waiting_interjections;
DROP TABLE messages;
DROP TABLE deliveries;
DROP TABLE attachments;

CREATE TABLE messages (
  -- Every message the hub carries, written once and never changed: a saying, which needs no answer. To the owner: what
  -- an agent tells the owner in a return, its answer to what the owner said to its work, the architect's reply to the
  -- owner's words, and what the engine tells of its own accord (a work stopped, or gone back in a new flow version,
  -- every work halted until a usage limit resets, a limit passed, an ask of the owner put); to the flow's keeper, a
  -- fault of wf-agents; to a line of a work, what the owner says to it, what the architect says to it, and what the
  -- engine says in the owner's name (a ruling on a decision card, an answer of the owner's relayed, the notice that an
  -- item depended on was cancelled). Whom each went to, and what became of it there, is in deliveries. Words to a line
  -- that no agent has taken up yet are deleted when the owner takes them back.
  -- Read by: agents, who look up with the query tool what was told the owner and what was said to a work; the owner's
  -- interface, which shows each in its conversation (conversation_entries); the engine, which gives each line what is
  -- said to it (line_sayings), adds a further cache miss to the message of a fault still open, and hands a fault passed
  -- on to an architect to it.
  id INTEGER PRIMARY KEY,
  -- Who sent it: owner (the owner's words, and what the engine says to a line in the owner's name), engineer (a line of
  -- an engineer's work), architect (a line of a project's architect's work), or engine.
  sender TEXT NOT NULL CHECK (sender IN ('owner', 'engineer', 'architect', 'engine')),
  -- The work it is about: an agent's to the owner, the work whose round handed it in; one to a line, the work whose
  -- line it is said to; the engine's, the work it tells of, NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- An agent's to the owner: the round whose return holds it, by its number in the work; NULL for any other.
  round INTEGER,
  -- The message, as its sender wrote it.
  text TEXT NOT NULL,
  -- What more it says, JSON; NULL when nothing. For a fault of wf-agents, its kind (`kind`): cache_miss, rounds that
  -- missed their session's prompt cache, each kept in cache_misses; flow_anomaly, an ask of the owner that came of a row
  -- counting a flow anomaly, where the way on an engineer chose did not fit the situation. For an agent's answer to the
  -- owner's question about a decision card, or the owner's question about one, that card (`design_card`), which the
  -- work's group shows the one as a reply to and the engineer is given with the other. For the owner's words to a line:
  -- who the caller declared to speak (`speaker`, owner, or delegate for a session that speaks for the owner; none for
  -- what the engine says in the owner's name, and for words kept before it was declared), and the parts a delegate
  -- declared to be the owner's own words (`relayed`, each found letter for letter in the text). For a word to a line
  -- that needs no answer and changes no way on, as the owner confirming every decision of a design card or the words
  -- said in a work's group to its architect alone (`informs`, 1): the line's next round is given it, and a work
  -- standing at an engine step is not taken by it to choose the way on. For the notice that an item the work's item
  -- depended on was cancelled, that item (`cancelled_item`): the cells where the work waits (`A2`, `A6`, `M1`) take
  -- such a notice to choose the way on by a row of their own. Read by: the owner's interface; the engine.
  body TEXT CHECK (body IS NULL OR json_valid(body)),
  -- For the engine's word of an ask of the owner put: that ask, which the owner's conversations date the ask by and
  -- link to; NULL for any other message. Read by: the owner's interface; the engine, which hands an architect the ask
  -- of a flow anomaly passed on to it.
  ask INTEGER REFERENCES owner_asks(id),
  -- When it was sent.
  at INTEGER NOT NULL,
  CHECK (sender IN ('engineer', 'architect') OR round IS NULL),
  CHECK (sender <> 'engineer' OR round IS NOT NULL),
  CHECK (round IS NULL OR work IS NOT NULL),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE TABLE deliveries (
  -- Each recipient a message went to, and what became of it there: the owner; the flow's keeper, a queue for
  -- wf-agents' own faults, which the owner and the assistant work; a project's architect, to whose queue a fault of
  -- wf-agents was passed on, or that the owner's words said in a work's group came to as well; or a line of a work
  -- (line: an engineer's, or one of the work's helpers; the architect's in a flow version before), which takes what is
  -- said to it at its next round, or during a round after a tool use. A fault goes to the keeper; what is said to a
  -- line, to that line; every other message to the owner.
  -- Read by: agents, who look up with the query tool which line a word was said to and which round took it, as they
  -- read the words said to a work before; the owner's interface, which shows what is unread, where each fault went and
  -- what no agent took up yet; the engine, which hands each line what is said to it and records the round that took
  -- it, and hands an architect the fault its matter is.
  id INTEGER PRIMARY KEY,
  -- The message delivered.
  message INTEGER NOT NULL REFERENCES messages(id),
  -- owner, keeper, architect or line.
  recipient TEXT NOT NULL CHECK (recipient IN ('owner', 'keeper', 'architect', 'line')),
  -- For a line: which of the message's work's lines it is said to (engineer, review, ...); NULL for another recipient.
  line TEXT,
  -- For an architect: the matter of its queue the message came as, or, for the owner's words of a talk with the
  -- architect as a flow version before held one, the matter of the words that started the talk; it names the project.
  -- NULL for another recipient.
  matter INTEGER REFERENCES architect_matters(id),
  -- For the owner and the keeper: when it was marked read, or passed on to an architect; NULL while it is unread, and
  -- for an agent.
  read_at INTEGER,
  -- For an agent: the round that took it, by its work and its number in the work; NULL until then. Read by: the
  -- engine, to find what is still to hand in; the owner's interface, for what no agent took up yet.
  taken_work INTEGER,
  taken_round INTEGER,
  CHECK ((recipient = 'architect') = (matter IS NOT NULL)),
  CHECK ((recipient = 'line') = (line IS NOT NULL)),
  CHECK (recipient IN ('owner', 'keeper') OR read_at IS NULL),
  CHECK (recipient IN ('architect', 'line') OR taken_round IS NULL),
  CHECK ((taken_work IS NULL) = (taken_round IS NULL)),
  FOREIGN KEY (taken_work, taken_round) REFERENCES rounds (work, n)
) STRICT;

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
  ask INTEGER REFERENCES owner_asks(id),
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

-- The words to a line, each by its own number, said when the owner's conversations dated it.
INSERT INTO messages (id, sender, work, round, text, body, ask, at)
  SELECT j.id, j.speaker, j.work, NULL, j.text,
    nullif(json_patch('{}', json_object('speaker', j.declared, 'relayed', json(j.relayed),
      'informs', CASE WHEN j.informs = 1 THEN 1 END, 'design_card', j.design_card,
      'cancelled_item', j.cancelled_item)), '{}'),
    NULL,
    coalesce((SELECT max(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id) AND c.old_row IS NULL),
             (SELECT min(x.started_at) FROM rounds x WHERE x.work = j.work), 0)
  FROM interjections j ORDER BY j.id;
INSERT INTO messages (id, sender, work, round, text, body, ask, at)
  SELECT m.id + s.by, m.sender, m.work, m.round, m.text, m.body, m.ask, m.at
  FROM messages_before_the_lines m, shift_before_the_lines s ORDER BY m.id;
-- The entries of a talk, each a message after every other.
CREATE TABLE talks_before_the_lines AS
  SELECT e.*, (SELECT coalesce(max(id), 0) FROM messages) + row_number() OVER (ORDER BY e.id) AS message
  FROM talk_entries e;
INSERT INTO messages (id, sender, work, round, text, body, ask, at)
  SELECT message, 'owner', NULL, NULL, text, NULL, NULL, at FROM talks_before_the_lines ORDER BY message;

INSERT INTO deliveries (id, message, recipient, line, matter, read_at, taken_work, taken_round)
  SELECT d.id, d.message + s.by, d.recipient, NULL, d.matter, d.read_at, NULL, NULL
  FROM deliveries_before_the_lines d, shift_before_the_lines s ORDER BY d.id;
INSERT INTO deliveries (message, recipient, line, taken_work, taken_round)
  SELECT j.id, 'line', j.line, CASE WHEN j.delivered_round IS NOT NULL THEN j.work END, j.delivered_round
  FROM interjections j ORDER BY j.id;
-- The owner's words said in a work's group to its architect too: the matter that holds them.
INSERT INTO deliveries (message, recipient, matter)
  SELECT j.id, 'architect', j.matter FROM interjections j WHERE j.matter IS NOT NULL ORDER BY j.id;
INSERT INTO deliveries (message, recipient, matter, taken_work, taken_round)
  SELECT message, 'architect', matter, taken_work, taken_round FROM talks_before_the_lines ORDER BY message;

UPDATE cache_misses SET message = message + (SELECT by FROM shift_before_the_lines) WHERE message IS NOT NULL;

INSERT INTO attachments (id, message, matter, ask, position, media_type, size, hash, bytes)
  SELECT id, interjection, matter, ask, position, media_type, size, hash, bytes
  FROM attachments_before_the_lines ORDER BY id;

DROP TABLE messages_before_the_lines;
DROP TABLE deliveries_before_the_lines;
DROP TABLE attachments_before_the_lines;
DROP TABLE shift_before_the_lines;
DROP TABLE talks_before_the_lines;
DROP TABLE interjections;
DROP TABLE talk_entries;

CREATE INDEX messages_by_work ON messages (work, round);
CREATE INDEX messages_by_ask ON messages (ask);
CREATE INDEX deliveries_by_message ON deliveries (message);
CREATE INDEX deliveries_by_matter ON deliveries (matter) WHERE matter IS NOT NULL;
CREATE INDEX deliveries_taken ON deliveries (taken_work, taken_round);
CREATE UNIQUE INDEX attachments_of_message ON attachments (message, position) WHERE message IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_matter ON attachments (matter, position) WHERE matter IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_ask ON attachments (ask, position) WHERE ask IS NOT NULL;

CREATE VIEW line_sayings AS
  -- Each message said to a line of a work, one row for each, with the columns the words said to a line have had: who
  -- said it (speaker, owner or architect), the work and the line, the words, who the caller declared to speak
  -- (declared) and the parts a delegate relayed as the owner's own (relayed), whether it needs no answer (informs), the
  -- decision card it asks about, the item whose cancellation it tells of, the architect's matter that holds the same
  -- words when they were said in the work's group to the architect too, when it was said, and the round of the work
  -- that took it (taken_round), none until one did.
  -- Read by: the engine, which gives each line what waits for it, and judges a decision by the words it cites; the
  -- owner's interface, which shows them and offers to take back what no agent took up yet; the views
  -- waiting_interjections and conversation_entries.
  SELECT m.id, m.work, d.line, m.sender AS speaker, m.text, json_extract(m.body, '$.speaker') AS declared,
    json_extract(m.body, '$.relayed') AS relayed, coalesce(json_extract(m.body, '$.informs'), 0) AS informs,
    json_extract(m.body, '$.design_card') AS design_card, json_extract(m.body, '$.cancelled_item') AS cancelled_item,
    (SELECT a.matter FROM deliveries a WHERE a.message = m.id AND a.recipient = 'architect') AS matter, m.at,
    d.taken_round
  FROM deliveries d JOIN messages m ON m.id = d.message
  WHERE d.recipient = 'line';

CREATE VIEW waiting_interjections AS
  -- Interjections not yet handed in, in the order they came, each with the decision card it asks about, by its number
  -- and key, if it does: the messages said to a work's lines that no round took yet (line_sayings). Each cell is given
  -- what waits for its line as its input.
  -- Read by: every agent cell; cells `T7`, `M1` and `E1`.
  SELECT s.work, s.id AS interjection, s.line, s.speaker, s.text, s.design_card, c.key AS card_key
  FROM line_sayings s LEFT JOIN design_cards c ON c.id = s.design_card
  WHERE s.taken_round IS NULL
  ORDER BY s.id;

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
  FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
  LEFT JOIN items i ON i.id = w.item
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
