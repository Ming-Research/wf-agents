# The architect's items

How the architect arranges the items: the tree they make, their dependencies, their size and how they are joined, what becomes an item, how one is written and which starts first.

### decision a-parent-delivers-one-thing
From the top down, items are related by dependency, never by class: a parent waits for its children and delivers one thing, and more than ten items side by side show a wrong arrangement.
- rejected: an agent proposed parents that name a class of work, such as a part of the code; the owner rejected it, because a parent waits for its children, so it must deliver one thing of its own.
- replaced: Gathering more than ten parallel dependencies of an item under intermediate items was on main; the owner changed it, because what matters is that items are arranged sensibly in layers of deliver items, not crammed together, and more than ten only shows that something is wrong.

### decision a-dependency-is-a-use-of-its-result
An item depends on another only when it uses the other's result; two works touching the same code are not a dependency, only a reason not to start them side by side.
- rejected: an agent proposed that, of two works changing the same code, the one that fewer items wait for depend on the other; the owner rejected it, because whether one depends on another is a matter of logic, not of code.

### decision a-work-is-worth-a-run-of-the-flow
A work is big enough to be worth a run of the flow, about ten rounds and half an hour, and at most what one engineer finishes in a day; a smaller change is joined into an item that carries it.
- rejected: an agent proposed works of one engineer, one branch, one CI run and one merge each, taking at most a day, which made 141 items on a trial; the owner rejected it, because every work pays for the whole flow.
- replaced: A work limited in size only from above, except that a change as small as a rewording could not run alone, was on main; the owner changed it, because the flow, as heavy as it is now, is for one large thing at a time, and a small change cannot go through three independent reviews quickly.
- rejected: an agent proposed conditions in the flow so that two of its reviews run only when they are needed; the owner rejected it, because those conditions cannot be written, and a lighter flow for small works had been turned down before.
- rejected: an agent proposed letting a change too small for a work be made by hand outside the flow; the owner rejected it, because a change made by hand would bypass the flow by which the project develops itself.

### decision only-related-items-are-joined
Items not yet begun are joined so that no item too small wastes a run, but only related things of one kind, related by what they deliver and never by the code they touch.
- replaced: Joining items only when they are nearly one thing, follow one another or change one place, and never to save a run, was on main; the owner changed it, because a run of the flow is not light, so joining is mainly to keep an item that is too small from wasting a run.
- rejected: an agent proposed joining unrelated items when each is small and in one repository; the owner rejected it, because joining must never put unrelated things together, only related things of one kind.
- rejected: an agent proposed counting changes of the same code as one work and as related; the owner rejected it, because that contradicted the rule that a dependency is a matter of logic and not of code.

### decision every-merge-is-an-item
Every piece of work that ends in a merge is an item and waits nowhere else, and work seen outside the items, such as an open pull request, is given an item that continues it.
- rejected: an agent proposed saying that pull requests the owner and the assistant work on by hand do not count as work outside the items; the owner rejected it, because that will not keep happening, and the architect can say so when it comes across one.

### decision a-step-that-merges-nothing-is-no-item
A step that makes no merge is not an item but the owner's to do, written in the item it follows, and what becomes neither an item nor an observation is written down, never just deleted.
- rejected: an agent proposed making such steps into items sent to an engineer; the owner rejected it, because they have nothing to merge, the system has no way to know they are done, and they are the owner's to do.

### decision an-observation-becomes-an-item-where-a-delivery-needs-it
An observation that no item can take in becomes an item, under the root it serves, when what is being delivered now needs it; otherwise it stays an observation.
- replaced: Making an observation into an item only when no item could take it in and it was urgent was on main; the owner changed it, because nothing measures urgency, while limiting the items to what is being delivered keeps them few and leaves nothing pressing among the observations.

### decision correctness-problems-are-always-items
A correctness problem, and anything that fails now and then, is always an item, whether anyone meets it now or not.
- rejected: an agent proposed keeping a test that goes red now and then as an observation; the owner rejected it, because by the project's principles nothing should fail now and then, and a correctness problem is a big problem.
- rejected: an agent proposed counting as a correctness problem only what fails today, so that a correct input being refused would be an observation; the owner rejected it, because that line belongs to the compiler's project.

### decision work-that-waits-on-a-result-is-written-in-its-parent
Work whose need can be judged only once a result is in is not an item: it is written in its parent item.
- rejected: an agent proposed writing it into the item that gives the result, so that its work would report when it is needed; the owner rejected it, because nothing makes a work report that.
- rejected: an agent proposed keeping it in the architect's notes, to be looked at when that item merges; the owner rejected it, because there is no such mechanism and the system does the merge itself, so the parent item is clearly the place.

### decision a-title-says-the-work-to-be-done
An item's title says the work to be done, never a state that reads as done.
- rejected: an agent proposed a title that states the result wanted, such as that more than half of the architect's calls are technical; the owner rejected it, because a title is an action, not a result.

### decision a-background-tells-the-story
An item's background tells, for a reader new to the problem: the part of the system and what it does, what happened with one case, why it is a problem, and what is to be done; never how.
- replaced: A background of three short paragraphs, opening with what the item is to do in a sentence, was on main; the owner changed it, because the backgrounds began with no subject and named things they never said what they were.
- replaced: A single first paragraph saying what the item is, why it is to be done, what brought it about and the result wanted was on main; the owner changed it, because the three mixed into one paragraph read badly.

### decision which-item-starts-first
Among the items that can start, one that makes a red main green goes first, then what the owner said comes first, then the one that most items not on hold wait for; there is no policy.
- rejected: an agent proposed that the policy and the dependencies decide which item that can start goes first, as on the board; the owner rejected it, because the new system has no policy.

### decision nothing-is-held-without-a-reason
An item that is not to start yet is held with what it waits for, an item's merge or the owner's word, and no item is held without such a reason.
- rejected: an agent proposed ordering the items by the holds they had, which said the owner wanted them after a fix list; the owner rejected it, because he had asked for no hold, and the architect must hold nothing without a reason, judging only by the project's needs.
