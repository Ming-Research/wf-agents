# The owner's page

What is common to the views of the owner's page.

### decision the-page-draws-markdown-itself
The page draws what agents write with a renderer of its own, for the Markdown they write, and uses no markdown library.
Why: Nothing an agent writes may become markup the page did not mean (principle agent-writing-is-data), and the page
draws Markdown with its own parts; a library was turned down for that.

### decision code-is-coloured-from-prisms-words
The words of code the page draws are coloured by their kind, keyword, literal or comment, as Prism reads them in the
language the file's name or the code's fence names; the page draws the words with its own parts, and each side of a
part of a file a diff changes is read whole.
Why: Prism's markup, and its plugin that colours a diff, which works only on that markup, were turned down: what an
agent writes would become markup the page did not mean (principle agent-writing-is-data). Read line by line, a comment
or a string over several lines would be read wrong past its first.

### decision a-drawing-stands-apart-from-the-pages-styles
A diagram's drawing stands in a shadow root of its own, which no style of the page reaches but the one that sizes it.
Why: Mermaid names the words of its shapes, its lines and a chart's key `label`, `edge` and `legend`, as the page
names parts of its own, and the page's label made every word of a flowchart small, muted and in capitals. Undoing the
page's styles in a drawing one by one was turned down: the next part the page names as mermaid names a class would
reach the drawings again.

### decision the-page-uses-no-component-library
The page is drawn with a small set of parts of its own (`client/src/parts.tsx`), and uses no component library.
Why: A library's parts spend room on padding and chrome the owner does not read, and one small set of our own is
learned once; Primer's was turned down for that.

### decision an-engines-halt-is-red
A work the engine halted is marked in the red of a fault, and one the owner paused is not.
Why: Red says there is a problem to look at, and a pause is something someone chose.

### decision a-call-waited-for-is-no-run
A work that waits at a cell for the results of calls of tools the engine carries out is said, wherever the page says
its state, the run's Step among them, to wait for each tool by its name, where it runs (in CI, linked to the run, or
by the engine) and for how long, in the yellow of a call; never to run or to have finished, and its console card is
one that waits, unfilled, its edge and its light in that yellow. An act of the engine's own under way is a round, and
runs.
Why: Said to run by the item's window and its card and to have finished by the run's panel, while its history said a
call was under way, one work said three things at once and none of them what it waited for (W11).

### decision one-set-of-marks-for-how-a-run-stands
How a run ended or stands has one set of marks, which the strip of a work's cells draws for each cell, as its run
stands, and a work's history for each run; a run whose return was sent back is drawn as `failed`, as one whose last
round failed.
Why: The strip and the history drew the same states in marks and words of their own (Article 8). A mark of its own
for a return sent back was turned down as one more to learn, and a run whose round failed drawn as one at rest as
hiding that it failed; that the two kinds of failing need no telling apart at a glance, the run's Step saying which,
rests on judgment.

### decision a-level-looks-as-the-level-it-runs-at
A console card, under its number, and the item's window, after the item's standing, write the level a work's cell
runs at as its item's level and how far the cell's line raises it, `L2` or `L2+1`, in the look of the level it really
runs at, the two added, past the highest level an item may be at too: the first muted, the second the text's colour,
the third the accent, and every one above the accent and strong. The cell the work stands at is not named beside it;
the window names it by its id before the strip.
Why: The look says how dear a model the cell runs on, so `L2+1` looks as `L3` does, and `L3+1`, which its line asks to
run above the third, looks apart from it. Telling the levels apart by hue was turned down: red, yellow, the accent and
green each mean a state already (principle scarce-emphasis), and no four hues clear of them were found. The strip marks
the cell the work stands at, so naming it as well said one thing twice (Article 8).

### decision a-cancelled-work-is-grey
The mark of a cancelled work, in the list of works and on the console, is grey and never red.
Why: The owner reads a red mark as a fault.

### decision one-stream-and-one-read-of-the-items
Every part of the page, the console too, listens to the engine's changes on one stream, opened again a few seconds
after the browser gives it up; each part reads again at most once at a time. The bar and the list of works read the
items once for both, the console reading its own, and an item that did not change keeps the object it was, so that
its row is not drawn again.
Why: A browser keeps only a few connections to one host open at once, and a stream for each part that listened took
four of them on the list of works alone. With 500 works a change of one drew the whole list again in about 120 ms of
rendering, measured on the owner's machine, and its row alone in about 8.

### decision the-server-is-asked-for-the-levels
To learn whether its server has the items and the works, the page asks for the levels items are added at, never for
the items.
Why: Only a server with the items has the levels, and it answers them without reading a work, in about a
millisecond, while it answers even the head of the items by building every item, 0.04 to 0.1 s with 337 works, on the
way to the page's first view.

