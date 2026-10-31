-- Takes a version 51 database to version 52: the check of a project's policy and the plan of what to do next are
-- matters of its architect's, of the kinds check_policy and make_plan, which come at the head of its queue; a new
-- policy is written in one cell, whose latest draft waits for the owner's approval until a new policy takes effect,
-- and the policy in force is that draft once approved; no hold on new starts is kept while a new policy is set; and a
-- work of the architect ends with no wrap-up. The setting of a policy a flow version before ran (its study, the
-- owner's reading of it, its text read by new architects and the owner's approval of it by buttons) is gone: an open
-- one is withdrawn, its study or its text no longer waits for the owner, and the hold on new starts it kept is let
-- go. The policy views of a setting's rounds, drafts, probes and readings go with it; the policy in force still reads
-- the texts those settings wrote. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

-- The changes below are the migration's, not a writer's, so the triggers that record changes to the rows they
-- write are dropped first; every trigger is made afresh once the migration is done.
DROP TRIGGER IF EXISTS track_architect_matters_update;
DROP TRIGGER IF EXISTS track_owner_asks_update;
DROP TRIGGER IF EXISTS track_dispatch_holds_delete;

UPDATE architect_matters
SET withdrawn = 'The setting of a policy no longer runs so; the next check of the policy decides again'
WHERE kind = 'set_policy' AND handled_round IS NULL AND withdrawn IS NULL;

UPDATE owner_asks SET answer = json_object('by', 'engine', 'at', CAST(unixepoch('subsec') * 1000 AS INTEGER), 'text',
    'No answer is needed: the setting of a policy no longer runs so; the next check of the policy decides again.')
WHERE answer IS NULL AND json_extract(body, '$.kind') IN ('study', 'policy_approval')
  AND id IN (SELECT asked FROM architect_matters WHERE kind = 'set_policy' AND asked IS NOT NULL
             AND withdrawn = 'The setting of a policy no longer runs so; the next check of the policy decides again');

DELETE FROM dispatch_holds WHERE reason = 'policy';

CREATE TABLE dispatch_holds_version_51 AS SELECT * FROM dispatch_holds;
DROP TABLE dispatch_holds;

CREATE TABLE dispatch_holds (
  -- What stops new works of a project from starting, one row for each: its architect's own hold, its architect setting
  -- it straight. While a project has a row here, the engine starts no new work on its items; works already running go
  -- on, and the other projects' works start as usual.
  -- Read by: the engine at cell `S0`; the architect, through its board; the owner's interface.
  project TEXT NOT NULL,
  -- architect: held by the architect's own action. set_straight: its plan of what to do next found the whole must be
  -- set straight.
  reason TEXT NOT NULL CHECK (reason IN ('architect', 'set_straight')),
  -- When it began.
  since INTEGER NOT NULL,
  -- Why, in the owner's language, as the architect said it for its own hold; NULL for the others, whose reason says
  -- why. Read by: the owner's interface.
  why TEXT,
  PRIMARY KEY (project, reason)
) STRICT;

INSERT INTO dispatch_holds (project, reason, since, why)
SELECT project, reason, since, why FROM dispatch_holds_version_51;
DROP TABLE dispatch_holds_version_51;

DROP INDEX architect_matters_settings;

DROP VIEW policy_readings;
DROP VIEW policy_trials;
DROP VIEW policy_probes_asked;
DROP VIEW policy_text;
DROP VIEW policy_draft;
DROP VIEW policy_current;
DROP VIEW policy_setting_rounds;

CREATE VIEW policy_current AS
  -- Each project's policy in force: the draft the architect wrote last (cell `P2`) before the round in which the
  -- owner's approval took effect (cell `P12`), in whichever of its works, with the round of that approval; none before
  -- the first policy takes effect. A flow version before set a policy in a setting of its own, whose text was written,
  -- or put together from a correction, last before the owner's latest approval in it, in whichever of the architect's
  -- works the setting went on in since its matter was raised; a setting a flow version before that held in a work of
  -- its own, or in one work of the architect, has the text in the work of the approval. Older versions stay readable in
  -- the rounds and are given to no agent. Read by: the engine, for the architect's cells; the owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, a.work, a.n AS round,
      coalesce(
        (SELECT json_extract(t.output, '$.outcome.policy') FROM rounds t JOIN works y ON y.id = t.work
         WHERE y.graph = 'architect' AND y.project = w.project AND t.cell = 'P2' AND t.transition IS NOT NULL
           AND (t.work < a.work OR (t.work = a.work AND t.n < a.n))
           AND json_extract(t.output, '$.outcome.policy') IS NOT NULL
         ORDER BY t.work DESC, t.n DESC LIMIT 1),
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
  -- Each project's draft of a new policy that waits for the owner's approval: the latest the architect wrote (cell
  -- `P2`), in whichever of its works, when no new policy took effect after it (cell `P12`); none otherwise. Read by:
  -- cell `P2`, which writes it again by the owner's reply, and cell `P12`, which makes it the policy in force; the
  -- owner's interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.policy') AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'P2' AND r.transition IS NOT NULL
      AND json_extract(r.output, '$.outcome.policy') IS NOT NULL) d
  WHERE k = 1 AND NOT EXISTS (
    SELECT 1 FROM rounds a JOIN works x ON x.id = a.work
    WHERE x.graph = 'architect' AND x.project = d.project AND a.cell = 'P12' AND a.transition IS NOT NULL
      AND (a.work > d.work OR (a.work = d.work AND a.n > d.round)));

DROP VIEW architect_state;

CREATE VIEW architect_state AS
  -- Each project's state of things as its architect last wrote it: what the project is now, as its latest
  -- understanding wrote it (cell `U1`), with what its wrap-ups learned and every note written since; a round that asked
  -- for tools, paused or answered the owner's words wrote none of them. Read by: cell `U1`; the owner's interface.
  SELECT project, work, round, state, learned, notes FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.state') AS state,
      (SELECT json_group_array(json_extract(x.output, '$.outcome.learned')) FROM rounds x JOIN works y ON y.id = x.work
       WHERE y.graph = 'architect' AND y.project = w.project AND x.cell IN ('E1', 'wrap_up')
         AND x.transition IS NOT NULL
         AND x.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers')
         AND (x.work > r.work OR (x.work = r.work AND x.n > r.n))) AS learned,
      (SELECT json_group_array(json_object('item', n.item, 'text', n.text)) FROM architect_notes n
       WHERE n.project = w.project AND (n.work > r.work OR (n.work = r.work AND n.round > r.n))) AS notes,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell IN ('U1', 'state', 'understand.state') AND r.transition IS NOT NULL
      AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner', 'answers'))
  WHERE k = 1;

DROP VIEW work_handled;

CREATE VIEW work_handled AS
  -- The matters each work of an architect handled, in the order it handled them: each matter, its kind, the cell and
  -- the round that closed it, and what that round said it did. Read by: the owner's interface.
  SELECT m.handled_work AS work, m.handled_round AS round, m.id AS matter, m.kind, m.detail, m.what, r.cell,
    json_extract(r.output, '$.progress') AS said
  FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
  ORDER BY m.handled_work, m.handled_round;
