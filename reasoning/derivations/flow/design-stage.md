Decision design-from-records: The design stage derives the design from the project's records, its cells numbered in the order they run: `D1` follows the derivations that govern the problem up to the articles and derives the solution, finding that the records decide it with one workable way (`decided`), leave several workable ways open (`trade_off`), or have no workable way without revising a record (`must_change`); a trade-off or a revision has the chain it stands on challenged (`D3`) before `D4` writes the design, the revision of the records with its entry in their log, and the cards; a project that keeps no records designs from the problem itself and goes straight to `D4`; instead of comparing at least three approaches, choosing one and asking again whether there is a better design in every work, because where the records decide a question with one workable way a comparison is time spent for nothing, and where they leave it open the comparison is a trade-off the owner rules, which the records then hold for every later work.

Rests on: C:efficient, C:information, D:reasoning-rules-by-role, D:way-cells

Rejected:
- Comparing at least three approaches, choosing one and writing the design, then asking in the engineer's own session whether there is a better design, in every work: rejected because it compares where the records leave one workable way, a round spent for nothing, and decides what the records leave open without putting it to the owner.

Decision fresh-review-sessions: The challenge of the chain (`D3`) and the coherence review (`D7`) are on the review line and each starts a new session of it every time the work comes to it, by the action `fresh_session` on the row into it, instead of going on in the session of the line that made what they judge or in their own earlier one, because a judgment made in a session that holds the reasoning behind what it judges leans on that reasoning, and a session started afresh holds none of it.

Rests on: C:information, F:session-anchors, F:fresh-session-empty

Decision records-back-to-derive: A cell that finds the records must change (the design, `D4`; implementing, `I1`; the check of the code against the records, `T5`; the fix, `T6`) hands in `records_must_change` with what it found, and the work goes through `A8`, which chooses by whether the project keeps records: back to derive again (`D1`), carrying what was found, even halfway through implementation, or, in a project that keeps none, back to the cell that found it; instead of the cell revising the records itself, because a revision made outside the design stage reaches neither the records check nor the coherence review, and a design whose records changed under it is derived again from them.

Rests on: C:information, D:design-from-records, D:records-check, D:way-cells

Decision code-against-records: In a project that keeps records, the table review is followed by a round of the review line that checks the code against them (`T5`), by the work extension's section on reviewing the work, one column for each of its points (choices governed, contradiction, orphans, commitments, the chain challenged), and it hands in `records_must_change` for a choice no derivation governs, instead of a round that reviewed the change to the design tree only when the tree changed, because the code is governed by the records whether or not the work changes them, and a missing derivation is put to the owner as a revision, not fixed as a finding.

Rests on: C:information, D:reasoning-rules-by-role, D:records-back-to-derive

Rejected:
- A round of its own for the design tree after the table review, when the change touched the tree, with the four points on code against decisions answered in the table review: rejected because it reviewed the code against the records only as one column among the table review's, and a work that left the tree alone had none of it.
