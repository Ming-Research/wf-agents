# Backup

How the engine keeps copies of its records off the machine.

### decision snapshot-is-a-release-asset
The copy of the database is compressed and uploaded as an asset of a release in a private repository.
- rejected: an agent proposed committing each copy to git; the owner rejected it, because git keeps every copy forever and refuses a file over 100 MB.
- rejected: an agent proposed Git LFS; the owner rejected it, because of its limits on volume and traffic.
