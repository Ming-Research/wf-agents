-- Takes a version 77 database to version 78: the health numbers. The engine samples once a minute what no other record
-- holds into the new table samples, and the indexes let the health numbers read a period of the rounds, the changes,
-- the architect's matters and the blockers by time. Each statement below that defines something is written exactly as
-- db/schema.sql writes it.

CREATE TABLE samples (
  -- The health numbers that no other record holds, a state at a moment: what the engine samples once a minute, of its
  -- own (the processor time its process used, the reads the owner's page made and how long serving them took, the
  -- works holding a slot and those with a step running) and of each project (how long the oldest matter in its
  -- architect's queue had waited). Every other health number is computed from the records it counts each time it is
  -- read. Written once; a minute the engine did not run has none. The measures, what each is and its unit, are those of
  -- the engine's registry of health numbers.
  -- Read by: the engine's health numbers, which the architect's numbers are made of.
  -- The measure, by its name in the registry, such as engine.cpu.
  measure TEXT NOT NULL,
  -- The project it is of; '' for the engine's own.
  project TEXT NOT NULL DEFAULT '',
  -- What of the project or the engine it is of, when a measure has several; '' when it has one.
  subject TEXT NOT NULL DEFAULT '',
  -- The start of the minute it was taken in, and its value.
  at INTEGER NOT NULL,
  value REAL NOT NULL,
  -- In this order, so that a measure's samples of a project over a period are read by their time.
  PRIMARY KEY (measure, project, at, subject)
) STRICT;

-- The rounds that started, and those that ended, within a time: the health numbers read a period by them.
CREATE INDEX rounds_by_start ON rounds (started_at);
CREATE INDEX rounds_by_end ON rounds (ended_at);
-- The changes to a table within a time, which the health numbers read a period by.
CREATE INDEX changes_by_time ON changes (table_name, at);
-- The matters of a kind that came within a time, which the health numbers read a period by.
CREATE INDEX architect_matters_by_kind ON architect_matters (kind, at);
-- The blockers raised before a time, which the health numbers read a period by.
CREATE INDEX blockers_by_time ON blockers (at);
