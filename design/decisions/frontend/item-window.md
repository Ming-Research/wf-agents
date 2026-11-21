# The item's window

What the console, and a work's page for the work's item, open for an item: its tabs, its lines and stops, the questions in its body, the owner's words to each line, and what its Work, Step and Usage tabs show.

### decision one-window-on-the-console-and-a-works-page
The window has four tabs, Item, Work, Step and Usage, alike on the console and on a work's page; Step shows the row of the history chosen, else the run under way, else the work's latest run.
- replaced: Step and Usage on a work's page alone stood on main; the owner changed it, because the console is the item, which stays, with its work moving under it, so what the work's page shows of the work the console should show too.
- replaced: A Step left empty on the console while no round ran stood on main; the owner changed it, because the Step should show the round running, or when none runs the last one's result, and never stand empty.

### decision the-windows-tabs-look-as-the-bars
The window's tabs are drawn as the bar's views are: each a muted word, the one shown in the text's colour over a line in the accent, the arrow keys moving from one to the next.
- replaced: A segmented switch, as the one of Auto, Light and Dark, stood on main; the owner changed it, because it did not look like tabs: a switch picks among options, where tabs change what the window shows.

### decision the-window-writes-the-level-by-the-item
The window writes the level on the item's line after its standing, and the id of the cell the work stands at before the strip of the work's cells, at that line's right.
- replaced: The level before the strip, in the place of the cell's id, the cell left to the strip's mark, stood on main; the owner changed it, because it is the item's level, which stands by the item's own facts at the left.
- replaced: The cell the work stands at named on the console's card alone stood on main; the owner changed it, because the window should name it as the card does.

### decision the-pause-switch-is-the-owners-own
Starting and pausing a work are one switch, named the owner's own pause and showing only that pause; the engine's pause of a work is lifted from its card alone, by Resume.
- replaced: A button named Let it on stood on main; the owner changed it, because the name was strange.
- rejected: an agent proposed Start, Pause and Cancel side by side, one greyed while another applies; the owner rejected it, because starting and pausing exclude each other, and so are one switch.
- rejected: an agent proposed the switch lifting the engine's pause too, beside the Resume of that pause's card; the owner rejected it, because it was a second way to end one stop.

### decision every-stop-is-one-card
Every stop of a work is one card under its state line, framed in red on the red tint, with the engine's one sentence of why and the buttons that end it, in red; a pause of all works is no work's card.
- rejected: an agent proposed a card of its own for each kind of stop, a purple frame with red words, each kind named in words of its own; the owner rejected it, because the purple card with red words was ugly and one thing was called by many names.
- replaced: A stop's reason said under the state line and again in its card, the engine's sentence followed by the program's message saying the same, stood on main; the owner changed it, because one thing was written over and over, outside the card and in it.
- replaced: A red card with Resume all on each work the engine's pause of every work held stood on main; the owner changed it, because the page's head already shows that pause with Start all, which does the same, and five held works showed five cards.
- replaced: A blue button in every card stood on main; the owner changed it, because the button of a red card belongs in its red.

### decision a-question-that-holds-the-work-is-red
A question that holds the work is drawn as a stop is, framed in red on the red tint; one that does not, framed in the accent on its tint; neither says in words that the work waits.
- replaced: A yellow question box stood on main; the owner changed it, because it did not stand out.
- replaced: Every question in one purple card under the words The work waits for this answer stood on main; the owner changed it, because those words are needless, and a question that holds the work is red.

### decision a-question-stands-where-a-text-points-to-it
A question is drawn at the first pointer to it, `{{name}}`, in the body's passages and then in an agent's messages; a later pointer is a link to it, and one no text points to stands after the body.
- replaced: Every question drawn at the end of the card's last section stood on main; the owner changed it, because a question is shown where it is asked, in the text it is about.
- rejected: an agent proposed gathering what waits for the owner at the top of the window, to handle there; the owner rejected it, because a question belongs with the text it is about, and at the top he must read the body below and come back up to answer.
- rejected: an agent proposed a field of a question's form naming the datum and quoting the sentence it arises in; the owner rejected it, because a text already points to a question by `{{name}}`, as the agents are told to write it.
- rejected: an agent proposed sending back a return whose question no text points to; the owner rejected it, because a question no text points to is simply drawn after the text.
- rejected: an agent proposed giving a question a copy of the passage of the design it is about; the owner rejected it, because the question stands in the body already, its context around it.

