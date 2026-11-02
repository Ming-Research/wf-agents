-- Takes a version 64 database to version 65: a design no longer puts decision cards to the owner, it hands in open
-- questions, each a matter of the project's architect of the new kind open_question, which needs no change of the
-- table, and the merge waits for the answer each comes to. The new view gives each work's open questions with their
-- answers. The statement below is written exactly as db/schema.sql writes it.

CREATE VIEW work_open_questions AS
  -- Each work's open questions as its design handed them in, each a matter of the project's architect, with the answer
  -- that came. Read by: cells `D1`, `D2` and `L2`; the engine, at cell `M1`.
  SELECT m.work, m.id AS matter, m.what AS question, json_extract(m.body, '$.answer') AS answer
  FROM architect_matters m
  WHERE m.kind = 'open_question';
