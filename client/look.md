# The page's look

The owner's page is an instrument panel. The owner watches and steers many works on it all day, so it is dense, quiet
and exact. This file is the start of the page's spec: the tokens (`src/styles/tokens.css`), the parts (`src/parts.tsx`,
`src/styles/parts.css`) and the rules, each with its reason. The page uses no component library: a library's parts
spend room on padding and chrome the owner does not read, and one small set of our own is learned once.

The values follow the console's graph of items on `main` (`client/src/console/snap.ts`, its `PALETTE`, and
`client/src/styles/tokens.css` there), the view the owner likes best. The views that come over next will sit with this
one as one page.

## Rules

- **Dense.** A 4px grid (2, 4, 8, 12, 16). The bar is 36px, the view's line 28px, a row 26px, a control 24px. The owner
  reads many works and many blocks at once, and every row spent on chrome is a row of content lost.
- **Full width.** No centred column, no cards round what is already in a table. A chart and a table need every column.
- **Four sizes, two weights.** 11px for labels and tags, 12px for secondary words and everything in monospace, 13px for
  body text, 15px semibold for the page's and a panel's title. Weights 400 and 600. Numbers are tabular. A few sizes far
  enough apart read as an order at a glance.
- **Monospace for names and values.** Ids, block names, values and versions are in monospace. They are what the owner
  types and compares letter by letter.
- **Colour only for state.** A colour marks a state or a mark, never decoration, so that a colour always means one
  thing. Marks and tags take the colour itself; small text takes its darker text step; a fill takes its tint.
- **Hairlines, no shadows.** Lines are 1px in the line colour. Corners are 3px on tags, buttons and inputs, none on
  panels and frames. Depth is not information.
- **Quiet motion.** Hovering changes the background by one step in 100ms. Nothing else moves, so that what changes on
  the page is the owner's doing or a real change.
- **Focus always shows.** A 2px accent outline, so that the page can be used with the keyboard.
- **Words.** Sentence case, no full stops in labels. A word where a word is clearer than an icon. The few signs the
  chart uses (people write, the outside writes, shown) are explained in the legend.
- **Light and dark.** The system's appearance, or the one picked with the bar's switch, kept in the browser.
- **Widths are the owner's.** An edge that divides two parts (the panel from the chart, the names' column from the
  cells, a cell's column from the next) drags, shows a hairline in the accent under the pointer, keeps its width in the
  browser, and goes back to the width the page works out on a double click. Cut words can then be read in full where
  they stand.
