# The item's window

The window that the console, and a work's page for the work's item, open for an item: its tabs, its lines and stops, the questions in its body, the owner's words to each line, and what its Work, Step and Usage tabs show.

### decision one-window-on-the-console-and-a-works-page
The window has four tabs, Item, Work, Step and Usage, the same on the console and on a work's page; Step shows the chosen row of the history, or else the run under way, or else the work's latest run.
- replaced: Step and Usage only on a work's page was on main; the owner changed it, because the console is about the item, which stays while its work moves on under it, so the console should also show what the work's page shows of the work.
- replaced: A Step left empty on the console while no round ran was on main; the owner changed it, because the Step should show the running round, or, when none runs, the last one's result, and never be empty.

### decision the-windows-tabs-look-as-the-bars
The window's tabs are drawn like the bar's views: each is a muted word, the one shown is in the text's colour over a line in the accent, and the arrow keys move from one tab to the next.
- replaced: A segmented switch, like the one for Auto, Light and Dark, was on main; the owner changed it, because it did not look like tabs: a switch picks among options, while tabs change what the window shows.

### decision the-window-writes-the-level-by-the-item
The window writes the level on the item's line after the item's standing, and the id of the cell the work is at before the strip of the work's cells, at the right of that line.
- replaced: The level before the strip, in place of the cell's id, with the cell shown only by the strip's mark, was on main; the owner changed it, because it is the item's level, which belongs with the item's own facts at the left.
- replaced: The cell the work is at, named only on the console's card, was on main; the owner changed it, because the window should name it as the card does.

### decision the-pause-switch-is-the-owners-own
Starting and pausing a work is one switch, named as the owner's own pause and showing only that pause; the engine's pause of a work is lifted only by Resume on that pause's card.
- replaced: A button named Let it on was on main; the owner changed it, because the name was strange.
- rejected: an agent proposed Start, Pause and Cancel side by side, with one greyed out while another applies; the owner rejected it, because starting and pausing exclude each other, so they are one switch.
- rejected: an agent proposed that the switch lift the engine's pause too, besides the Resume on that pause's card; the owner rejected it, because it was a second way to end one stop.

### decision every-stop-is-one-card
Every stop of a work is one card under its state line, framed in red on the red tint, with the engine's one sentence of why and the red buttons that end it; a pause of all works has no card on a work.
- rejected: an agent proposed a separate card for each kind of stop, with a purple frame and red words, each kind named in its own words; the owner rejected it, because the purple card with red words was ugly and one thing was called by many names.
- replaced: A stop's reason given under the state line and again in its card, where the engine's sentence was followed by the program's message saying the same, was on main; the owner changed it, because one thing was written over and over, outside the card and in it.
- replaced: A red card with Resume all on each work held by the engine's pause of all works was on main; the owner changed it, because the top of the page already shows that pause with Start all, which does the same, and five held works showed five cards.
- replaced: A blue button in every card was on main; the owner changed it, because the button of a red card should be in the card's red.

### decision a-question-that-holds-the-work-is-red
A question that holds the work is framed in red on its tint, as a stop is; one that does not, in orange on its tint; one answered, in the accent on its tint.
- replaced: A question answered in the grey hairline frame of the body's other parts was on main; the owner changed it, because now that every question not answered is red or orange, the accent tells an answered one from the body's text.
- replaced: A question that did not hold the work framed in the accent on the accent's tint was on main; the owner changed it, because one meaning has one colour, on the card and in the window.
- replaced: A yellow question box was on main; the owner changed it, because it did not stand out.
- replaced: Every question in one purple card under the words The work waits for this answer was on main; the owner changed it, because those words are needless, and a question that holds the work is red.

### decision the-stop-walked-to-stands-in-a-soft-light
The stop n or N goes to, or a click on its frame chooses, stands in the soft light of a card chosen on the console, in its own colour; one at a time, none after Escape.
- replaced: Nothing showing which stop the walk stood at was on main; the owner changed it, because n scrolled the window to a stop, but a question box cannot be selected, so nothing showed which stop was current.

