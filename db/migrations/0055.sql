-- Takes a version 54 database to version 55: the images the owner sends with words are kept in the database, each
-- with the words it came with, so that it reaches an agent in its round's message. The table is new and empty, and is
-- not tracked in changes. Each statement below that defines something is written exactly as db/schema.sql writes it.

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
  -- The words it came with, exactly one of: what the owner said to a work's line (an interjection); the owner's words
  -- an architect's matter holds (the words of a matter of the kind owner_words, or the owner's reply to an engineer's
  -- question a matter holds); or the owner's answer to an ask.
  interjection INTEGER REFERENCES interjections(id),
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
  CHECK ((interjection IS NOT NULL) + (matter IS NOT NULL) + (ask IS NOT NULL) = 1)
) STRICT;

-- The images each of the owner's words came with, in order: one in each place.
CREATE UNIQUE INDEX attachments_of_interjection ON attachments (interjection, position) WHERE interjection IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_matter ON attachments (matter, position) WHERE matter IS NOT NULL;
CREATE UNIQUE INDEX attachments_of_ask ON attachments (ask, position) WHERE ask IS NOT NULL;
