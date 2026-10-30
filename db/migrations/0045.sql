-- Takes a version 44 database to version 45: the setting of a project's policy is a stage of its architect's works,
-- in the architect's session, instead of a work of its own of the graph policy. A setting is a matter of the kind
-- set_policy, which may go on over several works of the architect, and the views of the policy being set read its
-- rounds by that matter; the view of the policy in force reads the text of a setting in whichever work it was written;
-- the round that puts a study before the owner, and holds the setting until the answer comes, is no entry of the
-- architect's log. A matter that said a setting of a work of its own had ended, whose path was what to do next, is
-- withdrawn: no such matter comes any longer, and the architect's next understanding thinks what to do next. A setting
-- still running as a work of its own is taken over by the engine when it next goes on, as every work on an older flow
-- version is: it ends, its hold on new starts is let go, and the architect's next check of the policy decides again.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

DROP VIEW policy_readings;
DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;
DROP VIEW policy_current;
DROP VIEW architect_log;

UPDATE architect_matters
SET withdrawn = 'The setting of a policy that ended is a stage of the architect''s works now, which goes on to what to do next itself'
WHERE kind = 'policy' AND handled_round IS NULL AND withdrawn IS NULL;

-- The settings of each project's policy, by which the views of a setting tell its rounds.
CREATE INDEX architect_matters_settings ON architect_matters (project, id) WHERE kind = 'set_policy';

CREATE VIEW architect_log AS
  -- Each project's architect's log, newest first, its latest forty entries: each judgment it handed in (whether the
  -- policy is right, whether what runs fits, each matter's answer, each rethink after a batch) and the owner's answer
  -- to each study of a new policy, by the round that gave it, with the matter it handled and what it said, its
  -- conclusion with its reasons; the round that put the study before the owner, and holds the setting until the answer
  -- comes, is no entry. Read by: cells `U1`, `P1` and `N1`.
  SELECT project, work, round, cell, at, matter, said FROM (
    SELECT w.project, r.work, r.n AS round, r.cell, r.ended_at AS at,
      (SELECT m.id FROM architect_matters m WHERE m.handled_work = r.work AND m.handled_round = r.n) AS matter,
      coalesce(json_extract(r.output, '$.progress'),
        json_extract(r.output, '$.ruling.choice') || ': ' || json_extract(r.output, '$.ruling.text')) AS said,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph IN ('architect', 'policy') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'held')
      AND r.cell IN ('P1', 'U2', 'policy_right', 'understand.policy', 'N1', 'U3', 'fit', 'understand.fit', 'H1', 'T1',
        'set_straight', 'handle.set_straight', 'H3', 'ways', 'handle.ways', 'H5', 'I2', 'take_in', 'handle.take_in',
        'H6', 'K1', 'card_sound', 'handle.card_sound', 'H7', 'F1', 'have_fixed', 'handle.have_fixed', 'H8', 'O1',
        'owner_words', 'handle.owner_words', 'H9', 'M1', 'answer_architect', 'handle.answer_architect', 'H10',
        'what_runs', 'handle.what_runs', 'B4', 'C1', 'rethink', 'batch.rethink', 'P3', 'owner_reads',
        'policy.owner_reads'))
  WHERE k <= 40;

CREATE VIEW policy_setting_rounds AS
  -- Each round of a setting of a project's policy at a cell of its stage, with the setting's matter, of the kind
  -- set_policy. A setting is a stage of the project's architect's works, which go on one after another: the check of
  -- the policy in force that sets it again raises its matter, and its rounds are those at the cells of the stage after
  -- that, in any of the works it goes on in, until the next setting's matter. A setting a flow version before held in
  -- a work of its own, or in one work of the architect, is not here. Read by: policy_draft and policy_trials.
  SELECT project, work, n, cell, transition, output, matter FROM (
    SELECT w.project, r.work, r.n, r.cell, r.transition, r.output,
      (SELECT max(m.id) FROM architect_matters m
       WHERE m.project = w.project AND m.kind = 'set_policy'
         AND (m.work < r.work OR (m.work = r.work AND m.round < r.n))) AS matter
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('P2', 'P3', 'P4', 'P5', 'P6', 'P7', 'P8', 'P9', 'P10', 'P11', 'P12'))
  WHERE matter IS NOT NULL;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the text written, or put together from a correction, last before the owner's latest
  -- approval in a setting of the project's policy, in whichever of its architect's works the setting went on in since
  -- its matter was raised, with the round of that approval; none before the first policy takes effect. A setting a flow
  -- version before held in a work of its own, or in one work of the architect, has the text in the work of the
  -- approval. A correction hands in only what it changes, and the engine puts the text together (`P5`); a correction
  -- of a flow version before that held the whole text. Older versions stay readable in the rounds and are given to no
  -- agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      coalesce(
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM architect_matters m JOIN rounds t ON t.work BETWEEN m.work AND a.work JOIN works y ON y.id = t.work
         WHERE m.id = (SELECT max(x.id) FROM architect_matters x
                       WHERE x.project = w.project AND x.kind = 'set_policy'
                         AND (x.work < a.work OR (x.work = a.work AND x.round < a.n)))
           AND y.graph = 'architect' AND y.project = w.project AND t.cell IN ('P4', 'P5', 'P8')
           AND t.transition IS NOT NULL
           AND (t.work > m.work OR t.n > m.round) AND (t.work < a.work OR t.n < a.n)
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
        (SELECT coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
           json_extract(t.output, '$.outcome.revision.policy'))
         FROM rounds t
         WHERE t.work = a.work AND t.cell IN ('P4', 'write', 'policy.write', 'P5', 'assemble', 'policy.assemble', 'P8',
           'correct', 'policy.correct')
           AND t.transition IS NOT NULL AND t.n < a.n
           AND coalesce(json_extract(t.output, '$.draft.policy'), json_extract(t.output, '$.outcome.policy'),
             json_extract(t.output, '$.outcome.revision.policy')) IS NOT NULL
         ORDER BY t.n DESC LIMIT 1)) AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY a.work DESC, a.n DESC) AS k
    FROM rounds a JOIN works w ON w.id = a.work
    WHERE w.graph IN ('architect', 'policy') AND a.cell IN ('P12', 'takes_effect', 'policy.takes_effect')
      AND a.transition IS NOT NULL)
  WHERE k = 1;

