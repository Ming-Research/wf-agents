-- Takes a version 56 database to version 57: the owner's start queue is read by the architect too, which is given it
-- with the items and asks the owner where its order differs, so its table is made afresh with its rows for the
-- description agents read. Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE dispatch_queue_version_56 AS SELECT * FROM dispatch_queue;
DROP TABLE dispatch_queue;

CREATE TABLE dispatch_queue (
  -- Items waiting for a work to start, in the order the owner sets by hand, ahead of the architect's order. When a
  -- slot is free the engine starts a work on the first one. This table goes away once the architect's order alone
  -- decides what starts.
  -- Read by: the engine at cell `S0`; the architect, which is given it with the items and asks the owner where its
  -- order differs.
  item INTEGER PRIMARY KEY REFERENCES items(id),
  -- Lower starts first. A real number, so an item can be put between two others without renumbering.
  position REAL NOT NULL UNIQUE
) STRICT;

INSERT INTO dispatch_queue (item, position)
SELECT item, position FROM dispatch_queue_version_56;
DROP TABLE dispatch_queue_version_56;
