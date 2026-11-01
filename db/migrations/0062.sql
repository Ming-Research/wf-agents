-- Takes a version 61 database to version 62: a draft of a new policy is dropped when the owner's reply to it keeps the
-- policy in force, which the architect hands in as kept, a round of `P2` with no policy; the draft that waits for the
-- owner's approval is the latest round of `P2`'s, and only when that round wrote one. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

DROP VIEW policy_draft;

CREATE VIEW policy_draft AS
  -- Each project's draft of a new policy that waits for the owner's approval: the latest the architect wrote (cell
  -- `P2`), in whichever of its works, when no new policy took effect after it (cell `P12`) and no later round of `P2`
  -- dropped it, the owner's reply keeping the policy in force (`kept`, with no policy); none otherwise. Read by: cell
  -- `P2`, which writes it again by the owner's reply, and cell `P12`, which makes it the policy in force; the owner's
  -- interface.
  SELECT project, work, round, policy FROM (
    SELECT w.project, r.work, r.n AS round, json_extract(r.output, '$.outcome.policy') AS policy,
      row_number() OVER (PARTITION BY w.project ORDER BY r.work DESC, r.n DESC) AS k
    FROM rounds r JOIN works w ON w.id = r.work
    WHERE w.graph = 'architect' AND r.cell = 'P2' AND r.transition IS NOT NULL) d
  WHERE k = 1 AND policy IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM rounds a JOIN works x ON x.id = a.work
    WHERE x.graph = 'architect' AND x.project = d.project AND a.cell = 'P12' AND a.transition IS NOT NULL
      AND (a.work > d.work OR (a.work = d.work AND a.n > d.round)));
