- The flow's chart is laid out by elkjs and drawn as SVG by the page's own components, each node a component. A node
  carries overlays, such as the cell a work is at and how many rounds it ran; the chart draws what a graph library does
  not (stage boxes round their cells, arrows at right angles with rounded bends and an arch where two cross, circles for
  rows to another part, rules and actions beside their cells, the stores) and must move and zoom as the other charts do,
  which a library's own handling of the wheel and keys does not allow. Mermaid gives only a fixed picture, so it draws
  no flow chart, and a library such as React Flow was rejected.

- There is no separate web page for reviewing the state machine: the flow page in the service is made good to use. With
  a separate page, one must still go and look at the flow page after reviewing and changing.

- Comparing with a flow read at a git ref shows, above what changed, what installing it would do with each work going
  on, grouped by what happens to it, those that cost something open; it is read once as the comparison opens and not
  kept current, so that the list does not move under the owner's eyes; and a flow that cannot be judged says why and
  what judges it instead, since a missing cost must not read as no cost. The owner approves a change to the flow with
  its cost before it, which only the command line gave.

- The flow page shows one graph at a time, each apart, switched beside the version's drop-down. Drawn together the chart
  is too large.

- The flow chart follows the design system in looks and not in drawing: layout, cell shapes, lines and stage frames are
  drawn by its own logic; text, borders, backgrounds and category colours take Primer's values. Primer has no flow chart
  component.

- For layout a cell and its attachments (the circles back to it, its rule box, actions and interjection mark) go to the
  layout engine as one whole, placed inside by the page's own code, the same for every cell. Attachments then sit the
  same on every cell, and the layout engine has far fewer things and lines to lay out. Nested groups in the layout
  engine were rejected: stage frames are a layer of groups already, three levels handle lines across layers badly, and
  inside each group it would decide differently. Attachments as small nodes of their own were pulled far away, since it
  does not know whom they belong to.

- A row to a node above its cell leaves the cell's top and one from below enters its bottom, running only up or only
  down, the layout given a row that runs up already turned round. A row the layout turned round itself to break a loop
  came out of the cell's bottom, round the cell and up in a tangle.

- Only agent cells are square, and the square's colour tells its session, one colour to a session. A coloured square
  reads as an agent cell of its own, so a tool cell drawn so cannot be understood.

- A cell a work waits on, an agent's or a tool's, has a colour, and an engine cell is grey: a work stays a while on a
  cell that waits, so such a cell shows wherever where a work stands is shown, such as the stage bar, while an engine
  cell is a judgement made in an instant, on which no work stays, and shows on the chart alone. The engine is no role,
  hence a neutral grey.

- A tool cell, coloured since a work waits on it, has round corners and one fixed quiet yellow, and purple is kept for
  the architect: an agent cell is square in its session's colour, and a tool cell waits on the outside, may fail and
  records output, unlike an agent's round.

- Colour goes only to the numbered cells the flow passes through, tool cells included; what sits on lines and
  attachments such as rules, and the stage boxes, are a light grey, the stage boxes rather than the palest warm colour,
  so that the cells stand out on them. Otherwise the chart is too busy.

- The numbered cells the flow really reaches have thick borders and bold titles, and the other shapes around them thin
  ones. Otherwise the formal cells and those around them look too alike.

- A rounded cell's corners are no rounder than the half-circle ends of a cell of one line, so that every line of its
  text lies inside it: drawn with half-circle ends, a tall cell's text ran out past them.

- A cell that opens a new session every time shows it by a badge, not by a dotted or dashed border, and dashes are kept
  for more important marks. Dotted and dashed borders are hard to see, and tool and engine cells cannot have an agent
  and differ in colour anyway.

- The badge of reasoning effort at a cell's top left has a background for each level: low green, medium blue, high
  orange, xhigh red, max purple, with white letters, except high in the text colour, since white does not read on it in
  the light theme. One purple in shades with dark letters was a black blur, and high and xhigh must be told apart at a
  glance.

- Text in a box is two sizes larger, with little padding. That reads far better with the box hardly larger, and the
  layout is sparse and has room.

- A stage's name is a step larger than the titles of the cells in it, so that it stands apart from them: at the same
  size it did not.

- Lines are not thick and black, which is ugly; arrows do not cross circles, are of moderate size, and do not join
  sideways when the line is very near its target.

- One colour to a meaning on a chart (frontend/design-system.md, "The palette is bright"): the main path is blue at an
  arrow's width rather than a heavier grey; the rows into the cell picked are green and out of it red, the page's
  success and danger colours, their labels filled in those colours with white letters; and the row pointed at is only
  heavier, keeping its colour, so that a row lit green or red stays green or red. A work's chart mutes what it never
  took, so the blue of the main path does not meet the blue of the rows a work took. A heavier grey main path and an
  orange row pointed at were the paths that failed.

- The main path is drawn along a cell's own answers alone, never a move such as a search, though its row is marked main,
  since a move is on no cell's way through the flow.