- **Say a thing once.** What belongs to the whole view (the flow's limits) is said in the view's line, and a panel says
  only what differs from it (a cell's own limit).

## Tokens

| Token | Light | Dark | For | From the console |
| --- | --- | --- | --- | --- |
| `--ground` | `#f6f7f9` | `#0f1012` | what the page is laid on: the bar, the chart's heads, the frame | `PALETTE.page` |
| `--card` | `#ffffff` | `#17181b` | what is read on the ground: the chart's rows, the panel | `PALETTE.card` |
| `--text` | `#1f2937` | `#e6e7ea` | words | `PALETTE.ink` |
| `--muted` | `#6b7280` | `#8b8f98` | facts, ids, labels | `PALETTE.muted` |
| `--line` | `#e3e6ea` | `#26272b` | hairlines | `PALETTE.hair` |
| `--accent` | `#8b5cf6` | `#a78bfa` | what is chosen, focus, the panel's handle, an edge under the pointer, the signs | `PALETTE.chosen`, `--console-chosen` |
| `--accent-text` | `#7c4ddb` | `#a78bfa` | links and the lines' names: the accent where it reads on white | ours |
| `--accent-tint` | `#f1ecfe` | `#241d38` | the row or column chosen | `--console-chosen-wash` |
| `--green` | `#2f9e6e` | `#45c08a` | put in, done | `PALETTE.green` |
| `--orange` | `#f5840c` | `#ff9a3c` | changed, waits for the owner | `PALETTE.asked` |
| `--blue` | `#1a9fff` | `#4fb4ff` | running, at rest between rounds | `PALETTE.running` |
| `--red` | `#e5484d` | `#ff6b70` | taken out, failed | `PALETTE.red`, `PALETTE.halted` |
| `--violet` | `#8250df` | `#ab7df8` | the way back | the flow chart's purple (Primer's done) |
| `--*-text` | darker steps | the colours | the colours as small text on white (4.5:1): green, orange, blue, red | ours |
| `--*-tint` | 10% over the card | 10% over the card | a fill that says the colour's meaning | `TINT` = 0.09 |
| `--on-colour` | `#ffffff` | `#0f1012` | words on a filled tag or button | the white on a count tag |
| `--step` | 4% black | 5% white | one step of hover | ours |

The dark values are ours: the console has no dark ground. They keep its relations: the card a step lighter than the
ground, the colours lifted to read on it.

Sizes: `--size-label` 11px, `--size-small` 12px, `--size-body` 13px, `--size-title` 15px; `--font` and `--mono` are the
system's text and monospace families. On the works page, what a work waits for grows to `--waits-height` and an opened
content to `--content-height` before each scrolls, so that the history keeps its room. The chart's own sizes (`--chart-*`) are named in `tokens.css` with what each is
for; the widths of its columns, which the owner drags, are in `Chart.tsx` (`WIDTHS`), and the panel's, one width for the panel of every view, in `sizes.ts` (`PANEL`).

The lines' names (engineer, designer, review...) are in the accent wherever they stand, so that the cells sharing a
session are seen at a glance; "engine" stays muted.

The way back keeps the purple the flow chart draws it in, a different purple from "chosen", which is the console's. On
the flow page the two never meet in one form: what is chosen is a tint over a whole row or column, and a way back is a
drawn line with an arrow.

## Parts

- **Bar** (`Bar`): the page's name, the views as text tabs (the current one in the text colour over a 2px accent line,
  the others muted), and at its right what belongs to the page as a whole: the switch of light and dark. The tabs are
  Console, Works and Flow, each a hash of the address (`#/console`, `#/works`, `#/flow`; a work is `#/works/<number>`,
  under the Works tab), so that back, forward, a reload and a link show the same view; nothing, or a hash the page does
  not know, shows the works.
- **View's line** (`.view-line`): one thin row under the bar that belongs to the view: what it shows, one muted sentence
  of the facts of the whole view (for the flow, its limits), its legend and its own switches. It wraps to a second row where the window is too narrow, the facts giving
  way first.
- **Button** (`Button`): words in a hairline box, 24px; `primary` filled with the accent, for the one action a view
  exists for; `submit` for the button that sends its form. A button that cannot be pressed yet, or whose act is on its
  way, is `disabled`: on the ground, muted, with no hover, so that nothing is sent twice.
- **Segmented switch** (`Switch`): two or three choices side by side, the one chosen on the card. For a choice that is
  always set, such as a version or the theme.
- **Tag** (`Tag`): a few words or a count of a state, filled in its colour, 16px high, monospace, 3px corners, as the
  console's count tags on a card.
- **Label** (`Label`, `.label`): 11px, upper case, letter-spaced, muted: what a column or a value is.
- **List row** (`ListRow`): one thing per row, at least 26px, a hairline under it.
- **Panel** (`Panel`): docked at the right of a view, 360px until dragged (280 to 720px), a hairline at its left that
  drags its width, laid out as the console's drawer: a handle on its edge, there from the start, that folds it away and
  brings it back, the view taking the whole width while it is folded; with nothing chosen it says what to choose; a head with the thing's id (monospace, muted) and title
  (15px semibold) on one line and what it belongs to at the right; one or more status lines; its sections; room at its
  bottom (`foot`) for a box that answers. Escape lets go of what it shows.
- **Status line** (`StatusLine`): a small dot in the colour of what it says, a word in semibold, the facts after it in
  muted text. The dot is green, red, orange, blue for running, or the way back's purple for a way round.
- **Section** (`PanelPart`): a 12px semibold label with its count, over what it holds, 16px apart; `last` ends a group
  of sections with a hairline.
- **Sign** (`Sign`): a 12px line drawing in the accent, for what a word would crowd: people write, the outside writes,
  shown. Muted, they were too faint to find.