### decision a-picked-accent-turns-the-default
A colour picked as the accent turns every tone of the default accent, the console's own among them, to its hue and its
strength, each tone at the default's lightness, moved from it only where it would read less well on its ground than
the default's.
Why: The default's tones were tuned together, so a turn keeps every relation between them; taking the picked colour's
own lightness was turned down, as each theme needs each tone at its own lightness to read; and CSS's relative colour
(`oklch(from …)`) was turned down, as it checks no contrast and the console's canvas reads only `#rrggbb`.

### decision an-accent-keeps-clear-of-the-meanings
A colour is refused as the accent where a tone of the accent it gives, in light or in dark, is within 45 degrees of hue
of red, orange, yellow or green, or where it is weaker than 0.4 of the default accent; the refusal names the nearest.
Why: One meaning has one colour (principle scarce-emphasis): near green a work running reads as done, near yellow a
cell running or the model's time as a call under way or the tools' time, and grey as a pause; moving a refused
colour to the nearest free hue was turned down, as a green's is a blue the owner did not pick. The bounds rest on
judgment, and the console's lit edges and map bars, which carry no meaning, are not held to them.

### decision the-accent-is-kept-in-the-browser
The accent is kept in the browser that picked it, as light or dark is, and is taken as it is picked, apart from the
settings that Save writes into the installation's file, whose part says nothing takes effect before it is saved (the
design of the owner's page, item 28).
Why: It is how the page looks to the one who reads it there, which the engine never reads (principle
every-field-has-a-reader); in the file, a look would be saved and committed.

### decision a-views-choices-are-kept-in-the-browser
How each view shows, as the owner left it (a switch's choice, a window's tab, whether a work's window is out, the order
of the list of works, the span of the system's view, the widths dragged), is kept in the browser and stands when the
view is left and shown again and when the page is loaded again; a choice the address names wins over it. Each tab of
the bar leads back to the address it was last at, such as a work open in the works; the tab shown leads to its first
view, such as the list of works. A view once shown is kept, hidden, while another is shown, the last work's page among
them, so that its scroll, its choices and its window stand; hidden, the console stops drawing and no view takes keys.
Why: Kept only in the view's memory, every choice went back to its first each time the owner went to another tab or
loaded the page. In the installation's file it would be a look saved and committed, as for the accent.

### decision the-way-back-is-in-the-accent
A way back is drawn in the accent in every view, and its words are not: its line and its arrow on the matrix and on the
flowchart, where it is thicker than an arrow a write runs and stays as it is under the pointer, and in a work's history
the line and arrow from a later cell's column, whose run wrote, to the earlier cell's run it woke. A run's Step shows no
way-back line of its own: the history's arrow at its mark already says it (principle say-a-thing-once).
Why: The way back is what the owner looks for, so it takes a strong mark (principle scarce-emphasis), and one thing has
one colour in every view (Article 8); in the muted colour it read as one more line. A colour of its own was turned down
as one more meaning to learn. What else is in the accent is told from it by shape (judgment): what is chosen is a tint
or an edge, a line's row a row of dots, a run under way its square mark and a pause in a work's history a dashed line
in a row half as high, while a way back is a solid line along a row or between cells, with an arrow; any other arrow
takes the accent only under the pointer. A dot has no shape to tell it
by, so no dot is ever in the accent for being a way back.

### decision a-works-past-is-drawn-from-its-outline
A work's page and its item window draw its history, its cells' titles and lines, its card, the owner's talk with its
cells and each run's inputs from the outline of the version it runs on, a work going on and one that ended alike, and
its time bar as decision a-work-keeps-what-its-time-went-to says; the whole flow is given only for a work going on, for
what the owner does with it. A cell of a work that ended with no run takes the strip's dash, with the words "did not
run". A work on no version has a cell for each id its runs name, in the order they first ran, with no title or line;
its card shows what was written for its item as it was added, it shows no talk, and it says that the flow it ran on
was not kept.
Why: Drawn on the flow installed, a work was read by a flow it never ran on: W11's T2 read as a test review skipped,
and the runs of W5, W9 and W10 at D4 had no column (principle the-past-as-it-ran). Drawn from the whole flow while its
version is installed and from the outline after, a work would change on the page when another flow was installed
(principle owner-keeps-their-place); why a cell did not run is the walk's reading on a whole flow, so "skipped" is not
said of it. For a work on no version, titles from the flow installed would name another cell where an id was given
again, as W2's D2, a review of the plan, would be named "Is there a better way"; and its records name no cell's words.
