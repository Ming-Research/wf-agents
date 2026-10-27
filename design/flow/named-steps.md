Decision: Everything the engine carries out has a name in the flow (each action, check and step done every time, besides the engine's own steps), with its prose kept as the description, and the engine has exactly one implementation per name; the compiler checks both ways, that every name used is implemented and every implementation is used, instead of the engine reading the prose, because prose cannot be checked and two readings of it drift.

Decision: An input read from the database names the table or view it reads, and the compiler loads `db/schema.sql` into an in-memory database to check that each one exists, because an input whose source is missing must fail when the flow is compiled, not when a work reaches the cell.

Decision: Rows on the main path are flagged in the flow, and the compiled flow carries the flags, because the anomaly view counts every transition off the main path without agents reporting it.
