-- Takes a version 21 database to version 22: a round that missed its session's prompt cache is a fault of wf-agents,
-- which the engine tells the owner in a notice that names the fault, each miss kept with the notice it adds to. Each
-- statement below that defines something is written exactly as db/schema.sql writes it.

ALTER TABLE engine_notices ADD COLUMN fault TEXT;

CREATE TABLE cache_misses (
  -- Each round that missed its session's prompt cache: it resumed the session within the cache's lifetime since the
  -- session's last call, a round's or a keep-warm ping's, and still wrote to the cache again at least half of what the
  -- session held, the last prompt of its round before as recorded. A miss is a fault of wf-agents, whichever project
  -- it came up in: the engine tells the owner in a notice of the fault, and a further miss with the same cause on the
  -- same line of the same project, while that notice is unread, adds to it instead of making a new one. Written once,
  -- never changed.
  -- Hidden from agents.
  -- Read by: the engine, which finds the notice a miss adds to; the owner's interface, which shows with each notice
  -- how many misses it holds, the tokens they wrote again and what they cost.
  -- The round that missed: its work, and its number in the work.
  work INTEGER NOT NULL,
  round INTEGER NOT NULL,
  -- The line of its session.
  line TEXT NOT NULL,
  -- The cause as far as the record tells it, JSON: each part of the command line that differs from the session's
  -- round before, with its value there and in this round; an empty list when none differs.
  differs TEXT NOT NULL CHECK (json_valid(differs)),
  -- The tokens the round wrote to the cache again: those its first call wrote, or the whole round's when its record
  -- does not list its calls.
  written INTEGER NOT NULL CHECK (written >= 0),
  -- The tokens the session held: the last prompt of its round before, as recorded.
  held INTEGER NOT NULL CHECK (held > 0),
  -- What writing them again cost beyond reading them, in US dollars, priced from what the round cost; NULL when its
  -- record does not tell.
  cost_usd REAL,
  -- The notice that tells the owner of it.
  notice INTEGER NOT NULL REFERENCES engine_notices(id),
  PRIMARY KEY (work, round),
  FOREIGN KEY (work, round) REFERENCES rounds (work, n)
) STRICT;

-- The cache misses a notice tells of.
CREATE INDEX cache_misses_by_notice ON cache_misses (notice);
