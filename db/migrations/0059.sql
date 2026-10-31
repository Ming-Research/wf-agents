-- Takes a version 58 database to version 59: a round that missed its session's prompt cache after what comes before
-- the session's messages changed since its round before (the model, the tools the envelope's schema is given with, the
-- command line's own version), as when a work moved to a new flow version, is no fault of wf-agents, and is kept for
-- what it cost with no notice. The cache misses' notice may then be NULL, so the table is made afresh with its rows.
-- Each statement below that defines something is written exactly as db/schema.sql writes it.

CREATE TABLE cache_misses_version_58 AS SELECT * FROM cache_misses;
DROP TABLE cache_misses;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and its first call still read from the cache less than half
  -- of what the session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents,
  -- whichever project it came up in: the engine tells the owner in a notice of the fault, and a further miss with the
  -- same cause on the same line of the same project, while that notice is unread or a matter forwarded from it waits
  -- in an architect's queue, adds to it instead of making a new one. A miss after what comes before the session's
  -- messages in the prompt changed since its round before (the model, the tools the envelope's schema is given with,
  -- the command line's own version), as when a work moved to a new flow version, is the command line writing the
  -- session again by design: no fault, kept for what it cost, with no notice. Written once, never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the notice a miss adds to, and hands the misses of a fault forwarded to an
  -- architect to it; the owner's interface, which shows with each notice how many misses it holds, the tokens they
  -- wrote again and what they cost, and counts the misses that are faults among what was spent for nothing.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line and each variable of its environment
  -- that differs from the session's round before, with its value there and in this round; an empty list when none
  -- differs.
  differs TEXT NOT NULL CHECK (json_valid(differs)),
  -- The tokens of what the session held that the round's first call did not read from the cache, and so wrote to it
  -- again.
  written INTEGER NOT NULL CHECK (written >= 0),
  -- The tokens the session held: the last prompt of its round before, as recorded.
  held INTEGER NOT NULL CHECK (held > 0),
  -- What writing them again cost beyond reading them, in US dollars, priced from what the round cost; NULL when its
  -- record does not tell.
  cost_usd REAL,
  -- The notice that tells the owner of it; NULL for a miss the engine expects, which is no fault.
  notice INTEGER REFERENCES engine_notices(id),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, notice)
SELECT work, round, line, differs, written, held, cost_usd, notice FROM cache_misses_version_58;
DROP TABLE cache_misses_version_58;

CREATE INDEX cache_misses_by_notice ON cache_misses (notice);
