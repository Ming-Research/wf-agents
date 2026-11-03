-- Takes a version 74 database to version 75: the owner can hold a project's new starts too, so a hold on them has one
-- more reason, owner, beside architect and set_straight. The table is made afresh with the reason it allows, and its
-- rows are put back as they were. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

CREATE TABLE dispatch_holds_version_74 AS SELECT * FROM dispatch_holds;
DROP TABLE dispatch_holds;

CREATE TABLE dispatch_holds (
  -- What stops new works of a project from starting, one row for each: the owner's hold, its architect's own hold, its
  -- architect setting it straight. While a project has a row here, the engine starts no new work on its items; works
  -- already running go on, and the other projects' works start as usual.
  -- Read by: the engine at cell `S0`; the architect, through its board; the owner's interface.
  project TEXT NOT NULL,
  -- owner: held by the owner. architect: held by the architect's own action. set_straight: its plan of what to do next
  -- found the whole must be set straight. The owner releases any of them; the architect only its own and the one to
  -- set straight.
  reason TEXT NOT NULL CHECK (reason IN ('owner', 'architect', 'set_straight')),
  -- When it began.
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the owner or the architect said it for their own hold; NULL for the one to set
  -- straight, whose reason says why. Read by: the owner's interface.
  why TEXT,
  PRIMARY KEY (project, reason)
) STRICT;

INSERT INTO dispatch_holds (project, reason, since, why)
SELECT project, reason, since, why FROM dispatch_holds_version_74;
DROP TABLE dispatch_holds_version_74;
