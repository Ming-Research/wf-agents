# The owner's page

What is common to the views of the owner's page: its bar, its look, its marks and colours, and how it draws what agents write.

### decision the-bar-has-six-tabs
The bar's tabs are Console, Works, Flow, Observations, System and Settings, in that order; the page has no Messages and no Items tab.
- rejected: an agent proposed the tabs Messages, Works, Console, Flow and Settings; the owner rejected it, because the page is centred on the console, the messages page retires, and the items have grown into the console.
- replaced: Eight tabs, Messages first, the architect second and Items among them, stood on main; the owner changed it, because the page is centred on the console, and the observations, the system and the settings each need a view.

### decision what-stands-at-the-bars-right
At the bar's right stand how the engine stands, Pause all or Start all, which asks nothing, the switch that keeps waiting sessions warm, the slots in use, and the switch of light and dark.
- rejected: an agent proposed leaving the keep-warm switch out of the bar and taking the switch of light and dark away; the owner rejected it, because he wanted the whole page at once, what is not built yet mocked, and the switch kept.
- replaced: A confirmation asked before Start all stood on main; the owner changed it, because the dialog said nothing worth asking.
- rejected: an agent proposed the keep-warm switch on the Settings page, as a new kind of setting; the owner rejected it, because the bar holds the installation's like controls, the pause of everything and the slots.

### decision a-cancelled-work-is-grey
The mark of a cancelled work, in the list of works and on the console, is grey and never red.
- replaced: An exclamation mark for a work that had ended stood on main; the owner changed it, because he took it for a fault.

### decision an-engines-halt-is-red
A work the engine halted is marked in the red of a fault and one the owner paused is not; the bar's dot is red where the engine paused every work, grey where the owner did.
- replaced: One word, stopped, for every halt stood on main; the owner changed it, because a pause is someone's choice and was called paused where the list filters, while red says there is a problem to look at.
- replaced: One dot for a pause of every work, the engine's and the owner's alike, stood on main; the owner changed it, because a pause the engine made by itself, and not he, should show at least some red.

### decision red-is-the-one-colour-of-a-stop
Red is the one colour of a stop: a work that waits for the owner and one halted by a fault are red in every view.
- replaced: Orange for what waits on the owner on the Works page and in the bar, red on the console, stood on main; the owner changed it, because one meaning has one colour, and he had chosen red for every stop.

### decision a-call-waited-for-is-no-run
A work waiting for the results of tools the engine runs is said, wherever the page says its state, to wait for each tool, where it runs and how long, in the yellow of a call, never to run.
- rejected: an agent proposed a mark of its own for the tool, after the cell that asked for it; the owner rejected it, because every call that takes time should keep the card's motion, its cell's mark turned yellow and still blinking.
- rejected: an agent proposed a cell in the flow for each tool, as the old system had; the owner rejected it, because the flow's new model does not fit cells for tools.
- replaced: Words of each place's own for such a work, finished in one, a call under way in another and running in a third, stood on main; the owner changed it, because every place should show the same thing.
- replaced: Teal for a tool's stretch of a work's time, while the cells marked a call in yellow, stood on main; the owner changed it, because one meaning in two colours breaks one meaning, one colour.

### decision one-set-of-marks-for-how-a-run-stands
How a run ended or stands has one set of marks, filled for a run that ran or runs and hollow for one still to run, which a work's strip of cells and its history both draw.
- replaced: Nine marks, blue, red and light-blue hollow frames among them, stood on main; the owner changed it, because so many shapes could not be remembered.
- replaced: Marks of the history's own, a dash for a run that handed in nothing new and a mark for one given up, stood on main; the owner changed it, because what means one thing is drawn one way everywhere, and the strip's dash already meant skipped.
- replaced: Two strips, the console card's and the window's, the second neither blinking nor marking a stop red, stood on main; the owner changed it, because the same kind of thing is drawn from the same parts.

