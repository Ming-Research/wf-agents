### decision snapshot-is-a-release-asset
The database's copy is compressed and uploaded as an asset of a release of a private repository. Only assets named as the engine names its copies are ever deleted there, since others may write to the release.
Why: An asset of a release can be large and can be deleted later. Committing the copy to git was turned down, since git keeps every copy for ever and refuses a file over 100 MB, and Git LFS for its limits on volume and traffic.

### decision backup-destination-is-settings-and-private
Where the copies go is the installation's `[backup]` settings with a token of its own, and the repository is checked to be private at every upload, since one can be made public after it was set.
Why: The database holds every conversation. The project's repository was turned down because it may be public, and the data directory's git remote because the database would go wherever that happens to point.

### decision backup-is-the-engines-own-tick
The backup is a periodic act of the installation, out of the flow, and neither the wait for the network nor the owner's pause of everything holds it.
Why: A flow is a work's and the backup belongs to no work, so the flow cannot say it (Article 7); each of its failures is a row and is shown on the page. The wait and the pause hold what works do (decisions the-wait-holds-what-its-cause-stops and pause-of-everything-is-the-installations), and a copy taken while everything is paused is the one that is wanted.

### decision backup-records-are-written-beside-the-engines-turn
The backup's records are written by the backup itself as it goes, not through the engine's one turn of changes to the works' records.
Why: Nothing decides by these records and they are only added to, so a second writer cannot make a work's records diverge (principle state-from-records). A row written at once after the act it records leaves the shortest stop between the two, which is a judgment and was not measured.
