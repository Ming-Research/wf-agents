-- Takes a database to one that keeps the name of each migration it has taken, in the new table migrations: from now
-- on a migration is named by the minute it was written and what it does, not by the next number, so that works written
-- side by side never take the same name, and the names, not the version, say which migrations a database took. Once
-- this has run the engine records every migration taken so far, 0002 to 0079 and this one, as it records every one it
-- takes after. The statement below is written exactly as db/schema.sql writes it.

CREATE TABLE migrations (
  -- Each migration the database has taken, by its file in db/migrations. A database made from this file has taken
  -- every one there is; one opened by a wf-agents with migrations it has not taken takes them, in the order their names
  -- sort, and its rows here name them. A migration is named by the minute it was written and what it does, so that
  -- works written side by side never take the same name, and these rows, not a count, say which a database took: a
  -- migration merged after one whose name sorts after it is taken after that one where that one came first. A database
  -- from before this table took the first migrations, as many as its version (`user_version`) less one.
  -- Hidden from agents.
  -- Read by: the engine, as it opens a database, to take the migrations it has not and to refuse one that took a
  -- migration it does not know, which a newer wf-agents made.
  -- The migration's file's name without `.sql`, as `0079` or `202611040140-record-migrations-taken`.
  name TEXT PRIMARY KEY
) STRICT;
