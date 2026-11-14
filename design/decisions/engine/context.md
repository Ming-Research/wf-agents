# The context manager

What a round of an agent's cell is sent as the cell is entered, and what its session holds.

### decision a-fill-counts-where-its-program-had-the-message
What a try's message fills is recorded before its program is given the message, and a session holds it only where the round reached the model, as the tokens the round took say, or its last try's program said, as its stream began, that it had taken its message in. What the round failed of says nothing of this. Whether a round begins its session is counted the same way.
Why: Recorded after the try, a try cut off or an engine stopped during it would leave the record short of what the session holds. The program writes the message into the session before it calls the service (decision `no-round-tries-the-network`), so a round tried again after it would put a second copy there; but a crash before the program read its input fails in passing as one after does, and a message referred to and lacking leaves the agent without it, which costs more than tokens.

### decision a-compaction-empties-the-session-at-its-round
A session compacted during a round holds, for the rounds after it, only what that round handed in: from where the compaction is recorded as the session reopened (decision a-compaction-reopens-the-session-under-its-id), the message the compaction's hook gives the session again within the round not counted as held.
Why: A compaction leaves only a summary of what the session was sent, which is no version of anything; the hook's message may be compacted again before the round ends, so what it gave is sent again rather than taken as held.

### decision a-datum-held-and-never-worked-from-is-not-named
Of a datum the session holds that is new to the cell, `held`'s line is sent only where the cell worked from an earlier version; where it worked from none, as a cell entered after the one that wrote the datum in the same session, nothing is sent of it.
Why: `held` names the version the cell last worked from, which such a cell has none of, and the session holds the datum already (design document `data-and-context`, 15); sending it whole again was turned down for what it costs (judgment, not measured).

### decision last-worked-from-a-datum-that-runs-no-cell
Of a datum that does not run the cell, the version the cell last worked from is the latest written before its last finished run's last round began; of any datum, a version a run of the cell wrote is one it worked from.
Why: Only a datum that runs a cell is consumed (design document `data-and-context`, 8), and every round is filled with the latest of each datum it is given, so that is the version the run worked from; a cell's own version is no change to it (design document `data-and-context`, 9).

### decision what-changed-in-the-tree-is-from-what-the-session-read
Of a datum whose versions the carrier compares itself, as the design tree's by git, what changed is what the carrier says, sent whatever its length, and from the version the session holds where the cell worked from none.
Why: Such a datum's content, the tree's commit and id, says nothing an agent reads, while the files that changed tell it what to read again; and it stands for what was read into the session, so what the session read is what a change is from, as the cells that are given the tree say.

### decision a-put-in-is-held-from-the-round-it-reached
A version put into a round counts as held by the session only from a round before the one entered, put in while it ran, and only where that round reached the model.
Why: A version is put into a round as it begins, as one its return would have written over; counted held for that round itself, it would never be filled while the message names it, and the next return would write over it unseen.

### decision a-version-written-over-is-given-with-why
Where a return was sent back for writing over a version it never saw, of a block the cell is not given, that version is given with why, in the words a datum given `latest` is given in; of a block the cell is given, the datum's own kind gives it.
Why: With no tool to read the blocks, nothing else shows the agent what it is to write over.

### decision a-version-is-held-by-what-it-is
A version is held by what it is, never by its content, so a new version that says what an earlier one said is given; a latest version the session holds is given by `held`'s line, though the session lacks the version the cell last worked from.
Why: Holding by content swallowed what was new: the owner saying the same twice, or a datum changed back, would not be given, while the cell was told of a version it never saw. Giving the latest whole where the version last worked from is lacking was turned down: the change since it is unseen either way, and the session holds every word.

### decision the-situations-written-stand-data-by-class
`wf-agents flow situations` writes each situation an entry of a cell can be in with the data in two classes, each standing alike, those of the triggers that ran the cell and the rest, and counts every combination of each datum's own state without writing it.
Why: The builder words a datum from its kind and how the session stands with it alone, so the product of every datum's states repeats the same texts, and runs to millions for a cell given ten data (`T1`, about 36 billion); by class every state of every datum is written, in a few hundred files a cell.
