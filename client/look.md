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

## Tokens

| Token | Light | Dark | For | From the console |
| --- | --- | --- | --- | --- |
| `--ground` | `#f6f7f9` | `#0f1012` | what the page is laid on: the bar, the chart's heads, the frame | `PALETTE.page` |
| `--card` | `#ffffff` | `#17181b` | what is read on the ground: the chart's rows, the panel | `PALETTE.card` |
| `--text` | `#1f2937` | `#e6e7ea` | words | `PALETTE.ink` |
| `--muted` | `#6b7280` | `#8b8f98` | facts, ids, labels | `PALETTE.muted` |
| `--line` | `#e3e6ea` | `#26272b` | hairlines | `PALETTE.hair` |
| `--accent` | `#8b5cf6` | `#a78bfa` | what is chosen, focus, the panel's handle | `PALETTE.chosen`, `--console-chosen` |
| `--accent-text` | `#7c4ddb` | `#a78bfa` | links: the accent where it reads on white | ours |
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
for.

The way back keeps the purple the flow chart draws it in, a different purple from "chosen", which is the console's. On
the flow page the two never meet in one form: what is chosen is a tint over a whole row or column, and a way back is a
drawn line with an arrow.

## Parts

- **Bar** (`Bar`): the page's name, the views as text tabs (the current one in the text colour over a 2px accent line,
  the others muted), and at its right what belongs to the page as a whole: the switch of light and dark.
- **View's line** (`.view-line`): one thin row under the bar that belongs to the view: what it shows, one muted sentence
  of facts, its legend and its own switches. It wraps to a second row where the window is too narrow, the facts giving
  way first.
- **Button** (`Button`): words in a hairline box, 24px; `primary` filled with the accent, for the one action a view
  exists for.
- **Segmented switch** (`Switch`): two or three choices side by side, the one chosen on the card. For a choice that is
  always set, such as a version or the theme.
- **Tag** (`Tag`): a few words or a count of a state, filled in its colour, 16px high, monospace, 3px corners, as the
  console's count tags on a card.
- **Label** (`Label`, `.label`): 11px, upper case, letter-spaced, muted: what a column or a value is.
- **List row** (`ListRow`): one thing per row, at least 26px, a hairline under it.
- **Panel** (`Panel`): docked at the right of a view, 360px, a hairline at its left, laid out as the console's drawer: a
  handle on its edge that folds it away and brings it back; a head with the thing's id (monospace, muted) and title
  (15px semibold) on one line and what it belongs to at the right; one or more status lines; its sections; room at its
  bottom (`foot`) for a box that answers. Escape lets go of what it shows.
- **Status line** (`StatusLine`): a small dot in the colour of what it says, a word in semibold, the facts after it in
  muted text.
- **Section** (`PanelPart`): a 12px semibold label with its count, over what it holds, 16px apart; `last` ends a group
  of sections with a hairline.
- **Sign** (`Sign`): a 12px line drawing in the colour of the text round it, for what a word would crowd: people write,
  the outside writes, shown.

## The flow chart

The blocks down the side, the cells across in the order of the walk. A block's name is in monospace, with its signs and
the values of a choice after it. A cell's head: its id in monospace, muted; its title in 13px semibold; its line, or
"engine", in 11px muted; when it runs and what it waits for in 12px muted, cut to two lines and whole on hovering and in
the panel. A cell's column is as narrow as its head allows, between 64px and 120px. The marks: a ring reads, a dot may
write, both where both, a diamond where its condition reads the block. A block's marks are joined by a line along its
row; a way back is the same line in the way back's purple with an arrow at the read it sends the work back to. The row
or column chosen is tinted with the accent. When comparing, what a change put in is green, what it took out red and
dashed, what it changed orange: on a cell's head and a block's edge, and on the marks.
