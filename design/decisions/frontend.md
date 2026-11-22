# The owner's page

What all views of the owner's page share: the bar, the look, the marks and colours, and how the page draws what agents write.

### decision the-bar-has-six-tabs
The bar's tabs are Console, Works, Flow, Observations, System and Settings, in that order; the page has no Messages and no Items tab.
- rejected: an agent proposed the tabs Messages, Works, Console, Flow and Settings; the owner rejected it, because the console is the centre of the page, the Messages page is being retired, and the items have become part of the console.
- replaced: Eight tabs, with Messages first, the architect second and Items among them, was on main; the owner changed it, because the console is the centre of the page, and the observations, the system and the settings each need a view.

### decision the-console-tab-counts-as-the-cards-do
Beside Console the bar shows, as a card's tags stand, the orange count of the questions the owner may answer ahead and, right of it, the red count of what waits; each only above nought.
- replaced: The red count alone beside Console was on main; the owner changed it, because an item's card showed an orange count that nothing beside Console said, and the bar and the cards should agree.

### decision what-stands-at-the-bars-right
The right end of the bar shows the engine's state, Pause all or Start all with no confirmation, the switch that keeps waiting sessions warm, the slots in use, and the light and dark switch.
- rejected: an agent proposed leaving the keep-warm switch out of the bar and removing the light and dark switch; the owner rejected it, because he wanted to see the whole page at once, with what is not built yet shown as a mock, and the light and dark switch kept.
- replaced: A confirmation before Start all was on main; the owner changed it, because the dialog's question was not worth asking.
- rejected: an agent proposed putting the keep-warm switch on the Settings page, as a new kind of setting; the owner rejected it, because the bar holds the installation's other controls of this kind: the pause of all works and the slots.

### decision a-cancelled-work-is-grey
The mark of a cancelled work, in the list of works and on the console, is grey and never red.
- replaced: An exclamation mark for a work that had ended was on main; the owner changed it, because he took it for a fault.

### decision an-engines-halt-is-red
A work the engine halted is marked in the red of a fault, and a work the owner paused is not; the bar's dot is red when the engine paused all works, and grey when the owner did.
- replaced: One word, stopped, for every halt was on main; the owner changed it, because a pause is someone's choice and the list's filters called it paused, while red says there is a problem to look at.
- replaced: The same dot for a pause of all works, whether the engine or the owner made it, was on main; the owner changed it, because a pause the engine made by itself, not one he made, should show at least some red.

### decision the-owners-pause-stands-beside-the-state
In the list of works and the System view, Paused stands beside a work's state while the owner's pause holds it and the state says something else; the engine's pause is the state itself.
- replaced: One place saying a work was paused and another that it waited for the owner was on main; the owner changed it, because he could not tell which of the two stopped the work.

### decision red-is-the-one-colour-of-a-stop
Red is the only colour of a stop: a work that waits for the owner and a work halted by a fault are red in every view.
- replaced: Orange for what waits for the owner on the Works page and in the bar, but red on the console, was on main; the owner changed it, because one meaning has one colour, and he had chosen red for every stop.

### decision a-call-waited-for-is-no-run
Wherever the page shows the state of a work waiting for the results of tools the engine runs, it says in a call's yellow that it waits for each tool, where it runs and how long, never that it runs.
- rejected: an agent proposed a separate mark for the tool, after the cell that asked for it; the owner rejected it, because every call that takes time should keep the motion on the card: its cell's mark turned yellow and still blinking.
- rejected: an agent proposed a cell in the flow for each tool, as the old system had; the owner rejected it, because cells for tools do not fit the flow's new model.
- replaced: Different words for such a work in each place, finished in one, a call under way in another and running in a third, was on main; the owner changed it, because every place should show the same thing.
- replaced: Teal for the part of a work's time spent in a tool, while the cells marked a call in yellow, was on main; the owner changed it, because one meaning in two colours breaks the rule that one meaning has one colour.

### decision one-set-of-marks-for-how-a-run-stands
One set of marks shows how a run ended or how it stands now, filled for a run that ran or is running and hollow for one still to run; a work's strip of cells and its history both use it.
- replaced: Nine marks, among them blue, red and light-blue hollow frames, was on main; the owner changed it, because so many shapes could not be remembered.
- replaced: Separate marks in the history, a dash for a run that handed in nothing new and a mark for a run given up, was on main; the owner changed it, because what means one thing is drawn the same way everywhere, and the strip's dash already meant skipped.
- replaced: Two different strips, one on the console's card and one in the window, where the window's neither blinked nor marked a stop red, was on main; the owner changed it, because the same kind of thing is drawn from the same parts.

