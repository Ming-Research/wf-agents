- A snapshot is compressed and uploaded as a Release attachment to the data directory's own private repository, with
  only the latest N kept locally and remotely. Git does not suit large binaries, and Release attachments allow large
  files that can later be removed. The database holds everything and all talk and must be private, as that repository
  already is. Committing to git would keep each snapshot for ever and is refused over 100 MB, Git LFS has limits on
  volume and traffic, and another private repository would be needless.