- The label on a line carries the name of the way out with no number. Opening a cell shows only the way's name, so a
  number on the chart means nothing.

- A label on a line stays near the line's start. The owner generally reads these when looking at a cell's ways out, and
  a label in the middle of a long line runs off the screen.

- A line clicked stays lit, not persisted, and is gone on reload. It helps to follow a long arrow.

- A row drawn into several cells lights only where it is pointed at: a line is lit to follow it (frontend/charts.md, "A
  line clicked stays lit"), and a row lit in every cell it reaches could not be followed.

- The cell picked, or the cell under the pointer, lights its rows in and out, a row through actions or a circle lit
  whole, and the arrows of rules and of what the cell does every time not lit; another cell hovered shows its own rows
  alone, so that two cells' rows never mix, and a click on the background leaves the cell picked, its panel staying
  open. The owner follows a cell's ways in and out across the chart.

- A round chosen lights the arrow it came in by in green and the one it left by in red, flowing the way the work went:
  the arrows a work took again and again are many and alike, and the owner could not tell which a round went by.

- On a chart showing the route taken, what was not reached greys as a whole, with what hangs beside it. Greying only a
  part looks odd.

- A work is drawn on its own flow version, its rounds placed on their cells, since a work runs only on the version it
  stands on and the current flow may no longer have its cells.

- On a work's chart the rows it took flow the way it went, and every motion on the charts moves whatever the browser's
  setting for less motion: the owner follows the path a work took and wants it to move.

- A work's chart opens at full size centred on the cell the work is at, and centres again only when a stage starts to
  work at another cell, never when the page reads the work again: the owner opens a work to read where it is, and a
  chart opened too small hides it; watching, the owner follows where it works.

- A running cell has a large, coloured, moving glow, on cells without an agent such as tools as well; it stays on the
  current cell, turning, does not flash, and does not jump back and forth on changing cells; a paused cell has a mark. A
  background animation was too inconspicuous.

- Where no stage works at the cell a work is at, a still tag beside it says in a few words why it waits, in full on
  hovering, in place of two bars; a round too short to be seen is shown by the arrow taken and the count at its cell,
  not by a glow. A work standing on a tool cell for half a minute with no sign of why left the owner guessing.

- Every extra mark on the chart, badge, boldness, colour or text, is explained. The legend's question mark keeps its
  place when opened, the legend shows above it, and pressing again or elsewhere closes it. Unexplained, the owner cannot
  read those marks.

- The legend is no taller than the chart's room under the tools in its corner and scrolls inside: a legend that grew
  upward past the window's top could not be read from its start.

- The controls over a chart are one floating surface, with the chart's map under it in the same corner, since the chart
  is read under them and a group of controls is one thing to the eye; each stage's box on the map carries its stage's
  letter, since boxes of one grey alone do not let a stage be found at a glance.

- Every chart has a small map in its top left corner: the whole chart in its cells' colours with no text and no arrows,
  the part in view framed, a click or drag moving the chart, the work's cell ringed and what it never reached muted; it
  hides while the whole chart is in view, and the zoom shows under it or in its place. The owner wants to see where the
  view is on the whole chart and go to another part at once. Finding a part only by showing the whole and going back was
  the path that failed, and arrows on the map run a few pixels between cells at that scale.

- An action the engine does on a line is drawn as a node on the line just above the cell the line goes into, shared by
  the lines with the same actions into that cell, whose hover says what it does; what every cell of a line does is in
  the legend, naming the line. If the engine's logic cannot be seen, the flow is hard to understand; actions in the
  middle of a line were read as part of another line, and the same action beside each of a line's cells made the chart
  busy and told nothing new.

- Opening a new session at a fixed place is a node on the line like other actions, and a session changed on occasion is
  explained in that round's record. A sentence on each cell would only repeat, and only what is special needs marking.

- The engine's fallback is not drawn on the chart; it is listed apart in the table of ways out. It is the same for every
  cell, so drawn it says nothing.

- What agent cells can do for themselves is drawn by how many can: what all can is only in the legend; what most can,
  the legend says except, with a small mark on the exceptions; what few can, a line from each to where it goes. An
  ability every cell has need not be drawn on each, and one only a few have should be a real line. A line from each cell
  to a common node, and a circle beside each, were rejected.

- No long arrows go to the stores: what a cell reads and writes is beside it, as small grey rounded blocks saying
  project, item or work, with dashed and solid lines and the arrow's direction showing reads and writes, and a click
  going to the main block. The stores are drawn beside the graphs in three boxes by what they belong to, and pointing at
  a cell, the actions on a line or a store marks the stores it reads or writes, since the owner could not see from the
  chart where what the engine writes is kept. Long arrows only mess up the chart.

- Line colours need only differ within a graph, and may repeat between graphs; where lines of different graphs meet in a
  session, a slight difference is enough. The graphs are viewed apart.