### decision a-level-looks-as-the-level-it-runs-at
A card, under its number, and the item's window write a cell's level as `L2`, or `L2+1` where its line raises it, coloured by the level it runs at: muted, text, accent, accent and strong.
- replaced: The name of the cell the work stood at, under the card's number, stood on main; the owner changed it, because that place is for the level, and the strip already marks where the work stands.
- rejected: an agent proposed a small grey number after the cell's name, and level 2 and cell level 3 in the window; the owner rejected it, because the cell's place is for the level, which must show the base level and the current one plainly.
- rejected: an agent proposed three looks, the third level in the accent and strong; the owner rejected it, because a level its line raises past the third is a higher one still, which takes the strong look alone.
- rejected: the owner proposed a hue of its own for each level; the owner rejected it, because red, yellow, the accent and green each mean a state already.

### decision the-way-back-is-in-the-accent
A way back is drawn in the accent in every view: a thick line and arrow on the matrix and the flowchart, and in a work's history from the later cell's column to the run it woke.
- rejected: an agent proposed keeping the ways back in a violet of their own, beside a blue accent; the owner rejected it, because the violets clashed with the page.
- replaced: Ways back in grey, told from other lines by their shape alone, stood on main; the owner changed it, because a way back should be in the highlight colour, and one thing in one colour in every view.
- replaced: A line of its own for the way round in a run's panel stood on main; the owner changed it, because the matrix's arrow back already says it.

### decision every-mark-is-in-one-table
Every mark the page draws is defined once, with a name of a word or two, in one table, and each view's legend lists on one line only the marks that view draws.
- replaced: Marks and legends of each view's own, naming one thing several ways, stood on main; the owner changed it, because a legend's words had grown too long for its line, and what was alike differed from view to view.

### decision the-page-draws-markdown-itself
The page draws the Markdown agents write with a renderer of its own, in its own parts, and uses no Markdown library.
- replaced: Markdown shown as it was written stood on main; the owner changed it, because shown so it could hardly be read.
- rejected: an agent proposed a Markdown library; the owner rejected it, because the page needs only a few forms, and nothing an agent writes may become markup the page did not mean.

### decision code-is-coloured-from-prisms-words
The words of code in a diff, and in a fence that names its language, are coloured by their kind, keyword, literal or comment, as Prism reads them, and drawn with the page's own parts.
- rejected: an agent proposed highlight.js; the owner rejected it, because a library may only cut an agent's code into typed words for the page to draw, never write it as markup, which Prism can do.
- rejected: an agent proposed a small highlighter of the page's own for the project's few languages; the owner rejected it, because the outside library's supposed breach of a rule did not hold, and Prism reads more languages and reads them right.
- rejected: an agent proposed Prism's own plugin for code in a diff; the owner rejected it, because the plugin works on the markup Prism writes, which would make an agent's code markup the page did not mean.
- replaced: Code in a Markdown fence left uncoloured stood on main; the owner changed it, because like things look alike.

### decision the-page-draws-diagrams
The page draws the mermaid and dot fences of what agents and the owner write as diagrams, loading each library as a diagram nears the screen, and shows the source of one it cannot draw.
- replaced: A diagram's source shown in place of the diagram stood on main; the owner changed it, because diagrams are key to how he understands a design.

### decision the-page-uses-no-component-library
The page is drawn with a small set of parts of its own, and uses no component library.
- replaced: GitHub's Primer, its tokens and its components, stood on main; the owner changed it, because Primer wastes room on the screen and is not good to look at.

### decision the-look-stays-square-and-cool
Tags, buttons, inputs and the console's cards have corners of 3 px, and panels and frames none, on cool greys; no part is a pane of glass or a pill.
- rejected: the owner proposed trying the Solid Glass style, round panes of glass, controls shaped as pills and soft shadows; the owner rejected it, because the console and the flow are drawings, not ordinary controls, and the round corners sat badly where the cool, square look did well.
- rejected: an agent proposed an instrument look, a grid of dots and square unfilled cards, or a terminal look, a grid of lines and titles in monospace; the owner rejected it, because neither did well, tried behind a switch beside the look as it is.