CREATE VIEW policy_draft AS
  -- Each setting's version of the policy being set, by the setting's matter: the round of the latest writing or
  -- correcting in it, its work and number, the text as it stands, and the probes with the answers the architect holds
  -- right. A correction hands in only what it changes, its edits and the probes it adds or changes beside those it
  -- keeps, and the text and the probes are then as the engine last put them together (`P5`), as the text is after a
  -- minor fix a reading found, which leaves the version as it was. A round of those cells that asked for tools, asked
  -- the owner, paused, answered the owner's words or gave way to a new session wrote no version. Read by: policy_text,
  -- policy_probes_asked and policy_trials; the cells on the architect's line take it from the returns that wrote it or
  -- put it together.
  SELECT t.matter, t.work, t.n AS round,
    (SELECT coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
       json_extract(x.output, '$.outcome.revision.policy'))
     FROM policy_setting_rounds x
     WHERE x.matter = t.matter AND x.cell IN ('P4', 'P5', 'P8') AND x.transition IS NOT NULL
       AND coalesce(json_extract(x.output, '$.draft.policy'), json_extract(x.output, '$.outcome.policy'),
         json_extract(x.output, '$.outcome.revision.policy')) IS NOT NULL
     ORDER BY x.work DESC, x.n DESC LIMIT 1) AS policy,
    coalesce(
      (SELECT json_extract(x.output, '$.draft.probes') FROM policy_setting_rounds x
       WHERE x.matter = t.matter AND x.cell = 'P5' AND x.transition IS NOT NULL
         AND (x.work > t.work OR (x.work = t.work AND x.n > t.n))
         AND json_extract(x.output, '$.draft.probes') IS NOT NULL
       ORDER BY x.work DESC, x.n DESC LIMIT 1),
      json_extract(t.output, '$.outcome.probes'), json_extract(t.output, '$.outcome.revision.probes')) AS probes
  FROM policy_setting_rounds t
  WHERE t.cell IN ('P4', 'P8') AND t.transition IS NOT NULL
    AND t.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'session_anew')
    AND NOT EXISTS (SELECT 1 FROM policy_setting_rounds x
                    WHERE x.matter = t.matter AND x.cell IN ('P4', 'P8') AND x.transition IS NOT NULL
                      AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers', 'session_anew')
                      AND (x.work > t.work OR (x.work = t.work AND x.n > t.n)));

CREATE VIEW policy_text AS
  -- The text of each setting's version of the policy being set, by the setting's matter, without its probes. Read by:
  -- cell `P5`; the engine, to check a reading of it.
  SELECT matter, work, round, policy FROM policy_draft;

CREATE VIEW policy_probes_asked AS
  -- The probes of each setting's version of the policy being set, by the setting's matter, the questions only, without
  -- the answers the architect holds right. Read by: cell `P6`, the new architect that reads the policy, which is given
  -- nothing besides them.
  SELECT d.matter,
    json_group_array(json_object('id', json_extract(p.value, '$.id'), 'ask', json_extract(p.value, '$.ask'))) AS probes
  FROM policy_draft d, json_each(d.probes) p
  GROUP BY d.matter;

CREATE VIEW policy_trials AS
  -- Each setting's version of the policy being set, by the setting's matter, with every reading of it by a new
  -- architect since it was written, each by its work and round, and the architect's judgment of each, oldest first.
  -- Read by: cells `P9`, `P11` and `P12`; policy_readings; the owner's interface.
  SELECT d.matter, d.work, d.round, d.policy, d.probes,
    (SELECT json_group_array(json_object('work', r.work, 'round', r.n,
       'reading', json(json_extract(r.output, '$.outcome.reading')),
       'verdict', json((SELECT json_extract(v.output, '$.outcome.verdict') FROM policy_setting_rounds v
                        WHERE v.matter = r.matter AND v.cell = 'P7' AND v.transition IS NOT NULL
                          AND json_extract(v.output, '$.outcome.verdict') IS NOT NULL
                          AND (v.work > r.work OR (v.work = r.work AND v.n > r.n))
                        ORDER BY v.work, v.n LIMIT 1))))
     FROM policy_setting_rounds r
     WHERE r.matter = d.matter AND r.cell = 'P6' AND r.transition IS NOT NULL
       AND json_extract(r.output, '$.outcome.reading') IS NOT NULL
       AND (r.work > d.work OR (r.work = d.work AND r.n > d.round)))
       AS readings
  FROM policy_draft d;

CREATE VIEW policy_readings AS
  -- The readings of each setting's version of the policy being set by new architects since it was written, by the
  -- setting's matter, each with the architect's judgment of it, oldest first, without the text and the probes they
  -- read. Read by: cell `P7`, which weighs a reading beside every reading of the version, and cell `P8`, which fixes
  -- what they left.
  SELECT matter, readings FROM policy_trials;
