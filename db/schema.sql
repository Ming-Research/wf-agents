-- The store: the items, the works done for them, and everything kept of each work.
--
-- Every table is only inserted into. An entry of a work's records is keyed by the work and by its place in the order
-- of what happened to the work (`at`); every other entry by the work and by what makes it unique within the work.
-- Times are milliseconds since the start of 1970, as the driver's clock gives them. A value that is an enum with data
-- is JSON text in one column; a name of a person, a writer or who paused is the name of its variant (`Owner`, `Person`,
-- `Engine`). A boolean
-- is 0 or 1.

-- The version of this schema, as the code that made the database numbers it: one row, written when the database is
-- made and replaced by each migration that brings it to a later version, the one row in the store that is changed. A
-- database of an earlier version is brought to this one by the migrations of `migrations/` as it is opened; one of any
-- other version is not opened.
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY
);

-- An item: a thing to be done, as it was added. Its number is given by the store, counting from one. Its level is the
-- name of the level of the installation it was given as it was added; null for an item kept before items had levels.
CREATE TABLE items (
    number INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    added INTEGER NOT NULL,
    level TEXT
);

-- An item depends on another as it is added, one added before it.
CREATE TABLE depends (
    item INTEGER NOT NULL REFERENCES items (number),
    on_item INTEGER NOT NULL REFERENCES items (number),
    PRIMARY KEY (item, on_item),
    CHECK (on_item < item)
);

-- A change made to an item since it was added, its nth counting from one, at this time (JSON): it was given a title,
-- put at a level, came to depend on an item or no longer depends on one. What an item is as it stands is what it was
-- added as with each of its changes made in turn: its latest title and level, or those it was added with where it has
-- none, and the items it was added depending on with each it came to depend on, but those it no longer depends on.
CREATE TABLE item_changes (
    item INTEGER NOT NULL REFERENCES items (number),
    n INTEGER NOT NULL,
    at INTEGER NOT NULL,
    change TEXT NOT NULL,
    PRIMARY KEY (item, n)
);

-- The work done for an item, on a flow: an item is added together with its one work. Its number is given by the
-- store, counting from one. `flow` is the flow's name only: which version of the flow the work ran on is not kept
-- yet.
CREATE TABLE works (
    number INTEGER PRIMARY KEY,
    item INTEGER NOT NULL REFERENCES items (number),
    flow TEXT NOT NULL,
    added INTEGER NOT NULL
);

-- A run of the work began, of a cell, because of what `because` says (JSON: a list of causes).
CREATE TABLE started (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    cell TEXT NOT NULL,
    because TEXT NOT NULL,
    PRIMARY KEY (work, at),
    UNIQUE (work, run)
);