### decision a-level-looks-as-the-level-it-runs-at
A card, under its number, and the item's window write a cell's level as `L2`, or `L2+1` where its line raises it, in the look of the level it runs at: muted, text, accent, accent in bold.
- replaced: The name of the cell the work stood at, under the card's number, was on main; the owner changed it, because that place is for the level, and the strip already marks where the work stands.
- rejected: an agent proposed a small grey number after the cell's name, and the words level 2 and cell level 3 in the window; the owner rejected it, because the place of the cell's name is for the level, which must plainly show both the base level and the current one.
- rejected: an agent proposed three looks, with the third level in the accent and bold; the owner rejected it, because a level that its line raises past the third is higher still, and only that level takes the bold look.
- rejected: the owner proposed a separate hue for each level; the owner rejected it, because red, yellow, the accent and green each already mean a state.

### decision the-way-back-is-in-the-accent
The flow's views draw a way back in the accent, a thick line and arrow on the matrix and the flowchart; a work's history draws it in red, as decision a-wake-is-an-arrow-from-the-writer says.
- rejected: an agent proposed keeping the ways back in their own violet, next to a blue accent; the owner rejected it, because the violets clashed with the page.
- replaced: Ways back in grey, told apart from other lines only by their shape, was on main; the owner changed it, because a way back should be in the highlight colour, and one thing should have one colour in every view.
- replaced: A separate line for the way round in a run's panel was on main; the owner changed it, because the matrix's arrow back already shows it.

### decision every-mark-is-in-one-table
Every mark the page draws is defined once, with a name of a word or two, in one table, and each view's legend lists only the marks that view draws, on one line, or on the console in a panel.
- replaced: Separate marks and legends in each view, which named one thing in several ways, was on main; the owner changed it, because a legend's words had grown too long for its line, and things that were alike looked different from view to view.

### decision the-page-draws-markdown-itself
The page draws the Markdown that agents write with its own renderer and its own parts, and uses no Markdown library.
- replaced: Markdown shown as raw text was on main; the owner changed it, because shown that way it could hardly be read.
- rejected: an agent proposed a Markdown library; the owner rejected it, because the page needs only a few forms, and nothing an agent writes may become markup the page did not intend.

### decision code-is-coloured-from-prisms-words
The words of code in a diff, and in a fence that names its language, are coloured by their kind (keyword, literal or comment) as Prism reads them, and drawn with the page's own parts.
- rejected: an agent proposed highlight.js; the owner rejected it, because a library may only split an agent's code into typed words for the page to draw and never turn it into markup, and Prism can work that way.
- rejected: an agent proposed a small highlighter of the page's own for the project's few languages; the owner rejected it, because the claim that an outside library breaks a rule did not hold, and Prism reads more languages, and reads them correctly.
- rejected: an agent proposed Prism's own plugin for code in a diff; the owner rejected it, because the plugin works on the markup Prism writes, which would turn an agent's code into markup the page did not intend.
- replaced: Code in a Markdown fence left uncoloured was on main; the owner changed it, because things that are alike look alike.

### decision the-page-draws-diagrams
The page draws mermaid and dot fences, in what agents and the owner write, as diagrams, loading each library when a diagram comes near the screen, and shows the source of one it cannot draw.
- replaced: A diagram's source shown in place of the diagram was on main; the owner changed it, because diagrams are key to how he understands a design.

### decision the-page-uses-no-component-library
The page is drawn with a small set of parts of its own, and uses no component library.
- replaced: GitHub's Primer, its tokens and its components, was on main; the owner changed it, because Primer wastes room on the screen and is not good to look at.

### decision the-look-stays-square-and-cool
Tags, buttons, inputs and the console's cards have 3 px rounded corners, and panels and frames have square ones, on cool greys; no part looks like a pane of glass or a pill.
- rejected: the owner proposed trying the Solid Glass style, with round panes of glass, controls shaped as pills and soft shadows; the owner rejected it, because the console and the flow are drawings, not ordinary controls, and the round corners looked bad where the cool, square look worked well.
- rejected: an agent proposed an instrument look, with a grid of dots and square unfilled cards, or a terminal look, with a grid of lines and titles in monospace; the owner rejected it, because neither worked well when tried behind a switch next to the current look.

### decision no-made-up-data-in-the-installed-page
The page built into the program shows only the engine's data: the page's made-up data exist only on the development server and in the tests.
- replaced: Made-up data built into the program, shown with `?mocks` or `?fixtures` in the address, was on main; the owner changed it, because the owner's program should hold no false data that one parameter could show.

### decision no-engine-numbers-on-the-page
The page shows no number by which the engine tracks a run, a round, a call or a try, except in an address; it names a moment by its time, and a datum's versions as v1, v2.
- rejected: an agent proposed keeping a run's number only in the hover on its cell's name; the owner rejected it, because a number put on the page must serve a purpose, and this one served none.
- replaced: A run's number and the engine's counters of sessions, calls, tries and rounds on the Step, the usage and the System view was on main; the owner changed it, because they told him nothing and served no purpose.
- replaced: An event's number in the record, written after an at sign, to name a moment of a work was on main; the owner changed it, because that number could not be read, but a time can.