### decision a-question-stands-where-a-text-points-to-it
A question is drawn at the first `{{name}}` that points to it, in the body's passages and then in an agent's messages; a later pointer links to it, and one no text points to is drawn after the body.
- replaced: Every question drawn at the end of the card's last section was on main; the owner changed it, because a question is shown where it is asked, in the text it is about.
- rejected: an agent proposed gathering what waits for the owner at the top of the window, to be handled there; the owner rejected it, because a question belongs with the text it is about, and at the top he would have to read the body below and come back up to answer.
- rejected: an agent proposed a field in a question's form that names the datum and quotes the sentence the question arises in; the owner rejected it, because a text already points to a question by `{{name}}`, as the agents are told to write it.
- rejected: an agent proposed sending back a return whose question no text points to; the owner rejected it, because a question no text points to is simply drawn after the text.
- rejected: an agent proposed giving a question a copy of the passage of the design it is about; the owner rejected it, because the question already stands in the body, with its context around it.

### decision an-option-is-picked-by-pressing-it
Each option of a waiting question is a grey box under the one before, with a radio mark at its left; a press picks it and turns its frame and mark to the accent, and pressing it again unpicks it.
- replaced: Options side by side was on main; the owner changed it, because they were hard to read.
- replaced: Radio buttons under the options, repeating them, was on main; the owner changed it, because the list of options is itself the choice.
- replaced: A row of short labels under the options, each to be matched to its option, was on main; the owner changed it, because it was easy to press the wrong one.
- replaced: Options told apart from the code above them only by a shade of grey was on main; the owner changed it, because what is to be pressed must differ in shape from what an agent wrote.

### decision one-confirm-sends-every-answer
One Confirm as wide as the body, at the end of its last section, counts the questions picked, can be pressed only once every waiting question has its answer, and then sends them all.
- rejected: an agent proposed holding each gate until the questions due there were answered, then judging them once; the owner rejected it, because answering one at a time is wasteful: Confirm stays off until every question is answered, whether it holds the work or not.
- rejected: an agent proposed keeping Confirm off while the plan's reviews are still running; the owner rejected it, because he can see how far along they are, and a choice they make void is simply void.

### decision discuss-quotes-the-question
Discuss on a question's card quotes the question into the message box, addressed to the line of the cell that asked it; the work goes on as it was, and a card with options has no text box of its own.
- rejected: an agent proposed a text box under every question's options, for words to go with any choice; the owner rejected it, because a question is only its options, and everything said about it is said in the discussion.
- rejected: an agent proposed that Discuss pause the work as it quotes the question; the owner rejected it, because he may want only the background of a question whose options he agrees with: a question, a discussion and a pause do not affect one another.

### decision a-question-names-the-cell-that-asked-it
A question's card names, on its frame at the top left, the id of the cell that asked it and then the line that cell runs on; a card that no cell asked keeps its frame unbroken.
- replaced: A card that did not say who asked it was on main; the owner changed it, because he wants to see at a glance where a question comes from.
- rejected: an agent proposed also marking a question the engine asked, as `I1 · engine`; the owner rejected it, because engine and engineer look too alike side by side.

### decision an-answered-question-folds
A question the owner has answered opens folded to its first three lines, with a bar to show it all and one to fold it; it folds with an animation and stays in view, and a waiting question opens whole.
- replaced: Every question drawn whole was on main; the owner changed it, because the window had grown far too long.
- rejected: an agent proposed folding and unfolding a question instantly; the owner rejected it, because a question folded that way ended up out of view.

### decision the-discussion-is-no-chat
Each message of the discussion is a block across the window, with its writer, who it is to and when on its first line; the owner's own messages stand on the accent's tint, as a question's card does.
- rejected: an agent proposed a chat layout, with the owner at the right and the agents at the left with pictures of them, in bubbles coloured by line; the owner rejected it, because it wastes room in a window that is already narrow.
- rejected: an agent proposed tying each reply to the message it answers, by the runs' records or by numbered messages; the owner rejected it, because the background of his own messages sets them apart well enough for now.