- **Edge** (`Edge`): the dragged edge of a part, as the rule on widths says.
- **Box** (`textarea`, `.select`): a hairline box on the card with 3px corners, as a button; a choice is a native
  select, 24px, which starts unset ("Pick a value") wherever the owner's pick is what is sent.
- **Read failed** (`.read-failed`): one line in red on the red tint under the view's line, when a view could not be read
  again; what is shown stays, from before, and so does what the owner typed.
- **Content** (`Opener`): a link, "Show content" or "Show result", that reads a content only when the owner opens it and
  shows it under the link as it is: preformatted monospace on the ground, in a hairline frame, wrapping, scrolling past
  its height. A content is read once; it does not change.

## The works page

What the owner reviews every work by, so that it says plainly why a work is where it is and what it waits for.

- **State mark** (`StateMark`): how a cell stands for a work, as an 8px square with 2px corners: filled green when its
  record knows everything (done), filled blue while a round of its run is under way, a blue ring while its run is at
  rest, an orange ring when it has a record and runs again, a hairline ring before its first run, a muted dash when
  it is skipped. A square, so that it is not taken for the flow chart's ring and dot, which mean reads and writes.
- **Strip** (`Strip`): a work's state marks, one per cell of its flow in the order of the walk, 2px apart; each says
  the cell, its state and how many runs it finished on the pointer's tip.
- **Now line** (`NowLine`): what a work does now, a dot and its words cut to one line, whole on the pointer's tip. The
  dot is blue while a round is under way, orange with orange words where it waits for the owner, red with red words for
  a fault, green once merged, muted otherwise; "paused" follows in a label where the owner paused a work and what it
  does now says something else (a round still under way, or what it waits for), and nowhere else.
- **List of works** (`WorksTable`): the one work of every item, one row of at least 26px each under a sticky head of
  labels: the item's number (monospace, muted), its title in semibold, the now line, the strip, what it has taken, and when something last happened. Works that wait for the owner
  come first, edged in orange at their left; then a fault, what goes on by itself, what is paused, and what has ended,
  each by what happened last. The whole row opens the work. Below 760px the strip and what was taken give way and the
  now line wraps to two lines; below 480px the time gives way too.
- **A work's line**: the view's line holds the way back to the works, the item's number and title, the now line, what
  was taken in all, and the buttons Pause or Let on, and Cancel, which asks again in words before it sends.
- **Waits** (`WaitsBand`): under the view's line, each thing the work waits for on a line of its own, in full: "Waits at
  `cell` for", the words, and what the owner can do there: a question's own words and a box to answer it, a primary
  button to let a run or a cell go on, a precondition's words. A line waiting for the owner is on the orange tint, edged
  in orange; a fault is on the red tint. A refusal from the interface is said in red after what was refused.
- **Write box** (`WriteBox`): at the foot of the work's panel, whatever it shows: the block to write (those people
  write), the value where it is a choice, the words, and under them which cells read the block, since those run again
  for it. Until the owner types or picks anything, it follows the cell chosen, the cell of the step chosen, or the cell
  the work is at, to the opinions that cell reads, and to no block where it reads none people write; from the first key
  or pick it holds the block it shows, whatever is chosen or let go, until what was written is taken. A choice's value
  starts unset, and Write waits for one. What is typed stays while other steps are chosen and while the work is read
  again.
- **Acts**: every control that sends something (Answer, Let go on, Pause, Let on, Cancel, Write) is disabled from its
  press until the answer comes; a refusal is said in red, as an alert, where the act was. Each thing the work waits for
  keeps its own box by what it is (a call by its number, a precondition by its test, a limit by its cell), so that an
  answer typed stays with its question.
- **Live**: the list and a work are read again on each change the interface sends, and whenever its stream of changes
  opens again, since changes may have been missed while it was lost; an answer older than the one shown is dropped.
- **Tabs kept**: the flow, once shown, is kept as the owner left it while another tab is shown, and is not read again.

## The history of a work

The flow chart's grid turned to time: the cells across in the order of the walk, the steps down in the order they
happened, the oldest at the top as the walk goes down the flow's chart, opened scrolled to the newest, which it follows
while the owner stays at the end. The same frame, sticky heads, column widths the owner drags, hairlines, tints for
what is chosen and under the pointer, and marks where they mean the same.

