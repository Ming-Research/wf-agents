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