### decision the-discussion-holds-the-owners-talk-alone
The discussion holds only the owner's words to the work's cells and the cells' answers; what the reviews and the cells they review say to each other is not drawn there.
- replaced: The reviewers' messages drawn in the item's discussion was on main; the owner changed it, because there, without their context, they only waste the screen.
- rejected: an agent proposed writing the review of the tree's changes and its back and forth into the item's discussion; the owner rejected it, because he does not want to read them.

### decision a-message-shows-a-clock-or-a-tick
Each message carries a mark: the owner's to a cell a clock until a run of the cell has begun with it, then a tick; a cell's reply to the owner a clock until the owner has read it, then a tick.
- replaced: A line at the bottom of the window saying who the words were sent to was on main; the owner changed it, because whether they were taken belongs on the sent message, not at the bottom.
- replaced: A cell's reply with no mark of whether the owner had read it was on main; the owner changed it, because the owner wanted each reply marked read beside it, and the two marks already say that the one a message is for has taken it in.

### decision a-reply-counts-as-read-once-it-has-stood-in-view
A cell's reply counts as read once it has been fully in view of the open window for about a second while the page is seen, or once its end has if it is taller; m and M bring the next one into view.
- rejected: an agent proposed that only a press counts as read, on a mark by each reply, with R and Mark as read clearing an item; the owner rejected it, because every reply would have to be pressed, and m would come back to the same reply until it was.
- rejected: an agent proposed that opening the Item tab read everything new in the item and all its replies at once; the owner rejected it, because a reply far down the window would count as read unseen, and the owner could not find it again.

### decision the-owners-words-go-to-a-line
The owner's words go to a line of the work's flow, picked from a row of buttons above the box: one for each line with a cell that takes words, each named by the line and that cell's id.
- replaced: Three cells written into the page's code, the research, the deep research and the implementation, was on main; the owner changed it, because a page that names cells breaks the rule that it names none and takes its words from the flow's files.
- rejected: the owner proposed attaching every message to the research, whose session would answer when the work came back to it; the owner rejected it, because after the deep research the plan comes from another session, and a remark on the implementation would take a detour through the research.
- rejected: an agent proposed a button for every line, including the reviews'; the owner rejected it, because a cell that may ask the owner takes his words, and one that takes none asks nothing (principle who-asks-can-be-spoken-to).
- rejected: an agent proposed naming the cell on its button by its title, as `engineer · Implement`; the owner rejected it, because he reads the cells' ids.

### decision only-a-line-that-takes-words-is-picked
A line on which the engine has no cell to give the owner's words to is grey, and pointing at it says why; of the others, the line the work is at is picked first, or else the one it reached last.
- replaced: A line of a cell the work had not reached, offered and picked first while the box under it said the words would go nowhere, was on main; the owner changed it, because a cell the work has not reached cannot be chosen, just as the deep research already could not be.

### decision the-step-shows-the-agents-last-two-actions
While a round runs, the Step shows the last two things its agent did, the earlier ones folded behind a link that shows them all.
- replaced: The last eight actions, reloaded at each poll, was on main; the owner changed it, because the list kept refreshing, grew longer, then vanished and began again.
- rejected: an agent proposed one line while a round runs, with its cell, its time, its tools, the files it changed and its tests; the owner rejected it, because he wants the latest two actions with the whole history at hand.

### decision the-step-names-no-round
The Step names no round: it shows a run's details once, and where a return was sent back, it says so with the reasons, and shows the details of the next time under it.
- replaced: A heading Round 1, with more rounds after a return was sent back, was on main; the owner changed it, because a round is the engine's concept, and all the owner needs to know is that a return was wrong and sent back.
- rejected: an agent proposed leaving out the heading only where a run has one round, and telling rounds apart where there are several; the owner rejected it, because a return is either handed in or sent back, and the owner need not learn what a round is.

### decision a-sent-back-reason-is-shown-by-its-key
The Step shows each reason a return was sent back by its key in `sent_back.toml` and the values recorded under their names, with no sentence of the page's own.
- replaced: The page's own 33 sentences for the reasons, worded apart from the file, was on main; the owner changed it, because two copies drifted: a reason the engine added had no words on the page for a day.
- rejected: an agent proposed also showing the file's sentence, read from the directory as it is now, with the values filled in; the owner rejected it, because a later change of the file would rewrite past returns, and the page would need the rule that fills it.
- rejected: an agent proposed keeping the files beside the flows with each version of a flow and showing the sentence of the work's version; the owner rejected it, because it changes the store and the files kept with a version, and old versions have none.

