### decision snapshot-is-a-release-asset
The database's copy is compressed and uploaded as an asset of a release of a private repository. Only assets named as the engine names its copies are ever deleted there.
Why: The database holds everything and every conversation, so the place must be private. Committing it to git keeps each copy for ever and is refused over 100 MB, Git LFS has limits on volume and traffic, and a release asset is large and can be deleted later. Others may write to the release, so what is not ours is left alone.
Not: Git commits of the file, Git LFS, a repository made only for the copies.

### decision copy-is-vacuum-into
A copy is made by SQLite's `VACUUM INTO` on a read-only connection of its own, never by copying the file.
Why: The store runs in WAL mode while the engine writes, so a copied file can lack what is still in the log or be torn; `VACUUM INTO` reads one snapshot and does not hold up the loop's writes.

### decision taken-and-uploaded-are-two-records
That a copy was taken and that it was uploaded are two records, and an upload is never started inside the step that takes the copy but by the check that finds a copy not uploaded, after a look at what the place holds. The copies kept are the newest by the order they were recorded in, not by the time in a name or a row; a copy past keeping is taken from the place first and its file last, because the file is what says its removal is not done.
Why: The engine may stop at any step (principle effects-known-as-done). The old engine started the upload once, on a thread after the copy, so a stop between the two lost that day's copy off the machine, and a failed upload waited for the next day's. A look first means a stop between the upload and its record does not upload it twice. A clock that jumped ahead and came back gives a copy a time later than every copy taken since, so ordering by time would keep it and remove the newest.
Not: Uploading in the step that copies; keeping the newest by time.

### decision backup-destination-is-settings-and-private
Where the copies go is the installation's `[backup]` settings with its own token, not the data directory's git remote nor the project's repository, and the repository is checked to be private at every upload.
Why: The project's token is limited to the project's repository, which may be public, and a remote read from git would send the database wherever it happens to point. A repository can be made public after it was set, so a check at configuration is not enough. With no `[backup]` the copies stay on the machine, which shares the disk they guard against losing, and the page says so (principle nothing-passes-for-normal).

### decision backup-is-the-engine-s-own-tick
The backup and the retry of a failed upload are a periodic act of the engine, one task that does one step at a time, not a cell of a flow.
Why: Article 7 asks that every step be a cell or a row and names a retry out of the flow's sight as a violation; but the exception is for acts that belong to no work, which have no flow to be in, and this one is in sight: a failure is a row in the store, the page shows the state in one line, and the period is fixed. Its checking is driven by the loop because the loop alone reads the clock (principle outside-behind-interfaces).

### decision backup-records-are-written-beside-the-loop
The backup's records are written by the backup's own task on a connection of its own, not through the loop.
Why: They belong to no work, no decision reads them, and their tables are only inserted into, so a second writer cannot make a work's records diverge (principle state-from-records); routing them through the loop would hold it up for a copy and an upload.