### decision an-option-is-picked-by-pressing-it
Each option of a question that waits is a grey box under the one before, a radio mark at its left; a press picks it, its frame and mark turning to the accent, and a press on the one picked lets it go.
- replaced: Options side by side stood on main; the owner changed it, because they were hard to read.
- replaced: Radio buttons under the options, repeating them, stood on main; the owner changed it, because the list of options is itself the choice.
- replaced: A row of short labels under the options, each to be matched to its option, stood on main; the owner changed it, because it was easy to press the wrong one.
- replaced: Options told from the code above them only by a shade of grey stood on main; the owner changed it, because what is to be pressed must be told by its shape from what an agent wrote.

### decision one-confirm-sends-every-answer
One Confirm as wide as the body, at the end of its last section, counts the questions picked and can be pressed only once every question that waits has its answer, then sends them all.
- rejected: an agent proposed holding each gate until the questions due there were answered, then judging them once; the owner rejected it, because answering one at a time is wasteful: Confirm stays off until every question is answered, whether it holds the work or not.
- rejected: an agent proposed keeping Confirm off while the reviews of the plan still run; the owner rejected it, because he sees how far they are, and a choice they make void is simply void.

### decision discuss-quotes-the-question
Discuss on a question's card quotes the question into the message box, to the line of the cell that asked it; the work goes on as it was, and a card with options takes no words of its own.
- rejected: an agent proposed a box under every question's options, for words with any choice; the owner rejected it, because a question is its options alone, and all that is said of it is said in the discussion.
- rejected: an agent proposed that Discuss pause the work as it quotes the question; the owner rejected it, because he may want only the background of a question whose options he agrees with: a question, a talk and a pause leave each other alone.

### decision a-question-names-the-cell-that-asked-it
A question's card names on its frame, at the top left, the cell that asked it by its id and then the line it runs on; a card of no cell's keeps its frame whole.
- replaced: A card that did not say who asked it stood on main; the owner changed it, because he wants to see at a glance where a question comes from.
- rejected: an agent proposed marking also a question the engine asked, as `I1 · engine`; the owner rejected it, because engine and engineer side by side read too alike.

### decision an-answered-question-folds
A question the owner has answered opens folded to its first three lines, with a bar to show it all and one to fold it; it folds as motion, kept in view, and a question that waits opens whole.
- replaced: Every question drawn whole stood on main; the owner changed it, because the window had grown far too long.
- rejected: an agent proposed folding and unfolding a question at once; the owner rejected it, because a question folded so was left out of view.

### decision the-discussion-is-no-chat
Each message of the discussion is a block across the window, its writer, whom it is to and when on its first line; the owner's own stand on the accent's tint, as a question's card does.
- rejected: an agent proposed a chat's layout, the owner at the right and the agents at the left with pictures of them, in bubbles coloured by line; the owner rejected it, because it wastes the room of a window already narrow.
- rejected: an agent proposed tying each reply to the message it answers, by the runs' records or by numbered messages; the owner rejected it, because the ground of his own messages tells them apart enough for now.

### decision the-discussion-holds-the-owners-talk-alone
The discussion holds only the owner's words to the work's cells and the cells' answers; what the reviews and the cells they review say to each other is not drawn there.
- replaced: The reviewers' messages drawn in the item's discussion stood on main; the owner changed it, because there, without their context, they only waste the screen.
- rejected: an agent proposed writing the review of the tree's changes and its back and forth into the item's discussion; the owner rejected it, because he does not want to read them.

### decision a-message-shows-a-clock-or-a-tick
Each message the owner sends a cell carries a mark, a clock until a run of the cell has begun with it, then a tick, each saying on pointing at it what it means.
- replaced: A line at the foot of the window saying whom the words were said to stood on main; the owner changed it, because whether they were taken belongs on the message sent, not at the bottom.

### decision the-owners-words-go-to-a-line
The owner's words go to a line of the work's flow, picked on a row of buttons over the box, one for each line with a cell that takes words, each named by the line and that cell's id.
- replaced: Three cells written in the page's code, the research, the deep research and the implementation, stood on main; the owner changed it, because a page that names cells breaks the rule that it names none and takes its words from the flow's files.
- rejected: the owner proposed hanging every message on the research, whose session would answer when the work came back to it; the owner rejected it, because after the deep research the plan comes from another session, and a remark on the implementation would go round through the research.
- rejected: an agent proposed a button for every line, the reviews' among them; the owner rejected it, because a cell that may ask the owner takes his words and one that takes none asks nothing (principle who-asks-can-be-spoken-to).
- rejected: an agent proposed naming the cell on its button by its title, as `engineer · Implement`; the owner rejected it, because he reads the cells' ids.

### decision only-a-line-that-takes-words-is-picked
A line on which the engine has no cell to give the owner's words to is grey and says why on pointing at it; of the others, the line the work stands at is picked first, else the one it reached last.
- replaced: A line of a cell the work had not reached, offered and picked first, the box under it saying the words would go nowhere, stood on main; the owner changed it, because a cell the work has not reached cannot be chosen, as the deep research already could not be.

