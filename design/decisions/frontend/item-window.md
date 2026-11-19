# The item's window

What the console, and a work's page for the work's item, open for an item: its tabs, what its flow's card names, the
questions in it and the owner's words to each cell.

### decision one-window-on-the-console-and-a-works-page
The window has the same four tabs on the console and on a work's page, Item, Work, Step and Usage, each page keeping
its own choice of tab; Step shows the row of the work's history the owner chose, where the page draws the history and
one is chosen, and else the run the work stands at: the run whose round is under way, or else the latest run of the
cell it stands at.
Why: The console holds the items and the works going on under them, and is where the owner works (principle
the-console-is-where-the-owner-works); Step and Usage on a work's page alone left the owner no way to see from the
console what a work does now and what it used, and one window drawn from one set of parts shows each thing one way
(Article 8).

### decision the-card-is-the-flows
What the item's card shows the owner is named by the work's flow in its own `flow.toml` (`card`), data of the flow in
their order; the page draws a section of each by its name, and names no datum, no cell and no flow of its own: a
cell is called by its title in the flow, and an item is a deliverable where its flow has no agent's cell, as the
work's route says.
Why: The page told one flow by its name, read three blocks by theirs and called three cells by words of its own, which
held for those flows alone (principle engine-knows-no-cell). A key on each block's file saying it is shown was turned
down: the order is the flow's, and is said once there. A key saying a flow delivers was turned down too: a flow with
no agent's cell does no work of its own and delivers what its item is made of, which its cells already say, and a key
beside them could only repeat them or contradict them.

### decision a-question-stands-where-a-text-points-to-it
A question of the work is drawn at the first pointer to it, `{{name}}` as the words of a cell's questions tell its
agent, in the passages of the card's sections and then in the messages an agent wrote, in the order the window draws
them; a later pointer to it is a link to it, a pointer in code is text, and a question no text points to stands after
the body (principle asked-where-it-arises).
Why: The form is said in the agents' words and read by the page, and neither shows the other: changed in one alone,
every question falls back to the body's end and every pointer shows as its raw marks. A name is the work's, the engine
reading each question by its name, so a pointer means one question whoever wrote the text: reading it as the writer's
own was turned down, as a plan the cell that looks for a better way wrote pointed to the initial research's question
(work 10).

### decision a-diff-is-drawn-as-the-text-it-changes
A ```diff block in the window is drawn as the text it changes reads after the change, each change in its place: the
words removed, struck through in red on a red tint, then the words added, on a green tint, so that the colour of the
words is left to their kind (decision code-is-coloured-from-prisms-words). The two sides' words are compared, their
blanks only where nothing else differs, and two changes fewer than four words apart are drawn as one. The diff's signs,
its numbers and what git keeps of a file are not drawn, a file's path only where the block changes several, and a
block that is no unified diff is drawn as code.
Why: Drawn as written, the diff's long lines folded away from their signs, and shown word by word ("the ~~cell~~
**flow**") changes close together could not be read either (work 11). Compared with their blanks, a paragraph wrapped
again would read as rewritten whole. How far apart two changes stay apart rests on judgment.

### decision an-option-is-picked-by-pressing-it
An option of a question that waits for its answer is a button: pressing it picks it, and pressing the one picked lets
it go. It carries its radio mark inside it, at its left. Once the question is answered its options are words alone,
with no frame and no radio mark, the one taken marked by a tick.
Why: A row of short labels under the options, each to be matched to its option, was turned down as easy to press
wrongly, and radio buttons under the options as saying each option twice. Drawn as the buttons they were, with the one
taken still framed in the accent, the options of an answered question read as a question still open (work 11).

### decision every-stop-is-one-card
Every stop of a work is one card, in the one look of a stop: the engine's one sentence of why, a stall's among them,
its marked names and values in monospace and the rest plain text, and inside it the buttons that end it, with nothing
of the page's own; a question's card is the question, whichever cell waits for it. A stop that holds every work is no
card of a work's own: it is said once and ended where it is set, and a work it holds says in what it does now that it
is held and where it stands.
Why: One pause of every work gave each work it held a card of its own, so five works held by it read as five things
waiting and each carried the button that lifts it over all of them, pressed from inside one work; the pause is one
thing wherever it holds, as a cause outside every work is one wait (decision a-shared-cause-is-one-wait). Drawn as a
card and again as a line with "Try again" beside it, the engine's pause of a work whose letting go failed read as two
stops; a line of the page's own over each card ("The work waits for you", "Still to confirm") was turned down as
saying again what the card's look says, and a line under a question saying the merge waits for it likewise: the red
card and the line's cell say it.

### decision only-a-line-that-takes-words-is-picked
Of the lines over the message box, one on which the engine has no cell to give the owner's words to is grey, cannot be
picked and says why on pointing at it; of the others, the line the work stands at is picked first, else the one it
reached last. Where none takes the words, none is picked, no box is drawn, and why is said once, under the lines.
Why: Offered and picked though no cell of it could take the words, a line of a work that had not run was picked first,
and the box under it then said the words would go nowhere (W11); the engine decides which cell a line's words
reach (design document `data-and-context`, 35), and the page only shows it.

### decision the-pause-switch-is-the-owners-own
The switch that pauses a work is named the owner's own pause and shows only that; the engine's pause is lifted from
its card alone, by Resume.
Why: Pressed for the engine's pause, the switch said the owner had paused a work they had not, and its Start beside
the card was a second way to end one stop.

### decision the-owners-words-stand-until-a-read-brings-them
A message the owner sends from the window is drawn under the ones the body was read with, marked as not taken yet until
a cell has taken it, and is dropped only by a read of the work begun after it was sent, which holds it.
Why: The message is kept beside the body, not written into it, because a read of the work already under way as the
owner pressed Send is served with a body the engine wrote the words after, and setting that body threw away what the
owner had just said while the box had already been cleared: the words went out of view altogether and the discussion
read as if nothing was said (principles owner-keeps-their-place and nothing-passes-for-normal). Dropping each by the
read begun after it, rather than by matching the words of the body, keeps the engine's record the one that says a
message stands and compares only times the browser itself took; a read begun before the send but served after the
engine wrote the words shows the message twice until the next read, taken over losing it.