- A cell's head: its id in monospace, muted; its title in semibold; its state mark and state in the state's colour,
  with how many runs it finished.
- The column at the left: the step's time in monospace, muted (with the day where it changes), then its words: the
  run's number and why it ran ("plan had no record", "for plan", or, in the way back's purple, "way round for
  opinions"), and for a run of a cell the flow no longer has, which stands in no column, that cell's name and so; for
  any other step, who did what ("a person wrote opinions_plan", "the outside wrote main", "the owner paused the work"),
  a write by a person or the outside with its sign. A cancel is in red, a pause and a let-on muted.
- A run, in its cell's column: an edge at its left in its state's colour (green finished, red and dashed abandoned,
  blue under way on the blue tint, blue and dotted at rest), its state word and how many rounds, how long and what it
  cost, and the blocks it wrote in monospace, each line cut to the column, all of it on the pointer's tip.
- A write: the flow chart's ring in the column of each cell that reads the block, a diamond where a cell's condition
  reads it. A let-go: "let go" in the accent in the column of the cell let go.
- A way round: the way back's line, in its purple, along the run's row from the furthest column of the cells whose
  writes it ran for, with a dot at each of them, to an arrow at the run; none where no such cell can be found. A run
  is a way round only when every one of its causes was written by a cell after it.

The panel of a step, docked at the right as the flow page's, and like it with the write box at its foot:

- A run: its number, its cell's title and id; status lines for how it stands and how long it took, what it took, and
  whether it is a way round; then why it ran, each cause with its block (a way round's in purple); what it started
  from, each block's version (`plan@12`) opening its content; each round, its times in its label, as pairs: what it
  was given (its task, the results of its calls, each opening its result, or each reason its return was sent back),
  what came back (what it asked for, or each block handed in with its value, whether the run said it matters,
  its length and its content), and what it took; then its calls, each with what it was, when it was made and done, and
  its result, a call of an abandoned run with none said to be withdrawn with it; and how it ended, with each block it
  wrote and what the engine made of it: whether that version changed the block's meaning (a choice changes it when its
  value does, whatever the run said). A cause from a cell after it is said to be so, and "a way round" only when every
  cause is; the run's dot is green finished, red abandoned, blue open.
- A write: its version, its block, who wrote it; whether it changed the meaning, when, its value; who reads it; its
  content.
- A cell, chosen by its head: how it stands, its record and its open run (each choosing that run), why it would run
  or the true reason it would not (it is skipped, a run of it is open, or its record knows everything),
  and for each block it reads the version now and the version its record knows, a change of meaning in orange.
- Nothing chosen: every block that has versions, each version with who wrote it, when, its value, whether it changed
  the meaning or has the meaning of an earlier one, its length and its content.

## The flow chart

The blocks down the side, the cells across in the order of the walk. A block's name is in monospace, with its signs,
the name of the source outside that writes it, and the values of a choice after it, the last two muted. A cell's head: its id in monospace, muted; its title in 13px semibold; its line, or
"engine", in 11px muted; when it runs and what it waits for in 12px muted, cut to two lines and whole on hovering and in
the panel. The cells' columns share the width the names' column leaves evenly, up to 160px each, so that ten cells on a
wide window stay readable; a dragged column keeps its width (88 to 480px), and the chart scrolls sideways below 88px a
cell. The corner says "Cells →" at its top right and "Blocks ↓" at its bottom left, so that neither word stands over the
other's things. The wheel scrolls the chart wherever the pointer is over it, the fixed heads and names too. The marks: a ring reads, a dot may
write, both where both, a diamond where its condition reads the block, directly or by the named test that is its
condition, and a square where a named test that is one of its preconditions reads it. The diamond and the square are
drawn round the ring and the dot in a thinner line, so that what a cell runs on stays the mark's centre and what decides
whether it runs, or waits, its frame. A block's marks are joined by a line along its
row; a way back is the same line in the way back's purple with an arrow at the read it sends the work back to. The row
or column chosen is tinted with the accent. When comparing, what a change put in is green, what it took out red and
dashed, what it changed orange: on a cell's head and a block's edge, and on the marks.

## The console

The items as a tree in columns on a canvas (`src/console/snap.ts`), drawn by WebGL so that hundreds of items move
smoothly, and the panel at its right with the item selected. It is the console of `main`, brought over as it was.

