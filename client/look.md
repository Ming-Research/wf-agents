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
| `--blue` | `#1a9fff` | `#4fb4ff` | running (not used on the flow page yet) | `PALETTE.running` |
| `--red` | `#e5484d` | `#ff6b70` | taken out, failed | `PALETTE.red`, `PALETTE.halted` |
| `--violet` | `#8250df` | `#ab7df8` | the way back | the flow chart's purple (Primer's done) |
| `--*-text` | darker steps | the colours | the colours as small text on white (4.5:1) | ours |
| `--*-tint` | 10% over the card | 10% over the card | a fill that says the colour's meaning | `TINT` = 0.09 |
| `--on-colour` | `#ffffff` | `#0f1012` | words on a filled tag or button | the white on a count tag |
| `--step` | 4% black | 5% white | one step of hover | ours |

The dark values are ours: the console has no dark ground. They keep its relations: the card a step lighter than the
ground, the colours lifted to read on it.

Sizes: `--size-label` 11px, `--size-small` 12px, `--size-body` 13px, `--size-title` 15px; `--font` and `--mono` are the
system's text and monospace families. The chart's own sizes (`--chart-*`) are named in `tokens.css` with what each is
for; the widths of its columns, which the owner drags, are in `Chart.tsx` (`WIDTHS`), and the panel's in `App.tsx`.

The lines' names (engineer, designer, review...) are in the accent wherever they stand, so that the cells sharing a
session are seen at a glance; "engine" stays muted.

The way back keeps the purple the flow chart draws it in, a different purple from "chosen", which is the console's. On
the flow page the two never meet in one form: what is chosen is a tint over a whole row or column, and a way back is a
drawn line with an arrow.

## Parts

- **Bar** (`Bar`): the page's name, the views as text tabs (the current one in the text colour over a 2px accent line,
  the others muted), and at its right what belongs to the page as a whole: the switch of light and dark.
- **View's line** (`.view-line`): one thin row under the bar that belongs to the view: what it shows, one muted sentence
  of the facts of the whole view (for the flow, its limits), its legend and its own switches. It wraps to a second row where the window is too narrow, the facts giving
  way first.
- **Button** (`Button`): words in a hairline box, 24px; `primary` filled with the accent, for the one action a view
  exists for.
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
  muted text.
- **Section** (`PanelPart`): a 12px semibold label with its count, over what it holds, 16px apart; `last` ends a group
  of sections with a hairline.
- **Sign** (`Sign`): a 12px line drawing in the accent, for what a word would crowd: people write, the outside writes,
  shown. Muted, they were too faint to find.
- **Edge** (`Edge`): the dragged edge of a part, as the rule on widths says.

## The flow chart

The blocks down the side, the cells across in the order of the walk. A block's name is in monospace, with its signs and
the values of a choice after it. A cell's head: its id in monospace, muted; its title in 13px semibold; its line, or
"engine", in 11px muted; when it runs and what it waits for in 12px muted, cut to two lines and whole on hovering and in
the panel. The cells' columns share the width the names' column leaves evenly, up to 160px each, so that ten cells on a
wide window stay readable; a dragged column keeps its width (88 to 480px), and the chart scrolls sideways below 88px a
cell. The corner says "Cells →" at its top right and "Blocks ↓" at its bottom left, so that neither word stands over the
other's things. The wheel scrolls the chart wherever the pointer is over it, the fixed heads and names too. The marks: a ring reads, a dot may
write, both where both, a diamond where its condition reads the block. A block's marks are joined by a line along its
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
- **Card.** 300px wide, white (`--card`) with a hairline in `--line`, 6px corners: the canvas's own, as on `main`. Its
  number in monospace, in the colour of the item's standing (muted, `--green` when merged, paler when cancelled), its
  title in 14px, the canvas's own size; at its right a tag of how many things its work waits for from the owner,
  `--on-colour` on `--red`, and, while its work has not begun, a lock framed in muted with how many items it depends on
  are not merged. A card whose work goes on is tinted with the colour of what the work does (`--blue` running,
  `--orange` waiting for the owner, the console's pale yellow `#e8cc6a` waiting for anything else, muted paused,
  `--red` halted), and has a second row: a short bar for each cell of the work's flow, done in a faint grey, the one it
  is at in the colour of what it does, pulsing while a round is under way, the others in `--line`; the first letter of
  that cell; what the work has taken, in dollars; and how long it has gone on, its colour by its age (muted, amber
  after half an hour, `--red` after an hour, violet after a day). The edge of a card above a work is a light accent, so
  that the branches with work on show from the roots.
- **Shadow.** Under an item that depends on an item besides the one it stands under, a card with a dashed edge and
  grey words points with an arrow to where that item stands; a click on it selects that item and brings it to the
  middle.
- **Funnel.** A filled grey shape joins each item to the first and the last of the items under it. The item under the
  pointer and those above it are ringed in light blue and joined by ribbons of it; the item selected and those above it
  in `--accent`, the item itself with a halo.
- **Map.** At the top left, a bar for each column, moved as the column is, marked in `--red` where works wait for the
  owner and in `--accent` where the item selected stands.
- **Panel.** The page's panel, 560px until dragged, as the console's window was. Its head: the item's number, its
  title, which brings the item into view, and the flow its work runs on. Two status lines (`.cw-line`): where the item
  stands and what it waits for to merge; what its work does, a link to the work's own page, and at the right the
  **cell marks** (`.stage-bar`): a 16px square for each cell of the flow with the first letter of its id, done on
  `--green-tint`, where the work is on `--blue`, or on `--orange` where it waits, the others framed on `--ground`; and
  what it has taken. The dot of a line is in the colour of the card's tint. Under them the buttons of what the owner
  does to the work: let it on or pause it, and cancel it. Then two **tabs** (`.cw-tabs`), the one shown in the text
  colour over a 2px accent line, as the bar's: the first says what the work waits for, the owner's things in
  `--orange-text`, and holds the **notes**: a **select** (`.cw-select`, 24px, monospace, as a button's box) of the
  blocks people write, a box to write in, and under it "Goes to" the cells that read the block chosen, with the button
  that sends it; the second, what the lines do not say of the work, a label and a value a row.

The canvas follows light and dark: the colours the tokens came from it takes from the tokens, and those that have no
token (the faint and pale greys, the ribbons, the map's bars) are worked out from the tokens in dark, keeping their
relations to the card and the ground.
