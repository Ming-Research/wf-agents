-- Takes a database to one whose keep-warm pings record the call they ran when it is not their session's last round's
-- command line run again: a ping of a project's architect's base runs the command line the base's forks run with, so
-- that the cache keeps the base's end under it, and the session manager reads that call to tell whether the cache keeps
-- the base for a fork, and to compare a round that missed the cache with what the cache kept its session under last.
-- The pings sent before record none. The statement below is written exactly as db/schema.sql writes the column.

ALTER TABLE pings ADD COLUMN call TEXT CHECK (call IS NULL OR json_valid(call));
