-- Takes a database to one whose indexes let the reads that scanned a whole table find their rows by a column: the next
-- transaction number, the rounds of a CLI session and of a matter's cell, the interjections and matters the owner's words
-- reach, the notices and blockers of an owner's ask, and the works of a flow version. Each statement below is written
-- exactly as db/schema.sql writes it.

CREATE INDEX changes_by_tx ON changes (tx);
CREATE INDEX sessions_by_cli_session ON sessions (cli_session);
CREATE INDEX rounds_by_session ON rounds (session);
CREATE INDEX rounds_by_cell ON rounds (cell);
CREATE INDEX interjections_by_matter ON interjections (matter);
CREATE INDEX interjections_by_work_delivered ON interjections (work, delivered_round);
CREATE INDEX architect_matters_by_work ON architect_matters (work, kind);
CREATE INDEX architect_matters_by_asked ON architect_matters (asked);
CREATE INDEX engine_notices_by_ask ON engine_notices (ask);
CREATE INDEX blockers_by_ask ON blockers (ask);
CREATE INDEX blockers_by_matter ON blockers (matter);
CREATE INDEX works_by_flow_version ON works (flow_version);
