-- Takes a version 30 database to version 31: what the architect understands of a project now holds facts only, and
-- the direction is the policy's, so the state of things it last wrote has no direction; its plan of what to do next
-- holds what is missing and what matters most beside whether what runs fits. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

DROP VIEW architect_last_fit;
DROP VIEW architect_state;

CREATE VIEW architect_state AS
  -- Each project's state of things as its architect last wrote it: what the project is now, as its latest
  -- understanding wrote it (cell `U1`), with what its wrap-ups learned and every note written since; a round that asked
  -- for tools or paused wrote none of them. Read by: cells `U1` and `E1`; the owner's interface.
  SELECT project, work, round, state, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell IN ('E1', 'wrap_up')
         AND x.transition IS NOT NULL
         AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('U1', 'state', 'understand.state') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner'))
  WHERE k = 1;

CREATE VIEW architect_last_fit AS
  -- Each project's architect's latest plan of what to do next: what was missing, what mattered most, and whether what
  -- ran fit; a plan of a flow version before what was missing and what mattered most were asked holds only the last.
  -- Read by: cell `N1`.
  SELECT project, work, round, missing, first, fit FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.missing') AS missing,
      json_extract(r.output, '$.outcome.first') AS first, json_extract(r.output, '$.outcome.fit') AS fit,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('N1', 'U3', 'fit', 'understand.fit') AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.fit') IS NOT NULL)
  WHERE k = 1;
