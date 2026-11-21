# Scheduling

What lets works go on and what holds them back: each work's own pause, the pause of every work, and the slots their agents' rounds take.

### decision a-work-is-added-paused
An item and its one work are added in one transaction, with the work paused, and the work starts nothing until the owner lets it go on.
- replaced: A work that started by itself once its item was added, within the free slots, was on main; the owner changed it, because starting by itself is a defect: starting and pausing stay manual, so that an urgent work can be put first.
- rejected: an agent proposed starting an item as it is added when whoever adds it says so; the owner rejected it, because a work that starts by itself is a defect.

### decision slots-in-the-scheduler
The engine starts an agent's round only when a slot is free, and otherwise the work's walk says that the work waits for a slot; what carries out the rounds sets no limit of its own.
- replaced: Each step of a work counting the free slots by itself was on main; the owner changed it, because two steps that count at the same time see the same free slot, and either use more than the slots or hold each other back.

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
