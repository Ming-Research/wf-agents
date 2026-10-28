-- Takes a version 20 database to version 21: a project's state of things is its architect's latest understanding and
-- what its wrap-ups learned, not a round of cell U1 or E1 that asked for tools or paused, which wrote none of them; and
-- the cells on the architect's line that judge and correct a policy being set read it from the returns that wrote it,
-- not from the views of it, whose descriptions say so. Each statement below that defines something is written exactly
-- as db/schema.sql writes it.

DROP VIEW architect_state;
CREATE VIEW architect_state AS
  -- Each project's state of things as its architect last wrote it: its latest understanding (cell U1), with what its
  -- wrap-ups learned and every note written since; a round that asked for tools or paused wrote none of them. Read by:
  -- cells U1 and E1; the owner's interface.
  SELECT project, work, round, state, direction, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      json_extract(r.output, '$.outcome.direction') AS direction,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell = 'E1' AND x.transition IS NOT NULL
         AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'U1' AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner'))
  WHERE k = 1;

DROP VIEW policy_draft;
CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the text and the probes, with the answers the architect holds right,
  -- of the latest writing or correcting of the text in the wake. Read by: policy_text, policy_probes_asked and
  -- policy_trials; the cells on the architect's line take it from the returns that wrote it.
  SELECT t.work, t.n AS round,
    coalesce(json_extract(t.output, '$.outcome.policy'), json_extract(t.output, '$.outcome.revision.policy')) AS policy,
    coalesce(json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('P3', 'P7') AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('P3', 'P7') AND x.transition IS NOT NULL);

DROP VIEW policy_text;
CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cells P4 and P5; the
  -- engine, to check a reading of it.
  SELECT work, round, policy FROM policy_draft;