### decision the-step-shows-the-agents-last-two-actions
While a round runs, the Step shows the last two things its agent did, the earlier ones folded behind a link that shows them all.
- replaced: The last eight actions, read anew at each poll, stood on main; the owner changed it, because the list kept refreshing, grew longer, then vanished and began again.
- rejected: an agent proposed one line while a round runs, of its cell, its time, its tools, the files it changed and its tests; the owner rejected it, because he wants the latest two with the whole history at hand.

### decision the-step-names-no-round
The Step names no round: it shows a run's details once, and where a return was sent back, says so with the reasons, and the details of the next time under it.
- replaced: A heading Round 1, with more rounds after a return sent back, stood on main; the owner changed it, because a round is the engine's notion, and all the owner needs is that a return was wrong and sent back.
- rejected: an agent proposed leaving out the heading only where a run has one round, telling rounds apart where there are several; the owner rejected it, because a return is either handed in or sent back, and the owner need not learn what a round is.

### decision a-runs-inputs-are-what-its-cell-is-given
The Step's Inputs are the blocks the run's cell is given, watches or reads, each at the version the run began with, those new to it first; what it may only write is under Handed in.
- replaced: A list headed Started from of every block the run read or might write, most with no version yet, stood on main; the owner changed it, because it could not be read: what was read and what was written stood mixed in it.
- rejected: an agent proposed listing only the blocks with a version, the rest in one grey line; the owner rejected it, because it still mixed what the run reads with what it writes.

### decision the-step-says-why-a-run-ran-plainly
The Step says in plain words why a run ran, as that a block is new to it, and what its agent said of a change it handed in: that it matters, or not, to the cells that read it.
- replaced: The engine's words, as changed in meaning which its record knew no version of, and said to matter, stood on main; the owner changed it, because they could not be read as sentences.
- rejected: an agent proposed leaving out what the agent said of its change; the owner rejected it, because that word decides whether the cells that read the block wake.

### decision the-context-curves-take-their-lines-colours
On the Usage each session's context curve takes the colour of its flow's line, its name at its end after a dash of that colour, a name that would lie over another moved down a row.
- replaced: The names set apart in a column at the chart's right stood on main; the owner changed it, because he could not match a name to its curve.

### decision a-session-is-named-by-its-line-and-number
The Usage names a session by its line and its number among the line's sessions, as `engineer 0`.
- rejected: an agent proposed `engineer · 1st session`, the number counting from nought; the owner rejected it, because `engineer 0` is clear and short and tells two sessions apart.

### decision the-changes-are-under-the-work-tab
The Work tab lists under Files changed each file the work changed, its lines added and removed, its path in the text's colour opening its diff at a press.
- rejected: the owner proposed a tab of its own for the changes; the owner rejected it, because the Work tab, nearly empty, could hold them.
- replaced: The paths in the blue of a link stood on main; the owner changed it, because he asked for them in the text's colour, giving no reason.

### decision a-diff-is-drawn-as-the-text-it-changes
A diff in the window is drawn as the text reads after the change, each line numbered as in the file after it: words removed struck through on the red tint, then words added on the green.
- replaced: A diff drawn as a block of raw code stood on main; the owner changed it, because it could not be read.
- replaced: Lines drawn with no numbers stood on main; the owner changed it, because he asked for line numbers in every diff, giving no reason.

### decision a-diff-in-a-text-folds-as-a-works-files-do
A diff in a text of the window is folded under a row, as each file under the Work tab is: The diff where it changes one file, each file's path where it changes several.
- replaced: Every diff drawn open stood on main; the owner changed it, because a question that held several was too long to read.
- rejected: an agent proposed folding each diff under the fold of a section of the window, named by its path; the owner rejected it, because that made a heading in the body and named the file twice.

### decision every-section-of-the-window-folds
Each section of the window, in every tab, is one fold: a row in a heading's colour with an arrow, its name and a line to the edge, kept folded or open from one item to the next.
- replaced: Sections not set apart stood on main; the owner changed it, because the plan was lost among the text.
- replaced: Small headings with no line for the Work, Step and Usage tabs, in a style of the console's own, stood on main; the owner changed it, because he asked for them made as the Item tab's are, giving no reason.

### decision the-card-is-the-flows
What the item's card shows is named by the work's flow in its `flow.toml`, as `card`, data in their order; the page draws a section of each by its name and names no datum or flow of its own.
- replaced: The page taking `delivered` where the flow was `deliver`, else `plan_owner`, stood on main; the owner changed it, because page code that names a block breaks principle engine-knows-no-cell, and would not pass a review in the engine.
