# The console

The tree of items in columns, their cards and folds, and how the page lights what runs and what the owner points at.

### decision the-console-is-the-old-engines-console
The console's tree is the old engine's console brought over, its layout and its motion as the decisions on its columns, cards and folds say; the page's new look is used only around it.
- rejected: an agent proposed restyling the console in the page's new look; the owner rejected it, because its graph shows the dependencies, the progress and the activity at a glance, and even its sliding had been tuned.

### decision the-tree-is-drawn-on-a-canvas
PixiJS draws the console's tree in WebGL on a canvas, only what is on the screen, with the titles from one shared set of glyphs; what an item opens is HTML.
- rejected: an agent proposed drawing the tree in HTML and CSS with React, reusing the flowchart's viewer; the owner rejected it, because the nodes must scale one by one, carry animations and keep 60 frames a second with many of them on the screen.
- rejected: an agent proposed three.js; the owner rejected it, because it needs glyph atlases for thousands of characters and its own picking and layout, and no test can see into it.

### decision the-tree-stands-in-columns
Items stand in columns, one for each level from the roots at the left; each scrolls by itself, and each column next to it glides only enough to keep the items joining the two level near the middle, and only the way the column it follows moves.
- rejected: an agent proposed nested strips, a flame graph; the owner rejected it, because its leaves cannot be seen without zooming.
- rejected: an agent proposed a magnifier over the room each branch gets; the owner rejected it, because a big screen would show little.
- rejected: an agent proposed rectangles packed to the screen's shape; the owner rejected it, because the boxes carry no information.
- replaced: A line across the middle, onto which an item of each column snapped, was on main; the owner changed it, because the snap still jumped, and nothing pulls a block of items away from its parent.
- replaced: A column next to it that could glide against the column it follows, as the wants near the middle changed, was on main; the owner changed it, because the third column ran up off the screen as he dragged the second.

### decision the-tree-follows-what-depends-on-what
The items form one tree by what depends on what: where one item depends on another, the other stands under it, directly or further down; nothing else shapes the tree.
- rejected: an agent proposed giving each item a parent, besides its dependencies, for the tree to show it under; the owner rejected it, because the parent relation and the dependencies can be one, merged into the dependencies.
- replaced: A flat list of the items by their class was on main; the owner changed it, because a flat list shows no dependencies.
- replaced: A grey padlock on a card with the count of the open items it depends on was on main; the owner changed it, because everything an item waits for stands under it, so the count only said again what the tree shows.

### decision an-item-stands-in-one-place
An item stands whole in one place, under the lowest-numbered item that depends on it; under each other item that depends on it, a shadow of it points there.
- rejected: an agent proposed a count, on the item, of the other items that depend on it; the owner rejected it, because a count does not say which they are.
- rejected: an agent proposed linking the item to the others that depend on it only while the pointer is on it; the owner rejected it, because the link could not be found without pointing at the item.
- replaced: A main dependent chosen for each item was on main; the owner changed it, because the code takes the lowest-numbered one, and a decision never runs ahead of the code.

### decision an-ended-item-stands-where-it-stood
A done or cancelled item stands under the item that depends on it, by the rule every item stands by; the tree works out no place of its own from the items' changes.
- replaced: A cancelled item standing at the top, as the items that depended on it stopped as it was cancelled, was on main; the owner changed it, because the cancelled items, once ticked, filled the screen as roots, so a cancel now keeps the dependencies on an item.
- rejected: the owner proposed drawing an ended item under the item that last stopped depending on it, worked out from the items' changes; the owner rejected it, because once a cancel keeps the dependencies on an item, it would be a second way of placing an item.

### decision an-item-folds-what-stands-under-it
A click on an item's funnel folds it and the items under it into a dot past the card, which ends the lit way of a running item in it; it is folded where nothing in it is open, or as the owner chose.
- replaced: The whole tree always drawn, with no way to fold a part of it, was on main; the owner changed it, because the tree grows with every item added, and would keep growing.
- rejected: an agent proposed folding the done rows by default, showing only their count; the owner rejected it, because he often needs to see the overall progress.
- rejected: an agent proposed a round knob past the card's edge, with a plus or a minus in it, that the lines ran through; the owner rejected it, because it belonged to neither the grey funnel nor the blue lit way, overlapped them, the blue light above all, and its meaning was unclear.
- rejected: an agent proposed counts of what is folded as framed grey tags on the folded card; the owner rejected it, because a folded card should carry nothing new, the counts being said on pointing at the funnel or the dot.

### decision a-card-has-a-row-for-its-work
A card has one row, with its number and its title, and a second row only once its work has run a round and while it goes on, with its level, the strip of its cells and the lines it changed.
- rejected: an agent proposed two rows on every card; the owner rejected it, because two rows on every card waste room.
- replaced: A second row for a work that had run no round, while it was paused, at fault or waited, was on main; the owner changed it, because items brought over that never ran showed a strip and a red age, and looked like works that had stopped partway.