### decision the-browser-tells-the-owner-what-waits
While the page is open, a browser notification tells the owner of each work that starts waiting for him, naming its cell and why; a click opens the console at that wait's card.
- rejected: an agent proposed a notification sent by a script on the machine the server runs on; the owner rejected it, because he uses the page in Chrome, and the server runs on his machine only by chance.
- rejected: an agent proposed a push notification to the owner's phone; the owner rejected it, because the browser's notifications are enough for now.
- rejected: an agent proposed a notification of the operating system, sent by the engine, as well as the browser's; the owner rejected it, because the browser's notifications are enough.

### decision the-page-moves-whatever-the-system-asks
The page's animations play whether or not the system asks for reduced motion.
- replaced: Animations stopped under the system's reduced-motion setting was on main; the owner changed it, because he wants them to move.
- rejected: an agent proposed changing at once, with no animation, when the system asks for reduced motion; the owner rejected it, because he has no use for that setting at all.

### decision a-views-choices-are-kept-in-the-browser
The browser keeps each view as the owner left it: its switches, window, order, span, widths and folds; each tab leads back to its last address, and a view the owner leaves is kept, hidden.
- replaced: Choices held only in the view's memory was on main; the owner changed it, because every choice went back to its default each time he went to another tab or reloaded the page.
- replaced: A tab that always led to its first view, the Works tab to its list, was on main; the owner changed it, because a work he had opened was lost when he came back, and there was no way from it back to the list.
- replaced: A view removed when another tab was shown, then built again with its scroll lost when he came back, was on main; the owner changed it, because coming back to the console was a poor experience.

### decision one-window-floats-over-the-right
On the console, the flow and a work's page, one window floats over the right of the view and slides in and out by a tab on its edge, while the view keeps its full width under it.
- replaced: A side column docked beside the chart was on main; the owner changed it, because opening, closing or widening the panel should not move the chart.

### decision the-accent-is-blue-unless-picked
The page's accent is a blue until the owner picks Indigo, Violet, Purple or a colour of his own in the Settings view.
- replaced: A violet accent was on main; the owner changed it, because he had picked this blue with the page's picker and wanted it as the default.
- rejected: an agent proposed leaving the picker off main, which would keep a fixed blue; the owner rejected it, because with everything on main he must have the picker to change the colour.

### decision a-picked-accent-turns-the-default
A colour picked as the accent gives every tone of the default accent its hue and strength, each tone keeping the default's lightness, changed only where it would be harder to read on its ground.
- rejected: an agent proposed taking the picked colour's own lightness; the owner rejected it, because each tone needs its own lightness to read well in light and in dark.

### decision an-accent-keeps-clear-of-the-meanings
The page refuses a colour as the accent if any tone of it is within 45 degrees of hue of red, orange, yellow or green, or has less than 0.4 of the default's strength; the refusal names the nearest.
- rejected: an agent proposed taking any colour the owner picks as the accent; the owner rejected it, because one meaning has one colour.
- replaced: Refusing a colour near the teal of the time spent in tools was on main; the owner changed it, because the time spent in tools became yellow, like a call's mark, and teal then meant nothing.

### decision a-works-past-is-drawn-from-its-outline
A work's page and item window draw its history, cells, card, talk and each run's inputs from its version's outline, going on or ended; the whole flow is given only for a work going on.
- replaced: Drawing every work on the flow installed was on main; the owner changed it, because a work was then read by a flow it never ran on, a cell added since read as skipped and runs at a cell since taken out drawn in no column (principle the-past-as-it-ran).
- rejected: an agent proposed drawing a work from its whole flow while its version is installed and from its outline after; the owner rejected it, because the work would then change on the page when another flow was installed (principle owner-keeps-their-place).

### decision a-cell-an-ended-work-never-ran-did-not-run
A cell of a work that ended with no run takes the strip's dash, with the words "did not run".
- replaced: Saying such a cell was skipped was on main; the owner changed it, because skipped is read by walking the whole flow again, the very reading that showed a cell of a flow the work never ran on as skipped.

### decision a-work-on-no-version-is-drawn-from-its-records
A work on no version has a column for each id its runs name, in the order they first ran, with no title or line; its card shows its item as added, with no talk, and says its flow was not kept.
- rejected: an agent proposed naming its cells by the titles of the flow installed; the owner rejected it, because where an id was given again, a title would name another cell.
- rejected: an agent proposed reading its talk with the owner from its records; the owner rejected it, because the records do not say which datum was which cell's talk.

### decision the-engine-is-stopped-on-the-command-line
The engine is stopped only by `wf-agents stop`, and at once only with `--now --yes`; the bar's engine state reads Stopping, and the System view gives what each round has to go.
- rejected: an agent proposed the stop and the stop at once as controls in the System view or beside Pause all at the bar's right; the owner rejected it, because a stop is for installing a new build, which the assistant does, and later the work that builds and installs itself, never the owner.
- rejected: an agent proposed a mark of its own for a stop on a work's time bar, with an entry in its legend; the owner rejected it, because the page changes only where a stop must be seen, so the bar draws a stop as a pause and the records keep the two apart.
