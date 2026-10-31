-- Takes a version 53 database to version 54: each project's architect keeps a notebook of four sections, what the
-- project is now, the plan, what is unclear and anything else, each version of a section kept as its cells wrote
-- it, and the notebook as it stands a view; the state of things, the notes, the log and the last plan, which the
-- notebook takes the place of, are no longer read, and their views go. Old notes are not carried over: the rounds
-- that wrote them keep them. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

DROP VIEW architect_state;
DROP VIEW architect_notes;
DROP VIEW architect_log;
DROP VIEW architect_last_fit;

CREATE TABLE notebook_edits (
  -- Each version of a section of a project's architect's notebook, as a round of its architect's line wrote it whole:
  -- what the project is now (project_now), the plan (plan), what is unclear (unclear), and anything else (other), in
  -- the owner's language. A section is what its latest version says; the versions before are kept for the owner.
  -- Read by: the engine, which gives each section to the architect's cells as an input of its own and checks a
  -- round's sections against their sizes, through notebook; the owner's interface, which shows each section with its
  -- versions.
  id INTEGER PRIMARY KEY,
  -- The project whose architect's notebook it is.
  project TEXT NOT NULL,
  -- Which section.
  section TEXT NOT NULL CHECK (section IN ('project_now', 'plan', 'unclear', 'other')),
  -- The section's whole text in this version.
  text TEXT NOT NULL,
  -- The round of the architect's line that wrote it: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

CREATE INDEX notebook_edits_by_section ON notebook_edits (project, section);

CREATE VIEW notebook AS
  -- Each project's architect's notebook as it stands: each section's latest version, with the round that wrote it.
  -- Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, section, text, work, round FROM (
    SELECT project, section, text, work, round,
      row_number() OVER (PARTITION BY project, section ORDER BY id DESC) AS k
    FROM notebook_edits)
  WHERE k = 1;
