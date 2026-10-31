-- Takes a version 60 database to version 61: an observation has a title, what is wrong as a short phrase, which the
-- architect gives one as it records it and which leads each row of a list of observations; one recorded before has
-- none. The column goes before what the observation is, so the table is made afresh with its rows, and the views over
-- it with the title. Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW observations_open;
DROP VIEW observations_all;

CREATE TABLE observations_version_60 AS SELECT * FROM observations;
DROP TABLE observations;

CREATE TABLE observations (
  -- Something worth doing in a project that no item does yet, as its architect recorded it from a report or its own
  -- finding: a record beside the items. The same thing reported again is counted, not recorded twice; an item takes
  -- observations in, and they close once its work merges. Who recorded or changed one, and when, is in changes.
  -- Read by: the architect's cells, through observations_open and the engine's overview; the owner's interface.
  id INTEGER PRIMARY KEY,
  -- The project it belongs to.
  project TEXT NOT NULL,
  -- What is wrong, as a short phrase in English of at most 20 characters, spaces not counted, as the architect wrote it
  -- when it recorded it; NULL for one recorded before observations had titles. Read by: the owner's interface, which
  -- leads each row of a list of observations with it.
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

INSERT INTO observations (id, project, what, tags, source, times_reported, refs, folded_into, closed, closed_why)
SELECT id, project, what, tags, source, times_reported, refs, folded_into, closed, closed_why
FROM observations_version_60;
DROP TABLE observations_version_60;

CREATE INDEX observations_by_project ON observations (project);

CREATE VIEW observations_all AS
  -- Every observation of each project as the owner's page shows it: closed or not and why (one an item took in is
  -- closed once that item's work merged), with when it was recorded and last changed, as changes has it.
  -- Read by: the owner's interface; observations_open; the engine, for the overview the architect is given.
  SELECT o.project, o.id, o.title, o.what, o.tags, o.times_reported, o.source, o.refs, o.folded_into,
    CASE WHEN o.closed IS NOT NULL THEN o.closed WHEN m.merged IS NOT NULL THEN 'folded' END AS closed,
    CASE WHEN o.closed IS NOT NULL THEN o.closed_why
         WHEN m.merged IS NOT NULL THEN 'Item ' || o.folded_into || ' took it in, and its work merged' END AS closed_why,
    (SELECT min(c.at) FROM changes c WHERE c.table_name = 'observations' AND c.row_key = json_object('id', o.id))
      AS created,
    (SELECT max(c.at) FROM changes c WHERE c.table_name = 'observations' AND c.row_key = json_object('id', o.id))
      AS updated,
    m.merged
  FROM observations o LEFT JOIN (
    SELECT w.item, max(r.ended_at) AS merged FROM rounds r JOIN works w ON w.id = r.work
    WHERE r.cell IN ('M2', '9', 'merge_queue', 'merge.queue')
      AND r.transition = 'merged' GROUP BY w.item) m ON m.item = o.folded_into;

CREATE VIEW observations_open AS
  -- Each project's open observations, each with its number, its title, what it is, its tags, how many times it was
  -- reported, where it came from, what it refers to and the item that took it in, if one has. Read by: cell `X2`,
  -- which searches them; the engine.
  SELECT project, id, title, what, tags, times_reported, source, refs, folded_into
  FROM observations_all WHERE closed IS NULL;