### decision a-runs-inputs-are-what-its-cell-is-given
The Step's Inputs are the blocks the run's cell is given, watches or reads, each at the version the run began with, those new to the run first; blocks it may only write are under Handed in.
- replaced: A list headed Started from of every block the run read or might write, most with no version yet, was on main; the owner changed it, because it could not be read: what was read and what was written were mixed in it.
- rejected: an agent proposed listing only the blocks with a version, the rest in one grey line; the owner rejected it, because it still mixed what the run reads with what it writes.

### decision the-step-says-why-a-run-ran-plainly
The Step says in plain words why a run ran, such as that a block is new to it, and what its agent said about a change it handed in: whether or not it matters to the cells that read it.
- replaced: The engine's own words, such as changed in meaning which its record knew no version of, and said to matter, was on main; the owner changed it, because they could not be read as sentences.
- rejected: an agent proposed leaving out what the agent said about its change; the owner rejected it, because that word decides whether the cells that read the block wake.

### decision the-context-curves-take-their-lines-colours
On the Usage tab, each session's context curve takes the colour of its flow's line, with its name at the curve's end after a dash of that colour; a name that would overlap another moves down a row.
- replaced: The names set apart in a column at the chart's right was on main; the owner changed it, because he could not match a name to its curve.

### decision a-session-is-named-by-its-line-and-number
The Usage names a session by its line and its number among the line's sessions, as `engineer 0`.
- rejected: an agent proposed `engineer · 1st session`, with the number counting from zero; the owner rejected it, because `engineer 0` is clear and short and tells two sessions apart.

### decision the-changes-are-under-the-work-tab
The Work tab lists, under Files changed, each file the work changed with its lines added and removed; its path is in the text's colour and opens its diff when pressed.
- rejected: the owner proposed a separate tab for the changes; the owner rejected it, because the Work tab was nearly empty and could hold them.
- replaced: The paths in the blue of a link was on main; the owner changed it, because he asked for them in the text's colour, giving no reason.

### decision a-diff-is-drawn-as-the-text-it-changes
A diff in the window is drawn as the text reads after the change, each line numbered as in the changed file: removed words struck through on the red tint, then added words on the green.
- replaced: A diff drawn as a block of raw code was on main; the owner changed it, because it could not be read.
- replaced: Lines drawn with no numbers was on main; the owner changed it, because he asked for line numbers in every diff, giving no reason.

### decision a-diff-in-a-text-folds-as-a-works-files-do
A diff in a text of the window is folded under a row, as each file under the Work tab is: a row named The diff where it changes one file, a row for each file's path where it changes several.
- replaced: Every diff drawn open was on main; the owner changed it, because a question that held several diffs was too long to read.
- rejected: an agent proposed folding each diff under the fold of a section of the window, named by its path; the owner rejected it, because that made a heading in the body and named the file twice.

### decision every-section-of-the-window-folds
Each section of the window, in every tab, is one fold: a row in a heading's colour with an arrow, its name and a line to the edge; it stays folded or open from one item to the next.
- replaced: Sections not set apart was on main; the owner changed it, because the plan was lost among the text.
- replaced: Small headings with no line for the Work, Step and Usage tabs, in the console's own style, was on main; the owner changed it, because he asked for them to be made like the Item tab's, giving no reason.

### decision the-card-is-the-flows
The outline of the version a work runs on names, from its flow's `card`, the data the item's card shows, in order; the page draws a section for each by its name and names no datum or flow of its own.
- replaced: The page using `delivered` where the flow was `deliver`, else `plan_owner`, was on main; the owner changed it, because page code that names a block breaks principle engine-knows-no-cell, and would not pass a review in the engine.
- replaced: Reading the card from the flow installed was on main; the owner changed it, because a work that ended would show the card of a flow it never ran on (principle the-past-as-it-ran).
