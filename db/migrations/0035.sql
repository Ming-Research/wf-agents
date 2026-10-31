-- Takes a version 34 database to version 35: a correction of the policy being set keeps the probes it does not change
-- by naming them, and the engine puts the probes together with the text (cell `P5`), which the version's probes are
-- read from; a minor fix a reading found leaves the version as it was; the new architect that reads the policy is given
-- its probes without the version's round; and the readings of a version are read without its text and probes. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;

CREATE VIEW policy_draft AS
  -- Each wake's version of the policy being set: the round of the latest writing or correcting in the wake, the text as
  -- it stands, and the probes with the answers the architect holds right. A correction hands in only what it changes,
  -- its edits and the probes it adds or changes beside those it keeps, and the text and the probes are then as the
  -- engine last put them together (`P5`), as the text is after a minor fix a reading found, which leaves the version
  -- as it was; a correction of a flow version before that held the whole text, or the whole probes. Read by:
  -- policy_text, policy_probes_asked and policy_trials; the cells on the architect's line take it from the returns that
  -- wrote it or put it together.
  SELECT t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM rounds x
     WHERE x.work = t.work AND x.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
       'correct', 'policy.correct') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.n DESC LIMIT 1) AS policy,
    coalesce(
      (SELECT json_extract(x.output, '$.draft.probes') FROM rounds x
       WHERE x.work = t.work AND x.cell = 'P5' AND x.transition IS NOT NULL AND x.n > t.n
         AND json_extract(x.output, '$.draft.probes') IS NOT NULL
       ORDER BY x.n DESC LIMIT 1),
      json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM rounds t JOIN works w ON w.id = t.work
  WHERE w.graph = 'architect' AND t.cell IN ('P4', 'write', 'policy.write', 'P8', 'correct', 'policy.correct')
    AND t.transition IS NOT NULL
    AND t.n = (SELECT max(x.n) FROM rounds x
               WHERE x.work = t.work AND x.cell IN ('P4', 'write', 'policy.write', 'P8', 'correct', 'policy.correct')
                 AND x.transition IS NOT NULL);

CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cell `P5`; the
  -- engine, to check a reading of it.
  SELECT work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each wake's version of the policy being set, the questions only, without the answers the architect
  -- holds right. Read by: cell `P6`, the new architect that reads the policy, which is given nothing besides them.
  SELECT d.work,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.work;

CREATE VIEW policy_trials AS
  -- Each wake's version of the policy being set, with every reading of it by a new architect since it was written and
  -- the architect's judgment of each, oldest first. Read by: cells `P9`, `P11` and `P12`; policy_readings; the
  -- owner's interface.
  SELECT d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('round', r.n, 'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM rounds v
                        WHERE v.work = r.work AND v.cell IN ('P7', 'judge_read', 'policy.judge_read')
                          AND v.transition IS NOT NULL
                          AND v.n > r.n
                        ORDER BY v.n LIMIT 1))))
     FROM rounds r
     WHERE r.work = d.work AND r.cell IN ('P6', 'fresh_read', 'policy.fresh_read')
       AND r.transition IS NOT NULL AND r.n > d.round)
       AS readings
  FROM policy_draft d;

CREATE VIEW policy_readings AS
  -- The readings of each wake's version of the policy being set by new architects since it was written, each with the
  -- architect's judgment of it, oldest first, without the text and the probes they read. Read by: cell `P7`, which
  -- weighs a reading beside every reading of the version, and cell `P8`, which fixes what they left.
  SELECT work, readings FROM policy_trials;
