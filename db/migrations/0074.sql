-- Takes a version 73 database to version 74: an observation's title is in the owner's language and at most
-- `items.title_max` wide, no longer English of at most 20 characters. Only the comment of `title` in the observations'
-- table changes, so the table is made afresh with its rows. Each statement below that defines something is written
-- exactly as db/schema.sql writes it.

CREATE TABLE observations_version_73 AS SELECT * FROM observations;
DROP TABLE observations;

CREATE TABLE observations (
  -- Something worth doing in a project that no item does yet, as its architect recorded it from a report or its own
  -- finding: a record beside the items. The same thing reported again is counted, not recorded twice; an item takes
  -- observations in, and they close once its work merges. Who recorded or changed one, and when, is in changes.
  -- Read by: the architect's cells, through observations_open and the engine's overview; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The project it belongs to.
  project TEXT NOT NULL,
  -- What is wrong, as a short phrase in the owner's language, at most `items.title_max` wide, spaces not counted and a
  -- character of East Asian wide or full width counting 2, as the architect wrote it when it recorded it; NULL for one
  -- recorded before observations had titles, which may be in English and of the width then allowed. Read by: the
  -- owner's interface, which leads each row of a list of observations with it.
  title TEXT,
  -- What it is, why it is worth doing, and what happens if it is not done.
  what TEXT NOT NULL,
  -- Its tags, JSON list, each one of the project's set in its configuration.
  tags TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags)),
  -- Who found it, in what situation, and the matter it came from.
  source TEXT NOT NULL,
  -- How many times it was reported: 1 when recorded, one more for each report merged into it.
  times_reported INTEGER NOT NULL DEFAULT 1 CHECK (times_reported >= 1),
  -- What it refers to, JSON list: PRs, branches, design cards, the owner's words, and what later reports added.
  refs TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(refs)),
  -- The item that took it in, which closes it once its work merges; NULL while no item has.
  folded_into INTEGER REFERENCES items(id),
  -- Why it was closed: outdated, folded (an item took it in), owner (the owner closed it on the page) or other; NULL
  -- while it is open, and for one an item took in, which closes once that item's work merges.
  closed TEXT CHECK (closed IN ('outdated', 'folded', 'owner', 'other')),
  -- What made it so, in a sentence; set with closed.
  closed_why TEXT,
  CHECK ((closed IS NULL) = (closed_why IS NULL))
) STRICT;

INSERT INTO observations (id, project, title, what, tags, source, times_reported, refs, folded_into, closed, closed_why)
SELECT id, project, title, what, tags, source, times_reported, refs, folded_into, closed, closed_why
FROM observations_version_73;
DROP TABLE observations_version_73;

-- A project's observations.
CREATE INDEX observations_by_project ON observations (project);
