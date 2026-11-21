# The console

The items as a tree in columns, their cards, and how what runs and what the owner points at are lit.

### decision the-console-is-the-old-engines-console
The console's tree is the old engine's console brought over, its layout, its motion and its constants as they were, and the page's new look is taken only around it.
- rejected: an agent proposed a reskin of the console in the page's new look; the owner rejected it, because its graph shows the dependencies, the progress and the activity at a glance, and even its sliding had been tuned.

### decision the-tree-is-drawn-on-a-canvas
The console's tree is drawn by PixiJS in WebGL on a canvas, only what is on the screen and the titles from one shared set of glyphs; what an item opens is HTML.
- rejected: an agent proposed drawing the tree in HTML and CSS with React, reusing the flowchart's viewer; the owner rejected it, because the nodes must scale one by one, carry animations and keep 60 frames a second with many of them on the screen.
- rejected: an agent proposed three.js; the owner rejected it, because it needs glyph atlases for thousands of characters and picking and layout of its own, and no test can see into it.

### decision the-tree-stands-in-columns
The items stand in columns, one a level from the roots at the left, each scrolled by itself, and each column beside it glides only as far as keeps the items it joins near the middle level.
- rejected: an agent proposed nested strips, a flame graph; the owner rejected it, because its leaves cannot be seen without zooming.
- rejected: an agent proposed a magnifier over the room each branch gets; the owner rejected it, because a big screen would show little.
- rejected: an agent proposed rectangles packed to the screen's shape; the owner rejected it, because the boxes carry no information.
- replaced: A line across the middle on which an item of each column snapped was on main; the owner changed it, because the snap still jumped, and nothing pulls a block of items away from its parent.

### decision the-tree-follows-what-depends-on-what
The items stand as one tree along what depends on what: where one item depends on another, the other stands under it, directly or under an item under it; nothing else draws a tree.
- rejected: an agent proposed a parent for each item beside its dependencies, under which the tree would show it; the owner rejected it, because the parent relation and the dependencies can be one, merged into the dependencies.
- replaced: A flat list of the items by their class was on main; the owner changed it, because a flat list shows no dependencies.

### decision an-item-stands-in-one-place
An item stands whole in one place, under the first by number of the items that depend on it; under each other item that depends on it, a shadow of it points there.
- rejected: an agent proposed a count on the item of the other items that depend on it; the owner rejected it, because a count does not say which they are.
- rejected: an agent proposed joining the item to the others that depend on it only while the pointer is on it; the owner rejected it, because the join could not be found without pointing.
- replaced: An item's main dependent chosen for it was on main; the owner changed it, because the code takes the first by number, and a decision is never ahead of the code.

### decision a-card-has-a-row-for-its-work
A card has one row, its number and its title, and a second only while a work goes on on it, holding the level its cell runs at, the strip of its cells and the lines it changed.
- rejected: an agent proposed two rows on every card; the owner rejected it, because two rows on every card waste room.

### decision a-card-writes-the-lines-its-work-changed-where-its-time-went
A card writes the lines its work added and removed, `+210 −40`, on its second row past the strip, an ended item's at the right of its one row; where the time went is in the window only.
- replaced: The line of where the work's time went, on the card's second row, was on main; the owner changed it, because the row had room for one of the two, and the time's line is in the window already.
- rejected: an agent proposed a taller card with the count under its second row; the owner rejected it, because the second row itself had room.

### decision a-delivery-item-is-a-heading
An item that delivers is no card: it has no box and no fill, only its number and its title over a line, as a heading.
- rejected: an agent proposed the card as it was with a small diamond before the title; the owner rejected it, because the diamond, changing least, stands out least, where a heading with no frame reads plainly in light and in dark.

### decision a-card-stands-still
No card is ringed or moved for its work's state: what runs shows by its running cell's pulse and by the way from its item to its root, lit, with a light running down it from the root.
- replaced: Red rings breathing round a card that waits for the owner was on main; the owner changed it, because the red ground and its glow looked bad.
- replaced: Red cards breathing was on main; the owner changed it, because the breathing could hardly be seen.
- replaced: A light going round a running card's edge was on main; the owner changed it, because motion should fall on the cell that runs, which carries information, and take little room.
- rejected: an agent proposed a ripple widening from the running cell's mark; the owner rejected it, because it was harder to see than the pulse, and the cell no longer looked as if it moved.
- replaced: Dashes scrolling along the way from the root to a running card was on main; the owner changed it, because he wanted that way lit by a running light, as in a reference he showed.
- rejected: an agent proposed the lit way in purple, its line and glow at rest tinged too; the owner rejected it, because at rest the way keeps the colour the settings give, and only the running light may take some purple.

### decision what-the-owner-points-at-stands-in-a-soft-light
The item selected and the item under the pointer stand in a soft light, a blurred glow behind the card in its own colour, the selected one's wider and its ring thicker.
- rejected: an agent proposed a bar at the selected card's left, a light fill, its words in the accent, a thicker ring, a spotlight or a card lifted on a shadow; the owner rejected it, because on a page drawn in WebGL he wanted an effect beyond these.
- replaced: A translucent frame drawn round the card under the pointer was on main; the owner changed it, because the soft light already looked like a hover effect.
- replaced: One blue round every card selected or under the pointer was on main; the owner changed it, because the light should be the item's own, red for red and grey for grey.

### decision the-boxes-choose-the-items-shown
Three boxes over the tree's corner, Open, Done and Cancelled, one under another, choose which items the tree shows: Open alone at first, kept in the browser.
- replaced: One switch, Open or All, was on main; the owner changed it, because done or cancelled items alone could not be shown with it.

### decision an-item-is-brought-just-into-view
An item selected off the screen, or half under the window, moves the tree only until it is wholly within what the window leaves.
- replaced: Moving the tree to put such an item in the middle of what is left was on main; the owner changed it, because moving it to the middle was too heavy.
