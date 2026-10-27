-- Takes a version 7 database to version 8: what the engine tells the owner of its own accord is kept, for the owner's
-- interface to show with the messages agents wrote. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

CREATE TABLE engine_notices (
  -- What the engine tells the owner of its own accord, one row for each notice: a work stopped by a new flow version,
  -- a design card posted, every work halted until a usage limit resets, and the like. It needs no answer. The same text
  -- goes to the notifications log.
  -- Hidden from agents.
  -- Read by: the owner's interface, which shows those unread with the messages agents wrote.
  id INTEGER PRIMARY KEY,
  -- The work it is about; NULL for one about no work.
  work INTEGER REFERENCES works(id),
  -- The notice, as the engine wrote it.
  text TEXT NOT NULL,
  -- When the engine wrote it.
  at INTEGER NOT NULL,
  -- When the owner marked it read; NULL while it is unread. Read by: the owner's interface.
  read_at INTEGER
) STRICT;
