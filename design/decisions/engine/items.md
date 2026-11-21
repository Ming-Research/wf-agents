# Items

What an item is, how items relate to one another, and what of an item can change after it is added.

### decision items-relate-only-by-dependency
Items relate to one another only by what each depends on, a graph with no cycle: a change that would close a cycle is refused, naming its path.
- rejected: an agent proposed two kinds of relation, a parent owning its children and one item waiting on another; the owner rejected it, because an item owns nothing: it only depends on others, and their relations are a directed graph with no cycle.

### decision an-items-kind-is-its-flow
The engine has no kinds of item: an item is added on a flow, and one that only delivers what the items it depends on make runs on `deliver`, whose one cell waits for them to merge.
- rejected: an agent proposed keeping the engine without kinds and telling a deliverable item apart on the page alone, by a rule that guesses it from the open items under it; the owner rejected it, because each kind of item has its own flow, with its state like any other item's.

### decision dependencies-until-the-work-starts
What an item depends on can change only until its work starts a run; its title, level and tags until the work ends, and after that only the reopening of a cancelled item is accepted.
- replaced: Changing an item's scope, including what it depends on, from outside while its work ran was on main; the owner changed it, because scope changed midway was the main cause of the works' redesigns, 8 of 24.
- replaced: No way to remove a dependency was on main; the owner changed it, because a dependency on items built on an old branch held a finished work back from its merge.
- replaced: A done item taking on a dependency on one not done was on main; the owner changed it, because a dependency says what must be done first, so a done item depending on an open one contradicts itself.
- rejected: an agent proposed checking the dependencies again before the merge and going back for one added late; the owner rejected it, because going back from the merge for a new dependency is complex and error-prone.

### decision a-title-is-counted-in-words
An item's title has at most `items.title_max` words, each Chinese character counting as one; a longer one is refused when the item is added or its title changed, saying how many it has.
- replaced: No limit on an item's title was on main; the owner changed it, because the titles had grown too long, while the old system had limited them.
