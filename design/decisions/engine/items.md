# Items

What an item is, how items relate to one another, and what of an item can change after it is added.

### decision items-relate-only-by-dependency
Items relate to one another only by what each depends on, a graph with no cycle: a change that would close a cycle is refused, naming its path.
- rejected: an agent proposed two kinds of relation, a parent owning its children and one item waiting on another; the owner rejected it, because an item owns nothing: it only depends on others, and their relations are a directed graph with no cycle.

### decision an-items-kind-is-its-flow
The engine has no kinds of item: an item is added on a flow, and one that only delivers what the items it depends on make runs on `deliver`, whose one cell waits for them to merge or be cancelled.
- rejected: an agent proposed keeping the engine without kinds and telling a deliverable item apart on the page alone, by a rule that guesses it from the open items under it; the owner rejected it, because each kind of item has its own flow, with its state like any other item's.

### decision dependencies-until-the-work-starts
What an item depends on can change only until its work starts a run, what it continues until its branch is made; its title, level and tags until the work ends, then only a reopen is accepted.
- replaced: Changing an item's scope, including what it depends on, from outside while its work ran was on main; the owner changed it, because scope changed midway was the main cause of the works' redesigns, 8 of 24.
- replaced: No way to remove a dependency was on main; the owner changed it, because a dependency on items built on an old branch held a finished work back from its merge.
- replaced: A done item taking on a dependency on one not done was on main; the owner changed it, because a dependency says what must be done first, so a done item depending on an open one contradicts itself.
- rejected: an agent proposed checking the dependencies again before the merge and going back for one added late; the owner rejected it, because going back from the merge for a new dependency is complex and error-prone.

### decision a-cancelled-item-keeps-its-place
A cancel takes away no dependency on its item, and a dependency on a cancelled item holds nothing, as one on a merged item: it may be added, and holds again once the item is reopened.
- replaced: Refusing to cancel an item while an open one depended on it, and to have an open one depend on a cancelled item, was on main; the owner changed it, because a cancelled item, depended on by nothing, stood as a root, while it belongs where it stood, and more cancels will come.
- rejected: an agent proposed that a deliver item all of whose parts were cancelled not deliver, but wait for the architect to cancel it; the owner rejected it, because a dependency on a cancelled item is the same as one on a merged item, and waits for nothing.

### decision a-cancelled-item-is-not-reopened-under-a-started-work
A cancelled item is not reopened while an item that depends on it has a work that started and has not ended; an item that depends on a cancelled one may be reopened.
- replaced: Reopening a cancelled item whatever depended on it, and refusing to reopen an item that depended on a cancelled one, was on main; the owner changed it, because a work started past a cancelled item would stop midway, or merge while it is open, and a cancelled item no longer holds anyone.

### decision a-title-is-counted-in-words
An item's title has at most `items.title_max` words, each Chinese character counting as one; a longer one is refused when the item is added or its title changed, saying how many it has.
- replaced: No limit on an item's title was on main; the owner changed it, because the titles had grown too long, while the old system had limited them.
