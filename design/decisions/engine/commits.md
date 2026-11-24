# The commits of a work

What the commits of a work say: whose they are, and which item and work they name.

### decision every-commit-names-its-item-and-work
Every commit of a work, and the one that merges it into main, ends with `Item: <n>` and `Work: W<n>` lines, added by a hook on the agents' git; find them with `git log --grep='^Item: 36$'`.
- rejected: an agent proposed writing the item's number with a # (`Item: #<n>`), following principle numbers-say-what-they-number to the letter; the owner rejected it, because GitHub links a # in a commit message to an unrelated pull request.
