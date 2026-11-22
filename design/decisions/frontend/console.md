# The console

The tree of items in columns, their cards and folds, and how the page lights what runs and what the owner points at.

### decision the-console-is-the-old-engines-console
The console's tree is the old engine's console brought over, with its layout, its motion and its constants unchanged; the page's new look is used only around it.
- rejected: an agent proposed restyling the console in the page's new look; the owner rejected it, because its graph shows the dependencies, the progress and the activity at a glance, and even its sliding had been tuned.

### decision the-tree-is-drawn-on-a-canvas
PixiJS draws the console's tree in WebGL on a canvas, only what is on the screen, with the titles from one shared set of glyphs; what an item opens is HTML.
- rejected: an agent proposed drawing the tree in HTML and CSS with React, reusing the flowchart's viewer; the owner rejected it, because the nodes must scale one by one, carry animations and keep 60 frames a second with many of them on the screen.
- rejected: an agent proposed three.js; the owner rejected it, because it needs glyph atlases for thousands of characters and its own picking and layout, and no test can see into it.

### decision the-tree-stands-in-columns
Items stand in columns, one for each level from the roots at the left; each scrolls by itself, and each column next to it glides only enough to keep the items joining the two level near the middle.
- rejected: an agent proposed nested strips, a flame graph; the owner rejected it, because its leaves cannot be seen without zooming.
- rejected: an agent proposed a magnifier over the room each branch gets; the owner rejected it, because a big screen would show little.
- rejected: an agent proposed rectangles packed to the screen's shape; the owner rejected it, because the boxes carry no information.
- replaced: A line across the middle, onto which an item of each column snapped, was on main; the owner changed it, because the snap still jumped, and nothing pulls a block of items away from its parent.

### decision the-tree-follows-what-depends-on-what
The items form one tree by what depends on what, or last did: where one item depends on another, the other stands under it, directly or further down; nothing else shapes the tree.
- rejected: an agent proposed giving each item a parent, besides its dependencies, for the tree to show it under; the owner rejected it, because the parent relation and the dependencies can be one, merged into the dependencies.
- replaced: A flat list of the items by their class was on main; the owner changed it, because a flat list shows no dependencies.

### decision an-item-stands-in-one-place
An item stands whole in one place, under the lowest-numbered item that depends on it; under each other item that depends on it, a shadow of it points there.
- rejected: an agent proposed a count, on the item, of the other items that depend on it; the owner rejected it, because a count does not say which they are.
- rejected: an agent proposed linking the item to the others that depend on it only while the pointer is on it; the owner rejected it, because the link could not be found without pointing at the item.
- replaced: A main dependent chosen for each item was on main; the owner changed it, because the code takes the lowest-numbered one, and a decision never runs ahead of the code.

### decision an-ended-item-stands-where-it-stood
A done or cancelled item that no item depends on any more stands under the item that last depended on it; one that no item ever depended on stands at the top.
- replaced: An ended item that no item depended on any more standing at the top was on main; the owner changed it, because every item that depends on an item stops depending on it as it is cancelled, so the cancelled items, once ticked, filled the screen as roots.

### decision an-item-folds-what-stands-under-it
An item with items under it folds them away at a mark past its card's right edge, its row then counting them by how they stand; it is folded where nothing in it is open, or as the owner chose.
- replaced: The whole tree always drawn, with no way to fold a part of it, was on main; the owner changed it, because the tree grows with every item added, and would keep growing.
- rejected: an agent proposed folding the done rows by default, showing only their count; the owner rejected it, because he often needs to see the overall progress.

### decision a-card-has-a-row-for-its-work
A card has one row, with its number and its title, and a second row only while a work is under way on it, holding the level its cell runs at, the strip of its cells and the lines it changed.
- rejected: an agent proposed two rows on every card; the owner rejected it, because two rows on every card waste room.

### decision a-card-writes-the-lines-its-work-changed-where-its-time-went
A card writes the lines its work added and removed, `+210 −40`, on its second row after the strip, or for an ended item at the right of its one row; where the time went is only in the window.
- replaced: The line showing where the work's time went, on the card's second row, was on main; the owner changed it, because the row had room for only one of the two, and the time's line is already in the window.
- rejected: an agent proposed a taller card with the count under its second row; the owner rejected it, because the second row itself had room.

### decision a-delivery-item-is-a-heading
An item that delivers is not a card: it has no box and no fill, only its number and its title over a line, like a heading.
- rejected: an agent proposed the card as it was, with a small diamond before the title; the owner rejected it, because the diamond changes the least and so stands out the least, while a heading with no frame reads plainly in light and in dark.

### decision a-card-stands-still
No card gets a ring or moves because of its work's state: what runs shows by its running cell's pulse and by the lit way from its item to its root, with a light running down it from the root.
- replaced: Red rings breathing round a card that waits for the owner was on main; the owner changed it, because the red background and its glow looked bad.
- replaced: Red cards breathing was on main; the owner changed it, because the breathing could hardly be seen.
- replaced: A light going round a running card's edge was on main; the owner changed it, because motion should be on the running cell, which carries information, and should take little room.
- rejected: an agent proposed a ripple widening from the running cell's mark; the owner rejected it, because it was harder to see than the pulse, and the cell no longer looked as if it moved.
- replaced: Dashes scrolling along the way from the root to a running card was on main; the owner changed it, because he wanted that way lit by a running light, as in a reference he showed.
- rejected: an agent proposed the lit way in purple, with its line and glow tinged purple at rest too; the owner rejected it, because at rest the way keeps the colour the settings give it, and only the running light may take some purple.

### decision what-the-owner-points-at-stands-in-a-soft-light
The selected item and the item under the pointer stand in a soft light, a blurred glow behind the card in its own colour; the selected one's glow is wider and its ring thicker.
- rejected: an agent proposed a bar at the selected card's left, a light fill, its words in the accent, a thicker ring, a spotlight or a card lifted on a shadow; the owner rejected it, because on a page drawn in WebGL he wanted an effect beyond these.
- replaced: A translucent frame drawn round the card under the pointer was on main; the owner changed it, because the soft light already looked like a hover effect.
- replaced: The same blue round every card that was selected or under the pointer was on main; the owner changed it, because the light should be in the item's own colour, red for red and grey for grey.

### decision the-boxes-choose-the-items-shown
Three check boxes over the tree's corner, Open, Done and Cancelled, one under another, choose which items the tree shows: only Open at first, and the choice is kept in the browser.
- replaced: One switch, Open or All, was on main; the owner changed it, because done or cancelled items alone could not be shown with it.

### decision an-item-is-brought-just-into-view
Selecting an item that is off the screen, or half under the window, moves the tree only until the item is wholly in the part of the view the window leaves free.
- replaced: Moving the tree to put such an item in the middle of that free part was on main; the owner changed it, because moving it to the middle was too heavy.
