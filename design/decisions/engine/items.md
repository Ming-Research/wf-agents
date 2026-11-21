# Items

What an item is, how items relate to one another, and what of an item changes after it was added.

### decision items-relate-only-by-dependency
Items relate to one another only by what each depends on, a graph with no circle: a change that would close one is refused, naming its way.
- rejected: an agent proposed two kinds of relation, a parent owning its children and one item waiting on another; the owner rejected it, because an item owns nothing: it only depends on others, and their relations are a directed graph with no circle.

### decision an-items-kind-is-its-flow
The engine has no kinds of item: an item is added on a flow, and one that only delivers what the items it depends on make runs `deliver`, whose one cell waits for them to merge.
- rejected: an agent proposed keeping the engine without kinds and telling a deliverable item on the page alone, by a rule guessing it from the open items under it; the owner rejected it, because each kind of item has its own flow, its state like any other item's.

### decision dependencies-until-the-work-starts
What an item depends on changes only while its work has started no run; its title, level and tags until the work ends, after which only a cancelled item's reopening is taken.
- replaced: An item's scope, what it depends on among it, changed from outside while its work ran was on main; the owner changed it, because scope changed midway was the first cause of the works' redesigns, 8 of 24.
- replaced: No way to take a dependency away was on main; the owner changed it, because a dependency on items built on an old branch held a finished work back from its merge.
- replaced: A done item taking on a dependency on one not done was on main; the owner changed it, because a dependency says what must be done first, so a done item over an open one contradicts itself.
- rejected: an agent proposed checking the dependencies again before the merge and going back on one added late; the owner rejected it, because going back from the merge for a new dependency is complex and errs easily.

### decision a-title-is-counted-in-words
An item's title has at most `items.title_max` words, a Chinese character counting as one, and one longer is refused as the item is added or its title changed, saying how many it has.
- replaced: No bound on an item's title was on main; the owner changed it, because the titles had grown too long, where the old system bounded them.
