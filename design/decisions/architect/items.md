# The architect's items

How the architect arranges the items: the tree they make, their dependencies, their size and joining, what becomes an item, how one is written and which starts first.

### decision a-parent-delivers-one-thing
From the top down items are related by dependency, never by class: a parent waits for its children and delivers one thing, and more than ten side by side show a wrong arrangement.
- rejected: an agent proposed parents that name a class of work, as a part of the code; the owner rejected it, because a parent waits for its children, so it must deliver one thing of its own.
- replaced: More than ten parallel dependencies of an item gathered under intermediate items was on main; the owner changed it, because what matters is items arranged sensibly in layers of deliver items, not crammed, and more than ten only shows that something is off.

### decision a-dependency-is-a-use-of-its-result
An item depends on another only where it uses that one's result; two works touching the same code is no dependency, only a reason not to start them side by side.
- rejected: an agent proposed that of two works changing the same code, the one waited on less depend on the other; the owner rejected it, because whether one depends on another is a matter of logic, not of code.

### decision a-work-is-worth-a-run-of-the-flow
A work is at least worth a run of the flow, about ten rounds and half an hour, and at most what one engineer finishes in a day; a smaller change is joined into an item that carries it.
- rejected: an agent proposed works each of one engineer, one branch, one CI run and one merge of at most a day, which on a trial made 141 items; the owner rejected it, because every work pays for the whole flow.
- replaced: A work bounded from above, with only a change the size of a rewording kept from running alone, was on main; the owner changed it, because the flow as heavy as it is now is for one large thing at a time, and a small one through three independent reviews cannot be quick.
- rejected: an agent proposed conditions in the flow so that two of its reviews run only where they are needed; the owner rejected it, because those conditions cannot be written, and a lighter flow for small works was turned down before.
- rejected: an agent proposed letting a change too small for a work be made by hand outside the flow; the owner rejected it, because made by hand it would go round the flow the project develops itself by.

### decision only-related-items-are-joined
Items not begun are joined so that none too small wastes a run, but only related things of one kind, related by what they deliver and never by the code they touch.
- replaced: Joining items only where they are nearly one thing, follow one another or change one place, never to save a run, was on main; the owner changed it, because a run of the flow is not light, so joining is mainly to keep too small an item from wasting one.
- rejected: an agent proposed joining unrelated items where each is small and in one repository; the owner rejected it, because joining must never put unrelated things together, only related things of one kind.
- rejected: an agent proposed counting changes of the same code as one work and as related; the owner rejected it, because that contradicted the rule that a dependency is a matter of logic and not of code.

### decision every-merge-is-an-item
Every piece of work that ends in a merge is an item and waits nowhere else, and work seen outside the items, such as an open pull request, is given an item that continues it.
- rejected: an agent proposed saying that the pull requests the owner and the assistant work by hand are no work outside the items; the owner rejected it, because that will not keep happening, and the architect can say so when it meets it.

### decision a-step-that-merges-nothing-is-no-item
A step that makes no merge is no item but the owner's, said in the item it follows, and what becomes neither an item nor an observation is written down, never just deleted.
- rejected: an agent proposed making such steps items sent to an engineer; the owner rejected it, because they have nothing to merge, the system has no way to know they are done, and they are the owner's to do.

### decision an-observation-becomes-an-item-where-a-delivery-needs-it
An observation no item can take in becomes an item, under the root it serves, where what is being delivered now needs it; otherwise it stays an observation.
- replaced: Making an item of an observation only where no item could take it in and it was urgent was on main; the owner changed it, because urgent has no measure, while bounding the items by what is being delivered keeps them few and leaves nothing pressing among the observations.

### decision correctness-problems-are-always-items
A correctness problem, and anything that fails now and then, is always an item, whether anyone meets it now or not.
- rejected: an agent proposed keeping a test that goes red now and then as an observation; the owner rejected it, because by the project's principles nothing should fail now and then, and a correctness problem is a large one.
- rejected: an agent proposed counting as a correctness problem only what fails today, a correct input refused being an observation; the owner rejected it, because that line belongs to the compiler's project.

### decision work-that-waits-on-a-result-is-written-in-its-parent
Work that can be judged needed only once a result is in is no item: it is written in its parent item.
- rejected: an agent proposed writing it into the item that gives the result, whose work would report when it is needed; the owner rejected it, because nothing makes a work report it.
- rejected: an agent proposed keeping it in the architect's notes, looked at when that item merges; the owner rejected it, because there is no such mechanism and the merge is the system's own, so the parent item is plainly the place.

### decision a-title-says-the-work-to-be-done
An item's title says the work to be done, never a state that reads as done.
- rejected: an agent proposed a title that states the result wanted, as that more than half the architect's calls are technical; the owner rejected it, because a title is an action, not a result.

### decision a-background-opens-with-three-paragraphs
An item's background opens with three short paragraphs: what it is to do, why with one case and of a cause only the conclusion, and the result wanted; never how to do it.
- replaced: One first paragraph saying what the item is, why it is to be done, what brought it about and the result wanted was on main; the owner changed it, because the three mixed in one paragraph read badly.

### decision which-item-starts-first
Among the items that can start, one that makes a red main green goes first, then what the owner said comes first, then the one most items not held wait for; there is no policy.
- rejected: an agent proposed that the policy and the dependencies decide which item that can start goes first, as on the board; the owner rejected it, because the new system has no policy.

### decision nothing-is-held-without-a-reason
An item not to start yet is held saying what it waits for, an item merged or the owner's word, and none is held without such a reason.
- rejected: an agent proposed ordering the items by the holds they had, which said the owner wanted them after a fix list; the owner rejected it, because he asked for no hold, and the architect is to hold nothing without a reason, judging by the project's needs alone.
