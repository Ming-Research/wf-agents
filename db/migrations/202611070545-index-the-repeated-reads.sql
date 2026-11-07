-- Takes a database to one with an index on each column its repeated reads filter by, which had none: the change log's
-- transaction numbers (read under the write lock by every write), the rounds of a session and of a cell, the
-- interjections of a matter and of a work's delivered rounds, the architect matters of a work and of an ask, the
-- notices and blockers of an ask, the blockers of a matter, and the works of a flow version. Each index below is
-- written exactly as db/schema.sql writes it.

-- The largest transaction number in the change log: Db::write reads the next one under the write lock (NEXT_TX).
CREATE INDEX changes_by_tx ON changes (tx);
-- The rounds a session ran: read by of_cli_session (crates/sessions/src/round.rs) for a CLI session's rounds.
CREATE INDEX rounds_by_session ON rounds (session);
-- The rounds at a cell: read by rounds_of_matter (crates/engine/src/work.rs) for an architect matter's rounds.
CREATE INDEX rounds_by_cell ON rounds (cell);
-- The interjections of an architect matter: read by conversation_entries for the matter's words and its talk.
CREATE INDEX interjections_by_matter ON interjections (matter);
-- The interjections a work took in, by the round that took them: read by words_given
-- (crates/engine/src/verification.rs) for the owner's words said since a merge.
CREATE INDEX interjections_by_work_delivered ON interjections (work, delivered_round);
-- The architect matters of a work and a kind: read by review::asked (crates/engine/src/review.rs) for a design review.
CREATE INDEX architect_matters_by_work ON architect_matters (work, kind);
-- The architect matters an owner's ask put: read by conversation_entries for an ask the architect holds.
CREATE INDEX architect_matters_by_asked ON architect_matters (asked);
-- The engine's notices about an ask: read by conversation_entries for the time and the text of an ask.
CREATE INDEX engine_notices_by_ask ON engine_notices (ask);
-- The blockers an ask holds: read by conversation_entries for whether an ask waits on the owner.
CREATE INDEX blockers_by_ask ON blockers (ask);
-- The blockers a matter holds: read by conversation_entries for whether a matter waits on the architect or the owner.
CREATE INDEX blockers_by_matter ON blockers (matter);
-- The works of each flow version: read by flows (crates/api/src/read.rs) for the number of works a version has.
CREATE INDEX works_by_flow_version ON works (flow_version);
