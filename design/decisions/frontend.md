# The owner's page

What is common to the views of the owner's page.

### decision the-page-draws-markdown-itself
The page draws what agents write with a small renderer of its own, for the few formats it shows, and uses no markdown
library.
Why: The page needs only a few formats, and nothing an agent writes may become markup the page did not mean (principle
agent-writing-is-data); a library was turned down for that.

### decision the-page-uses-no-component-library
The page is drawn with a small set of parts of its own (`client/src/parts.tsx`), and uses no component library.
Why: A library's parts spend room on padding and chrome the owner does not read, and one small set of our own is
learned once; Primer's was turned down for that.

### decision an-engines-halt-is-red
A work the engine halted is marked in the red of a fault, and one the owner paused is not.
Why: Red says there is a problem to look at, and a pause is something someone chose.

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

### decision the-way-back-is-in-the-accent
A way back is drawn in the accent in every view, and its words are not: its line and its arrow on the matrix and on the
flowchart, where it is thicker than an arrow a write runs and stays as it is under the pointer, and a way round's line
and arrow in a work's history. A run's Step shows no way-round line of its own: the history's arrow at its mark
already says it (principle say-a-thing-once).
Why: The way back is what the owner looks for, so it takes a strong mark (principle scarce-emphasis), and one thing has
one colour in every view (Article 8); in the muted colour it read as one more line. A colour of its own was turned down
as one more meaning to learn. What else is in the accent is told from it by shape (judgment): what is chosen is a tint
or an edge, a line's row a row of dots and a run under way its square mark, while a way back is a line along a row or
between cells, with an arrow; any other arrow takes the accent only under the pointer. A dot has no shape to tell it
by, so no dot is ever in the accent for being a way back.
