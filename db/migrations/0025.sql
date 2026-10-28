-- Takes a version 24 database to version 25: a correction of the policy being set hands in only what it changes, and
-- the engine puts the text together (cell P4), which the views of the policy being set and of the policy in force read;
-- a correction of a flow version before that held the whole text, which they read as before. Each statement below
-- that defines something is written exactly as db/schema.sql writes it.

DROP VIEW policy_current;
CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's
  -- latest approval in a wake of the project's architect, with the round of that approval; none before the first
  -- policy takes effect. A correction hands in only what it changes, and the engine puts the text together (P4); a
  -- correction of a flow version before that held the whole text. Older versions stay readable in the rounds and are
  -- given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
         json_extract(t.output, '$.outcome.revision.policy'))
       FROM rounds t WHERE t.work = a.work AND t.cell IN ('P3', 'P4', 'P7') AND t.transition IS NOT NULL AND t.n < a.n
         AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
       ORDER BY t.n DESC LIMIT 1) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph = 'architect' AND a.cell = 'P10' AND a.transition IS NOT NULL)
  WHERE k = 1;

DROP VIEW policy_draft;
CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the round of the latest writing or correcting in the wake, the text
  -- as it stands, and the probes with the answers the architect holds right. A correction hands in only what it
  -- changes, and the text is then as the engine last put it together (P4); a correction of a flow version before that
  -- held the whole text. Read by: policy_text, policy_probes_asked and policy_trials; the cells on the architect's
  -- line take it from the returns that wrote it or put it together.
  SELECT t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM rounds x WHERE x.work = t.work AND x.cell IN ('P3', 'P4', 'P7') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.n DESC LIMIT 1) AS policy,
    coalesce(json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('P3', 'P7') AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('P3', 'P7') AND x.transition IS NOT NULL);

DROP VIEW policy_text;
CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cell P4; the engine, to
  -- check a reading of it.
  SELECT work, round, policy FROM policy_draft;