### decision no-made-up-data-in-the-installed-page
The page built into the program shows the engine's data alone: its made-up data are only on the development server and in the tests.
- replaced: Made-up data built into the program, shown with `?mocks` or `?fixtures` in the address, stood on main; the owner changed it, because the owner's program should hold no false data a parameter away from showing.

### decision no-engine-numbers-on-the-page
The page shows no number the engine keeps a run, a round, a call or a try by, but in an address, and names a moment by its time; a datum's versions show as v1, v2.
- rejected: an agent proposed keeping a run's number only in the hover on its cell's name; the owner rejected it, because a number put on the page must serve a purpose, and this one served none.
- replaced: A run's number and the engine's counters of sessions, calls, tries and rounds on the Step, the usage and the System view stood on main; the owner changed it, because they told him nothing and served nothing.
- replaced: The number of an event in the record, written after an at sign, for the moments of a work stood on main; the owner changed it, because that number could not be read, where the time can.

### decision the-browser-tells-the-owner-what-waits
The open page tells the owner by the browser's notifications of each work that newly waits on him, naming its cell and why, and a click opens the console at that wait's card.
- rejected: an agent proposed a notification by a script on the machine the server runs on; the owner rejected it, because he uses the page in Chrome, and the server runs on his machine only by chance.
- rejected: an agent proposed a push to the owner's phone; the owner rejected it, because the browser's notifications do for now.
- rejected: an agent proposed a notification of the machine's own, which the engine sends, beside the browser's; the owner rejected it, because the browser's notifications are enough.

### decision the-page-moves-whatever-the-system-asks
The page's motion plays whether or not the system asks for reduced motion.
- replaced: Animations stopped under the system's reduced-motion setting stood on main; the owner changed it, because he wants them to move.
- rejected: an agent proposed switching at once, with no motion, where the system asks for reduced motion; the owner rejected it, because the whole setting is superfluous to him.

### decision a-views-choices-are-kept-in-the-browser
How each view shows as the owner left it, its switches, window, order, span and widths, is kept in the browser; each tab leads back to its last address, and a view left is kept, hidden.
- replaced: Choices held in the view's memory alone stood on main; the owner changed it, because every choice went back to its first each time he went to another tab or loaded the page.
- replaced: A tab leading to its first view, the works back at their list, stood on main; the owner changed it, because a work he had opened was lost on coming back, with no way back from it to the list.
- replaced: A view taken down when another tab was shown, read anew and its scroll lost on return, stood on main; the owner changed it, because coming back to the console was a poor experience.

### decision one-window-floats-over-the-right
On the console, the flow and a work's page, one window floats over the view's right, sliding in and out by a tab on its edge, the view keeping its whole width under it.
- replaced: A side column docked beside the chart stood on main; the owner changed it, because opening, closing or widening the panel should not move the chart.

### decision the-accent-is-blue-unless-picked
The page's accent is a blue until the owner picks Indigo, Violet, Purple or a colour of his own in the Settings view.
- replaced: A violet accent stood on main; the owner changed it, because he had picked this blue with the page's picker and wanted it as the default.
- rejected: an agent proposed leaving the picker off main, which would keep a fixed blue; the owner rejected it, because with everything on main he must have the picker to change the colour.

### decision a-picked-accent-turns-the-default
A colour picked as the accent turns every tone of the default accent to its hue and strength, each at the default's lightness, moved only where it would read less well on its ground.
- rejected: an agent proposed taking the picked colour's own lightness; the owner rejected it, because each tone needs a lightness of its own to read well in light and in dark.

### decision an-accent-keeps-clear-of-the-meanings
A colour is refused as the accent where a tone of it is within 45 degrees of hue of red, orange, yellow or green, or weaker than 0.4 of the default's; the refusal names the nearest.
- rejected: an agent proposed taking any colour the owner picks as the accent; the owner rejected it, because one meaning has one colour.
- replaced: Refusing a colour near the teal of the tools' time stood on main; the owner changed it, because the tools' time became yellow, as a call's mark is, and teal then meant nothing.
