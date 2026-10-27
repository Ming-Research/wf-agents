-- Takes a version 6 database to version 7: a round's record keeps each large text once, in contents, by the hash
-- of the text. Rounds recorded before keep their texts in full. Each statement below that defines something is written
-- exactly as db/schema.sql writes it.

CREATE TABLE contents (
  -- Each large text a round's record holds, once: a conditional input given in a message, a cell's task, the
  -- structured-output schema of a command line. The record refers to it by the SHA-256 of the text, as
  -- {"content": hash}, and the text it was is that record with each reference put back. Written once, never changed.
  -- Read by: the engine and the owner's interface, putting a round's record back together.
  hash TEXT PRIMARY KEY,
  text TEXT NOT NULL
) STRICT;
