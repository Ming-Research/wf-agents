### decision snapshot-is-a-release-asset
The database's copy is compressed and uploaded as an asset of a release of a private repository. Only assets named as the engine names its copies are ever deleted there.
Why: The database holds everything and every conversation, so the place must be private. Committing it to git was turned down because it keeps each copy for ever and is refused over 100 MB; Git LFS was turned down for its limits on volume and traffic; a repository made only for the copies adds nothing a release does not give. A release asset is large and can be deleted later. Others may write to the release, so what is not ours is left alone.

### decision copy-is-vacuum-into
A copy is made by SQLite's `VACUUM INTO` through a read-only reader of its own, never by copying the file.
Why: The store runs in WAL mode while the engine writes, so a copied file can lack what is still in the log or be torn; `VACUUM INTO` reads one snapshot and does not hold up the engine's writes.

### decision taken-and-uploaded-are-two-records
That a copy was taken and that it was uploaded are two records, and whether a copy needs uploading is read from the records, never taken from having just made it, even in the same pass.
Why: The engine may stop at any step (principle effects-known-as-done). Taking the upload to follow from the copy just made loses that day's copy off the machine when the engine stops between the two, and a failed upload would wait for the next day's copy.

### decision backup-destination-is-settings-and-private
Where the copies go is the installation's `[backup]` settings with its own token, not the data directory's git remote nor the project's repository, and the repository is checked to be private at every upload.
Why: The project's token is limited to the project's repository, which may be public, and a remote read from git would send the database wherever it happens to point. A repository can be made public after it was set, so a check at configuration is not enough. With no `[backup]` the copies stay on the machine, which shares the disk they guard against losing, and the page says so (principle nothing-passes-for-normal).

### decision backup-is-the-engines-own-tick
The backup and the retry of a failed upload are a periodic act of the installation, done one at a time and out of the flow. Neither the wait for the network nor the owner's pause of everything holds it: an upload that meets the network gone fails as any other does, is shown on the page, and is tried again at the next period.
Why: Article 10 puts a behaviour in the flow unless the flow cannot say it; a flow is a work's, and an act of the installation that belongs to no work has none to be in. What Article 7 aims at, that every step can be seen, is kept by a row for each failure and the line on the page. The wait for the network (decision the-wait-holds-what-its-cause-stops) and the owner's pause of everything (decision pause-of-everything-is-the-installations) are worked out from the works' records and hold what a work does; a backup is no work's, and a copy taken while everything is paused or the network is down is the one that is wanted.

### decision backup-records-are-written-beside-the-engines-turn
The backup's records are written by the backup itself as it goes, not through the engine's one turn of changes to the works' records.
Why: The owner's pause of everything is an installation's record and goes through that turn because a decision reads it; nothing decides by these records, and they are only added to, so a second writer cannot make a work's records diverge (principle state-from-records). Writing each row at once after the act it records, without waiting for the engine's turn, keeps a stop between the act and its row as short as it can be (a judgment, not measured).
