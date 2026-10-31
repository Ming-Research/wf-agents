-- Takes a version 16 database to version 17: cell P11, which asks of the draft policy whether it would need setting
-- again at once, reads the draft's text too, which the view's description says. Each statement below that defines
-- something is written exactly as db/schema.sql writes it.

DROP VIEW policy_text;
CREATE VIEW policy_text AS
  -- The text of each wake's version of the policy being set, without its probes. Read by: cells P4, P5 and P11.
  SELECT work, round, policy FROM policy_draft;
