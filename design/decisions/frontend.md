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
### decision the-accent-turns-the-purple
A colour picked as the accent turns every tone of the page's purple, and the console's own purples, to its hue and
strength (its chroma as a part of the most sRGB holds at its lightness), each tone at the purple's lightness, made
darker or lighter only where it would then read less well on its card or ground than the purple's does; a tint keeps
the purple tint's chroma, which is how far it stands from the card. The page's tokens write each such tone as its own or
the purple, and the purple is read from them, not copied. A pale or a dark colour of a free hue gives the strong accent
of its hue.
Why: The purple's tones were tuned together, which tone is deeper, how faint the tint, the heads over the text, so a
turn keeps every relation; a set made by hand for each hue keeps none for a colour of the owner's own. Taking the
picked colour's own lightness was turned down: each theme needs its tones at their own lightness to read, and the
accent would weigh more or less than red beside it. The relative colour of CSS (`oklch(from …)`) was turned down: it
checks no contrast, each browser fits it into sRGB its own way, and the console's canvas reads only `#rrggbb`.

### decision an-accent-keeps-clear-of-the-meanings
A colour is refused as the accent where a tone it gives, in light or in dark, stands within 45 degrees of hue of a
colour with a meaning of its own (red, orange, yellow, green, the tools' teal, and their text steps), or where it is
weaker than 0.4 of the purple's strength; the refusal names the nearest, and the accent stays as it was. The way back's
violet is not among them: it is the purple's own hue, which the page's own accent has.
Why: One meaning has one colour (principle scarce-emphasis): an accent near green draws a work running as one done,
near yellow a cell running as a call under way, near teal the model's time as the tools', and a grey one draws what
runs as a pause. Moving a refused colour to the nearest free hue was turned down: the nearest of a green is a blue the
owner did not pick. The 45 degrees and the 0.4 rest on judgment; a colour added to the tokens with a meaning of its own
is one more the accent keeps clear of, and the tokens' check fails until it is named.

### decision the-accent-is-kept-in-the-browser
The accent is kept in the browser that picked it, as light or dark is, and is taken as it is picked: it is no setting
of the installation's file, and the Settings view's Save does not write it.
Why: It is how the page looks to the one who reads it there, which the engine never reads (principle
every-field-has-a-reader); in the file, it would make the owner save and commit a look.
