# The system

The view of the state of the whole system, and the settings of the installation and of each project.

### decision the-copies-stand-on-the-system-view
The state of the database's copies is shown in the System view's section on what is going on now, and nowhere on the Works page.
- replaced: A grey, red or orange line at the top of the Works page was on main; the owner changed it, because the backup belongs to the installation and not to a work, and the line told him nothing while all was well.

### decision a-red-main-stands-on-the-system-view-only
A project whose main is red is a row of the System view's section on what is going on now, in the colour of a failure; the console carries no mark of it.
- rejected: an agent proposed a mark beside the project's name in the console's list as well; the owner rejected it, because the same fact would stand in two places, with one more mark to explain, and the list means what waits for the owner, while a red main waits for the architect.

### decision a-work-whose-state-cannot-be-worked-out-has-a-tile-of-its-own
The System view counts a work whose state the engine cannot work out in a tile named Broken, after Halted, and not in Halted; the tile says that the work's card tells why.
- rejected: an agent proposed leaving it in the Halted tile and widening that tile's description to cover it; the owner rejected it, because Halted would then hold two meanings, and the tile's name would differ from the state word on the card (principle one-name-one-meaning).

### decision the-shared-wait-is-said-without-the-trace
The wait every work shares is described by what is down and since when, what the engine's check found, and what the wait holds; never the round that met it, its error, or when the engine checks next.
- rejected: an agent proposed six sentences, among them the round that met the wait and the command line's own words; the owner rejected it, because the last two sentences and the words saying it was not signed in said one thing three times.
- replaced: The same last sentence for every cause, that no round begins until the command line runs again, was on main; the owner changed it, because it did not fit a limit, the network or the service, so each cause says what ends it.

### decision the-waited-part-stands-after-produced
The System view's Waited part, after Produced, counts the works' stops by cause, what CI took and the slots held, over the span and by week, and says what the engine does not know.
- rejected: an agent proposed counting the stops in Produced and the slots held in Spent, with no part of its own; the owner rejected it, because Produced and Spent would each hold a meaning that is not theirs, and the week table would grow wider.
- rejected: an agent proposed giving these figures only in the System data and by `wf-agents system`, and drawing them later with the monitoring of what is past its line; the owner rejected it, because the owner would still not see the numbers on the page.

### decision the-rounds-sent-back-are-listed-by-item
The Gone wrong part's tile Rounds sent back unfolds them by item, the most sent back first: how many, each reason by its key with how many, the cells, the cost and the latest.
- replaced: A list of the rounds sent back one by one, the latest first, with no reason, was on main; the owner changed it, because why and where the returns were sent back could not be seen at a glance.

### decision the-page-changes-what-is-tuned-while-the-engine-runs
The Settings view changes the language, each level's model, the highest level of an item and of a round, the context past which a line starts a new session, and the slots; the rest only in the file.
- replaced: Four settings from the installation's file, shown but not changeable on the page, was on main; the owner changed it, because the page showed too little, and what is tuned while works go on should be changed there.
- replaced: The levels' models set only in the installation's file was on main; the owner changed it, because he wanted the levels' models, and what goes with them, in the page's settings.

### decision the-accounts-are-used-removed-and-added-there
Beside them the Settings view lists the accounts: the one in use marked, each other with Use, which acts at once, and Remove, which asks once; an account is added by its name and its token.
- replaced: A table of the program's directories, one for each account, each signed in by hand and switched to only after the owner said yes, was on main; the owner changed it, because switching no longer loses the sessions, so one press may switch.

### decision the-usage-column-names-the-services-windows
An account's Usage, its table's second column, shows on one line each window the service names, how much of it is used, and how long until it resets and when, counting down while the view is open.
- replaced: The columns Account, Added, Token lasts until and Usage, with a reset given as a date, was on main; the owner changed it, because the day added and the token's end were of no use, and he wanted to see how long until each reset.
- replaced: A usage limit shown only for an account under which a round had hit it was on main; the owner changed it, because when switching he most wants to know which account has the most left.

### decision the-switch-and-the-engines-moves-stand-with-the-accounts
The Settings view puts the switch Move on at the limit above the accounts and, once the engine has moved, lists its latest five moves below, each with the account it left and when that limit resets.
- rejected: an agent proposed only the switch, each move shown on the in-use mark as it is pointed at; the owner rejected it, because only the latest move would be kept, and an earlier one, or the account it left, could not be found.
- rejected: an agent proposed the record of moves in the System view's section on what is going on now; the owner rejected it, because the switch and its record would stand on two pages, and a thing is shown in one place.
- rejected: an agent proposed the switch at the bar's right end beside the keep-warm switch; the owner rejected it, because it only manages the accounts and is set once, unlike Pause all and the slots, which are pressed at any time, and it belongs beside the accounts it moves between.

### decision each-project-has-a-section-of-its-own
After the installation's parts, the Settings view has a section for each project: its tables of install.toml, then its declaration as the engine read it as it started, or why it holds the project.
- replaced: A project's tables of install.toml among the installation's own, and nothing of its declaration, was on main; the owner changed it, because what is a project's is in two files, the installation's and its declaration, and he governs the whole configuration in one place.
- rejected: an agent proposed a page of its own for the levels alone; the owner rejected it, because he governs the whole configuration in one place.

### decision a-round-under-way-shows-its-figures-against-their-lines
The System view's rounds under way have the columns Context, Added, Per call and Cost; the last three read value of line, and the one value past its own line is in the warning colour.
- rejected: an agent proposed the columns Context, Added and Cost without the growth for each call; the owner rejected it, because Added's mark would also flag a normal long round, and the owner would be led to rounds that are well.
- rejected: an agent proposed the four columns with values only, the lines in the hover and no colour for a figure past its line; the owner rejected it, because which round to look at could not be seen without opening each hover.

### decision a-merges-cost-is-what-the-whole-span-spent-for-each
The Produced part gives a merge's cost on average as what the whole span spent for each merge, what was wasted included, with the merged works' own average beside it, each said in its own range.
- replaced: A merge's cost on average as the merged works' own cost alone was on main; the owner changed it, because it left out what was wasted, so the owner could not see what a merge really cost.
- rejected: an agent proposed keeping the cell and adding a cell Spent per merge; the owner rejected it, because two numbers in two cells read as one number that contradicts itself.
- rejected: an agent proposed a column Span share in the list of merged works and no average; the owner rejected it, because the overall average could not be seen, and the list has to be unfolded first.

### decision the-spending-is-split-by-project
The System view's Spent part has a table By project before those by cell and by model, its rounds under way a Project column, and the totals at its top count every project.
- replaced: Spending split by cell, by model and by work alone, and rounds under way that named no project, was on main; the owner changed it, because the installation names several projects and each item belongs to one.
