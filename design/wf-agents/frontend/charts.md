- There is no separate web page for reviewing the state machine: the flow page in the service is made good to use. With
  a separate page, one must still go and look at the flow page after reviewing and changing.

- The flow page shows one graph at a time, each apart, switched beside the version's drop-down. Drawn together the chart
  is too large.

- The flow chart follows the design system in looks and not in drawing: layout, cell shapes, lines and stage frames are
  drawn by its own logic; text, borders, backgrounds and category colours take Primer's values, and what Primer lacks is
  listed in one place with its reason. Primer has no flow chart component.

- For layout a cell and its attachments (the circles back to it, its rule box, actions and interjection mark) go to the
  layout engine as one whole, placed inside by the page's own code, the same for every cell. Attachments then sit the
  same on every cell, and the layout engine has far fewer things and lines to lay out. Nested groups in the layout
  engine were rejected: stage frames are a layer of groups already, three levels handle lines across layers badly, and
  inside each group it would decide differently. Attachments as small nodes of their own were pulled far away, since it
  does not know whom they belong to.

- Only agent cells are square, and the square's colour tells its session, one colour to a session. A coloured square
  reads as an agent cell of its own, so a tool cell drawn so cannot be understood.

- Engine cells and tool cells are drawn apart, both with round corners: a tool cell a quiet yellow, an engine cell grey,
  and purple kept for the architect. An engine step is worked out on the spot, while a tool cell waits on the outside,
  may fail and records output, so they really differ. The engine is no role, hence a neutral grey, and round corners
  already set them apart from square agent cells.

- Colour goes only to the numbered cells the flow passes through, tool cells included; what sits on lines and
  attachments such as rules are a light grey. Otherwise the chart is too busy.

- The numbered cells the flow really reaches have thick borders and bold titles, and the other shapes around them thin
  ones. Otherwise the formal cells and those around them look too alike.

- A cell that opens a new session every time shows it by a badge, not by a dotted or dashed border, and dashes are kept
  for more important marks. Dotted and dashed borders are hard to see, and tool and engine cells cannot have an agent
  and differ in colour anyway.

- The badge of reasoning effort at a cell's top left has a background for each level: low green, medium blue, high
  orange, xhigh red, max purple, with white letters, except high in the text colour, since white does not read on it in
  the light theme. One purple in shades with dark letters was a black blur, and high and xhigh must be told apart at a
  glance.

- Text in a box is two sizes larger, with little padding. That reads far better with the box hardly larger, and the
  layout is sparse and has room.

- Lines are not thick and black, which is ugly; arrows do not cross circles, are of moderate size, and do not join
  sideways when the line is very near its target.

- The label on a line carries the name of the way out with no number. Opening a cell shows only the way's name, so a
  number on the chart means nothing.

- A label on a line stays near the line's start. The owner generally reads these when looking at a cell's ways out, and
  a label in the middle of a long line runs off the screen.

- A line clicked stays lit, not persisted, and is gone on reload. It helps to follow a long arrow.

- On a chart showing the route taken, what was not reached greys as a whole, with what hangs beside it. Greying only a
  part looks odd.

- A running cell has a large, coloured, moving glow, on cells without an agent such as tools as well; it stays on the
  current cell, turning, does not flash, and does not jump back and forth on changing cells; a paused cell has a mark. A
  background animation was too inconspicuous.

- Every extra mark on the chart, badge, boldness, colour or text, is explained. The legend's question mark keeps its
  place when opened, the legend shows above it, and pressing again or elsewhere closes it. Unexplained, the owner cannot
  read those marks.

- An action the engine does on a line is drawn as a node on the line, whose hover says what it does. If the engine's
  logic cannot be seen, the flow is hard to understand.

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
  going to the main block. Long arrows only mess up the chart.

- Line colours need only differ within a graph, and may repeat between graphs; where lines of different graphs meet in a
  session, a slight difference is enough. The graphs are viewed apart.
