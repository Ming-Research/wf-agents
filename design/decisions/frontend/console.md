# The console

The items as a tree in columns.

### decision the-tree-is-drawn-on-a-canvas
The console's tree is drawn in WebGL with PixiJS, only the items on the screen and their titles from one shared set of
glyphs; what an item opens is HTML.
Why: Each item moves and scales by itself and many are drawn sixty times a second: drawn so, some 300 items scaling at
once took 3 to 5 ms a frame on the owner's machine whatever the tree's size, where a texture for each title stalled a
frame for up to 150 ms as items came into view. Items as HTML were turned down, since scaling one makes the browser
lay out and draw its words again each frame, and three.js as a 3D engine, more than a flat tree needs; what an item
opens holds words to select and copy, markdown and answers to type, which only HTML does well.

### decision the-tree-follows-what-depends-on-what
The items stand as one tree along what depends on what: where one item depends on another, the other stands under it,
directly or under an item under it. No second tree is drawn by anything else.
Why: Reading down from an item shows everything it needs, and how far it has got is how far what stands under it has
got. A tree drawn by anything besides would be a second relation to keep in step with the dependencies.

### decision an-item-stands-in-one-place
An item stands whole in one place, under the first by number of the items that depend on it; under each other item
that depends on it, it is only a line that points there.
Why: Shown whole in several places, an item would be counted, read and answered more than once, and each thing has one
place (Article 8).
