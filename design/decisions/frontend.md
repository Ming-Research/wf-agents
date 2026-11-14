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
The views listen to the engine's changes on one stream for the whole page, read again at most once at a time, and
read the items once for the bar and the list of works; an item that did not change keeps the object it was, and its
row is not drawn again.
Why: A browser keeps only a few connections to one host open at once, and a stream for each part that listened took
four of them on the list of works alone. With 500 works a change of one drew the whole list again in about 120 ms of
rendering in a test, and its row alone in about 8.