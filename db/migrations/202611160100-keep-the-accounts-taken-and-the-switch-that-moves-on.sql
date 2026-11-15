-- From version 18 of the schema to version 19: the installation keeps each account the agents were set to run under, by
-- the owner or by the engine moving on at the limit of the one in use, and each setting of the switch that lets the
-- engine move on. Both tables are new and hold nothing yet. The version's one row is replaced last.
-- The account the agents run under was taken at `time`, `n` counting from one: `account` is its directory as the
-- installation's settings list it, `taker` who took it (`Owner` on the page, or `Engine` moving on at the limit of the
-- one in use), and for the engine's, `left_account` the account it left and `resets` when that one's limit resets,
-- where it was known. The latest row whose account the settings still list is the account in use; with none, the first
-- they list. It is the installation's, not a work's. Hidden from agents.
CREATE TABLE accounts_taken (
    n INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    taker TEXT NOT NULL,
    time INTEGER NOT NULL,
    left_account TEXT,
    resets INTEGER
);

-- The switch that lets the engine move on to another account at the limit of the one in use was set on (1) or off (0)
-- at `time`, `n` counting from one: the latest row says how it stands, and none that it is off. It is the
-- installation's, not a work's.
CREATE TABLE moving_on (
    n INTEGER PRIMARY KEY,
    moving INTEGER NOT NULL CHECK (moving IN (0, 1)),
    time INTEGER NOT NULL
);
UPDATE schema_version SET version = 19;
