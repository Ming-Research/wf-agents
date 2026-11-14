# The owner's page

What is common to the views of the owner's page.

### decision the-page-draws-markdown-itself
The page draws what agents write with a small renderer of its own, for the few formats it shows, and uses no markdown
library.
Why: The page needs only a few formats, and nothing an agent writes may become markup the page did not mean (principle
agent-writing-is-data); a library was turned down for that.

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