-- Takes a version 2 database to version 3: the flow no longer asks for the reason a review column does not apply or
-- a blind finding was withdrawn, for the checks found flaky or known red on main, or for how far a paused or
-- redirected round got apart from its progress, so the views that carried them go, and the view of each round's
-- progress keeps the progress alone. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

DROP VIEW not_applicable_columns;
DROP VIEW withdrawn_findings;
DROP VIEW flaky_reds;
DROP VIEW work_progress;

CREATE VIEW work_progress AS
  -- What each round said it did, how far a paused or redirected round got included. Read by: the owner's interface.
  SELECT r.work, r.id AS round, r.cell, json_extract(r.output, '$.progress') AS progress, r.ended_at
  FROM rounds r
  WHERE r.output IS NOT NULL;