- **Columns.** One column for each level, the items nothing depends on at the left; in each, the level's items one
  under another, a row of 32px, those under one item standing apart from those under the next. The wheel over a
  column moves that column and the columns beside it follow, each where the most of what joins it to the next near
  the middle of the screen is level; a sideways scroll or a drag moves all the columns sideways. The tree is laid on
  `--ground`, and fades into it at the top and the bottom of the screen.
- **Card.** 300px wide, white (`--card`) with a hairline of the console's own `#e5e7eb` (a step off `--line`), 6px
  corners: the canvas's own, as on `main`. Its number in monospace, in the colour of the item's standing (the
  console's faint grey `#9aa1ad` while open, `--green` when merged, paler when cancelled), its title in 14px, the
  canvas's own size, in Primer's sans-serif stack, which the console drew in (a canvas resolves a stack by its own
  rules, so the page's `--font` could draw another face); at its right a tag of how many things its work waits for from the owner,
  `--on-colour` on `--red`, and, while its work has not begun, a lock framed in muted with how many items it depends on
  are not merged. A card whose work goes on, once it has begun or while it is paused, at fault or waits, is tinted with the colour of what the work does (`--blue` running,
  `--orange` waiting for the owner, the console's pale yellow `#e8cc6a` waiting for anything else, muted paused,
  `--red` halted), and has a second row: a short bar for each cell of the work's flow, done in a faint grey, the one it
  is at in the colour of what it does, pulsing while a round is under way, the others in `--line`; the first letter of
  that cell; what the work has taken, in dollars; and how long it has gone on, its colour by its age (muted, amber
  after half an hour, `--red` after an hour, violet after a day). The edge of a card above a work is a light accent, so
  that the branches with work on show from the roots.
- **Shadow.** Under an item that depends on an item besides the one it stands under, a card with a dashed edge and
  grey words points with an arrow to where that item stands; a click on it selects that item and brings it to the
  middle.
- **Funnel.** A filled grey shape, Primer's neutral muted as the console drew it, joins each item to the first and the last of the items under it. The item under the
  pointer and those above it are ringed in light blue and joined by ribbons of it; the item selected and those above it
  in `--accent`, the item itself with a halo.
- **Map.** At the top left, a bar for each column, moved as the column is, marked in `--red` where works wait for the
  owner and in `--accent` where the item selected stands.
- **Panel.** The page's panel, 560px until dragged, as the console's window was. Its head: the item's number, its
  title, which brings the item into view, and the flow its work runs on. Two status lines (`.cw-line`): where the item
  stands and what it waits for to merge; what its work does, a link to the work's own page, and at the right the
  **cell marks** (`.stage-bar`): a 16px square for each cell of the flow with the first letter of its id, done on
  `--green-tint`, the one the work is at in the tone of its card (`--blue` running, `--orange` only where it waits for
  the owner, the pale yellow waiting for anything else, muted paused, `--red` halted), the others framed on `--ground`;
  and what it has taken. These lines are the look's status line (`StatusLine`) in all but two things, which is why
  the console draws its own (`.cw-line`): the dot takes the five tones of a work, among them the pale yellow of a work
  waiting for anything but the owner, which the status line has no tone for, and the line holds a link and, at its
  right end, the cell marks. Under them the buttons of what the
  owner does to the work: let it on or pause it, and cancel it, each waiting while what it sent is under way. Then two
  **tabs** (`.cw-tabs`), the one shown in the text colour over a 2px accent line, as the bar's; the look has no part
  for tabs inside a panel, and the drawer's text tabs are kept as they were, with its layout. The first says what the work waits for, the owner's things in
  `--orange-text`, and holds the **notes**: a **select** (`.cw-select`, 24px, monospace, as a button's box) of the
  blocks people write, a box to write in, and under it "Goes to" the cells that read the block chosen, with the button
  that sends it; the second, what the lines do not say of the work, as label and value pairs (`.pairs`).

The canvas follows light and dark: the colours the tokens came from it takes from the tokens, and those that have no
token (the faint and pale greys, the ribbons, the map's bars) are worked out from the tokens in dark, keeping their
relations to the card and the ground.