-- The version a run started from, of one block its cell reads or may write: the place of the entry that wrote it, or
-- null where the block had none.
CREATE TABLE basis (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    block TEXT NOT NULL,
    version INTEGER,
    PRIMARY KEY (work, run, block),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A round of a run began.
CREATE TABLE rounds (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- The round of a run that was under way ended.
CREATE TABLE returned (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A run rests at a call until it has its result. `who` is the person who gives the result, or null when the engine
-- makes the call itself.
CREATE TABLE called (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    call INTEGER NOT NULL,
    run INTEGER NOT NULL,
    who TEXT,
    PRIMARY KEY (work, at),
    UNIQUE (work, call),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A call has its result.
CREATE TABLE done (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    call INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, call) REFERENCES called (work, call)
);

-- A run finished, writing the blocks of `writes`. `dealt_with` is what its return says it has dealt with (JSON: a list
-- of names, each of a block of the run's `basis` or one put in to it, in the order the return named them, empty where
-- it dealt with nothing); null where the return did not say, as none of the form before it did: such a run dealt with
-- every block of its `basis`.
CREATE TABLE finished (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    dealt_with TEXT,
    PRIMARY KEY (work, at),
    UNIQUE (work, run),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A block a run wrote as it finished, `n` its place among the run's writes, the first being one: whether the change
-- matters, which is 0 where the run handed the version in as changing nothing in meaning and 1 otherwise, and its value
-- where it is a choice.
CREATE TABLE writes (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    n INTEGER NOT NULL,
    block TEXT NOT NULL,
    matters INTEGER NOT NULL CHECK (matters IN (0, 1)),
    value TEXT,
    PRIMARY KEY (work, run, n),
    FOREIGN KEY (work, run) REFERENCES finished (work, run)
);

-- A run was abandoned.
CREATE TABLE abandoned (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A version of a block (`version`, the place of the entry that wrote it, before this one) was put into the round of a
-- run that was under way, after the round began, so that the round was told of it.
CREATE TABLE put_in (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    block TEXT NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A person or the outside (`writer`) wrote a block: whether the change matters, and its value where it is a choice.
CREATE TABLE written (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    block TEXT NOT NULL,
    writer TEXT NOT NULL,
    matters INTEGER NOT NULL CHECK (matters IN (0, 1)),
    value TEXT,
    PRIMARY KEY (work, at)
);

-- The owner let a cell go on after it reached its limit.
CREATE TABLE let_go (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    cell TEXT NOT NULL,
    PRIMARY KEY (work, at)
);

-- The work was paused (1) or let on again (0), by `pauser` (`Owner` or `Engine`), and why where it was paused with a
-- reason: the engine gives one, the owner none.
CREATE TABLE pauses (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
    pauser TEXT NOT NULL,
    why TEXT,
    PRIMARY KEY (work, at)
);

-- The work's item was cancelled: one row for a work that was, with why where whoever cancelled it said.
CREATE TABLE cancelled (
    work INTEGER PRIMARY KEY REFERENCES works (number),
    at INTEGER NOT NULL,
    why TEXT
);

-- Every work was paused (1) or that pause lifted (0), by `pauser` (`Owner`, or `Engine` as it started with works going
-- on), at `time`, `n` counting from one: the latest row says whether the pause holds and whose it is, and none that it
-- does not hold. While it holds no work begins a round but those of a run that answers a note, as the work's own pause
-- holds them back. It is the installation's, not a work's.
CREATE TABLE everything_paused (
    n INTEGER PRIMARY KEY,
    paused INTEGER NOT NULL CHECK (paused IN (0, 1)),
    pauser TEXT NOT NULL,
    time INTEGER NOT NULL
);

-- What the work held outside the engine was let go, once it had ended and nothing of it was under way: one row for a
-- work that did.
CREATE TABLE released (
    work INTEGER PRIMARY KEY REFERENCES works (number),
    at INTEGER NOT NULL
);

-- The content of a version of a block, the version named by the place of the entry that wrote it.
CREATE TABLE contents (
    work INTEGER NOT NULL REFERENCES works (number),
    block TEXT NOT NULL,
    version INTEGER NOT NULL,
    content TEXT NOT NULL,
    PRIMARY KEY (work, block, version)
);

-- What a round of a run was given (JSON), the first round being one.
CREATE TABLE given (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    given TEXT NOT NULL,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- Something was filled into the session of a try of a round of a run, the round's first being one and the try's first
-- in its round being one: `filled` is what (JSON: a list, in the order it was filled, of which work and item the
-- session is for, the cell's task with the digest of its text, a version of a block whole or as what changed since an
-- earlier one, or the results of calls), empty where the session lacked nothing.
CREATE TABLE filled (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    filled TEXT NOT NULL,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- What came back at the end of a round of a run (JSON), the first round being one. A round that ended after its run
-- was abandoned has none: what came back from it was dropped.
CREATE TABLE back (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    back TEXT NOT NULL,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- What a call is (JSON): what an agent asked for, or a limit the run reached.
CREATE TABLE calls (
    work INTEGER NOT NULL,
    call INTEGER NOT NULL,
    what TEXT NOT NULL,
    PRIMARY KEY (work, call),
    FOREIGN KEY (work, call) REFERENCES called (work, call)
);

-- The result of a call.
CREATE TABLE results (
    work INTEGER NOT NULL,
    call INTEGER NOT NULL,
    result TEXT NOT NULL,
    PRIMARY KEY (work, call),
    FOREIGN KEY (work, call) REFERENCES called (work, call)
);

-- What a round of a run that ended took, the first round being one: its cost in millionths of a dollar, its tokens,
-- the context it ended with in tokens (NULL where it is not known), and how often its session was compacted.
CREATE TABLE spent (
    work INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    usd_micros INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    context_tokens INTEGER,
    compactions INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (work, run, round),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A new session of a line of the work began, or begins with the line's next round, and why (JSON).
CREATE TABLE sessions (
    work INTEGER NOT NULL REFERENCES works (number),
    at INTEGER NOT NULL,
    line TEXT NOT NULL,
    why TEXT NOT NULL,
    PRIMARY KEY (work, at)
);

-- A try of a round of a run began, the round's first being one and the try's first in its round being one, on this
-- level, model, reasoning effort (NULL where the program ran at its own) and account (NULL where it ran with its own
-- default directory).
CREATE TABLE ran (
    work INTEGER NOT NULL,
    at INTEGER NOT NULL,
    run INTEGER NOT NULL,
    round INTEGER NOT NULL,
    attempt INTEGER NOT NULL,
    level TEXT NOT NULL,
    model TEXT NOT NULL,
    effort TEXT,
    account TEXT,
    PRIMARY KEY (work, at),
    FOREIGN KEY (work, run) REFERENCES started (work, run)
);

-- A thing that happened to the work was taken at `time`, and its entries begin at the place `place`.
CREATE TABLE moments (
    work INTEGER NOT NULL REFERENCES works (number),
    place INTEGER NOT NULL,
    time INTEGER NOT NULL,
    PRIMARY KEY (work, place)
);

-- A copy of the database was taken, under this name, at `taken`. `seq` is the order the copies were recorded in, which
-- a clock that jumped cannot change: the copies kept are the newest by it. The rows are only added, so the number of a
-- row is one more than the newest's. A copy belongs to no work.
CREATE TABLE backups (
    seq INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    taken INTEGER NOT NULL
);

-- The copy of this name was uploaded off the machine, at `at`.
CREATE TABLE backup_uploads (
    name TEXT PRIMARY KEY REFERENCES backups (name),
    at INTEGER NOT NULL
);

-- A step of keeping a copy failed at `at`, for this reason: `step` says which (`snapshot`, `private`, `upload` or
-- `prune`), and `name` the copy it was about. `seq` is the order the failures were recorded in.
CREATE TABLE backup_failures (
    seq INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    name TEXT NOT NULL,
    step TEXT NOT NULL,
    why TEXT NOT NULL
);

-- A step of keeping the copies ended at `at` with nothing failing, and so mended the failures up to the one numbered
-- `mended`, the newest recorded when it ended. A failure is mended by the order they were recorded in, not by the
-- time, which a clock that jumped makes wrong. It is kept only when there was a failure to mend.
CREATE TABLE backup_steps (
    mended INTEGER PRIMARY KEY REFERENCES backup_failures (seq),
    at INTEGER NOT NULL
);
