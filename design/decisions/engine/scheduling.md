# Scheduling

What lets works go on and what holds them back: each work's own pause, the pause of every work, the slots their agents' rounds take, and the turn of a project to check and merge.

### decision a-work-is-added-paused
An item and its one work are added in one transaction, with the work paused, and the work starts nothing until the owner lets it go on.
- replaced: A work that started by itself once its item was added, within the free slots, was on main; the owner changed it, because starting by itself is a defect: starting and pausing stay manual, so that an urgent work can be put first.
- rejected: an agent proposed starting an item as it is added when whoever adds it says so; the owner rejected it, because a work that starts by itself is a defect.

### decision slots-in-the-scheduler
The engine starts an agent's round only when a slot is free, and otherwise the work's walk says that the work waits for a slot; what carries out the rounds sets no limit of its own.
- replaced: Each step of a work counting the free slots by itself was on main; the owner changed it, because two steps that count at the same time see the same free slot, and either use more than the slots or hold each other back.

### decision the-slots-change-at-once
The slots set on the Settings view take effect at once and are written to the file: added ones go to the works that wait; with fewer, no round is stopped, and none begins until fewer are under way.
- replaced: Slots read from the installation's file as the engine started, and kept until it started again, was on main; the owner changed it, because a change of the slots must never need a restart.

### decision begun-works-first
A slot that comes free goes to a work that has started an agent's round before one that has not, each in the order of its number; a work that goes straight on keeps its slot.
- replaced: Giving a free slot by the works' numbers alone, whether they had begun or not, was on main; the owner changed it, because works already under way waited twenty or thirty minutes for a slot while new ones started.

### decision pause-of-everything-is-the-installations
The owner's pause of every work is one record of the installation, separate from each work's own pause and from what a work waits for; lifting it lets no work go on that its own pause holds.
- replaced: A hidden Run or Hold switch in the engine, which a restart set back to Run, was on main; the owner changed it, because he wants one control that pauses every work or starts them all, so that they do not all set off at once.
- replaced: Writing a wait of its own on every held work, and ending it when the pause was lifted, was on main; the owner changed it, because the pause of every work and a work's own waits are two different things, and one pause was counted as five things waiting.

### decision a-start-pauses-everything-only-over-works-going-on
If the engine starts while a work is going on (not ended, and not held by its own pause), it pauses everything itself until the owner lifts the pause; if none is going on, it pauses nothing.
- replaced: An engine that, when it started again, ran whatever had been let go on before it stopped was on main; the owner changed it, because the works are not to set off again at once: he starts them himself once the engine is up.
- rejected: an agent proposed keeping, across a stop, the state that every work was in and the state of everything as a whole; the owner rejected it, because an engine that starts is to stand still until he starts the works.

### decision a-pause-says-why-and-what-it-waits-for
The owner's pause of a work may say why and the items it waits for to merge or be cancelled; it stays the one pause, which only the owner lets on, and the owner's page does not change.
- rejected: an agent proposed a state of its own beside the pause, with its own command to let it on; the owner rejected it, because it is a new kind of thing: the page, the lists, `wait` and the command to let on would each need one more state.
- rejected: an agent proposed that a pause carry only a text of why and no items; the owner rejected it, because the engine could not see that what was waited for had come, and the architect would still read the text to find out.

### decision one-work-checks-and-merges-at-a-time
The works of a project take turns at the cells that say they take it: a work holds the turn from its round of one while its walk stays at them, and the next is the work that came to want it first.
- replaced: A turn kept in the acts' memory, in the order the checks began to wait, let go fifteen minutes after a check that passed, was on main; the owner changed it, because its wait was drawn as the check running, was lost when the engine restarted, and was out of the flow's sight.
- rejected: an agent proposed two records beside that memory, one as a check began to wait and one as its turn came; the owner rejected it, because the wait would stay outside the flow and be lost at a restart, the records saying again what the memory holds.
- rejected: an agent proposed bringing each branch that is behind main up to date with it and checking it again, with the works side by side; the owner rejected it, because one pull request was brought up to date with main four times in an hour while main broke twice.

### decision the-slots-set-are-kept-with-their-time
Each number the slots are set to, at the engine's start and on the Settings view, is kept with its time, to set the slots held against the slots there were; no wait is read from it.
- replaced: The slots known only from the installation's file and the engine's memory, so that a change left no trace, was on main; the owner changed it, because the past held no record of how many slots there were against how many were taken.