### decision a-card-counts-the-questions-to-answer-ahead
Beside the red count of what holds its work, a card counts in orange its work's questions that hold it at nothing yet, as decision a-question-that-holds-the-work-is-red draws them.
- replaced: A card that counted only what held its work was on main; the owner changed it, because he could not see where something would wait on him until it turned red and the work stopped.

### decision a-cards-strip-shows-every-cell
A card's strip draws every cell of its flow, the marks of the whole tree made smaller by one factor until the longest strip fits, and never larger than on the works page.
- replaced: Marks at the works page's size, the strip cut where the card's room ended with a count of the cells left out, was on main; the owner changed it, because the engineer flow's last cells were hidden behind the count.

### decision a-cancelled-items-title-is-struck-through
A cancelled item's card draws a line through its title, in the title's own colour, so that it is told from a done one.
- replaced: A cancelled item's card that differed from a done one's only by the paler grey of its number was on main; the owner changed it, because a cancelled item could not be told from a done one on the tree.

### decision a-card-writes-the-lines-its-work-changed-where-its-time-went
A card writes the lines its work added and removed, `+210 −40`, on its second row after the strip, or for an ended item at the right of its one row; where the time went is only in the window.
- replaced: The line showing where the work's time went, on the card's second row, was on main; the owner changed it, because the row had room for only one of the two, and the time's line is already in the window.
- rejected: an agent proposed a taller card with the count under its second row; the owner rejected it, because the second row itself had room.

### decision a-delivery-item-is-a-heading
An item that delivers is not a card: it has no box and no fill, only its number and its title over a line, like a heading.
- rejected: an agent proposed the card as it was, with a small diamond before the title; the owner rejected it, because the diamond changes the least and so stands out the least, while a heading with no frame reads plainly in light and in dark.

### decision a-card-stands-still
No card gets a ring or moves because of its work's state: a work under way, in a round or in a call the engine runs, shows by its cell's pulse and by the lit way to its root.
- replaced: Red rings breathing round a card that waits for the owner was on main; the owner changed it, because the red background and its glow looked bad.
- replaced: Red cards breathing was on main; the owner changed it, because the breathing could hardly be seen.
- replaced: A light going round a running card's edge was on main; the owner changed it, because motion should be on the running cell, which carries information, and should take little room.
- rejected: an agent proposed a ripple widening from the running cell's mark; the owner rejected it, because it was harder to see than the pulse, and the cell no longer looked as if it moved.
- replaced: Dashes scrolling along the way from the root to a running card was on main; the owner changed it, because he wanted that way lit by a running light, as in a reference he showed.
- rejected: an agent proposed the lit way in purple, with its line and glow tinged purple at rest too; the owner rejected it, because at rest the way keeps the colour the settings give it, and only the running light may take some purple.
- replaced: A lit way only to a work whose round ran, and none to a work in a call the engine ran for it, such as its tests, was on main; the owner changed it, because he counts a work in such a call as under way.

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

### decision showing-an-item-in-the-tree-flashes-its-card
A click on the item's title in its window brings the item into view and flashes its soft light once, wider and stronger, easing back to rest.
- replaced: A click on the title that only moved the tree was on main; the owner changed it, because with the card already in view beside the window the click did nothing he could see.

### decision an-item-is-found-from-a-box-the-slash-opens
`/` opens a box over the console's top that finds any item by its number or a part of its title, eight at most; a pick selects it, its kind ticked and the folds over it opened.
- replaced: A field always shown under the boxes at the tree's corner was on main; the owner changed it, because what it found had no room under it, and he wanted it only while he searches, as Spotlight is.
- replaced: Finding an item by its path from its root, as 26-27, was on main; the owner changed it, because with the search he needs no path.

### decision what-the-search-finds-is-lit-in-the-tree
While the search is open the tree is dimmed, as round the item under the pointer, but for the items it finds; the row the keys or the pointer stand on moves nothing in the tree.
- replaced: The tree left as it was while the search was open was on main; the owner changed it, because he wanted the tree to dim as the search opens, as Spotlight does, and what it finds to stand out as the item under the pointer does.
- rejected: the owner proposed that the row the keys or the pointer stand on bring its item into view; the owner rejected it, because a key pressed or a pointer passing by mistake moves the tree away from what he looks at.
- replaced: Finding an item only by scrolling the tree was on main; the owner changed it, because once many items were imported he could not find one he looked for.

### decision the-owner-walks-the-stops-by-keys
n goes to the next of what the red and orange counts count, N to the one before, in the tree's order and each window's, round from the last; Next, left of Discuss on each but the last, does as n.
- replaced: A button Next question on a row of its own under Discuss was on main; the owner changed it, because the two buttons go together on one row, and Next is shorter.
- replaced: Finding each thing to handle by looking for a card with a count and scrolling its window was on main; the owner changed it, because he works by those counts, and wanted to go from one thing he handles to the next by keys.

### decision the-map-draws-each-item-and-the-screen
The map draws each item as a piece of its column's bar in the colour of its card's state, and frames in a thin light line the part of the tree the screen shows.
- replaced: Each column as one light bar, with red marks where works waited for the owner, was on main; the owner changed it, because the map should show how each item stands and where the screen is in the tree.
