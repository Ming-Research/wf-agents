# The owner's page: UI spec

How the owner's page looks and behaves, and why. A rule's reasons are part of it: what it does, what it does instead of,
and because of what, so that a later change can tell whether a rule still holds. An article named here is one of the
constitution's, `constitution.md` at the repository's root.

## How the page is held to this spec

### The page follows this spec

The owner's page is built to this spec, which the UI check holds in CI (`npm run check:ui` in `client/`): a change that
needs what the spec lacks changes the spec in the same change, saying why, and a change to the page that departs from
the spec without changing it is a finding of the review, because like things look alike only when every part is built
from one set of rules, and a rule kept only in the code is a second copy of them, which drifts.

### The UI check

A change to the page is checked against this spec by `npm run check:ui` in `client/`, a step of the `ci` workflow: it
refuses, in the client's CSS and in its inline styles, a raw colour (hex, rgb, hsl, a named colour, `color-mix`), a raw
font size, font shorthand or weight, a raw spacing value (a margin, padding or gap other than 0 or auto), a raw radius
or border width, and a rule that makes muted text bold; a value is a `var()` of a Primer token or of the page's token
file; the exceptions are the token file itself, the chart's named values, each with its reason, and the styles not yet
converted, each named with the step that converts it, a list that only shrinks, because a rule that nothing checks
drifts with each change, as the page's own look did.

### What a reviewer checks

What the check cannot see a reviewer checks against this spec: each thing on the page uses the part its line here maps
it to; a heading is a heading element in the type step of its level; a box holds only what an agent gave; a new
recurring thing gets its line here in the same change that first draws it, because which part a thing is, and whether a
box holds an agent's words, is a judgment no pattern sees. A project's head (`ProjectHead`) is looked at in a pane 320px
wide, in one 220px wide (the narrowest the list of conversations drags to) and at a phone's width, with every state's
button drawn: the name is cut with an ellipsis before anything wraps, and the buttons wrap onto a row of their own, at
the right, only where they alone are wider than the row, the name staying on the first; a test in a layout-less DOM
cannot measure this, so the reviewer does.

## What the page is

### Only what the owner uses

The page shows the owner only what the owner uses: a block of values the engine keeps for itself, such as a work's
machine, worktree, pushed head or recorded values, is not shown, because what is drawn is there for the owner to read,
and what the owner cannot use crowds out what the owner needs.

### Like things, one set of parts

The page draws like things with one set of parts wherever it shows them, in every tab and view: what an agent wrote is a
block, its head (what it is, as chips and a title) and who wrote it (the cell, or in a conversation who to whom; when;
and the round) over its text; a section is a line that folds, its heading, how many it holds as a count and who wrote it
on that line over what it holds, a page's own sections one under another as one document, parted by a line between them
and not by a rule under each heading, as this spec draws them (below); an item is its number and title, leading to its
page; a round is its name, opening it where the page shows it in place, else on its work's page; and the form that
answers or says something is what goes above its words (a choice, whom to), its words and the button that sends them on
the right, Ctrl or Cmd with Enter sending too. A conversation's entry stays an entry, its head and its text drawn with
the same parts.

Instead of each tab drawing its own boxes, heads, folds, links and forms, because the owner reads one tab after another,
and like things drawn apart read as different things.

### React, Vite and elkjs

The page is built with React and Vite, and the charts it draws are laid out by elkjs, instead of mermaid, because each
node is then a component that can carry overlays and badges, such as a work's current cell and its round counts, while
mermaid renders a fixed picture.

### One message catalog

Every text of the page is in English, from one message catalog, because the owner reads the interface in English, and
one catalog keeps the names of the flow's parts the same wherever the page shows them.

## The design system

### Primer

The owner's page follows GitHub's Primer design system (primer.style): every colour, type size, weight, spacing, radius
and border width is a token of `@primer/primitives` (the functional tokens, such as `--fgColor-default`,
`--text-title-shorthand-small` and `--base-size-8`), and every control and recurring part is the `@primer/react`
component this spec maps it to, instead of values and parts the page made up view by view, because the owner found the
page's look unreasoned (small grey bold labels used as headings) and asked for an industry model to copy and a spec that
every later change keeps to.

Rejected:

- The page's own colours, sizes and parts, tidied: rejected because they were chosen view by view with no rule to hold
  the next change to, which is how small grey bold labels came to head the item page.
- Primer CSS (`@primer/css`, class names on plain elements): rejected because the page is React, `@primer/react` carries
  the same look with the behaviour (focus, keyboard, overflow menus) built in, and Primer itself now ships its look
  through the React components and the primitives.
- Another system (Material, Fluent, Ant Design): rejected because the owner chose GitHub Primer, whose look the owner
  reads every day on GitHub, and whose dense, text-first pages fit a page that is mostly what agents wrote.

### Where Primer says nothing: "ours" and the token file

Where Primer says nothing, the page decides in Primer's spirit, and each such rule here says "ours"; a value Primer has
no token for is named once in the page's own token file, `client/src/styles/tokens.css`, beside the reason, instead of
being written where it is used, because a value written twice drifts.

### Light and dark

The page follows the system's light or dark appearance with Primer's light and dark themes: the root element carries
`data-color-mode="auto" data-light-theme="light" data-dark-theme="dark"`, and the app is wrapped in `ThemeProvider`
(from `@primer/react/next`, colorMode auto) and `BaseStyles`, instead of a switch of the page's own, because the owner
sets the appearance once in the system, and one source of the mode keeps the tokens, the components and their overlays
in step.

## Colour

### The palette

The page's colours are its own palette (ours), set once in `client/src/styles/theme.css` as the values of Primer's
colour tokens of the roles, for the light theme and the dark, over Primer's: one colour to a role, the same in both
themes, for a state's fill, its edge and its mark alike, with white words on the fill: accent `#3b87ff`, attention
`#f29a1f`, neutral `#8d9bb3` (a slate with a little blue), danger `#f45d58` and success `#24b574`; a role's muted
background (`--bgColor-*-muted`) a tint of its colour and its muted border the same tint, so that nothing is edged in a
colour other than its fill; a role's foreground (`--fgColor-*`), which colours words, the colour itself in the dark
theme and, in the light theme, where the colour is too light to read on white, a darker step of the same hue (accent
`#2567d0`, attention `#9b6315`, neutral `#5e6c84`, danger `#c6342f`, success `#217952`); a mark, an icon or a line in a
role's colour takes its fill or its edge (`--bgColor-*-emphasis`, `--borderColor-*-emphasis`), so that it is the colour
itself in either theme.

Primer's components that draw in a role's colour (a Label, a counter, the primary button, a checked control, a Flash,
the focus ring) follow from the same tokens, the theme setting besides only the few tokens of theirs that Primer writes
out instead of deriving (the primary button's edge, which takes no colour other than its fill, and the steps of a button
or a checked control under the pointer, pressed and disabled); each of its rules is Primer's own selector with one
`[data-color-mode]` more, so that the palette wins whatever order the stylesheets load in.

Instead of Primer's own colours, because the owner found them too heavy and dark and asked for a light, cheerful, modern
palette of low saturation, white words on solid fills of a medium weight, and no border in a colour other than its fill.

### A colour is a fill

A chip or a button that carries a colour is a fill only, with no edge in a colour (ours): a state's chip and a count are
filled, a button in a role's colour is filled in it, and a button that is on or off, such as a state's button over the
items' tree, is filled in its colour while on and in its tint (`--bgColor-*-muted`) while off, so that on and off differ
by the fill alone; where Primer draws an edge round such a part, the edge is its fill's own colour or none. Uncoloured
chips and the grey edges of boxes, cards, tables and fields are not chips in a colour and keep their edges, and an
uncoloured button that is pressed or open (a work's choice of a new session, a chart's "?") is drawn while pressed as
the option picked, its tint edged in the same tint (below), and carries no colour while not.

Instead of solid chips beside outlined ones, or a button outlined while off and filled while on, because the owner,
keeping the palette, found the two styles mixed and asked for one, kept as a rule of the page's design.

### Colour tokens

Colour comes only from Primer's colour tokens, never a hex value, a named colour or a colour mixed (`color-mix`) where
it is used: what has a role on the page takes the functional tokens, `--fgColor-default` for what is read,
`--fgColor-muted` for facts about a thing (who wrote it, when, its round, a hint), `--fgColor-accent` for links and the
place chosen; `--bgColor-default` for the page and its boxes, `--bgColor-inset` for the bar, `--bgColor-muted` for the
head of a box, a card or a table, `--bgColor-neutral-muted` for code and a row under the pointer,
`--bgColor-accent-muted` for the option or row picked; a state's `--bgColor-*-emphasis` under `--fgColor-onEmphasis` for
its chip (below); `--borderColor-default` round a box, a control or a table and for the line between a page's sections,
`--borderColor-muted` for a line inside a box; what has a kind or a series instead of a role (a role's colour, a chart's
kinds of cell, the series of a chart of numbers) takes Primer's display and data visualisation tokens, as the parts
below say; the values the page's own palette gives the functional tokens are written once for each theme in
`client/src/styles/theme.css` (above); and a colour mixed of two tokens is named once as a token, beside why, because
the functional tokens carry their meaning into both themes, the display tokens their hues, and a colour picked by hand
is right in one theme at most.

### A state's colour

A state has one colour wherever it shows, through Primer's functional state tokens and a Label's variant: done,
confirmed, merged and took are success; running and working are accent; waiting for someone, waiting for a free slot,
paused, sent back, queued, provisional and to check are attention; failed, cut off, halted (a work the engine stopped
for a problem), cancelled and disagreed are danger; new, ready, a work ended (grey as GitHub draws a run cancelled,
since a work ends mostly to have a new work take its item over, and the owner read it in red as broken) and any other
state of no meaning of its own (replaced, closed, not judged yet, left open) are neutral, grey
(`--bgColor-neutral-emphasis`, which the page gives Primer's primary variant, unused otherwise), so that it is a state's
chip as every state is and not a tag's, instead of secondary, the look of a tag, because the owner learns a colour once
and reads it on every view.

## Type and text

### Six type steps

The page's type has six steps and no others, each from Primer's text tokens, a chart's text, read at a distance, taking
the sizes its own rules give it (below): the page's title is title medium (`--text-title-shorthand-medium`, 20px
semibold); a section's heading is title small (`--text-title-shorthand-small`, 16px semibold); a sub-heading, the name
of a part inside a section (such as "Why" or "Where this differs from the item as written"), is body medium semibold
(`--text-body-size-medium`, `--base-text-weight-semibold`, 14px); body text is body medium
(`--text-body-shorthand-medium`, 14px normal, line height 1.5); small text is body small (`--text-body-size-small`, 12px
normal); code is `--fontStack-monospace` at `--text-codeBlock-size` in a block and `--text-codeInline-size` inline,
because a few steps far enough apart read as an order at a glance, and Primer's steps are the ones GitHub's readers
already know.

### Muted is never bold

A heading is in the default foreground (`--fgColor-default`) and is either larger than what it heads or, at the same
size, semibold; muted text (`--fgColor-muted`) is never bold and never a heading, and no label is small, muted and bold
at once, instead of small grey bold labels over its fields and stages, because muting says "less important" while bold
says "look here", and the owner read the two together as a contradiction.

### Weight marks structure

Weight marks structure only: semibold (`--base-text-weight-semibold`) for headings, the head of a card, who wrote or
says something and to whom at the head of what they wrote (a byline, a bubble's head, as GitHub names a comment's author
and a mention), a cell's title on a chart, `strong` in text an agent wrote, and the one current choice (the view in the
bar, the line of the checklist the work is at, the label of a row lit or pointed at on a chart); medium only where a
Primer component sets it itself (a Label); everything else normal; bold (700) is not used, because weight spent on
decoration stops marking what is a heading.

### Prose in the page's font

Text the agents and the owner wrote (an item, what a round handed back) reads in the page's font, and only a message as
sent, a file and what a tool printed keep a fixed width, because that text is prose, and a fixed width makes prose
slower to read.

### Text an agent or the owner wrote

Text an agent or the owner wrote reads as one document in the page's font, as GitHub's markdown reads (ours, after
Primer's markdown style): body medium with line height 1.5, paragraphs and list items `--base-size-8` apart, lists
indented `--base-size-24`, a heading the agent wrote drawn as a sub-heading whatever its level, inline code and code
blocks as Primer's markdown draws them (`--bgColor-neutral-muted`, `--borderRadius-medium`), with no background or
indent of its own, an agent's text inside the box of what an agent handed back (below) where it stands among the page's
own words, and a document that fills a section by itself as that section's body text (below), because the agent's
headings must not outrank the page's own.

### The page's own markdown

The markdown agents write in that text (paragraphs, lists, headings, fenced and inline code, bold, links) is drawn by a
small renderer of the page's own, and a paragraph that is plainly code, such as a command, is kept as code, instead of
the text shown raw or a markdown library, because raw markdown is hard to read, the page needs only these few forms, and
nothing an agent writes may become markup the page did not mean.

### Diagrams

A diagram in a text an agent or the owner wrote, a fence of `mermaid` (any kind mermaid draws) or of `dot` (Graphviz's),
is drawn as that diagram wherever the text shows, by the one part that draws the markdown (ours), with mermaid for every
kind mermaid draws and Graphviz compiled to WebAssembly (`@viz-js/viz`) for dot, each library loaded only when a fence
of its kind first comes near the screen; mermaid draws at its strict security level with no HTML in its labels, the keys
of its configuration that keep the page's look and security closed to a diagram's own, and Graphviz's drawing is
sanitised as SVG with no script and no foreign object (DOMPurify); a diagram drawn once is kept and not drawn again
while it is shown again in the same theme; it is drawn on the text's background in the page's palette, light or dark as
the page is, its words in the page's font at body medium, a box `--bgColor-accent-muted` edged
`--borderColor-accent-emphasis`, a line `--fgColor-muted`, a group `--bgColor-muted` edged `--borderColor-default`, a
note `--bgColor-attention-muted`, and the parts a diagram colours one by one (a mind map's branches, a timeline's
periods, a pie's slices) in Primer's display colours filled in their emphasis with `--fgColor-onEmphasis` words, as a
state's chip; at its own size when its box is as wide as it, and scaled down to the width of its box when it is wider,
never scrolling sideways and never widening the page, a Gantt chart, which mermaid draws as wide as its box,
drawn no narrower than 600 px and then scaled to the box like any other; its source as a code block while it is not
drawn yet and when it cannot be, then with a note under it in small `--fgColor-muted` text that it could not be drawn,
never a blank space; and in its top right corner invisible small `IconButton`s, a `ScreenFullIcon` that enlarges it
(only while it is scaled down) and a `CodeIcon` that shows the source in the diagram's place and the diagram again,
shown under the pointer or the keyboard's focus and always on a screen with no pointer; instead of the fence shown as
code, the page drawing diagrams itself, or the libraries in the page's first load, because the owner understands a
design by its diagrams, of every kind and not only mermaid's, the two libraries draw every kind the agents are told to
write, and a page that shows no diagram does not load them.

A diagram scaled down to its box opens enlarged, by its `ScreenFullIcon` button or a click on the diagram (the zoom-in
cursor over it; a drag over the diagram does not open it, so that its text can be selected, and a press that moves
further than the viewer lets a click move is a drag), in a Primer `Dialog` titled "Diagram" that is as large as Primer
lets it be (the window less its margin), closed by its cross, Escape or a click outside it. It holds the same drawing, the markup already drawn and
sanitised, under an id of its own so that the two copies do not meet, and never a drawing made again from the source, in
the viewer the flow charts are moved and zoomed in (Moving and zooming): shown whole when it opens, on
`--bgColor-default`, the background the diagram is drawn for, with the gestures told in the dialog's footer in small
`--fgColor-muted` text. A diagram that is not wider than its box has no such button. Instead of the diagram scrolling
sideways in its box, because the owner read a diagram cut off at the edge as one cut off (a scroll bar that shows only
once used does not say there is more), and a diagram is taken in whole before it is read closely; instead of only
scaling it down, because a wide diagram's words come out too small to read and the owner must be able to read them;
instead of a new tab, because the page itself draws the diagram, in its theme, and a tab takes the owner away from the
words round it, which is not so of an image, a file the server holds; instead of opening it in place, because the dock
is narrow and the diagram would stay narrow and push the messages under it away; instead of drawing the diagram again
narrower or turned, because that changes what its author drew, holds for some kinds only, and still leaves no way to
read a large one.

### An item named by its number

An item named by its number in a text an agent or the owner wrote (`#12`) is a link to the item's page, the page reading
the list of items once for every text that names one and again when an item changes, drawn as an item's number is
everywhere (`ItemRef`, the number in `--fgColor-muted`), its title on hovering; a `#` and number in code, in an address
or a path, joined to a word (`C#`, `#12a`, `#12.5`), after a word that names another numbered thing (`PR #7`, a pull
request, an issue, a matter, a question, an ask, a card, a work, a round, a change, a commit, a comment), or a number no
item has, stays as written (ours), instead of every number as plain text or each number read on its own, because the
owner asked to reach an item from wherever a text names it, and items are named by their number.

## Space, lines and width

### Spacing

Spacing (margin, padding, gap) uses Primer's size scale only, `--base-size-4`, `-8`, `-12`, `-16`, `-24` and `-32` (and
`-2` and `-6` only inside a control or a label, where Primer uses them), with this rhythm (ours): 4 between a name and
its value and between marks on one line; 8 between paragraphs and the lines of a list and under a heading; 12 between
the parts of a section and inside a box beside its text; 16 inside a card and round a pane's content; 24 between
sections, on each side of the line that parts them (below), because a fixed rhythm lets the eye group what belongs
together.

### Corners, lines and shadows

Corners are `--borderRadius-medium` for boxes, cards, controls, code and chips (a Label, whose pill Primer draws the
page overrides once, below), and `--borderRadius-full` for counters and dots; lines are `--borderWidth-thin`, with
`--borderWidth-thick` for a marked edge or a rule at a thing's left (the option recommended, the items an item depends
on, an earlier version) and `--borderWidth-thicker` for a left edge that sets a thing apart by its shape (the sheet of
what is sent to an agent, an item's card in its state's colour); the page casts no shadow, and what floats over it uses
Primer's `--shadow-floating-small`, because Primer's own components already draw with these, and the page's parts beside
them must not look like another kit.

### Spend the screen's space reasonably

The owner's principle for every page, the charts being what takes most of the screen: use the screen's space as
reasonably as possible. Every row a page spends has an answer to what the owner does with it. A control that belongs to
a heading stands at the right end of the heading's own row (`ProjectHead`, the one head of every project's group), never
on a row of its own under it. One kind of thing is one full-width table or list, its groups parted by their heads, never
a permanent column beside it that takes room from the chart or the table on every view. What a row can open takes no tab
of its own in the bar. A line, a heading or a count that repeats what the row under it says is not there.

Instead of the buttons on a row under each project's name, a second folded table for the works that ended, a permanent
column of groups beside the works, and a tab in the bar for the architect that the works view could open from a row,
because the owner found each of them took room that the chart and the table needed, and asked that no screen be spent
on what is not used.

### Reading width

A reading page (an item, the settings) is one column at most `--breakpoint-large` wide (1012px, the width of Primer's
large PageLayout) with `--base-size-24` round it (`--base-size-16` in a pane narrower than `--breakpoint-medium`); a
view that is a chart or a list over the window (the works, the flow, an architect, the system) takes the whole width
with `--base-size-16` round it, because prose wider than that is slow to read, while a chart and a wide table need every
column.

## The bar

### One row

The bar is one row 48px high, as tall as Primer's UnderlineNav, on `--bgColor-inset` with the nav's line under it (ours,
after GitHub's header): the name `wf-agents` as title small, the views as an `UnderlineNav`, and at the right end the
engine's state, its switch and keep-warm and the slots, instead of two rows (GitHub's header over its nav), because the
flow's chart fills the window under the bar, and a second row would take its height.

### What it holds

The bar holds the views, the engine's state in words (running; paused, saying which works the owner started run
meanwhile; pausing while the last round finishes, with how many; or waiting for what stops every work, until when),
beside it the switch of every work plainly as a button, "Pause all" while works go on and "Start all" while they are
paused, which asks first, saying what goes on and how many works on an older flow version are brought up, the switch
"Keep warm", as tall as the button beside it, that one click turns over, the slots in use and the unread messages, and
nothing of a view: a work's facts are its row in the list of works and its line above its chart, and its own controls
are in the head of its group (below), and the flow page's choice of version and comparison floats over the chart's top
left corner, instead of a panel behind the engine's state that switched between running and holding, which the owner did
not find, and instead of the controls and facts of the view shown in the bar, because a control pressed from a view
about something else did what the owner could not see, and a work's facts in the bar repeated its row just above.

### Never wider than the window

The bar is never wider than the window (ours, on Primer's overflow): the views it has no room for are in the nav's
"More", as Primer's UnderlineNav puts them, the nav never narrower than that button; what of the engine it has no room
for is behind a button at its right end, as Primer puts what a bar has no room for behind its mark of more: an invisible
`IconButton` with `KebabHorizontalIcon` that opens under it Primer's `AnchoredOverlay` holding those parts one under
another, drawn by the very parts the bar draws them with, `--base-size-12` apart and `--base-size-12` `--base-size-16`
inside. Below the full width (Primer's extra large breakpoint, `--breakpoint-xlarge`, 1280px) it holds Keep warm and the
slots, which the owner sets and reads now and then, so that the views, which the owner moves between all the time, keep
their room; below the narrow breakpoint (`--breakpoint-medium`, 768px) the state's words too, the dot staying in the bar
and saying them on hovering; Start all or Pause all, which the owner uses often, stays in the bar at every width; and
the state's words, which can be long, give way first, cut with an ellipsis and whole on hovering.

Instead of every part of the engine in the bar at every width, because at a phone's width they ran past the window's
right edge, the page 592px wide and scrolling sideways, and at a tablet's every view was in "More" while Keep warm and
the slots kept their room.

### The views

A view in the bar is an `UnderlineNav.Item`, the current one with `aria-current="page"` (Primer draws its text semibold
and its underline); its counts are `CounterLabel`s, what waits for the owner `variant="primary"` and the rest secondary,
because the views are the page's places, as a repository's tabs are on GitHub, and a count beside a view says what waits
there before it is opened. A view holds every page that is one of its uses: the works view is the list of works, a
work's page and an architect's page (below), so the bar has no tab for the architect, instead of a tab for each, because
a project's architect is a row of the works (below) and the owner asked that the screen not be spent on a tab for what a
row opens.

### The engine's state

The engine's state in the bar is a `DotFillIcon` Octicon coloured by the state, in the colour a chip of it is filled in
(`--bgColor-success-emphasis` running, `--bgColor-attention-emphasis` pausing, `--fgColor-muted` paused,
`--bgColor-danger-emphasis` offline or halted), before its words in body medium, the words of a halt in
`--fgColor-danger`, because the owner reads whether anything runs at a glance, and the colour says it before the words.

## Splits and narrow windows

### A split

A split between a list and what is picked from it, or between what is read and what is kept beside it (the items' tree
and an item, the observations and the one open, the conversations and the one open, the one-to-one with an architect and
the panel beside it, a page's panel and what is open on the chart in the window floating over a chart) is Primer's
PageLayout (`containerWidth="full"`, no padding or gaps) with a resizable `PageLayout.Pane` whose width the browser
keeps (`widthStorageKey`), the pane `sticky` under the bar while what is picked scrolls the page; the items' split and
the observations' are one part (`ListSplit`, parts.tsx), whose list fills the view with no divider while nothing is open
(its pane set to the whole width and its divider hidden through the names Primer gives them, since Primer has no pane
that fills its layout and keeps its width for later), the pane staying the one element so that opening something keeps
the list where it was scrolled and the width the owner gave it; where the split needs more than the pane gives (a double
click that resets the width, the edge of the window floating over a chart, which drags its width, and the divider in it,
whose height the browser keeps once for every page), it is ours and drawn as PageLayout's divider (a
`--borderColor-default` line that turns `--borderColor-accent-emphasis` under the pointer), because one way to drag a
split is learned once.

### One thing at a time on a phone

Below the narrow breakpoint (Primer's, `--breakpoint-medium`, 768px, where PageLayout no longer sets a pane beside its
content), a phone's width, the page shows one thing at a time and never scrolls sideways: a split shows one side at a
time, the whole width (ours, through Primer's responsive `hidden` of a pane and a content, with no divider): the list
while nothing is picked, nothing opening by itself there (neither the first conversation nor the first item of the
tree), else what is picked, under the way back to the list, one part (`BackLink`, parts.tsx): a `Link` with an
`ArrowLeftIcon` and the list's name ("Messages", "Items", "Observations") in small text, as the way back over a panel
is, `--base-size-12` over it and `--base-size-16` at its left, hidden from the narrow breakpoint up, where the list is
beside what is open; a close that leads where the way back does (an observation's) is not drawn there.

A panel kept beside what it is about is over it, in its place, there, its close or its fold the way back: the window
over a chart takes the chart's whole width, the chart showing while it is minimised and the panel while it is expanded;
and the panel beside the one-to-one with an architect, a split inside the messages' split, takes the conversation's
place below the full width (`--breakpoint-xlarge`, 1280px), where the list, a conversation of Primer's least content
width (the 511px its panes leave the content below that breakpoint) and the panel no longer fit side by side at their
first widths, showing there only while the address names one of its tabs, opened by the button at the head's end, so
that the one-to-one opens on the conversation, which stays drawn, hidden, so that the words in its box wait.

Instead of the pane stacked over or under the content, as PageLayout lays a pane out there by itself, because the list
stacked over what is open put the item, the observation or the conversation under the whole list, and in a view that
fills the window the conversation's box out of reach, while a phone shows one thing at a time, as the owner's chat apps
do; and instead of the panel beside the one-to-one drawn over the conversation's head and stream at a phone's width and
the conversation squeezed under it to a sliver at a tablet's, which the owner found on the trial board.

## Pages and sections

### One part per thing

Each recurring thing has one part, and a new view uses it instead of drawing its own; the list follows, one thing to a
line, and a thing not yet listed gets its line here before a view draws it, because "like things look alike" (Article 8)
holds only if the mapping is written down and checked.

### A page's head

A page's title and what it is about is a `PageHeader`: `PageHeader.Title` as the h1, a thing with a number titled by its
number in `--fgColor-muted` then its title ("#1 Document the commands"), as an item is written everywhere (below),
instead of "Item 1"; its facts (below) in body text, after a bar on the title's line in `PageHeader.TitleArea`, aligned
with the title's words and wrapping under it where the line is short, an item's its project, its tier, the way its work
went when that is not through the design, its state's chip with the items it waits for after it, and its pull request ("#1 Document the
commands | wf-agents | Tier 2 | working | PR #3"); under them, in `PageHeader.Description`, while a work goes on on it, a second line of that work's facts: the
work as a link with its state's chip beside it and no bar between them, its stage, its cell, what it waits for and its
conversation ("Work 5 ready | test and review | 16 Owner | Conversation"); a page's controls, where it has any (an
observation's close), in `PageHeader.Actions`, an item's page, read only, having none; instead of one line of facts
under the title, its state's chip first, then its tier, what it delivers, its project, its pull request and the work,
because the owner asked for the item's own facts beside its title and the work's on a line of their own, and found what
an item delivers of no use on its head.

### A section

A section of a page is ours on Primer's `Details`: one summary line holding a `ChevronRightIcon` or `ChevronDownIcon` in
`--fgColor-muted`, the title as an h2 in title small, its count as a `CounterLabel`, and who wrote it in small
`--fgColor-muted` text after them, with no rule under the line; a `--borderWidth-thin` `--borderColor-default` line the
whole width of the page over each section parts it from what comes before, `--base-size-24` on each side of the line,
and none over the first thing of a page; no box round it. One part draws it for every page, a panel's sections (a
round's, a cell's) among them, and what a view sets apart as a section without the part (two sections side by side, a
part of a work's usage or of the settings, the period the System view counts over, which heads the sections it counts
for) takes the same line from the same rule.

Instead of a box round each section, because a page of boxes read as a heap of separate things where the owner reads one
document; and instead of a `--borderColor-muted` rule under each section's title, because the owner decided that a line
between sections, with room round it, says where one ends and the next begins, which a rule tight under a title did not.

### A fold inside a section

A part inside a section that folds (the approaches compared, earlier versions, why this one and not the others) is the
same `Details` one step down: its title a sub-heading (h3), no rule, its body indented by the chevron's width, because a
fold inside a section is a smaller thing of the same kind.

### A project's head

Where a list or a table groups its rows by project (the list of conversations, the works, the items' tree, the
observations), each project's group begins with its head (`ProjectHead`, parts.tsx, ours on Primer's `Heading`): the
project's name, an `h2` in the small step, at the left, and the controls over the group's rows (the buttons that show
or hide its rows by state or pick them, a pick's clear) at the right end of the same row, in a container of their own
beside the name and not in the heading, so that the heading says the name alone. The name gives way first: it is cut
with an ellipsis and keeps at least `--base-size-64`; the controls keep their width and wrap onto a row of their own, at
the right, only where they alone are wider than the row, the name staying on the first. Where the group folds (the
works), the name is a button inside the heading, a chevron before it, so that the heading is still the name alone. A
group with no control, such as
wf-agents' own and the groups of connected projects in the list of conversations, has the name alone or none. Every view
that groups its rows by project draws its head with this part, which a test checks of the four, instead of a row of
buttons under the name or a head of its own in each view, because the owner asked for the buttons at the right end of
the name's row, with space between, and the same look wherever a project's rows are grouped.

### A labelled text

A labelled text, one field of what an agent handed back (Why, Cause, Where this differs from the item as written), is
its name as a sub-heading (h3) over its text, `--base-size-4` between them and `--base-size-12` before the next, instead
of a small grey bold label, because the name heads what follows it.

## Cards and documents

### A card the owner rules on or answers

A card the owner rules on or answers (a decision card, an ask) is drawn as GitHub draws a comment: a box with a head on
`--bgColor-muted` holding its name semibold, where it stands with the owner as its state's chip (a `Label`, a fill only,
as every chip), the round it came from and its facts in small `--fgColor-muted` text joined by a bar, then its body
`--base-size-16` inside, because a card is something the owner answers, as a comment is on GitHub, and its head says
where it stands before its body. The facts say what the chip does not: who ruled or answered last and when ("ruled by
you at 10-30 10:40:00"), and on a decision card that waits that it blocks nothing until the work merges; its state's
chip is waits for you attention, confirmed success, you chose another accent, disagreed danger, replaced and withdrawn
neutral.

A decision card stands out from the design around it: a `--borderWidth-thick` frame, `--base-size-24` above and below,
and its whole body filled with the muted background of where it stands, its frame in the same state's emphasis border:
`--bgColor-attention-muted` and `--borderColor-attention-emphasis` while it waits, `--bgColor-success-muted` and
`--borderColor-success-emphasis` once ruled, `--bgColor-neutral-muted` and `--borderColor-neutral-emphasis` once
replaced or withdrawn, so that its colour says whether it waits; its head on `--bgColor-muted` as every card's; its
option boxes on `--bgColor-default`, a picked one's accent tint laid over it, so that they read apart from the card on
either tint and in either theme.

Its body is its question in body text semibold, which heads it; its options as option boxes (below); a `Fold`
"Considered, not workable (n)" holding them as a list of the document; how sure, a labelled box whose note is five pips
filled to the level and the level's words, holding why and "Overturned if" semibold before what would overturn it; where
it departs from an earlier ruling, a box with the attention edge; a `Fold` "Changes the records (n)", each record its
reference in code and what was done to it as a secondary `Label` over its words in a box, struck through when removed;
the link to the flow it carries; and, while it waits, `--base-size-12` under a `--borderWidth-thin`
`--borderColor-muted` line, the owner's words in a `Textarea` full width, then the primary small `Button` Confirm,
Choose this with another option picked, and the small `Button` Disagree, enabled once words are written and no other
option picked, and at the row's end whom a question goes to, `Checkbox`es as the group's box has them (`@architect`
ticked, `@engineer`), and the small `Button` Ask with a `ReplyIcon`, enabled once words are written and one is ticked.
In a work's group, under its body, the questions about it and their answers are one line in `--fgColor-muted`, a
`ReplyIcon`, "Asked about it:" and a link drawn as a button for each ("your question, 10:31", "the answer, 10:40"),
which brings that message into view and lights it.

Once ruled, first in its body the owner's ruling as the owner's text, a sheet of what is sent to an agent (below) under
the sub-heading "Your ruling", the option that stands semibold over the words and marked among the options; the rulings
before the latest are a `Fold` under it, each who ruled and when over its verdict. A design card of before decision
cards keeps each decision's verdict as a state's chip after its number, read only. An ask's answer, where its card shows
outside its conversation, is the owner's text alike under "Your answer", the option chosen semibold over the words and
marked among the options, and one the engine closed says why in the page's own words.

Instead of a `Flash` under the head saying where the card stands (success when confirmed or answered, warning while it
waits, danger when disagreed), who ruled or answered and when, and each verdict, because, as the owner decided, the band
repeated the chip, and a design card that waits, pinned in the band of what waits for the owner, said it three times.

### A decision card

A decision card is drawn as the mock the owner approved: its question first, semibold; its options as boxes one under
another, each its name, what it does, its cost and its risks, the one recommended marked, picked by a click while it
waits; those found not workable folded, with how many; how sure as five pips with why and what would overturn it; where
it departs from an earlier ruling; the records it changes folded, with how many, each its reference with its words; the
flow drawn from its branch, when it carries one; and, while it waits, a box of the owner's words and Confirm, which
becomes Choose this on another option picked, Disagree, which sends the words alone, and Ask, which asks about it
(below); once ruled, the owner's ruling, the option that stands over the owner's words; once withdrawn, why. It is ruled
in a work's group, folded in the bubble of its stage, the bar of what waits opening it; in its item's design it stands
at its anchor, read only, saying where it is ruled. It stands out from the text around it: a thick frame, room above and
below, and its whole body filled with the tint of where it stands, attention while it waits and success once ruled, its
options on the page's own background, the one recommended keeping its mark whatever is picked.

Instead of a card of several decisions, each a numbered fold with its own verdict, sent at once after a summary, because
a card is one question, ruled on its own, and the owner reads a question with the design's text before it; and instead
of a card drawn as a comment, a hairline round its head and body, because the owner could not tell where a card in the
design ends, and a tint that says whether it waits lets the owner see it at once.

### Asking about a decision card

The owner asks about a decision card that waits on the card itself, in the work's group: Ask, beside Confirm and
Disagree, sends the words in the card's box as a question, which rules nothing, the card waiting on; to the architect
unless the owner ticks the engineer, or both, as the group's box allows; the architect is given the card, the design it
stands in and the change to the records with the owner's words (`H8`), the engineer the words naming the card by its
number and key; the question and its answer each name the card over their words, a click bringing the card into view,
and the card links to them; when the answer finds the card wrong, the architect has the engineer change it, a new
version coming to the owner.

Instead of asking in the box at the bottom of the group, which names no card, because the owner asks where the card is
read, the messages view being where the owner acts, and whoever answers needs the card the question is about.

### One card for every card

Decision cards and asks of the owner share one card wherever the page shows them (an item's design, a work's group, the
round panel): a frame whose head gives the card's number, its status as a chip (waits for you, confirmed, you chose
another, disagreed, replaced, withdrawn; answered), for an ask that waits how long it has waited since the owner was
told of it, since an ask has no deadline and the engine only reminds the owner of the asks waiting once an hour, the
round that handed it in with its cell and time, the card it replaces, and who ruled or answered last and when; no band
under it; and boxes that hold what the agent wrote, word for word, under the page's labels. Among a work's cards the
current one of each key (the one waiting for a ruling, else the one ruled) is open and every other is folded to its
head, a replaced card's head naming the card that replaced it, at which round and when; a replacing card says first, in
a box of its own, what it changed from the card it replaced, in its agent's words. A decision card's body is drawn as
above; a card ruled more than once keeps its earlier rulings folded under it; a design card of before decision cards is
drawn as it was, each decision a numbered fold, read only; an ask's answer, where its card shows outside its
conversation, is the owner's text as a card's ruling is.

Instead of the replaced card open and the current one folded, a card as runs of labelled text and each place drawing its
cards its own way, because the owner opened a work's cards to find the replaced one open, read the rest as a pile of
text, and asked for one card style for every card the page shows; and instead of a band under the head saying where the
card stands with the owner, green, amber or red, with who ruled, when and each verdict, because the band repeated the
chip, as the owner decided.

### Options are the choice

The options of a decision card are themselves the owner's choice: a click picks one and a second click clears it, the
owner's pick and the option the agent recommends marked apart (both can be one), another pick than the one recommended
turning Confirm into Choose this; an ask's options are picked on their own boxes in its bubble, a click picking one and
a second clearing it, above the box that answers it; each option, and each approach compared on the item, is a box of
its own, one under another, its name over what it does, its cost and its risks, instead of a table with a column for
each, or the options repeated under the list as radio buttons or in a menu, because the owner said the list was being
said twice and that the columns were hard to read, and asked for an ask's choices as buttons in the bubble that asks it,
with the box that answers it after them.

### An ask's points have no kind

An ask shows each point as what it asks and what was checked, with no kind, nor any for an ask recorded with one before
asks lost their kinds, instead of the kind as a chip coloured by the group the flow put it in, because an ask carries no
kind: what it is about reads from what it says, and the architect, not a table of kinds, decides how to handle it.

### One style of document

Every document the owner reads, a policy, a design, renders as one document in one style wherever it shows (in a card of
a conversation, in the panel beside the one-to-one with an architect, on an item's page): each part under a heading in
plain words, and every list alike, numbered items, each its lead in bold and each other part a labelled paragraph in the
text's own style, with no box inside it (the approaches compared too, the one chosen marked), instead of one list as
bold-led bullets and another as boxes with "What follows" among fields as markdown, because the owner read the study's
two lists of one shape drawn two ways as something set apart, and a box inside what an agent handed back says nothing
more.

### A document the owner reads

A document the owner reads (a policy, the project's constitution, a design) is one document, `--base-size-12` between
its parts, each part's name a sub-heading, and every list of it an ordered list `--base-size-24` in, its items
`--base-size-8` apart, each its lead `strong` and each other part a paragraph in body medium opening with its name and a
colon (ours), instead of option boxes for a list of approaches already compared, because a document is read through, and
a box inside it would say it is something else. Where it fills a section or a tab by itself (the design on the item's
page, the policy in force, the draft awaiting approval, the constitution and each part of the notebook in the panel
beside the one-to-one with an architect) it has no box: it is that section's body text under its heading. Where it
stands among the page's own words it keeps the box of what an agent handed back: folded under a card of the page's own
in a conversation (a stage's design, the policy that took effect), or in a message, whose box it is in already.

Rejected:

- The box of what an agent handed back once round the whole, `--base-size-12` `--base-size-16` inside: rejected because
  a box round all a section holds frames it a second time and tells nothing its heading has not, as the owner decided,
  while a box still sets an agent's words apart where they stand among the page's.

### Option boxes

Options and approaches to choose from are ours: a box each, one under another, its name in title small over its fields,
whose names are sub-headings; the one recommended carries a `Label` `variant="success"` and a `--borderWidth-thick`
`--borderColor-success-emphasis` left edge; the one the owner picked a `Label` `variant="accent"` on
`--bgColor-accent-muted`, edged all round in `--borderColor-accent-emphasis`, a mark of its own beside the one
recommended, whose edge and `Label` stay whatever is picked, the one recommended picked too; a box that can be picked
turns `--bgColor-neutral-muted` under the pointer, because Primer has no card choice, and its colours for recommended
and selected are these.

## States, kinds and counts

### A state or a kind as a Label

The state of an item, a work or a round, and a kind (of a ground, a deviation, a decision), is a `Label` in the variant
its meaning has under the states above; a kind with no state meaning is `variant="secondary"`; an ask's point has no
kind to show, because a state read in one colour on one page must mean the same on the next.

### One look per kind of thing

One kind of thing has one look on every view (ours, the owner's rule): a state is a coloured chip, filled in its state's
emphasis colour (`--bgColor-accent-emphasis`, `-success-`, `-attention-`, `-danger-`, `-neutral-`) with its words in
`--fgColor-onEmphasis`, the same colour for the same state everywhere, vivid; a tag or a kind is an uncoloured chip,
outlined (Primer's secondary Label); a fact is `--fgColor-muted` words, facts on one line joined by a vertical bar, " |
" (one part, `joined()` in voices.tsx, or the same bar where only words go); a link among facts is Primer's `Link` in
`--fgColor-accent`, never muted. A chip is a rounded rectangle, `--borderRadius-medium`, not Primer's pill, and a
coloured and an uncoloured chip have one shape, height and corner; a chip's words are the size of the text of the line
it stands in, and a line of chips, facts and links has one text size. The look is set once for every Label
(`[data-component='Label']` in parts.css), so that a view that draws a Label gets it, and the states take their colours
from one part (`StateChip` and the badges of a work's, an item's and a round's state, components.tsx). It holds on the
lists of works, a work's line over its chart and an item's head first; the items' tree, as the owner chose, shows an
item's state by the colour of its card's edge and of its number instead of a chip (below).

Instead of Primer's Label as it comes, a pill outlined in its colour at body small whatever its line, a state's chip and
a tag's told apart only by the colour of their outline, and facts in a size of their own, because the owner found a line
of large and small words hard to read, asked that coloured and uncoloured chips be the same shape, size and corner and
not pills, and asked that each kind of thing look one way on every view; and instead of facts joined by middle dots with
the links among them muted, because the owner could not tell on a line of muted words and dots which parts could be
clicked: what can be clicked must look clickable, and a bar reads as a separator.

### A state as an icon

A state shown as an icon, where a row has room for a mark and not for a chip (the list of conversations: each
architect's one-to-one and each work's group), is one part, ours (`StateMark`, components.tsx), and its map of a work's
states (`WORK_MARK`) is the one every place that shows a work's state as an icon takes: running is the halo the chart
draws round the running cell (`RunningMark`, the one mark of a state that moves) at an icon's size, a ring 13px across
in the 16px of an Octicon, its arcs `--chart-halo-width-mark` wide so that it reads beside an Octicon's strokes; ready
`CircleIcon`, the halo's ring at rest; asking `QuestionIcon`; waiting `ClockIcon`; waiting for a free slot
`HourglassIcon`; paused `PauseIcon`; halted `AlertIcon`, GitHub's mark of an alert; stopping `NoEntryIcon`; done
`CheckCircleIcon`; ended `StopIcon`, GitHub's mark of a run stopped before its end (and a bubble of a thing that runs
and then ends takes these marks for how it stands, below); each other than running in the colour its state's chip is
filled in (`--bgColor-neutral-emphasis` ready and ended; `--bgColor-attention-emphasis` asking, waiting, waiting for a
free slot, paused and stopping; `--bgColor-danger-emphasis` halted; `--bgColor-success-emphasis` done), and each saying
on hovering its
chip's words; while nothing is known yet the mark's room stays empty, so that the rows stay in line.

The state an icon shows is worked out once, from what the work is doing now (`course` and `liveState`, components.tsx),
and the words beside it, the running mark in the head of its group and the halo on its chart are worked out from the
same, so that they never disagree. A stage is working, and wears the running halo, where a round of it runs or a tool
of it is under way, or where the work goes on and its next step is due and the engine takes it; it is not working while
it waits (for the owner, the architect, other items, a free slot, or a failed round to be run again), is held back, the
engine has halted every work, or the page cannot reach the engine, and once it is over. Ready is the work whose next step
is due and that nothing takes: the engine does not run it, as when it is out of reach or stopped, so that a ready ring
at rest is a true sign that the work does not move. The engine's step of a work that waits, which only looks whether
what it waits for came and writes no round, is not a round running and does not make the work look like it, and a work
waiting for a failed round to be run again (the wait before a retry, which grows with each failure) is waiting, the
clock, not ready, with the words "waiting to run the failed round again"; a work held back is not given that wait,
since the engine opens no round of a work held back, which waits for nothing and may be given a Step. A paused work the
owner started, which waits for a free slot, takes the icon of waiting for a free slot, as its words ("started, waiting
for a free slot") say, not the paused icon. While the page cannot reach the engine, no work is marked as working.

Instead of an icon worked out apart from the words, which drew a ring at rest for a work whose words said "paused by
Pause all", and of a halo that came on for every step of the engine that looked at a waiting work, because the owner
asked that the icon and the words say the same and that the mark show only what is at work.

An architect takes the mark of a work in the state it stands in, as the head of its one-to-one works it out
(`architectNow` and `architectMark`, architect.ts): working, where a round of it runs or its next round is due and the
engine takes it, is running; held back (paused by the owner,
with every work or by itself) is paused, and halted by the engine is halted; waiting for the owner's next words, its
answer to the owner's words having left the talk open, is waiting; a question of its own pinned for the owner's reply
while it neither runs nor is held back is asking; between rounds, idle or never run is ready; and hovering says its
head's words.

Instead of each conversation's kind as its mark (`ProjectRoadmapIcon` an architect's one-to-one, `CommentDiscussionIcon`
a work's group), because the owner asked that the list say at a glance how each work and the architect stand, a running
one plainly in motion, while the list's headings and each row's name already say what kind of row it is; wf-agents' own
and a group of projects, which stand for no work, keep their kind's Octicon. An item's state has its own map beside it
(`ITEM_MARK`), the one every place that shows an item's state as an icon takes (the items' tree's buttons, below), none
of its marks a work's mark of another meaning: new `IssueOpenedIcon`, GitHub's mark of an issue open, filed and not
taken up yet; queued `ListOrderedIcon`, the item's place in the queue; working `PlayIcon`, under way; done
`CheckCircleIcon` and stopped `StopIcon`, the marks of a work done and of a work ended, since an item is done or stopped
as its latest work is; cancelled `SkipIcon`, GitHub's mark of an issue closed as not planned.

### Halted and paused

A work the engine stopped for a problem the owner looks at is halted, a state of its own that the interface gives the
page (`halted`), drawn in the danger tone with GitHub's alert mark wherever a work's state shows (the list of
conversations and its buttons, the head of its group, the System view, the works' lists), the list's buttons showing it
at first; a pause is paused whoever made it, worded by who: paused by you, by the architect or by Pause all, with its
reason. A work Pause all holds, one the owner has not started since, that is ready, waits or has asked, is paused
(`PauseIcon`) wherever a work's state shows as an icon, a chip or a count (the list of conversations, the bubble of its
stage, the item's page, the works' lists and their buttons), as its words say, "paused by Pause all"; a round of it still
running goes on to its end and shows running until then. The list keeps in the browser the states the owner hid, not
those shown, so that a state the page comes to have shows until the owner hides it.

Instead of every pause one state, which the list drew as a yellow pause while the head drew the engine's stop in red,
and of "stopped (you paused it)", because the owner asked that the same work read the same on the left and in the head,
and that one word mean one state; halted, since stopped already names an item whose work ended. The usage limit's halt
of every work stays the bar's, since it is the engine's and lifts by itself.

### Waiting for a free slot

A work whose wait is over, the architect's answer come or what its row waited for, and that waits on, holding no slot,
until its machine has a slot free, is waiting for a free slot, as the engine's status names it, and not ready, because
the owner must see why a work whose answer came does not move; its chip says on hovering that it goes on once its
machine has a slot free, before any new work starts. So is a paused work whose pause was asked to be lifted, by the
command line or the architect, while its machine has none free, because a resume that does not show looks as if it
failed. It has Pause and no Start or Step, as a work the owner started and that waits for a slot has, because it is on
its way to going on and the owner's later word must be able to keep it paused: Pause withdraws the request.

### Stopping

A work whose cancel or start over was asked and that the engine has not stopped yet is stopping, as the engine's status
names it, in the attention tone with `NoEntryIcon`, in place of the state it reads, whatever that is (a round running,
ready, waiting or paused), because it goes no further than the step it is in, and a work that looks as if it goes on
after the owner cancelled it reads as a cancel that failed. Its chip says on hovering that it ends once the step it is in
has ended, its round's timer is not shown, and it has no Start, Pause, Step or Start over, since none of them would
change that. It is a state of its own and not a note beside the running chip, because the same word then stands for it
on the left and in the head, and the list's buttons can show or hide it as any other state.

### A count

A count is a `CounterLabel`: `variant="primary"` for what waits for the owner, secondary otherwise; a count in a
sentence stays words, because a count in a pill is read apart from the words round it, and the emphasis is kept for what
the owner must act on.

## Commands and forms

### A command

A command is a `Button`: `variant="primary"` for the one action a form or view exists for (Start all, Send, Rule on the
card), default for the rest, `variant="invisible"` for an action inside a line of text, `IconButton` with its
`aria-label` for an action drawn as an icon; size small inside a row or a list, medium in the bar and in a form, because
one primary button tells the owner which action a form exists for.

### Form controls

A setting that one click turns on or off (Keep warm) is a `ToggleSwitch` with its label beside it, size medium in the
bar so that it is as tall as the button beside it, its On and Off words from the page's message catalog; one verdict of
two that starts unset (confirm or disagree) is a `ButtonGroup` of small `Button`s, the one set marked by a leading
`CheckIcon` and `aria-pressed`, instead of a `SegmentedControl`, which always shows one segment chosen; a choice from a
short fixed list (an item's tier) is a `Select` size small; the words the owner writes are a `Textarea`, full width,
resized only in height; beside the button that sends them (the box at the bottom of a conversation, the box that answers
a message) they are one part (`SayBox`, ours), one line at rest, as tall as the button beside it and level with it,
growing upwards with what is written while the button stays on its bottom line, up to twelve lines, past which they
scroll, with no handle to drag, instead of a box two rows tall the owner drags to a size with Send at its bottom right,
because the owner found a box taller than its button, the button hanging at its corner, ugly, and chose one line that
grows.

### Images in messages

Every box of the owner's words in a conversation (the one at the bottom, and the small one in a bubble that answers it)
takes images with the words: picked from the computer with the button beside the words, several at once, or pasted into
the words; they wait over the words, each a thumbnail the owner can take out, with how many against the ten a message
carries, and go with the words as one message when sent, or alone. Only PNG, JPEG, GIF and WebP images of at most 5 MB,
at most ten to a message, are taken, and the box says plainly why it did not take a file; the interface refuses the
same. The images are kept in the database with the words they came with, shown under the words in their bubble and in
the rounds that were given them, each opening whole in a new tab, and reach the agents with the words, in their rounds'
messages.

Instead of a file the owner's words point to, for each agent to be let read and to find, because the owner asked for
images sent the clean way, in the message, and for the box to take local files and pasted images that wait there to go
with the words.

### Images in the box

The words the owner writes take images with them, in the same part (`SayBox`) wherever it stands (the box at the bottom
of a work's group, of the one-to-one with an architect and of a group of projects, and the box that answers a message):
at the left of the words an `IconButton` with a `PaperclipIcon`, as tall as the words' one line (small beside a small
box), saying on hovering what it adds (PNG, JPEG, GIF or WebP images, at most 5 MB each and 10 to a message, and that
one can be pasted), opens the file picker, several at once; an image pasted into the words is added, while a paste that
carries text pastes the text; the images added wait over the words and the button, a row that wraps, `--base-size-12`
apart, each a thumbnail `--base-size-48` square cut to fill it, `--borderWidth-thin` `--borderColor-default` and
`--borderRadius-medium`, with an invisible small `IconButton` with an `XIcon` over its top right corner on
`--overlay-bgColor`, `--borderRadius-full` and `--shadow-floating-small`, which takes it out, and after them how many
against the ten ("2 of 10") in small `--fgColor-muted` text; what the box refuses (a file that is no such image, an
image over 5 MB, one past the tenth) is said under the images, plainly and file by file, in small `--fgColor-danger`
text, until the box takes or loses an image or is emptied; at ten the button is disabled. Send sends the words and the
images as one message, or the images alone, and empties both.

Instead of a file area of the page's own beside the box, or images sent apart from the words, because the owner asked
for a way to add local files and to paste images into the box, the images waiting there to go with the words that
follow, each removable, several at once, ten at most, and a thumbnail with its cross is how the owner's chat apps show
what waits to be sent.

### Images sent

The images the owner sent show under the owner's words as thumbnails (`Thumbs`, ours), in the owner's sheet in a
conversation and in the owner's answer in the bubble it answers alike: a row that wraps, `--base-size-8` apart and under
the words, each `--base-size-96` high and at most four times as wide, its proportions kept, `--borderWidth-thin`
`--borderColor-default` and `--borderRadius-medium`; a click opens it at full size in a new tab, as the interface serves
it by its id. A round's panel shows under In the images its message named with the same thumbnails, after the message as
sent, each a row ("image 12 you sent", `image:12`, sent after the text or already in its session, with the words that
named it).

Instead of a `Dialog` that holds the image, because a tab is the simplest way to see an image whole and at its own size,
and the browser already zooms and saves it.

### Words not yet sent

What the owner has typed into a box and not sent is kept, in every box of the page that takes words: the box at the
bottom of a conversation (its words, its images and whom it is for), the small box in a bubble that answers an ask and
the one that answers an engineer's question, the box on a decision card, the reason for closing an observation, the
settings form, the name of an account to add, and the ref of the flow box's comparison. The owner leaves a page for
another, opens another conversation, reloads the page, or the browser drops a tab it kept in the background, and finds
the words where they were left, because the owner said that losing them is a serious fault and that every box of the
page should keep what is typed in it. Instead of keeping them while the box is shown (a `useState`), which loses them
when the box goes away, and instead of keeping every page shown, which is what the page was made lighter to stop doing.

- **One draft for each box, under a key made of the number of what the box belongs to.** A box takes its words from
  `useDraft(key)` (`src/drafts.ts`): `conv:<conversation key>`, `ask:<id>`, `question:<work>:<matter>`, `card:<id>`,
  `obs-close:<id>`, `settings`, `account:<cli>`, `flow-compare-ref`. A key is never made of where the box stands or
  of a title, so that one box never shows another's words; two boxes with one key are one box. The keys are not
  tied to a database: a draft the owner never sent goes after 30 days.
- **Kept at each change, in this browser.** Every change is written to `localStorage` at once, with no timer and no hook
  on leaving, because a browser that drops a tab does not promise to run one. The images of a box are kept apart, when
  they change, so that images too big for the browser cost the words nothing; they stay in memory then. Where the
  browser keeps nothing at all, drafts live in memory for as long as the page does. A change in another tab to a draft
  (a send, a clear) is followed. A draft is not kept on the server and does not follow the owner to another browser,
  because the owner asked for neither.
- **Words come before images.** Images and words share the browser's room, so images could crowd out the words of
  any box. When a write of words fails because the browser is full (and for no other failure), the images of drafts are
  taken out of the browser one draft at a time, the draft changed longest ago first and the draft's own last, and the
  write is tried again after each, until the words are in. The draft whose images were taken out keeps the time it was last
  changed, because the order drafts give way in and their 30 days go by it. An image that does not fit when its own box
  is written is left out the same way, and no other draft's images are taken out for it, since neither box's images are
  clearly worth more.
- **Images not kept are said, after the page is loaded again.** The entry of a draft records how many of its images the
  browser did not keep (`lost`). A box says that after a reload, in the line under the images where it says what it
  refused, "N images could not be kept when the page was left; paste them again", and not on the page where the images
  are still in the box, since then it would be wrong. The line goes when an image is added or taken out, when the box is
  emptied, and when what it holds is sent, as the line about what the box refused does; typing does not end it, and
  there is no button to close it. An entry that holds only this count (a draft of images alone that lost them) goes
  after 30 days like any other.
- **Sending is the draft's.** A box sends with `draft.send(request, after)`. While the request is out the draft is
  `busy`, and a box shown again meanwhile is greyed; a request that comes through empties the draft in the store, so
  that a box that went away while it was on its way does not bring the words back; one that fails leaves the words and
  says why (`error`), also in a box shown again.
- **Not kept**: the option picked on an ask or a card, whom a reply answers, and the "hand it back" tick. They are not
  words, and a box that comes back without them shows it; kept, they could turn into something else unseen.
- **The settings form is kept against the configuration commit it was filled from.** The unsaved changes come back
  where the commit is the same; where the files changed since, they are dropped and the form says so, because writing
  them back would put old values over the new ones.
- **Words put in from outside** (a quote, words taken back) go through `putDraft(key, words, 'append' | 'replace')`,
  which works whether the box is shown or not.
- **A new box takes a draft, and the lint holds it to that.** `SayBox`, `SendForm`, `DraftArea` and `DraftInput` (all in
  `src/parts.tsx`) take a `Draft`, and `FieldInput` is for the fields of a form kept as one draft; Primer's `Textarea`
  and `TextInput`, and the HTML `input` and `textarea`, are drawn nowhere else, and the page's lint refuses them.

### Asking before a command

A command whose effect reaches past what it names (switching the agents' login account, after which every line starts a
new session; starting a work over, which cuts off its round under way and ends it for a new work on its item; removing a
dependency, after which a work that waits for the other item goes on without it and may merge first, and which the page
cannot make again) asks first in Primer's `ConfirmationDialog`, its title the question, its body what follows, its
confirm button the command's verb, instead of acting at the click, because the owner must know what follows before it
happens, and a question the page asks over itself is answered once.

## Links, names and lists

### A link

A link is Primer's `Link` (`--fgColor-accent`, underlined under the pointer), never `muted`, in small text and among a
line's facts as anywhere else; a round's name and an item's number and title are links of this kind, an item's number in
`--fgColor-muted` before its title in the colour of the text around it, because a link is recognised by its colour,
instead of a muted link inside small text and among a line's facts, which the owner could not tell from the facts round
it.

### A link drawn as a button

A link drawn as a button (`as="button"`, which opens something on the page) can be selected as text can (ours,
`user-select: text`, where Primer makes it unselectable), because the owner copied a round's panel to say what was wrong
with it, and every round and cell in it, all links, was left out of the copy.

### A code of the flow in a sentence

A code the flow names a thing by (a cell's code, a row, a node's path) in a sentence of the page's is
`--fontStack-monospace` at `--text-codeInline-size` in the colour of the text round it (after a thing's name in words it
is small and muted, below), with no background and no weight of its own (ours, as GitHub writes a commit's hash), while
code an agent wrote inline keeps the markdown code's background, because a cell's code beside its title is a name, not
code to copy.

### Label and value pairs

Label and value pairs are ours in Primer's spirit: a two-column grid, the name in body medium `--fgColor-muted` (never
bold), the value in `--fgColor-default`, `--base-size-4` between rows and `--base-size-16` between the columns, because
the value is what is read and the name only says which it is.

### A byline

Who wrote something, when and in which round is one line of small text: who in `--fgColor-default` semibold when there
is a who (as GitHub names a comment's author), the round at its cell (below), the round a link as every link is, and the
time in `--fgColor-muted`, `--base-size-8` apart, because who wrote something is read before when and where, and the
rest is checked only now and then.

### A round at a cell

A round at a cell, wherever the page names the two together (a round panel's title, the round beside the row it came in
by and the row it left by, who wrote something, a decision card's head, a tip of the usage chart), is one part, ours,
written one way: the round's name first, then the cell's code and title in parentheses, "R10-55 (N0 What can be
picked)", the round alone where its cell is not known and the cell alone where no round has run there yet, each a link
where it opens something, instead of "R10-55 N0 What can be picked" on one line and "N2 Route the matter, R10-57" on the
next, because the owner found the two lines of one panel written two ways and asked for one.

### A round's name

The page names a round R<work>-<n> wherever it shows one (a work's rounds and its rounds at a cell, a round's panel and
the rounds before and after it, the decision cards, asks, messages and interjections, the item's blocks and checklist)
and in its addresses (`/works/5?round=R5-9`), and reads a round from the interface by its work and its number in the
work (`/api/v1/works/5/rounds/9`), instead of by one number across all works, because the owner reads which round of its
work a round is, and a number across all works told the owner nothing of it.

### A work's and an observation's name

The page names a work W<n> wherever it shows one (the Works view and a work's page, the System view, an item's works,
and the page's own sentences about a work: "taken over by W31", "Start W5 over?"), "#" being an item's alone, and a
work's group in the conversations by its item's number and title, then the work's name, "#5 Run works in parallel W35",
the item's words cut where the line ends and the work's name whole in `--fgColor-muted`, in the list and the group's
head alike; instead of "#35" for work 35, because the owner read "#5" in the list of conversations, work 5 on item 1, as
item 5, a different item the items' tree names "#5", and a group is a work's, one of several an item may have had, only
one of them live. An observation is O<n> alike, on the page (a matter's result, the observation's own head) and in the
engine's words and inputs ("recorded as observation O69", the architect's overview keyed O<n>), so that "#" is an item's
alone there too, instead of "#69" beside "#17" for an item in one list, which the owner could tell apart only by the
words before them.

### Names before codes

What the owner reads is headed by its name in words, the code the flow names it by after it, small and muted: an input
by the name the flow gives it, the flow giving every input one, which no agent is sent but for the inputs every agent's
cell has, which the agents know by their names; a field of a return by a title the flow gives it, a few words written
for the owner, which the compiler keeps out of everything an agent is sent (the return schema, the formats and the
message), since the agent reads the field's description, and which `wf-agents flow check` requires of every field, as it
requires a name of every input; a field the flow gives no title yet by its key in words; a cell by its code and title. A
field's type and flags (text, a list of at least one, optional, if any, shown on the item, read by `R2`) are small muted
words after its code.

Drawn, a thing the flow names where its name heads what the owner reads (an input, a field, a form, a tool) is its name
in words in the step of the heading it is, then its code in `--fontStack-monospace` at `--text-body-size-small` in
`--fgColor-muted`, then its type and flags as body small `--fgColor-muted` words joined by a bar, with no border and no
background, a mark the owner looks for (the main path) a secondary `Label`; a list of such things that each open (the
inputs, the rules and formats, what a session held) is rows `--base-size-2` apart, each a `Details` whose summary is the
chevron, its number if it has one, its name, its code and its facts, wrapping under the name, its content indented by
the chevron's width. The titles have one place, the flow, and the item page takes its names for the fields of the
understanding and the design from them instead of from its own code.

Instead of keys in bold code heading what was handed back and the return format, a code set in the colour of the text
round it, and each type in a bordered chip, because the owner reads "Where this differs from the item as written" at
once and `vs_item` only once learned, scans a list by names in words and looks at a code only to match it, and a row of
bordered chips reads as things to click.

### Lists of objects

A list of objects that are read (what an item depends on, the notes under a line of the checklist) is rows one under
another, `--base-size-8` apart, each its link first and its facts after it, a dependency's row, both ways on an item's
page, ending in the owner's removal of it, an invisible small `IconButton` with an `XIcon`; a list of objects one opens
is Primer's `NavList` (the conversations, the observations), with counts as trailing `CounterLabel`s, or for the items'
tree ours (below), because a list is scanned by its first words, and a list one moves through is a navigation, not
prose.

## The items

### A tree of what depends on what

The items view is each project's items as a tree of what depends on what, on the left, and the item picked, in place, on
the right (while none is picked, the first the tree shows, rather than an empty pane asking for one, but at a phone's
width, where it would take the tree's place, none), the split between them dragged and kept in the browser, instead of a
flat table by state and a page of its own for an item, because the owner reads the items by what waits for what, as the
board before this one showed them, and goes from item to item, such as to rule on cards one after another. The items
nothing waits for are at the top, and under each, indented with a rule down their left and foldable, the items it
depends on; an item several items depend on is in full under the first and a line that leads to it under the others, so
that the tree stays a tree. A row is a card whose left edge is in its item's state's colour; it says the item's number,
a fixed-width number in white on that colour that says the state on hovering, and its title, what it needs from the
owner as chips, how many of what it depends on are done, its place in the queue and its work's stage in words where the
row has room, how far through the stages on hovering, rather than a bar behind the row, and no chip of its state,
instead of that chip at its end, because the owner, shown the board before this one, found its tree better: the colour
on the left says the state and the indent reads at a glance.

### The states' buttons

Buttons, each in its state's colour, show or hide that state's items, the done and the cancelled hidden at first,
because what is settled is not what the owner looks for; each button is its state's icon and its count with no word, the
state and the count said on hovering, instead of the state's name, because the owner found the row of named buttons too
long for one line. They stand in the project's head (`ProjectHead`, above), at the right end of the row of the project's
name, instead of on a row of their own under it, because the owner asked that no row be spent on them.

### The tree's cards

The items' tree is ours, after the board before this one (v1): each item a card, a `--borderWidth-thin`
`--borderColor-default` outline with `--borderRadius-medium` on `--bgColor-default`, its left edge
`--borderWidth-thicker` in its state's colour, `--base-size-4` `--base-size-8` inside, as a row of a list rather than a
card of `--base-size-16`; in it, on one line, the toggle of the cards under it (a `ChevronDownIcon` or
`ChevronRightIcon` in `--fgColor-muted`, in `--base-size-24` of room at the card's left, left empty on a card with none,
so that the numbers stand in one column), the item's number as a chip of a fixed width (five characters of
`--fontStack-monospace` at `--text-codeInline-size`, `--base-size-6` either side, centred) filled in its state's
`--bgColor-*-emphasis`, its digits white (`--fgColor-onEmphasis`), saying its state on hovering, its title on one line,
cut with an ellipsis and whole on hovering, what it needs from the owner as chips, and at its end its facts in muted
words joined by a bar; no chip of its state.

A state's colours are those of its tone (`ITEM_TONE`), as its chip has it everywhere: the edge
`--borderColor-*-emphasis`, and the buttons over the tree that show or hide each state's items filled in
`--bgColor-*-emphasis`, their mark white and their count on a white translucent pill (`--counter-onEmphasis-bgColor`,
ours), while its items show, and filled in its tint (`--bgColor-*-muted`) with no edge, their mark and count its
`--fgColor-*`, while they hide, as a chip or a button in a colour is (above), all from one set (items.css), so that the
buttons and the cards match. The items an item depends on are a list under its card, `--base-size-4` under it and
`--base-size-8` in, with a `--borderWidth-thick` `--borderColor-default` rule down their left and `--base-size-16`
between the rule and their cards; cards are `--base-size-4` apart. The card of the item open is on
`--bgColor-accent-muted`, a card under the pointer on `--bgColor-neutral-muted`. A click on a card opens its item, and
its words can be selected and copied, since it is a button with `user-select: text` (a browser drags a link instead of
selecting its words); a double click on a card with cards under it folds or unfolds them, into the same fold its toggle
keeps, and selects no word, while on a card with none under it a double click does what a click does.

Where the pane is too narrow for the title's least width (12ch) beside what follows it, what follows wraps under it, so
that the title keeps a few words at any width; the stage shows only where the pane is wider than 420px. Each card and
each toggle is a stop of the Tab key, Enter opening or folding it.

Instead of Primer's `TreeView`, whose rows the page could draw as cards and set in by a visible step only through the
names Primer gives its parts unhashed, and whose arrow keys and type-ahead the tree gives up; instead of each row's
state as its chip at its end; and instead of the row's left edge left to Primer's mark of the row picked, the coloured
edge dropped earlier because the owner read it as the row picked now standing on a card outlined all round, the row
picked shown by its fill; and instead of the number in its state's colour on its state's tint, since the owner chose
white words on the states' solid colours (the palette, above); because the owner, shown the v1 board's tree, found it
better: the colour, the number and the name on the left, the colour standing for the state and the indent read at a
glance, the number a fixed-width number, and no chip of working or done needed after it.

### The buttons as one part

The buttons are one part, ours (`StateFilter`, components.tsx), which the list of conversations and the works table
show or hide each project's works with too (`WorkStateFilter`, the work's states in the one order, tones and marks),
and which the observations pick theirs by tag and by state with in the part's pick mode (below), each list's choice one
for all its projects and kept in the browser (`useShownStates`), every part that reads a list's choice reading the same
one, so that the bar's counts change with the list's buttons, and the works table's with the conversations'. In every
list they stand in the project's head, at the right end of the row of its name. The pick mode differs in
what a button means and nothing in how it looks: a button on is one picked rather than a state shown, the list shows
every row while nothing is picked and else only what the picks match, and every button stays drawn while picked though
its count is 0, so that a pick can be taken back; the items' tree and the conversations keep their show-or-hide mode,
since there the owner sets which states are out of the way, done and ended hidden at first. Each button is its state's
mark and its count, with no words: a small invisible `Button`, the mark its leading visual and the count its `count`,
the mark a work's state's or an item's from the maps of a state shown as an icon (above), the running halo's arcs white
while its works show, as every mark on a filled button is, since its blue arc would not show on the running state's
blue; its name and what it says on hovering are its state, its count and what a click does ("4 works are paused. Click
to hide them."); whether its rows show is said by the fill alone, with no tick.

Instead of the state's name in words beside its count, with a tick while its rows show, because the owner found the row
of worded buttons too long for one line of the list's pane.

### An item's head

An item's page is read only; its head is the item's number, muted, and its title ("#1 Document the commands"), with its
facts after it on the title's line: its project, its tier, the way its latest work went when that is not through the design,
as the rows the work took give it ("Implemented straight from the research" for a work whose research judged it could be
started directly, or "Sent to the design after it began" for one that began so and found the plan did not hold; what
the way leaves out, or why the work went on as it did, said on hovering), and nothing for a work that went through the
design, nor before the research answers, since nothing marks an item for a way, its state with the items it waits for,
and its pull request; and under them, while a work goes on
on it, that work once, with how it stands, its stage and cell, what it waits for and its group (above), instead of
"Item 1", its state beside the title, the work and what it waits for on lines of their own, what the item delivers, and
the way through the design, which most works go, is not written, because the owner decided, and because which works
skip the design, and which began so and turned back to it, is for the owner to see at a glance.

### An item's page

An item's page puts first what needs the owner (the asks a work that still goes on waits on, each saying where it is
answered, and one line naming the decision cards that wait in its design, each leading to its card), then where its
latest work stands as a checklist, folded at first since the owner reads the item for its design, then the problem, the
design, the acceptance, the changes on the way and what it depends on, each open, and its history folded (decision cards
replaced or withdrawn, design cards of before, earlier asks, the works that ended, never the one that goes on, which is
at the top), each block naming the round it came from; the tier the item is at is said, and set in its work's group, or,
with no work, by telling the architect in the one-to-one; instead of what the works handed back as one run of folded
values in the order handed in, because the owner opens an item to see what it needs and where it stands, and reads the
problem and the design as the thing itself.

The design block shows the approach chosen, then the design itself, each decision card standing in it at its anchor,
read only and saying where it is ruled, its work's group, then, folded, why this one and not the others, instead of why
first, because the owner reads a design to see what it is; while the approaches compared wait for a choice, it says
plainly that none is chosen, where the engineer stands on them as its latest round said it, and what the choice waits
for, and an approach is marked chosen only by a choice made after the approaches it chose among, instead of six boxes
with no word on whether one was chosen, or a choice matched by its name to approaches compared again after it. The
checklist has one line per cell the work has been at, in the order it first got there, ticked, a cell reached again
staying one line that counts how many times; the cell it is at, with what it waits for and its latest progress note; and
the cells ahead on the main path as the chart marks it, since a loop cannot be foreseen; the engine's steps that only
route the work on have no line, and the rounds of a cell that serves another, such as the tools, count under the cell
that asked; each line opens to its progress notes.

The problem is the engineer's understanding once research has run, the item as written folded under it; the design is
the approach chosen as its heading, the design itself and the measure, with where it changes, what it leaves out, its
pitfalls, why and the approaches compared folded; the acceptance is shown once, the locked one with the round that
locked it, each condition with how the work said it was met. A condition checked after the merge says so in a sentence, in
place of how it was met, with what result means it holds; what its checks came to is not shown yet.

### Fields drawn by their shape

Each field of an item's page is drawn by its shape in the flow's formats, and a field the page does not know is shown as
labelled text, never as a tree of JSON, because a tree of JSON is not read.

## The observations

### The view

The observations, what each project's architect recorded on the project's behalf (a problem, an idea, feedback not yet
worth an item), are a view of their own in the bar, right after the items (ours on Primer's parts): each project's under
its head (`ProjectHead`, above) as a `NavList.Group` of one `NavList`, as the list of conversations groups its rows; in
the head, after the name and at the right end of the same row, the buttons that pick them (`StateFilter` in its pick
mode, above), in two groups `--base-size-12` apart, their buttons `--base-size-4` apart: one for each of the project's tags that any of them carries, in the project's order, then one for
those that carry none of its tags ("no tag"); and "open" and "handled" (taken in by an item or closed), each where any
is so; each its name and its count, since a tag has no mark, filled in the neutral tone, which stands for no meaning of
its own, what the tag covers said on hovering after the count and what a click does. Nothing is picked at first, and
every observation of the project shows, the handled ones with the rest; once something is picked only what matches
shows: within a group a row matches any pick, and across the groups it must match each; each count says how many a
button would show under the picks of the other group, and a pick the project has nothing for keeps its button, with 0,
to be taken back; while anything is picked a small invisible `Button` with an `XIcon`, "Clear filters", ends the buttons
and takes every pick back; the buttons wrap among themselves, to the right, where they alone are wider than the pane.

The picks are one for every project and kept in the browser. Then a row for each, the open ones first, each newest
first, one block parted from the next by a `--borderWidth-thin` `--borderColor-muted` line, `--base-size-8` above and
below its lines: its title, as the markdown part draws it, with no link since the row is one, cut to one line in the
body's weight (rather than the semibold Primer gives a label over a description), and under it its content, the words
of its blocks one after another, cut to three lines in the muted colour and whole on hovering; one recorded before
observations had titles leads with its content alone, cut to three lines; and under them, as a block
`NavList.Description`, its tags as uncoloured chips, how
it was handled as neutral chips ("taken in by #12", "closed: outdated", why on hovering) and its facts joined by a bar
(how many times it was reported, when it was recorded, where it came from), cut to one line with an ellipsis and whole
on hovering, a row handled greyed in place as a work's conversation over is; the one open stays in its list whatever is
picked.

The split is the items' (`ListSplit`, above): while none is open the list fills the view, and a row opens its
observation beside the list as a reading page: a `PageHeader` titled with the observation's title, the one its row has
(for one with no title, the words it starts with), and after a bar its number ("O12"), its project, tags and facts, and
at its end an invisible small `IconButton` with `XIcon` that leaves the list alone; then what it is, whole, as
the markdown part draws a text an agent wrote, so that an item's number links and a diagram draws; then, each a labelled
text, where it came from, what it refers to, the item that took it in (an item's link) and why it was closed; and, while
it is not closed, the owner's close as a `Fold` ("Close it") holding the form that answers or says something
(`SendForm`), the words saying why, which it cannot be closed without, and Close, in the page's own box rather than the
browser's prompt, which the page's frame may block. An address names the observation open and the one project whose
observations alone the view shows, which says so over the list with a link to every project's; an architect's matter's
bubble lists the observations it recorded by their titles, each leading to it, and leads there by the counts of those it
merged or closed.

Instead of a tab of the architect's page, which does not change with the round chosen, a card of each in a conversation,
which showed each a second time, or a folded part under each project's items, because observations belong to the
project, not to a round of the architect: they are its pool of what is not an item yet, and there will be far more of
them than of items, which they would crowd; and filtered by their tags, rather than by a choice of one tag and a search,
because there will be many and each carries the project's tags. Picked rather than shown or hidden, with every
observation shown until something is picked, a clear of every pick, and a line between rows, instead of every tag shown
at first, handled hidden, and rows run together, because the owner found the dense rows hard to tell apart and wanted to
start from everything and narrow it by a few picks, then go back to everything in one click. A row leads with the
observation's title and shows its content under it, rather than the start of its text cut to one line, which showed only
the first words and left the owner to open each to know what it was; the open observation is titled as its row is, since
"Observation 12" said nothing the number among its facts does not; one with no title keeps the start of its text, and
none is given a title it was not written with.

## Tables, tabs, numbers and menus

### A table

A table is ours in Primer DataTable's look: a `--borderColor-default` box with `--borderRadius-medium`, its header row
on `--bgColor-muted` in body medium semibold `--fgColor-default`, its cells `--base-size-8` `--base-size-12` inside and
lines of `--borderColor-muted` between rows, numbers right-aligned, instead of Primer DataTable's muted header, because
a header is a heading and is not muted.

### Tabs

Tabs inside a view or a panel are Primer's `UnderlinePanels` (from `@primer/react/experimental`), each tab's count its
`counter`; tabs that change the address are an `UnderlineNav`, because Primer's tabs carry the keyboard and the overflow
a panel needs.

### Tabs on one line

The tabs over the panel stay on one line, close together with their counts, and scroll sideways when they still do not
fit, because tabs wrapping onto two lines took a line from what they show.

### A number on its own

A number shown on its own (a work's cost, the asks to answer) is a tile, ours: what it counts in small `--fgColor-muted`
text over the number in title medium and a line of small `--fgColor-muted` text under it, on `--bgColor-muted` with
`--borderRadius-medium` and `--base-size-8` `--base-size-12` inside and no border, a tile that leads to its list turning
`--bgColor-neutral-muted` under the pointer; the marks that draw numbers (bars, a stacked bar, columns) take their
series' colours from Primer's data visualisation scale in one order (`--data-blue-color-emphasis`, then orange, green
and purple) and what is none of them `--data-gray-color-muted`, the shares of a round's cost blue, orange and green
wherever they show, with their legend's keys as dots, their labels small text and their tips Primer's tooltip colours
(`--tooltip-bgColor`, `--tooltip-fgColor`), because a border round it would make a tile read as the box of an agent's
words and an edge at its left as the sheet of what is sent to one, and one order of hues lets a colour be learned once.

### A work's controls

The owner's Start, Pause and Step of a work, the choice of a new session for them, and, for a work on an item, its Start
over after them, are small `IconButton`s, the choice an `IconButton` with `aria-pressed` drawn as the option picked
(`--bgColor-accent-muted` edged `--borderColor-accent-muted`, `--fgColor-accent`) while pressed; their icons are the
page's own at Octicons' size (a triangle, two bars, a triangle against a bar, a speech bubble holding a plus, an arrow
that turns back round to where it began), since Octicons has no step, no new session and no start over and draws play
and pause in circles; what each does, or why it cannot now, is said on hovering, because the five are one set and must
look it.

### A choice from a list that grows

A choice of what a view shows from a list that grows (the flow's stored versions) is an `ActionMenu` whose button, size
small, names what is shown ("Flow v12"), its items a single-selection `ActionList` with each one's facts as an
`ActionList.Description` and the one in use a `Label` `variant="success"` as its trailing visual (as GitHub marks a
release Latest), instead of a native select whose options carry the facts as characters, because the button reads as a
name, and the facts belong to the moment of choosing.

### The period counted over

The period a view counts over (the System view's, an architect's usage) is one part, ours (`PeriodSwitch`): "Counted
over" in `--fgColor-muted`, then each period the view offers as a `Link`, the one counted over in `--fgColor-default`
semibold, kept in the address; an architect's usage offers the last day, the last 7 days (at first), the last 30 days
and all time, instead of a period switch of each view's own or a `Select`, because the System view drew it so first, and
a period is read with the others beside it.

### A command to one of a list that grows

A command the owner sends to one of a list that grows (a fault of wf-agents forwarded to a project's architect) is an
`ActionMenu` whose button, size small, names the command with an ellipsis ("Forward to..."), its items a plain
`ActionList`, each a target in words, one the command already went to `disabled` with the word for it ("Forwarded") as
its trailing visual; where the command went shows after it as small `--fgColor-muted` lines, instead of a button for
each target, because the targets grow with the board, and a target done already must show and must not be picked again.

## The window over a chart

### A chart fills the window under the bar

A flow's state machine fills the window under the bar, with a "?" in its corner that opens what every mark on it means
(who does each cell's work, the reasoning effort each agent's cell runs at, the rules and the interjection, the circles,
the engine's actions, the stores and their lines, the moves an agent cell makes of its own that the chart does not draw,
and on a work's chart each of the work's own marks), no taller than the chart under the tools over its top corner, what
does not fit scrolling inside it, and what is open on it (a cell, a rule, the interjection, the actions on a line, a
store, a comparison, a round) shows in a window floating over its right side while it is open, which never moves the
chart, whose width drags at its left edge and is kept in the browser, a double click giving the window its own width
again, and which minimises to its one head row, the chart centring what it centres on in the part the window leaves,
instead of a legend that grew upward past the window's top, behind the bar, where its start could not be read, and
instead of a column docked beside the chart, because the chart is what the owner reads the flow from: a column kept for
a panel that is often empty halves it, and moved it each time it opened, closed or was dragged, while a window over it
is kept off what the owner reads by the chart centring beside it and by minimising it to a row; and a mark nothing
explains is not read.

### Controls that float over a chart

Controls that float over a chart (the flow's version and Compare) are ours: one surface on `--overlay-bgColor` with a
`--borderWidth-thin` `--borderColor-default` border, `--borderRadius-medium`, `--shadow-floating-small` and
`--base-size-4` inside, its controls one size, `--base-size-8` apart, and what the chart floats in the same corner (its
map) under it at a distance the surface's height sets, the map's cells and stores filled and edged as the chart's are,
by the same classes, at the map's own line width, each stage's box on it with its stage's letter in its middle, its only
text, body small semibold `--fgColor-default` ringed in the surface's colour, so that a stage is found on the map at a
glance, which boxes of one grey alone do not allow, and under the map, `--base-size-8` apart, or in its place while it
hides, the chart's zoom ("85%") in small `--fgColor-muted` text on a surface of the same look with `--base-size-4` at
its sides; what floats over the whole page (Primer's menus and tooltips) is above the bar and the chart, at
`--zIndex-dropdown`, because the chart is read under them, and a group of controls is one thing to the eye.

### The window

A chart's panels (on a work's page and an architect's the page's panel in tabs, and on every chart page the panel of
what is open on the chart) are in one window floating over the chart's right side (ours, `Dock`, one part and one piece
of code for the flow page, a work's page and an architect's): on the surface of the controls that float over a chart
(above), `--base-size-8` in from the chart's top, right and bottom edges, it covers the chart, which keeps its whole
width under it, and opening, minimising, expanding or dragging the window never lays the chart out again and never moves
it. Its left edge drags its width (at least 320px and at most the chart's width less 160px; half the chart's width at
first on a work's page and an architect's, 560px on the flow page), kept in the browser once for the three pages, read
and written so that the page works without it. A button at the right end of its top row, an invisible small `IconButton`
with a `ChevronUpIcon`, minimises it to that row alone at the chart's top right, as wide as it was: the head of what is
open, its title with where it stands, then the button, the way back and who does it left out; with nothing open, the
tabs with their counts, which go on counting, then the button. The button, then with a `ChevronDownIcon`, expands it to
where it was (the same tab, what each part had scrolled to, what is open); a tab picked in the row expands it to that
tab.

A click on the top row itself, minimised or expanded (on the head of what is open, its title with where it stands, or on
the row beside the tabs; not on a tab, a link or a button in it, which does what it does), minimises or expands the
window as the button does, while a drag across the row's text selects the text and does neither. The window starts
minimised each time a chart page opens, and nothing hides it. The window has no close, and neither Escape nor a click on
the chart's background closes what is open in it: what is open stays until something else is opened on the chart or the
way back is taken, the page's tabs on a work's page and an architect's staying over it, and on the flow page, which has
no panel of its own, the window is there from the first thing opened, what is open alone in it with no close of its own.
On a work's page and an architect's, what is open split under the page's panel has its close while the window is
expanded, an invisible small `IconButton` with an `XIcon` at the end of its head, which lets go of it (the round chosen,
with the arrows it lights on the chart, or the cell) and gives the page's panel the window's whole height again, neither
minimising nor closing the window. A click on the chart that opens what it lands on (a cell, a store, a rule) expands
the window to show it, while a click on the chart's background leaves it as it is.

Wherever the chart centres on a cell (opening on it, following the cell a stage works at, going to it from a link or the
map), it centres in the part of the chart the expanded window leaves, keeps a cell in view only while it is in that
part, shows the whole chart in it and frames that part on its map, while panning moves the chart under the window as
anywhere, so that every part of it can be brought out from under it. The window floats over the chart's other floating
parts (its map and zoom, its "?" and legend, the flow's tools), which keep to their corners on the chart's left, where
the window reaches only when dragged nearly to the chart's whole width; below the narrow breakpoint, as every split
there shows one side at a time (above), it takes the chart's whole width, expanded or minimised, its width not dragged,
and the chart then centres as if nothing covered it.

Instead of a column docked beside the chart, which narrowed the chart and laid it out again each time the column opened,
closed or was dragged, and instead of a window that hides when it is folded away, because the owner asked that opening,
closing or resizing the panel not move the flow chart, and that minimised it keep its tabs in sight, folded to one row,
instead of hiding; and instead of a close on what is open and a window kept minimised or expanded as the owner left it,
because the owner asked that a click on the title fold the window, that the window not close, and that the window start
minimised; only the window itself has no close, since letting go of what is split under a page's panel closes nothing of
the window and leaves the page's list standing alone again.

### A panel open on a chart

A panel open on a chart (a cell, a round, a rule, a store, a comparison) is ours: its head holds the way back as a small
`Link` with an `ArrowLeftIcon` over its title, the title in title small (`h2`) with a code in it as a code is written
and where it stands in `--fgColor-muted` at normal weight, and who does it in small `--fgColor-muted` text, with its
close only where it is split under a page's panel (above), a `--borderColor-default` line under the head and
`--base-size-16` round what it holds; on a page with a panel of its own in the window floating over the chart (a work's,
the architect's) it opens under that panel, split from it by the window's divider, never in the panel's place, and one
part draws this for every page, because a panel is a small page with a title, and its way back is in the same place on
every one, and the owner, who had to go back to the architect's list after each round while a work's page kept its list
in view, asked that both pages show what is picked under the list, from one piece of code.

## The works and the architect

### One page in two uses

A work's page and an architect's page are one page in two uses (ours), each of their like parts drawn by one part with
no variant of either page's: the line over a work's chart (`HeadLine`: back to the works; which it is; how it goes on;
where; its facts in `--fgColor-muted` joined by a bar, its conversation a link among them), which an architect's page
has none of (below); the chart with the rounds listed placed on it (`WorkChart`); the page's panel in the window over
the chart (`ChartPanel`: an `UnderlineNav` whose items change the address, each its count as a `CounterLabel` and what
waits for the owner as a primary one, the window's button at its right end while it is the window's top row, a click
with a modifier key opening the tab as the browser would, a tab shown from its top); its list of rounds (`RoundsList`,
below), on a work's page under what its rounds tab says when the page was opened on only the rounds sent back or failed,
with the way to all of them (`RoundsTab`), which the marks after a count of rounds in the list of works open
(`RoundsMarks`); and its usage tab (`Usage`, an architect's counting its works' rounds over the span of time the owner
picks as one), instead of each page wiring its chart, its panel, its list of rounds and its usage in code of its own,
because the owner found one page changed and the other left behind, and asked that the logic be one.

### The works view

The works view is one table over the whole width, with no area of its own that scrolls, and a group of rows for each
project, in the order the list of conversations has its projects: the project's head (`ProjectHead`: its name and, at
the right end of the same row, the buttons that show or hide its works by state, below), then the project's architect,
then its works, newest first by number. Each group is a `tbody` of the one table, parted from the one before by room
above its head, which is on a `--bgColor-muted` ground, with no line across the table; the table is one so that the
columns and their widths are one. Each work's row leads to its work's group in the messages view, where the owner acts
on it; a work picked from the table takes the whole window under the bar in the table's place, its chart under one line
that goes back to the table,
says which work it is (its number, its item, and its project while there is more than one), how it goes on in the list's
words, where it is (its cell and stage, and while it is on an older flow version a chip saying so, or once it moved to
the version the engine runs a chip saying to which, since the owner wants to see a work an upgrade moved apart from one
that started on the version it runs), its rounds, cost and pull request, linked, and leads to its group, with no links
to the other works, which the owner found of no use; the address names the work, and going back in the browser returns
to the table as it was. The view has no heading above its table (the bar's tab says it) and the table fills the width, and
holds no control of a work, which is in its group's head. Where the window is narrower than the table's columns, the
table scrolls sideways and each project's head sticks to the left, as wide as the window, so that its buttons stay in
sight (`position: sticky` in a block `100cqw` wide in a cell that does not clip, the scroll container being the
size container).

Instead of a section of the works running or waiting with its heading and count, and a folded section of those ended
with a line over it, each its own table, an area of its own that scrolls whose height dragged, or a column of groups
beside the works, because the owner asked for the works of each project in one list, those ended hidden by the buttons
as in the list of conversations and not by a fold of their own, every row at once and the chart over the whole window,
and a heading repeating the tab, a line across the table or a column beside it spends room the works need.

### The works table

Each project's group is its head outside its table (`ProjectHead`, which folds the group by its name) and its table.
The tables have the same columns at the same widths, which the owner drags at the right edge of each column's header
and the browser keeps, a double click giving a column its own width again, and every cell stays on one line, cut where
its column ends and whole on hovering; a work's number leads to its conversation, with the mark of a conversation beside
it, a work's item opens from its title and the work from the rest of its row, and how the work goes on or why it ended
is written in words in its row, the state on one line (a round running and how long it has run, ticking, or pausing
after this round; what holds it back, paused by the owner or by the architect, or halted by the engine for a problem, a
work held only because every work is paused reading as what it is, ready or what it waits for, since the bar already
says that nothing runs until Start all and "paused by you" there made the owner take a work for paused that was not;
what it waits for, linked to where the owner gives it; the decision cards and asks the owner has to do as small chips;
the engine's reason after them), because the groups are read as one table, a reason prefixed with the state its badge
already shows says it twice, and a row that wraps makes the table slow to scan. A work's item is not followed by its
project, which the group it is in says, instead of the project after each title, since it would say it twice.

The project's head is outside its table, instead of a row of the table, because it is not part of a table's rows: the
owner asked for the name and the buttons outside it, and for the name to fold the group, which a project among several
makes worth having. The name is a button in the heading, a chevron before it, so that the heading still says the name
alone; the folded groups are kept in the browser, as the columns' widths are.

How far a work has come is one column, a row of narrow cells (`StageBar`), one for each stage of the flow version the
work runs on, each as wide as the one letter in it, the letter the one most of the stage's cells' codes start with, as
the chart's minimap has it (`stageLetter`), instead of the cell it is at and the stage in words, because the owner reads
how far a work is at a glance, and a cell wider than its letter adds nothing but takes room. The stages gone through
are the colour of success and the one the work is at stands out in the accent, in the colour of waiting while the work
waits for the advisor, the architect, the owner or anything else, with no cell added for the wait; the stages ahead are
plain; a work that goes back to an earlier stage has the later cells plain again; each cell says its stage on hovering.
The cells are read from the stage the work is at and the flow version it runs on, which is a column of its own holding
the version's number alone, with no words in the cell or on hovering, because the owner asked for the number to compare
by eye.

Where a work's time went is one more column, the track of the usage tab's timeline (`TimeTrack`, charts.tsx), one part
for both: a block for each stretch a model worked and each stretch the tools ran, one for each stretch no round ran,
from the work's first round to its last; the row's holds the span cut into equal cells, because a list of every
project's works would otherwise carry every round of every work, and says on hovering how long models, tools and the
engine's steps worked and how long no round ran. The time of its last activity and its pull request are not columns,
because the owner never read them there; its cost is.

A project's architect is the first row of its group, in the same columns: its work "Architect", leading to its
one-to-one with the mark of a conversation, its item "Architect of" the project as a link to its page (which the row
opens too), its state the chip the head of its one-to-one has and the mark in the list of conversations is (one
worked-out state, `architectMark`, so that the two never differ), the cell of the work that goes on, or else the newest,
in words, its rounds, run time and cost summed over all its works, with no bar, flow version or time track. The buttons
leave it alone: they count and hide works, as they do in the list of conversations, whose architect row they leave alone
too. The row's state is read from the lean read of every architect that the list of conversations' marks use
(`/api/v1/architects`), instead of from each architect's whole page, which also reads its notebook, policy, matters
handled, item changes and constitution and is read again at every event, since the row needs its works, its queue and
whether a question of its own waits, no more.

A work's run time in the table is how long its agents and its tools ran, the round running counted to now, its rounds of
asks and of the engine's left out as waits, instead of when its first round started, because the owner judges from it
whether a work ran too long, which when it started does not tell.

### The marks in the rounds column

The rounds column's marks of returns sent back and of runs that failed or were cut off say what they count on hovering
and open the work with only those rounds listed, because a sign with no words is not read.

### How a work goes on

How a work goes on is said on the line over its chart, as its row in the list of works says it, never in a box over its
rounds: its state as its state's chip (`StateChip`: a round running, with how long it has run in `--fgColor-muted`,
ticking each second from the one clock every view shares; what holds it back, saying why on hovering; what it waits for,
a chip that links to where the owner gives it; once what it waited for came, while it waits for a free slot, that
state's chip in its place), the owner's asks and decision cards to do as such links, and only for a stop of the engine's
or the architect's, why in muted words after them; that it is on an older flow version is a tag's chip "old flow" beside
its cell (`TagChip`), saying in full on hovering how it moves, and a work moved to the version it is on says
so ("moved to version N") as a tag's chip too; and what Start, Pause, Step and Start all do in the state the work is in
is said on hovering each of them.

Instead of a `Flash` in the attention colour at the top of the work's rounds saying the state again with paragraphs of
what the controls would do and the flow version's note, because the owner found the yellow box said again what the line
says, and what a control does belongs where the control is.

### A work's panel

A work's rounds, its decision cards, what its helpers and its engineer said to each other, and its usage are tabs of the
work's panel, on top of the window floating over the right of its chart and at the window's full height until a cell or
a round is opened on the chart, which then shows in the window's lower part, split from the top by a divider that drags
and is kept in the browser, a double click sharing the window evenly, and stays there until its close lets go of it and
the panel has the window's full height again, the window itself having no close: the decision cards with the one waiting
for the owner's ruling first, read only, saying where it is ruled on; the helpers' tab one thread for each helper (the
advisor, the review, the adversarial testing), never mixed, each a talk drawn with the conversation's bubbles, each
helper in its own role's colour rather than one colour for all of them: what was handed to the helper (the question of
the round that asked it, or the commit handed in by the engineer's latest round that handed one in, with what came with
it, never a later round that only carried on), the helper's answers (its advice, its findings), and what the engineer
did with them next (its return at the first cell it went to after, each finding fixed or disputed and why), each bubble
what its round said it did, the progress note of its return, with the return word for word folded under it in small
text, a return with no progress note saying so, and each opening its round on the chart, a return sent back or a run
that failed not being said to anyone, instead of the decision cards and messages under the rounds, reached by scrolling,
instead of the work's conversation as a tab, and instead of each bubble the return as JSON, because the owner acts on a
work in its group in the messages view alone, and the helpers are in no group: the owner never speaks to them, and what
they said to the engineer is read here, by helper; and the owner found the JSON, shown first, unfriendly, and asked for
a short passage of what each round did, the JSON under it to open when needed.

### A work's usage

A work's panel has a fourth tab, Usage, drawn from the work's recorded rounds alone: tiles of its cost, its time from
the first round's start to the last round's end and the time it ran, its rounds and those sent back, the tokens read
with the share from the cache, the tokens written with the thinking, and its compactions and turns; where the time went,
as one bar (models, tools, not running, which is one block since nothing records why a work stood still) and as a
timeline with a block per round and one per stretch no round ran, saying when, how long, between which rounds, and what
the record shows the work waited on then (an answer or a ruling of the owner's), its times marked under it at the finest
step of the clock (minutes, hours or days) whose labels its width keeps apart, the day said at its ends and at midnight
when it spans more than one; each model at the level its rounds recorded, never the configuration's now, and its
reasoning effort, with its rounds, turns, time, tokens and cost, and the subagents that ran on other models, found as
the models whose count grew in the session during a round, since the command line counts per model for the whole
session; where the cost went by kind of token, weighed by the models' prices as each round's cost line is, and by cell;
each session of each line, with why it started (the line's first, a cell its line starts one at, or the one before past
the context limit and how large it had grown), its rounds, turns, time, context, compactions and cost, its keep-warm
pings' cost counted, with a chart of each session's context through each round, from where its first call stood to
where its last call ended, a line to each session, so that how far a round grew the context shows and a session that
opened small does not look like the line before it carried on; the keep-warm pings, each with when, the
session, where the work waited and for what, and its tokens and cost, which the work's cost counts; and how long each
tool ran. Every mark says what it is on hovering, the labels are text that keeps its size as the charts shrink with the
panel, and the colours are the page's, in light and dark, because the owner asked to see what a work spent, on which
models and where, in charts.

### The architect's view

Each project's architect has a page under the works, at `/works/architect`, laid out as a work's page, read only: its
graph, on the flow version its latest work runs on, with the rounds its list holds placed on it and the cell it is at
(the bare graph before its first work), and the window floating over the chart with its panel and what is open on it;
the architect's row of its project's group in the works opens it, with none named the page opens on the first
project's, and another project's is reached from that project's row or its one-to-one; the works it ran are not rows of
the works table, the architect's row standing for all of them, instead of the architects' works as rows of the table
or a tab of its own in the bar, because an architect runs for a long time and belongs to no item, the owner reads what
it does as one thing, and a tab for what a row opens spends the bar's room. An address of a work of the architect's
leads to its page, on the round it names, since such a work has no page of its own. The page's old address,
`/architect`, goes on to `/works/architect` with every parameter it had (the project, the tab, the span, the round and
the cell), as the spend view's old address goes on to the system view, and no other address is moved.

### No line over the architect's chart

The architect's page has no line over its chart, the chart and its panel starting right under the bar, and what the
architect does now is said where the owner acts on it, in the head of its one-to-one, never naming its works, which to
the owner are only how it is run: the matter it handles and the cell it is at, a round running or what holds it back,
that it waits for the owner's next words, or that it is idle, takes up the matters waiting, or is held back while every
work is paused; and at the head's end each hold on the project's new works with why; its matters left, its notebook and
its policy are the panel beside the one-to-one (below); and an architect's page is reached from its row in the works
and from its project's one-to-one, whose head names the architect as a link to its page.

Instead of a line over the chart saying the same, a state chip and the matters left, with links to the panel's tabs (the
notebook and the policy) and to the one-to-one, the switch to another project's architect and the switch between the
stretches it worked in, because the owner found the line told it nothing it needed there, each part of it having its
place in the one-to-one, and said the architect is always there to it, handling matters one after another; its rounds
are one list (below), not picked stretch by stretch, and how many matters a stretch handled or what it cost is not asked
for, the Usage tab holding costs.

### The window over the architect's chart

The window over the architect's chart is split as a work's is, by the same part: what is open on the chart (a round, a
cell, a rule) shows in the window's lower part under the panel, split from it by the divider that drags, kept in the
browser at one height for both pages, instead of what is open taking the place of the panel's list with a way back to
it, because the owner had to go back to the list after each round to pick the next, and asked that the two pages show a
picked round alike, so that a change to one is never missing from the other.

### The architect's tabs, by stage

Each tab of the architect's panel holds what one stage of its flow, or one set of its cells, produced, and each part of
it names the cell that wrote it, as the flow names it, with the round, which opens on the chart: the Item changes tab
what the batches made or dropped. What the architect keeps (its notebook, the plan among it) and its policy are in the
panel beside the one-to-one, one place each, and the questions whether to research a new policy and whether to approve a
draft are the architect's questions in the one-to-one. There is no log of every round's own words, because each of its
parts is in the tab of the stage that made it, and what became of each matter is its card in the one-to-one. Because the
owner finds a thing by the stage that made it, and reads it right only knowing which cell wrote it, and a tab that mixed
the policy with the records of setting it read as a heap.

### The order of the architect's tabs

An architect's panel's tabs run in the owner's order: its rounds, which it opens on, the item changes, and its usage;
its conversation, its notebook, its policy and its queue are in the messages view, the one-to-one and the panel beside
it, one place each, because the owner reads what the architect keeps and works by beside the talk with it.

### The architect's panel reads only

An architect's panel is read only: the item changes its batches made or dropped (each with why, what it made or what the
review found, its matter and round); and its rounds, because the owner reads what the architect decided and made and
checks its intent is understood here, and acts in the one-to-one. The project's observations are not in it but in a view
of their own (above).

### The architect's usage

The architect's page has the same Usage tab, drawn by the same part, counting the rounds of the architect's works that
started in a span of time the owner picks, the last 7 days at first, or the last day, the last 30 days or all time, kept
in the address, its works' rounds, sessions and pings counted as one (a session a work went on in from the work before
counted once), instead of no usage of the architect's, because the owner asked to see what the architect spends as a
work's usage shows it, and an architect works on without end, so its usage is read over a time, not a work.

### Lists of rounds

A work's rounds, the architect's, and the rounds at a cell in the cell's panel, are listed with the cell and who did its
work (the agent's line, or the engine, also for a gate it answered itself because the project lacks what the gate needs,
the tools, or the owner) in the colour the chart gives that cell, the row taken, or that the round ended the work, in
plain words on one line, cut and whole on hovering, with a badge only for what is not the usual course (sent back,
failed, cut off, running), what an agent's round cost, beside it a small bar of the shares of its cost read again from
the cache, newly cached and output with its steps, the whole line on hovering, its key, what each colour and the number
are, in the chart's "?" rather than a line above the list, in light colours the round's cost line and the usage tab
share, and how long it took, the round chosen, however it was chosen, brought to the middle of the list when it is out
of sight; a round that started a new session of its line is marked with why; a round left open while the engine runs
nothing of the work (one the engine stopped in) says so in plain muted words and the work shows as ready, so that its
next round, which closes it first, can be run; instead of a badge on every round, a grey badge for the end of a work
that every other row's words did without, the same look for a round no agent ran, and every input's whole description,
because the owner scans the rounds for what went wrong, what cost much, which rounds really ran an agent and where a
session began.

### The round that started a session

A round records the session it ran in, and a session why it started (`sessions.started`: its line had none going on, a
cell starts one, the one before was past the context limit, the model or the login account changed, the CLI's prompt
cache let the one before go, the owner asked for one, or it goes on from the architect's work before); the first round of a session in its work is the round that started
it, and the page marks it in the list of rounds of a work's page and the architect's, the engineer's lines and helpers
and the architect's lines alike, saying why on hovering and in the round's panel, but not a round of a cell on no line,
each of which starts a session of its own, as its cell's badge on the chart says. Every session the session manager
opens records why, so that nothing more is recorded for the mark; a session opened before why was recorded holds the
reason it was given then, that its line had none going on.

### The architect's rounds, one list

The architect's rounds are one list across all its works, read in windows: the page opens on the newest each time, reads
the newer or the older rounds as its list scrolls to either end, reads again the newest while it shows them as the
architect's rounds change, and opens around a round a link names, with as many rounds before it as after, more on one
side where the other has too few; the interface gives a project's architect's rounds in the order they started, across
its works and the settings of its policy an older flow version ran as works of their own, the newest, those before or
after a round, or a round with those around it, each window saying whether older and newer are left (`GET
/api/v1/architect/rounds`); a rule parts each of its works from the one before.

Instead of the rounds of one stretch of its work at a time, picked by when it began from a list of the stretches with
the matters each handled, its rounds and its cost, because cutting the rounds by stretch cut the line of a session,
which the architect renews only at its first work, when the owner asks, past the context limit or when the model or the
login changes, and keeps across many works; and the owner asked for one complete list, opened at the newest, with the
round of a link at its middle.

### The list of rounds as one part

A work's page and an architect's list their rounds with one part (ours, `RoundsList`), newest first, a `table.list` that
scrolls by itself under what its tab says over it. A row holds, each cell on one line at any width: the round's name,
`--fgColor-muted`, with a mark after it when the round started a new session of its line (the new session's speech
bubble of the owner's controls, above, in `--fgColor-muted`, saying why on hovering and to a screen reader: its line had
none going on, the cell starts one, its line's session was past the context limit, its line's model or the agents' login
account changed, or the owner asked for one; a round of a cell on no line, each of whose rounds starts a session of its
own as its cell's badge on the chart says, has none, and a round whose why is not recorded the mark without why); the
cell's code, then who ran the round as a chip filled in the tint its cell has on the chart and edged in the same, its
words in that colour's foreground (an agent's line its role's colour, `--role-fill` and `--role-text` of roles.css; the
engine's step the chart's neutral grey, a tool its quiet yellow, the owner its orange, the chart's own tokens for them);
what it came to, taking the room left and cut, whole on hovering: the row it took, or that it ended the work, in plain
words, a round left open by a stopped engine in plain muted words, saying why on hovering, and a state's chip only for
what is not the usual course (sent back, failed, cut off) and a round running, whose words are what its agent did last;
what an agent's round cost, after the small bar of where its cost went; and how long it took. Where the list is narrow
(below 480px, then 380px, as a container query reads its own width), the bar gives way first, then how long the round
took, the cells then keeping `--base-size-4` at their sides and who ran the round cut, whole on hovering, so that what
the round came to keeps room for its first words. Before the first round of each of the architect's works, between two
works' rounds, a thin `--borderColor-muted` rule runs across the list with when that work began on it in small
`--fgColor-muted` text, distinct from the session's mark, since a session may start within a work and go on across
several; it says nothing of why the architect worked. The architect's list is one list across all its works: it opens on
the newest rounds each time, and scrolled near its top or its bottom it reads the newer or older rounds left there and
they join it, the rows in sight staying where they are, while at its top with nothing newer left the newest come into
sight as they run. A link to a round anywhere on the page (a round's name in a bubble, a matter's "on the chart", an
observation's source, an item's progress) opens its page with that round chosen and, when it is not listed, the rounds
around it read in place of the list's, more readable above and below; the round chosen, however it was chosen, comes to
the list's middle when it is out of sight, on a work's page as on the architect's. The chart's counts of the rows taken
(xn) and of the rounds at each cell cover the rounds the list holds, and the round chosen lights the arrows it came in
by and left by, as on a work's page; the architect's chart is drawn on the flow version its latest work runs on, rounds
of works on another placed by their cells' codes and rows' names where it has them.

Instead of the architect's rounds of one stretch at a time, picked from a list of the stretches it worked in with what
each cost, every row's words wrapped, a grey state's chip for the round that ended a work while every other round's row
was plain words, and no mark of a new session, because the owner asked for the architect's rounds as one complete list,
with the stretches parted by a rule and not picked, the round of a link brought to the middle of it, every round drawn
alike, who ran each in the colour the chart gives its cell so that the architect's rounds are found at a glance, the
rounds that started a session marked, and at a phone's width no cell squeezed to a letter a line; the cost of a stretch
is the Usage tab's.

## The conversations

### The one place the owner acts

The messages view is the one place on the page where the owner acts: the owner rules on a decision card, answers an ask
of the owner and a question the architect holds, replies to a question the architect pinned, says something, pauses,
starts and steps a work and the architect, sets a work's item's tier there alone, every other view reading only and
leading to where it is done, with exceptions that govern the whole system or are rarely changed, the bar's Start all,
Pause all and Keep warm, and the Settings view, and one that belongs to no conversation, the close of an observation, in
the Observations view beside the observation it closes; an ask is laid out as each point it asks about (its kind, what
it says with any code kept as code, what was tried) and its choice with the option recommended; the page writes what the
command line writes and wakes the engine, instead of a control of a work in its row, its window and its item, the
architect's on its page, and forms wherever a card showed, because the owner reads a thing to understand it and then
acts, and with every action in one place the other views become tools to look with.

### The conversations

The messages view is the owner's conversations, a list on the left and the one open on the right: the owner's one-to-one
with each project's architect, for the policy and what belongs to no work; wf-agents' own, its faults; a group for each
engineer's work, whose members are the owner, its engineer and its project's architect; and a group for each set of
projects the configuration connects through `other_projects`, whose members are their architects and the owner.
Everything said in a group reaches all three: what the owner says to the engineer the architect is given at its next
pick or wrap-up, what the owner says to the architect the engineer is given as a word that needs no answer, what the
engineer and the architect say to each other the owner sees, and what the engineer tells the owner the architect is
given too. The helpers (the advisor, the review, the adversarial testing) are in no group. The entries are gathered from
the tables that hold them by one view, `conversation_entries`, hidden from agents so that the index of tables the flow
gives them stays as it is; the page, and the talk a new session of an architect is told, read the view built on it,
`conversation_shown`, which leaves out what the page does not show (the engine's notices and the helpers' messages),
beside what the engine writes from the flow for the page (the bubble of each stage a work
entered, of each matter an architect took up and of each version of its plan, the policy as it took effect).

The observations are no card in a conversation, which showed each a second time beside its place: what the owner must
know at once of one reaches the owner as the architect's message, and an architect's matter's bubble lists the
observations it recorded, each leading to it.

Instead of a conversation of each work with its engineer and helpers apart from the architect's, because the owner, the
engineer and the architect act on one work together, and what one says the others need.

### The list's order

The list of conversations is in an order that never changes for what waits: wf-agents' own first, then a group for each
project, by name as the items view orders its projects, holding the one-to-one with the project's architect and then a
group for each of its works, newest first by when it began, which greys out in place when its work ends, then the groups
of connected projects, which like wf-agents' own belong to no project; each has a red count of what waits for the owner
in it (an open blocker the owner holds, a decision card not ruled, a fault of wf-agents not read) and a grey count of
what came in it since the owner last read it; the view's tab in the bar counts the same, red and grey, and the page
tells the owner of what newly waits by the browser's notifications, asking once for leave and saying nothing more when
it is refused, instead of putting first those where something waits, because a list that reorders moves under the
owner's hand, and a mark says enough.

### Buttons in each project's head

Each project's group is headed by its name and, at the right end of the same row, buttons (`ProjectHead`, ours, above),
the same as in the items' tree, the works table and the observations, each a state's icon, the one its works' rows
show, and the count of its works in that state, with no word, the state and the count said on hovering, that show or
hide its works in that state, the done and the ended hidden at first, the owner's choice one for every project and kept
in the browser, the same choice for the list of works, which counts and hides by the same state, as it is in effect, so
that a button never counts what the rows do not show; the conversation open stays in the list whatever its state, and a
work whose state is not read yet shows once it is; instead of one group of every work, because the owner asked for the
list grouped by project, so that it holds several projects later, and filtered as the items are, what is over hidden;
instead of each state's name on its button, because the owner found the row of buttons too long to fit on one line of
the list; and instead of the buttons on a row under the project's name, because the owner asked that they stand at the
right end of the name's row, as no screen is spent on a row for them.

### A hidden work counts nowhere

A work the buttons hide counts nowhere: the view's tab in the bar sums the counts of the conversations the list shows,
the browser tells of what newly waits in those alone, and the conversation open, kept in the list while its state is
hidden, shows no counts on its row, so that the bar is the sum of the counts the list shows, on every view and whichever
conversation is open; instead of every work counted, because the owner asked that the numbers at the top drop what the
filter hides, as a number with nothing in the list to find for it misleads. A row's name is cut with an ellipsis before
its counts, whole on hovering.

### What is unread

How far the owner has read a conversation is kept (`conversation_reads`), and what came after that was said to the owner
is unread, what waits counted apart; what the members say to each other, a stage's bubble and the engine's own records
are not, instead of whatever came from anyone but the owner, because the owner asked that only what is said to the
owner, and what waits pinned, raise the count and notify.

### The messages view

The messages view is Primer's `PageLayout` (as the items' split), the conversations a `NavList` in its pane in groups
(wf-agents' own; each project's, a `NavList.Group` with no title of its own, named by the heading of its first row, its
head (`ProjectHead`, above: the project's name and, at the right end of the same row, the buttons that show or hide its
works in each state, the items' tree's own part, `StateFilter` (above), one button for each state of its works, its
mark and its count), then its rows; the connected projects, each also headed by its name), each with its mark as its leading visual (an architect's
one-to-one and a work's group the state's mark of that architect or that work, above; wf-agents' own `CpuIcon` and a
group of projects `StackIcon`) and its counts as trailing `CounterLabel`s, what waits `variant="primary"` and what is
unread secondary, a work's group over in `--fgColor-muted`, a row's name on one line, cut with an ellipsis before its
counts and whole on hovering, the row of the conversation open marked as `NavList` marks the item current even while its
work's state is hidden, and then without its counts; the conversation open fills the rest: its head (its name in title
small, its state as a `Label`, its facts small and muted with the owner's controls as the work's small `IconButton`s and
a tier's `Select`; in the one-to-one with an architect, while the panel beside it is closed, an invisible small
`IconButton` with `SidebarExpandIcon` at the row's end that opens it; a work's group's head one line, nothing on it said
twice, the one place that says where the work is: the work's Start, Pause, Step and Start over while it goes on, "#9
Missing data directory", its state as a state's chip in the word the list's filter has for it (paused by you, by the
architect or by Pause all; halted; waiting; ready; running, then who is at work in plain words, "the review is at
work"), then after a bar its facts joined by bars, its item and its rounds as links ("Item 4", "Rounds") and what it
cost, and while it goes on its stage, what holds it and what it waits for in the order they act, as one sentence ("Kept
as it is while the trial board moves to a new build. Once started, it is waiting for your ruling on decision card 15."),
and the tier's `Select`, instead of the facts on a second line with the stage of an ended work, which said its state
again, because the owner asked for the head on one line with nothing that repeats; and instead of "stopped (you paused
it)" beside "waiting for your ruling", because the owner read the two as a contradiction: both hold, the pause first),
the bar of what waits for the owner under the head (below), the stream that scrolls, and the one box at the bottom, whom
to as checkboxes, the `Textarea` and Send as the primary `Button`, which answers no bubble but replies to an architect's
question the strip over it names, because the owner moves between conversations as between a chat app's, and acts in one
place. A group for each project, its works shown or hidden by state with the very buttons of the items' tree, instead of
one group of every work, because the owner asked for a list that holds several projects and keeps what is over out of
the way, and the same filter has one look and one code; a name cut before its counts, instead of running under them,
since a name drawn through them cannot be read.

### A conversation's head

A conversation's head puts the Start, Pause and Step first in its row, at the left. The architect's one-to-one has one
row: those controls, the architect's name as the link to its page, what it does now, and at the row's end the holds on
the project's new works and, while the panel beside the one-to-one is closed, the button that opens it; its queue is
that panel's Queue tab (below).

Instead of three rows (the name and state, then a link and the controls, then the queue), because the owner found them
scattered, and the controls are what the owner reaches for first.

### Start over

A work's group's head has, after the work's Start, Pause and Step, a fourth control, its Start over, while a work on an
item goes on, and nowhere else, since the owner acts on a work in its group: pressed, it asks first, in plain words,
what follows (the round under way cut off, its agent's process finishing before the new work begins; a new work on the
item taking over the branch, the worktree and the pull request from the first cell, given the owner's rulings and
answers; and whether the new work starts paused, as only the owner's pause of this one carries over), and only on the
owner's confirmation is it sent; it acts whatever the global state, and unlike a pause it does not wait for the round
under way, which hands in nothing and is cut off once its agent's process has ended; instead of a pause and a
cancellation in turn, because an item whose work went wrong can be begun again without losing what it built, and what a
round of such a work hands in is of no use.

### What stops new works

What stops a project's new works from starting (the owner's own hold, the project being set straight, the architect's
own hold) stands at the right end of the architect's title row, in the conversation's head and on the architect's page
alike, as a sign and its words in the attention colour with no box, instead of a Label under the title like the
architect's own state, because it is not what the architect does but a rule on the project's works, and the title row
had the room.

The owner's controls of it stand in the same row, since the sign is the one place that says what holds the project, and
the owner acts where a thing is said: while the owner has no hold of their own on the project, a small `Button`, "Hold
new starts", at the row's end; and on each sign, whoever's hold it is, a small invisible `Button`, "Release", since the
owner may release any of the three. Each asks first, in a form of its own in a row under the title row, never in a
dialog that covers the conversation: holding says in plain words what follows (no new work of the project starts, the
works already running go on and merge, a work waiting for a result goes on once it comes, and Pause all in the bar is
another thing), asks why in a box of words whose button is greyed with the reason while it is empty, and is sent only
on that button; releasing says what it lets happen (for the architect's own hold and the one to set the project
straight, that the architect may hold again in its next plan) and is sent on its "Release". The sign says "You stopped
new works from starting" with the owner's words for an owner's hold.

Pause all and the hold are two things and look two ways: the hold is one project's rule on its new works, kept in the
database, so it stands across a restart of the engine and for every work that is waiting or running; Pause all is every
work's stop, is not kept, and the engine starts with it on. Neither sets or lifts the other; Start all leaves a hold in
place.

Instead of one control in the bar that holds every project's new starts, because the stop wanted is one project's and
the bar's two switches already say that every work goes on or stops; instead of a dialog over the page for the words,
because the owner reads the row and the architect's state beside it to decide whether to hold, and a dialog hides
them; and instead of a Release only on the owner's own sign, because an architect's hold that the architect does not
release would leave the project stopped, the owner's only way being to ask the architect, one round later, to let it
go.

### A conversation reads as a chat

A conversation reads as a chat (ours, after the chat apps the owner knows, in Primer's tokens): an agent's message
beside its avatar, in the box of what an agent handed back filled and edged in its role's colour (below: its tint
`--display-<colour>-bgColor-muted` and the tint's edge `-borderColor-muted`); the owner's on the right in the sheet of
what is sent to an agent; at the head of every bubble, one part (`BubbleHead`) in a work's group, the one-to-one and a
work's helpers' threads, small and in one order: who says it (the engineer, the architect, a helper), the only part in
semibold; to whom as a mention (`@you` for what is said to the owner, `@engineer`, `@architect` or a helper between
agents, semibold in `--fgColor-accent`), none for a bubble of an agent's own work; what such a bubble is about (a stage
with its pass, a matter's kind) in the small `--fgColor-muted` look of a kind (`talk-card-kind`), semibold so that it is
found (`talk-about`), never a chip and never in the speaker's colour; for a thing that runs and then ends, how it stands
as a state's mark (below); when; then its round and its links; the owner's own sheet named by its side and look alone.

Instead of a head of who and to whom on a message and a head of the stage's name in the speaker's semibold on a stage,
with no speaker, because the owner found the heads mixed, some saying who and some a stage, and read a stage's name set
like a speaker's as a speaker, and instead of what it is about in normal weight, because the owner found the stage or
the matter, what matters most in the bubble, too faint to find; what the owner said and was never taken in, or not yet,
said on the message in `--fgColor-danger` or `--fgColor-attention` small text; an ask is the asking agent's message,
answered in it where it was asked, the bar of what waits pinning it while it waits (below).

Everything a line says in a conversation is a bubble, an ask above all, whichever line said it, with what answers it inside
it, and every ask or review that waits for the owner is pinned on the bar over the stream, which leads to that bubble:
the owner decided this, reads conversations as a chat, and cannot go looking in other places for what waits. A line the
flow gains changes nothing of it.

In wf-agents' own conversation, the engine's notices are the page's own voice, a condensed `Timeline` between the
messages: a card's badge its state's Octicon and state colour, its first line its name in semibold, its state as a
`Label`, its facts in small `--fgColor-muted` and its links, what it holds folded under it, and a notice one small
`--fgColor-muted` line, an item of the `Timeline` as a card is, its badge `CpuIcon`; every badge of a run of cards and
notices stands in one column on the `Timeline`'s one unbroken line, centred on its item's first line, a card's title
line and a notice's words alike (ours: the first part of an item stands level with its badge's box, the badge and the
room above and below it; a line of chips and controls is as tall as that box, its words in the middle, and a run of
words that may wrap, a notice's or a matter's, has its first line's middle at the box's).

Instead of every entry a comment box with who to whom in its head, because the owner reads a group of three as a chat,
who says it first, and the engine's cards must look apart from what the members say; and instead of a notice as a line
of its own with its icon beside its words, out of the `Timeline`, because the owner found its icon in line neither with
its words nor with the cards' badges over and under it.

### Events in time

Events shown as one run in time (the engine's notices in wf-agents' own conversation) are Primer's `Timeline`
(`Timeline.Item`, a `Timeline.Badge` holding the event's Octicon, `Timeline.Body`), condensed in a conversation; a
history kept by kind (an item's earlier cards, asks and works) stays sections, one for each kind, because a run of
events is read in time, while a history kept by kind is looked up by kind.

### A chat holds its members

A work's group is a chat and nothing else: every entry is the bubble of who said it, the engineer's words to the owner
headed `@you`, its questions to the architect `@architect` and the architect's answers `@engineer`, and nothing the
engine writes is an entry of its own there; each stage the work entered is a bubble of its own in the engineer's look,
as the work's own voice, updated in place and standing where it last changed (`StageBubble`, drawn by the live one-line
bubble, below), and what the engine did to the work (a pause, a halt, a limit passed, a takeover, main broken by its
merge) shows on the bubble of the stage the work was at and in the head, never as a notice; instead of the stage cards
and the engine's notices on a `Timeline` between the messages, because the owner read the group, mixed with them, as
chaos, and asked that a chat hold nothing but who said what. The one-to-one with an architect is a chat alike: what the
architect says to the owner headed `@you`, and what it does in bubbles of its own with no one named, one for each matter
it took up, each version of its plan and the policy as it took effect (drawn by the live one-line bubble, below),
nothing the engine writes an entry of its own, the architect's own trouble (halted by the engine) on the bubble of the
matter in hand, in the head and pinned while the owner must act.

What a helper says, a work's (the review, the adversarial testing, the advisor) or the architect's (the review of its
batch, the search of its observations), goes to the one it helps and is no entry of either chat, whatever its age, since
the one it helps reads it and tells the owner what the owner must know, and a helper's line in the owner's chat was a
voice the owner does not talk with.

### A work's group

A work's group is a chat of its members and nothing else, every entry the bubble of who said it: the engineer's words to
the owner (`tell_owner`, which the flow keeps to what only the owner can do, a risk that cannot be undone and a boundary
crossed), its questions to the architect and the architect's answers, the owner's words and the replies to them, the
asks, answered in place, and the rulings. The work's progress is a bubble of its own for each stage it entered, standing
where it last changed, in the engineer's look as the work's own voice: while the work is at the stage one line of what
happens now, where it is in words, who is at work and what holds it or what it waits for, as the head says it, with its
latest progress note, live; once over one line of how it ended (done, stands, no findings, merged with its pull request
and commit, sent back with what its reviews found, or ended with why and the work that took it over) and the gist of its
last progress note, after who said it when another than the engineer; folded under it each of its rounds with what it
said, the design, the decision cards handed in and what the engine told of the work meanwhile. The merge's judge sending
the work straight back to fixing, a stretch of the engine's own steps alone, is how the testing before it ended, not a
bubble of its own; a stage the work comes back to is a bubble of its own, counted ("Testing and fixing, 2nd time").

What the engine did to the work (paused, halted and why, a limit passed, taken over, main broken by its merge) shows on
the bubble of the stage the work was at and in the head, never as a notice of its own; the side reports a work's lines
hand in are the architect's, in its queue, and what a helper told the owner is its round's progress note in the stage's
bubble. Where the work is now is said in the head alone, once. The decision card that waits for the owner's ruling is
pinned on the bar of what waits under the head and ruled in its stage's bubble, and a decision card that replaced one
the owner ruled on, directly or through cards between, says on its head that it changed since that ruling. The engine
keeps no notice of a decision card waiting, new or replacing another, nor of a work ready to merge but held: the
notifications log alone says them.

Instead of every entry the tables hold (forty-four for one work: its stage cards placed when each stretch began, two of
them a judge's instant sending back, side reports addressed to the architect, notices that named the work in its own
group, said again where the work stands or that one decision card replaced another, and what every helper said), because
the owner read the group as chaos and asked that it read as the work's progress, as a chat where each says what is
theirs, once, densely, with an inline card or a link where more is needed.

### What an agent tells the owner shows once

What an agent tells the owner shows once, in its conversation, as the agent's with its line, and the notifications log
says it too, instead of a notice of the engine's repeating it, because each message showed twice with the same text and
time and flooded the view; a notice of a new ask is not shown beside the ask, which is in the conversation itself.

### Where the architect tells the owner

What the architect tells the owner while it handles a matter of a work, from the round after the pick that took the
matter to the round that closed it, is in that work's group; what it tells the owner otherwise is in the one-to-one,
instead of every message of the architect's in the one-to-one, because the owner reads about a work in its group, and
the matter's card in the one-to-one leads there.

### A thing that runs and then ends

A thing that runs and then ends (a stage a work entered; a matter an architect took up, a version of its plan, its
policy as it took effect) is one bubble of one line, drawn by one part (`LiveBubble`, Bubble.tsx), in the bubble's look
of who says it: its head every bubble's (above): who, what it is about in the look of a kind, how it stands as a state's
mark, when it began, the links to where more of it is (a stage's rounds) and, once it is over, how long it took and what
it cost; the mark drawn as the list of conversations draws a work's state (`StateMark`, and `ToneMark` for a state that
is no work's, components.tsx): an Octicon in the colour of its state's chip, its words on hovering and as its name,
taken from the maps of a work's and an item's marks where a mark already means the same, so that like states look alike,
and the mark of going back, `UndoIcon` (`BACK_MARK`), which neither map has; a stage the work is at takes the work's own
mark (the running halo while a stage works, paused, halted the alert, and so on), and a stage over how it ended: a work's
tick of done for done, stands, no findings, merged and main green, the mark of going back in the attention colour for
sent back and does not stand, a work's mark of ended for ended; its one line, packed, in words with nothing before them,
so that no line holds a chip alone: while it goes on, what happens now in the owner's words, as the head says it, with
no cell's title or code, which the link to its rounds leads to: who is at work ("the review is at work"), what holds it
and then what it waits for ("paused by you; then it waits for your ruling on decision card 15 before it can merge", the
merge stage saying what it waits for holds up the merge, since there that is where the work stands, the other stages
naming where it waits by their head), a free slot, or its next round due; a halt in the engine's own words; once over,
how it ended and why in a few words ("sent back: 4 findings in "Table review"", the name in curly quotes; the flow's or
the engine's words) with its links (the pull request, the merge commit as GitHub writes it, the work that took it over);
then the gist of what was said last in `--fgColor-muted`, after who said it ("review: ...") when another than the
engineer, whose bubble it is; the whole line on one line, cut with an ellipsis where it ends and whole on hovering, the
bubble no wider than its row and the row no wider than the stream, each part of the line that holds words shrinking
below its words' length so that the cut falls at the bubble's edge, and the conversation never scrolls sideways; under
it, while it goes on, what the engine told of it in small `--fgColor-attention` text; and what more there is, each a
`Fold` that opens in place (each round of it, a cell run more than once each time: its round, its cell's title, who, the
row it took and what it said, if it said anything; the design; the decision cards handed in, each folded to its head
line; what the engine told of it once it is over). The bubble stands in the conversation where it last changed, its head
still saying when it began: a stage at the end of its last round, or that round's start while it runs; a matter where it
was last handled, withdrawn or held by an ask, the one in hand at the latest round of the architect's. A decision card
that waits for the owner's ruling is drawn in its stage folded, as every card there, where the bar of what waits leads
and opens it (below); once ruled or replaced it is folded in its stage as any other, because a thing has one place and
is shown there once. A stretch of the engine's own steps that sent the work straight back is no bubble of its own: its
sending back is how the stage before it ended. A matter's head names what it is about, its kind ("Go through side
reports"), then where it stands as a mark: back in the queue the mark of going back and held by your ask a work's mark
of asking, both in the attention colour; done a work's tick; withdrawn an item's mark of cancelled, neutral; in hand the
architect's own mark, as its row in the list of conversations shows it, the running halo while it works and
its trouble while it is held. Its line says in words: while in hand what happens now, as a stage's line says it; back in
the queue or held, so; once withdrawn, why ("withdrawn: the work ended"); once done, what came of it, in one shape for
every kind of result and every kind of matter, worked out by one function (`matterResult`, architect.ts) and drawn by
one part (`ResultLine` on the line and `ResultThings` under it, each thing by `Thing`): the line a summary, its parts in
the order the owner reads them joined by a bar (replied to work 30, leading to its group; told you; the ask put; the
items made, cancelled and moved, other item changes, those waiting for the review and those dropped; the observations
recorded, merged into one already there and closed; what it did at once; whom it wrote to; "nothing changed" when
nothing did), what it changed by name counted ("4 observations recorded"); under the line, not folded, one line for each
thing it changed by name, in the same order and one look: what was done to it in words ("recorded observation", "merged
into observation", "closed observation", "made item", "cancelled item", "moved item"), then the thing by its number in
`--fgColor-muted` and its title, a link to it (an observation to it among the observations, an item to its page,
`ItemRef`), the title cut where the line ends, and for an item moved, where to ("after #6"), whole; the one rule of
density: when a matter changed one thing and nothing else came of it, its line is that thing, drawn as its line under it
would be and leading to it, and no line under it says it again; otherwise every thing has its line. The interface names
each thing (`made_by`): an observation recorded by the matter its source names, one a report was merged into or one
closed by its path's changes to the observations, each by its number and title, and an item by its batch's changes. The
sentence of what it asks is whole on hovering and in a `Fold` under the line ("The matter in full"); a matter that
changed nothing leaves the conversation once done. A version of the plan reads "Changed the plan:" and the first line it
has that the version before had not, or "Made the plan", the plan whole in an inline card that opens under it; the
policy that took effect, one line and a link to it in the panel beside the one-to-one.

Instead of a card of the engine's for each stretch, placed when it began among the messages, with the cell's code and
title as its state, and instead of matter cards on a `Timeline`, because the owner asked for one bubble for each stage
and for each matter, its one line updated as the work moves on, saying as much as a line holds, with what needs more an
inline card that opens or a link that leads there, and what the architect made listed by name where it can be read at a
glance.

And instead of the cell's title before a stage's live line and a matter's whole sentence on its line, which pushed the
bubble and the stream wider than the window, because the owner reads where a work is in words, not by its cell, a thing
is shown once, and a line holds what a few words say.

Instead of the state as a chip at the start of the line, because a matter done had its done chip alone on a line, which
the owner found a waste of room, while a mark in the head says it in an icon's space; and instead of a matter's result
in two shapes, a merge a summary link beside the chip ("Observations: 1 merged into one already there") and a record the
chip alone with a line "Recorded observation" for each under it, because the owner asked why the two differed: like
things look alike (Article 8), and the observation a report was merged into is named and linked as a recorded one is.

And instead of a bubble standing where its stage or matter began, updated in place far up the stream, the rounds that
said nothing left out of its fold and the gist on the line with no one named, because the owner reads a bubble updated
as the newest thing said and looked for it at the bottom, looked in a stage's bubble for each of its rounds and read one
line for a cell run several times as rounds missing, and took a tester's words on the line of the engineer's bubble for
the engineer's.

### A helper's talk

A helper's talk with the engineer on a work's page (the advisor's, the review's and the adversarial testing's, each a
thread of its own) is drawn with the conversation's bubbles, one part for both (`Bubble`), with no look of its own: the
side that handed the helper its work, the engineer, on the left in its colour, the helper on the right in its own role's
colour (below), its avatar after its bubble; each bubble what its round said it did, the progress note of its return, as
a text an agent wrote (above), and under it, a `Fold` at first folded on a line of small `--fgColor-muted` text, the
round's whole return word for word as the round's panel shows what an agent handed back (the question asked, or the
commit handed in with what came with it; the advice or the findings; what the engineer did next, each finding fixed or
disputed and why), read only once opened; a return with no progress note says so in small `--fgColor-muted` text over
the fold, and a round still running that it runs, with nothing to fold; one part (`RoundTold`) draws this for every talk
between agents; its head every bubble's (above): who, to whom (the other side of the thread), when, and the round at its
cell, a link that opens the round on the chart.

Instead of each helper's rounds as their own words of what they did in a box under who said it, because the owner asked
to read what was handed to the helper, what it said back and what the engineer did with it, as the two sides of one
talk; instead of one colour, pine, for every helper, because each helper is a role of its own, and its colour is the one
its cells have on the chart; and instead of the return as JSON filling the bubble, because the owner found the JSON,
shown first, unfriendly, and asked for a short passage of what the round did, with the JSON under it, small and folded,
to open when needed.

### What waits, and the stream

In a conversation, what waits for the owner is pinned in one place, the bar under the head (below), and each thing waits
where it came in the stream and is answered there: an ask of the owner from a work as the asking agent's bubble in full,
its options picked on their boxes in its card, a click picking one, a second clearing it and the one picked marked, then
a small box of the owner's words beside Answer, the words free unless the ask is in words alone, Enter sending them; a
question the architect holds is answered in a small box in its bubble too, and the box at the bottom of the conversation
never answers a bubble but replies to an architect's question (below). Once answered, the ask leaves the bar, and its
bubble in the stream holds the question with the owner's answer under it, folded when long, headed as the owner's answer
to that ask or question, with the choice made and the words, and it shows nowhere else: the conversation's view holds an
ask as one entry, the answer in it, and the owner's reply to a question shows in the question's bubble, not as words of
the owner's of its own; an ask the engine closed says there why no answer was needed.

A decision card that waits for the owner's ruling, whether or not its work waits on it yet, waits in its stage's bubble
with its verdicts, folded to its head line as every card there, since open it took most of the conversation, the bar
opening it; and a work the architect paused for what only the owner can do (a credential, a permission, a configuration
in the data directory) is pinned on the bar while the pause holds it, with the architect's why and Start, which lets it
go on once the owner has done it, since such a pause is how the architect puts an action to the owner.

The stream runs oldest first, the newest at the bottom and scrolled to: each agent's message beside its avatar in its
role's colour, headed by whom it is for, the owner's on the right; in wf-agents' own conversation the engine's notices
on their `Timeline` (above); a work's group a chat with nothing the engine writes as an entry of its own (above); in the
one-to-one a bubble of the architect's of one line for each matter it took up but the owner's words, with no one named,
updated in place and standing where it last changed, its head saying when the architect took it up, so that what changed
last is at the bottom, the one in hand with it (a matter it never took up is only in the queue, one withdrawn untaken
coming in when it was withdrawn, with why): its kind in its head with where it stands as a mark (above), and its line
saying in words: working, live, with what holds the architect when anything does; held by your ask; back in the queue;
done, with what came of it in a few words and what it made listed by name under the line, not folded (each observation
it recorded by its title, each item it made, cancelled or moved by its number and title, each leading to it) and what
else came of it (the engineer it answered, leading to the exchange in that work's group; the works it paused, let go on
or started over); or withdrawn, with why; what it asks whole on hovering and folded under it, a plan fix's steps folded
too, leading to its cell or round on the architect's chart, a matter about a work leading to that work's group; a matter
that changed nothing leaving the conversation once done, kept in the queue's list of those handled with why; each
version of the architect's plan a bubble of its own saying what changed, the plan whole in a card that opens under it;
what the architect says to the owner headed `@you`; nothing the engine writes an entry of its own, the architect's own
trouble on the bubble of the matter in hand, in the head and pinned while the owner must act; the question whether its
draft of a new policy is approved with Approve in its bubble, which replies exactly what the flow takes as the approval;
the owner's words no card, but once the architect took them up one small line under their bubble saying where they stand
("the architect is replying", "replied"), in the one-to-one and in a work's group alike, leading while it handles them
to its cell on the architect's chart; the queue in the panel beside it (how many are left, the oldest wait, how many
were handled today, over the list with the matter in hand marked), and the policy as it took effect. Opening a
conversation reads it up to its latest entry, its messages and notices with it; a fault of wf-agents is marked read
where it shows. The owner's words that were sent and never taken in, a work having ended first, say so on themselves, in
place of a notice of the engine's.

Instead of what passes between agents folded, a card or a form of its own in each place, every ask pinned under the head
and answered by the box at the bottom switched to it, the owner's answer shown again beside the ask as the owner's
words, and each matter's card where its matter came, because the owner reads one work's whole story in one place and
answers it there, found a second place for an ask and a box whose words meant something else once a button was clicked
odd, found the answer said twice, read the cards of matters queued at once and taken up in another order upside down,
handled, in hand and queued mixed in one block, and asked what the card under the owner's own words was, which only
repeated them.

And instead of a band under the head holding the asks and the card themselves, the stream keeping one line of each where
it was asked, because the owner found the band odd in a conversation of bubbles; the bar keeps everything that waits in
one place to find, while each thing is answered where it was asked, not by the box at the bottom switched to it, so that
the box still means one thing.

### The bar of what waits

What waits for the owner in a conversation is pinned in one bar under its head, as a chat app pins a message (ours,
after Telegram's and WhatsApp's pinned bar): one line the conversation's width on `--bgColor-attention-muted`, the tint
of what waits rather than a grey, which does not stand out, a `--borderWidth-thin` `--borderColor-default` line under
it, `--base-size-4` `--base-size-16` inside: a `PinIcon` in `--fgColor-attention`, then one of what waits, the oldest at
first, after its place among them in small `--fgColor-muted` text ("2/3") while more than one waits, its words in body
text cut where the line ends and whole on hovering, a decision card by its number ("Decision card 15 waits for your
ruling"), an ask and an architect's question by their first line; a click on it, a button with no frame whose words are
underlined under the pointer, brings that thing into view in the stream, its head at the top, a folded card opened, lit
for a moment in the tint of what waits with a `--borderWidth-thick` `--borderColor-attention-emphasis` edge that fades,
and shows the next; while more than one waits, an invisible small `IconButton` with a `ListUnorderedIcon` at its end
opens Primer's `ActionMenu` listing them all, each in two lines at most, a pick bringing that one into view.

Each thing stays where it came in the stream and is acted on there: each decision card that waits for the owner's
ruling, whether or not its work waits on it yet, in the bubble of its stage, folded to its head line as every card
there, ruled in it, and shown read only in its item's design at its anchor; an ask of the owner from a work as the
asking agent's bubble, answered in it (below); an architect's question as its message with Reply (below). What has no
place in the stream is its words alone on the bar, with its one action at its end: while the architect's pause holds a
work, why it paused it, with the primary small `Button` Start, which lets the work go on and tells the architect as the
head's Start does; while the engine halted the architect's work, its trouble, the bar then on `--bgColor-danger-muted`
with the pin in `--fgColor-danger`. Scrolled up away from the newest, a round `IconButton` with an `ArrowDownIcon` at
the stream's bottom right, `--shadow-floating-small`, goes back to it.

The stream holds the latest 100 entries of the conversation and, whatever their age, every entry something waits on (an
ask or a question that waits, the bubble of the stage that holds a decision card to rule on, words not taken up yet), so
that nothing the bar pins is left out; the bar, the counts and the box that replies are worked out from the whole
conversation. Where older entries are left out, a line of `--fgColor-muted` small text in the stream says "N earlier
entries are folded away", with a small invisible `Button`, Show earlier, that reads 100 more of the latest, the line
closing up as the entries reach the next one that stayed; the stream stays as it is while they are read, and is not
scrolled to the newest when they join it above. A bubble that links to an entry left out has no link. At the most the
interface gives (1000), the line has no button. It is a line and not a scroll that reads on its own, because the owner
goes back that far now and then, and a read that follows the scroll would make the page read as often as a long
conversation is scrolled.

Instead of a band holding the cards, asks and questions themselves, under the head and over the box, with a firm edge
the owner dragged, a cap of 45% to 85% of the conversation and a fade where it held more, because the owner found the
band at the top and the bottom odd in a conversation where everything else is a bubble, and what a chat app pins is a
message kept in its place with one line that leads to it: the bar keeps everything that waits in one place to find, the
stream one place for each thing to read and answer, and nothing is drawn twice or stands in one line for itself.

### Answering in the message

What answers a message is in that message's box (ours, in Primer's parts), under what it says and a `--borderWidth-thin`
`--borderColor-muted` line apart, `--base-size-8` between its parts: an ask's options picked on their own boxes in the
ask's card above it, as the options of a decision card are (above): a click on a box picks it and a second clears it,
the one picked marked as the owner's, the boxes a radio group; then the owner's words in a `Textarea` full width, one
line as tall as the small Answer `Button` beside it, growing as the words beside a button do (above); an architect's
question holds neither but Reply, a small `Button` with a `ReplyIcon` that sets the box at the bottom to reply to it,
and, while it asks whether a draft of a new policy is approved, the primary small `Button` Approve before Reply, which
replies exactly what the flow takes as the approval (above); once answered, the owner's answer in their place, headed as
a message is ("Your answer to ask 12" semibold, its time small and muted) over the sheet of what is sent to an agent; a
message the owner answers takes the row's width; an answer of more than 1,000 characters folded as a text in a sheet is.
An ask that waits is answered where it was asked, the bar of what waits leading there (above).

Instead of the buttons and a box apart from the message, or the box at the bottom switched to answer it, because the
owner reads a question and its answer as one, and a box inside the question says at once what it answers.

### An answer to a question in hand

The owner's answer in a group to a question a work of the architect has in hand is refused, saying to tell the architect
instead, instead of two answers racing to the work, because the architect may be writing its reply at that moment; a
question the architect holds and has not picked is answered, its matter leaving the queue with the owner's answer as its
reply.

### A question about a card

The owner's question about a decision card, and its answer, have over their words the strip that names what a reply
answers (below): a `ReplyIcon` and "About decision card 15:" with the card's question on one line in `--fgColor-muted`,
cut where it ends and whole on hovering, a `--borderWidth-thick` `--borderColor-accent-emphasis` edge at its left,
`--base-size-4` over the words; it is a button, its words in `--fgColor-accent` on hovering, that brings the card into
view, opened and lit, as the bar of what waits does.

Instead of the question naming the card in its words, because a question and its answer read as replies to the card, as
a chat app quotes the message a reply answers, and the owner goes from one to the other.

### The box in the one-to-one

In the one-to-one with an architect the box at the bottom adds to its queue, or replies to a question the architect put,
which a strip over it names as a chat app quotes what a reply answers (a `ReplyIcon`, "Reply to:" and the question on
one line in `--fgColor-muted`, cut where it ends and whole on hovering, a `--borderWidth-thick`
`--borderColor-accent-emphasis` edge at its left, and an invisible small `IconButton` with an `XIcon` that takes the
reply off), which question it names being worked out as the architect's questions say (below). It is a `Textarea` and
Send, as every box at the bottom of a conversation (ours), with no other choice of what the words do and no row of whom
they are for; an interjection of the owner's to the architect shows to whom at its head as a group's message does,
`@architect`, and what a policy setting said is the architect's message.

Instead of a `SegmentedControl` choosing between Interject and Add to queue, with `@architect` and `@policy setting` to
tick, because the owner's words reach the architect only through its queue.

### The architect's questions

The questions an architect put to the owner for a reply, each an ask of its own that waits and parks the matter it had
in hand, as the engine reads them, wait where they were asked in the one-to-one, each the architect's message with no
box, pinned on the bar of what waits (above) while it waits; under it Reply, which sets the box at the bottom to reply
to it, a strip over the box naming it ("Reply to:" and the question), the owner's words then the reply to that question,
which the interface's say to the architect takes with `reply_to`, its matter coming back to the cell that asked with the
reply; with the strip taken off, the words are the owner's words to the architect, which H8 answers. The strip names at
first the question the architect waits on, as its look at the queue (`N0`'s row `waits`) works it out from the same
reading of the database (the newest question pending that does not pin an open question of an engineer's design, put
less than the installation's wait for the owner's reply ago, with nothing said by the owner since), which the interface
gives the page with the questions; a question that pins an open question is never the one the strip names, because the
architect does not wait for it, and the owner answers it with the Reply under it (words typed in the box with the strip
taken off are the owner's words to the architect, not the answer to it, and the open question stays unanswered); once the owner clicks
Reply on a question or takes the strip off, it stays so until the words are sent. Replied to, a question holds the reply
in its bubble; one whose matter is gone, handled or withdrawn, is pinned no longer, with no Reply.

Instead of the questions waiting at the bottom of the one-to-one over the box, the newest open and the others folded,
with a tick over the box saying the words reply to the one open, because the owner found the band at the bottom odd in a
conversation of bubbles, and a chat app replies to a message by quoting it over the box.

### The box at the bottom

The box at the bottom of a conversation says something: in the one-to-one, to the architect, as the owner's words to its
queue, which it takes up first once the matter in hand is done and answers in one round, or, with the strip over it, as
the reply to the question it names (above); in a work's group, to its engineer as an interjection (the default), to its
architect as the owner's words naming the work, or to both, as one message whose head names whom it is for; in a group
of projects, to one architect or every one; Enter sends what is written and Shift with Enter starts a new line, an Enter
that only ends a word typed through an input method sending nothing, and the Send button stands level with the box's
middle; whom it is for is ticked in a row above the box, each party a checkbox, the engineer ticked at first, both
ticked meaning both and every architect ticked meaning all, instead of a choice in a menu beside the box, which the
owner found out of line and which could not say several at once; and an engineer's question the architect holds is
answered in the group by a reply to it, written in the small box in the question's bubble, which the work takes as it
takes the architect's reply and the architect is told; the box at the bottom never answers a bubble. At a work's end
(`E1`) and after, the box of its group is closed and says why where its prompt would be, with no row of whom it is for,
since its way on no longer changes.

Instead of a line chosen among the work's helpers, because the owner speaks to the engineer and the architect, and the
helpers take nothing from the owner.

### An architect's matter

An architect's matter is one part wherever it shows, its bubble in the one-to-one and the queue in the panel beside it
(ours): its kind in small `--fgColor-muted` text ("Plan fix: something missing"), then what it asks in one sentence in
body medium, for a plan fix where it is and what is wrong there, whole and wrapping, then its facts, its state as a
`Label` and its links in the same run of text; in its bubble its kind is what the head says it is about, its head says
where it stands as a mark and its line what came of it, the sentence whole on hovering and in a `Fold` under the line,
since a line of the conversation says the matter in a few words and the sentence, written for the queue, ran it past the
stream's width; and the steps of a plan fix are a `Fold` under it, holding them as an ordered list in the box of what an
agent handed back.

Instead of the engine's sentence for agents cut to one line with the whole in a tooltip, and the queue's words in a look
of their own, because the owner reads a matter alike in the queue and in the conversation, its kind already says what is
to be done, and a sentence cut short told the owner nothing.

### Where the owner's words stand

The owner's words to an architect are no matter card in a conversation: once the architect took them up, where they
stand is one line in small `--fgColor-muted` text under the owner's sheet, outside it and at its right (ours, as a chat
says under a message what became of it), `--base-size-4` under it, in the page's words for the state ("the architect is
replying", "replied", "held by your ask", "withdrawn"), followed while the architect handles them by a bar and a `Link`
"On the chart" to its cell on the architect's chart, in the one-to-one and in a work's group alike; words it does not
have yet keep only "not taken up yet" at their head. The queue in the panel beside the one-to-one lists the owner's
words that wait, as any matter.

Instead of a matter card for the owner's words, kind "Your words" and "The owner says: ...", under their bubble, because
that card repeated the bubble over it and the owner asked what the box was.

### The panel beside the one-to-one

The one-to-one with a project's architect has a panel on its right, split from it by a divider that drags its width,
kept in the browser, which the owner closes and opens again, the panel's close at the right end of its tabs and, while
it is closed, a button at the end of the head's row that opens it, whether it is open being kept in the browser for
every one-to-one, open at first, and an address that names one of its tabs opening it there; where the window is too
narrow for the list, the conversation and the panel side by side (below 1280px), the panel is over the conversation, in
its place, open only while the address names one of its tabs, the button at the head's end opening it and its close
going back to the conversation; its tabs: Notebook, the two parts of the architect's notebook (how it understands the
project, and the plan) as they stand, under the round and cell that last wrote them, or that nothing wrote them yet, the
earlier versions folded under each, newest first, each under the round and cell that wrote it and when; then what came
since the base wrote them, oldest first, each matter handled with its record and the lines it added, and each item
merged or cancelled, which a new base clears;
Policy, the draft that waits for the owner's approval on top while one does, headed as such, then the policy in force
with the project's constitution folded under it; and Queue, counting the matters left, how many are left, how long the
oldest waited and how many were handled today over the list of the matters, the one in hand marked, then those handled
with nothing to do, which leave the one-to-one once done, each with why folded. The panel reads the notebook, the policy,
the constitution and the matters handled itself (`/api/v1/architect/panel`), while it is open, and not with the
architect the one-to-one reads at every event; until they are read, its tabs say "Loading…" where they would show them.

The question whether to approve a draft does not carry the draft: its bubble leads to the panel's Policy tab.

Instead of the notebook, the policy and the plan on the architect's page, in its Policy and Next tabs, and the queue in
a few words at the end of the head's row that opened its list under the row, because the owner asked that the
architect's notebook and its policy be a window on the right of the one-to-one that closes, in two tabs, the notebook's
content and the policy, and that the queue, whose place and use the owner found odd, go there too; the plan is the
notebook's plan, shown once.

### The panel's look

The panel beside the one-to-one with an architect is drawn by the part of a page's panel (`ChartPanel`): an
`UnderlineNav` of its tabs, Notebook, Policy and Queue, each changing the address, the Queue's count the matters left as
a secondary `CounterLabel`, and at the row's right end its close, an invisible small `IconButton` with an `XIcon`; each
section of the notebook, the draft that waits for approval, the policy in force and the constitution a section of the
page (above) under who wrote it, a section's text and a policy a document filling its section, each section of the
notebook and of the policy folded at first, as the owner asked, since open ones ran the panel long, the notebook's
earlier versions a section one step down, folded, each version a block under who wrote it; the queue's facts small
`--fgColor-muted` words joined by a bar over the list of matters as the matter part draws each. It is the
`PageLayout.Pane` at the end of a split (above), its width kept in the browser, beside the conversation from the full
width up and over it, in its place, below that width (the rule for a split below the narrow breakpoint, above).

Instead of a look of its own, because like things look alike: a panel in tabs is learned once.

### The constitution in the Policy tab

The panel beside the one-to-one with an architect shows the project's constitution, the file its configuration names
(`constitution`), as the project's main has it, whole, in its Policy tab under the policy in force, each a section, the
constitution folded at first and saying which file it is and at which commit of main it was read, the commit short and
with when it was committed, so that the owner sees how new the text is, instead of on the Settings page, because the
constitution is the project's own file, not a setting, and the architect judges the project by it with the policy, so
the owner reads the two together; the settings name only its path. The commit is the one the engine last fetched: the
page fetches nothing, so after main was pushed to with the engine idle the page shows the earlier commit, and its time,
until the next round that reads main. A constitution that cannot be read is never an empty one: its section opens to
say why, in a sentence that names the file and the commit, and the rest of the panel is shown as it is.

### Forwarding a fault

A fault of wf-agents in wf-agents' own conversation is forwarded by the owner to any project's architect: its entry has
"Forward to...", a menu of every project's architect on the board ("The architect of p"), and the one picked has the
fault in its queue as its next matter, of the kind `report` with the detail `fault`, the fault then read; a fault goes
to several architects but to each once, the menu marking "Forwarded" those it went to, which cannot be picked again; and
under the fault's words a line for each architect it went to says so and how it stands there, "Forwarded to the
architect of p | in the queue", then "Forwarded to the architect of p | handled in R12-4", the round a link to it on
that architect's chart. The conversation's head holds the setting "Forward new faults to", Nobody at first or "The
architect of p" for any project, which the owner sets by hand and the board keeps across restarts (`owner_choices`, who
set it and when in `changes`): a new fault is then in that architect's queue at once and read, its entry saying
"Forwarded automatically to the architect of p". In that architect's one-to-one the fault is a matter in the queue like
any other ("Look into a fault of wf-agents").

Instead of every fault going by itself to the architect of the project it came up in, which would hand it on to
wf-agents' own architect as well, or to the project that is wf-agents alone, a name the page would have to know and a
board without it could not use, because which architect looks into a fault is the owner's to say, from all of them, and
nothing goes on by itself that the owner did not set.

## Roles and small marks

### Avatars

The page has no pictures of people, since nobody on it has one (ours): who is named in words; in a conversation and a
helper's thread each agent has an avatar, its line's Octicon (`ToolsIcon` the engineer, `ProjectRoadmapIcon` the
architect, `LightBulbIcon` the advisor, `CodeReviewIcon` the review, `BugIcon` the adversarial testing) in
`--fgColor-onEmphasis` in a circle (`--borderRadius-full`, `--base-size-32`) of its role's colour (below:
`--display-<colour>-bgColor-emphasis`), the owner none, and elsewhere an Octicon in a `Timeline.Badge` (`CpuIcon` the
engine), instead of made-up pictures or initials, because those would read as people the page does not have, while a
mark and a colour of each agent's own tell the two members apart at a glance, as the owner asked.

### A role's colour

A role has one colour wherever the page shows it (ours): its cells on a chart, its avatar and its bubbles, in a work's
group, in the one-to-one with an architect and in a helper's thread alike. The flow gives each line its colour by name
(`colour` on each of `sessions.lines`), the compiler refusing a name the page does not know, two lines of the flow with
one and a line with none in a flow that runs, while a flow it compiles only to be shown, such as the flow page's at an
older commit, may have lines with none; the interface serves the lines of the flow built into the program with their
colours (`GET /api/v1/flows/lines`), which the page reads once, as it starts, for every part that shows a role; and the
page draws a colour through one lookup, a class to a colour (`role-<colour>`, `roles.css`) setting four of Primer's
display tokens of that colour: `--display-<colour>-borderColor-emphasis` a cell's edge on a chart, `-bgColor-muted` a
cell's fill and a bubble's, `-borderColor-muted` a bubble's edge and `-bgColor-emphasis` an avatar's circle. The
engineer is blue, the deep research plum, the review teal, the adversarial testing auburn, the advisor pink, the
architect purple, the review of
the architect's batch indigo, the reader of a work's history green and the pick of the
architect's next matter cyan; orange (the owner's cells), yellow (the tools') and red (danger) are kept from roles, and
grey is the engine's own steps'; an agent with no colour of its own is the neutral grey of the engine's own steps, as a
box.

Instead of the chart colouring the lines of each graph by their order, so that the engineer and the architect were both
green there, and the conversation one colour each for the engineer and the architect and one for every helper, because
two systems that disagree make a role's colour mean nothing; written in the flow, beside the lines it colours, so that a
new line gets its colour where it is made and the page knows no role by name, and a work on any version of the flow
shows its roles in the colours the flow gives them now.

### Display colours

A role's colours are Primer's display colours, for its cells on a chart as for its avatar and bubbles (ours), instead of
the data visualisation colours a chart's cells had, because the display colours have each of the role's colours in both
themes and the data visualisation colours have no indigo; for blue, auburn, pink and purple a cell's edge and fill are
the same as the data colours were, and teal's edge a step darker.

### The engine's grey

The engine's own steps on a chart, and an agent with no colour of its own (a cell on no line, a line the flow no longer
has, any agent while the lines are not read yet), are the page's neutral grey (ours): an edge
`--borderColor-neutral-emphasis` and a fill `--bgColor-neutral-muted`, the engine's step rounded and an agent's cell a
box, instead of the display grey, because the display grey's tint is the stage boxes' own fill, on which a step in it
would look empty, while the neutral tint, translucent, reads over a stage's box and the chart's background alike; and
instead of purple, which is the architect's.

### Small marks

A fold marker is `ChevronRightIcon` closed and `ChevronDownIcon` open, in `--fgColor-muted`; a done step is
`CheckCircleFillIcon` in `--bgColor-success-emphasis`, the current step `DotFillIcon` in `--bgColor-accent-emphasis`, a
step ahead `CircleIcon` in `--fgColor-muted`; how sure an agent is shows as five dots `--base-size-8` wide, filled
`--bgColor-success-emphasis` (ours), instead of characters (a tick, a triangle, a circle), because a character used as a
mark takes its size and weight from the font round it, so that one mark would differ from text to text, while Primer
draws its marks as Octicons.

### A notice of the page

Something the page or the engine tells the owner about what the view shows, said nowhere else (a request that failed, an
item cancelled, why a round's return was sent back or its run ended without one) is a `Flash` in the variant of its
meaning, while a work the engine stopped says so on its line (above), and a hint beside a control is small
`--fgColor-muted` text; loading is the words "Loading" in `--fgColor-muted`, because a notice about the page must not
look like something an agent wrote.

## The charts

### The page's own SVG

A flow's state machine is drawn as SVG by the page's own components in a pan-and-zoom view of its own, instead of a
graph library such as React Flow, because it draws what such a library does not (stage boxes around their cells, arrows
routed at right angles with rounded bends and an arch where two cross, small circles for rows to another part of the
chart, rules and the interjection beside their cells, the engine's actions on lines beside the cells they lead into, the
stores and the lines to them) and is moved and zoomed as the owner's approved state-machine page was, which a library's
own handling of the wheel and the keys does not allow.

### Moving and zooming

Every chart, and the enlarged view of a diagram (Diagrams), moves and zooms alike: two fingers or the wheel move it,
Shift with two fingers or a pinch zooms at the pointer, dragging moves it, and Space shows the whole and goes back; the
pointer is the pointing finger over what a click or hovering does something with (a cell, an arrow, its label and the
wider band around it, a circle, a badge, the "?"), the plain arrow over empty space, and the grabbing hand only while
the button is down, because one chart that behaves differently from the others makes every gesture a guess, and a hand
everywhere said that nothing could be clicked.

### The map

Every chart (the flow page's, a work's and an architect's) has a map over its top left corner, under the flow's tools on
the flow page: the whole chart drawn small, its stage boxes and cells, and the stores' boxes and stores, as plain shapes
with no text and no arrows, each cell and store in the colours the chart draws it in, from the one rule that colours it
there, instead of every cell dark, as the owner asked, on a work's and an architect's chart the cell the work is at
ringed and the cells it never reached muted, as on the chart, and the part in view as a frame; a click on the map brings
the point clicked to the middle at the zoom the chart has, dragging the frame moves the chart, and the wheel and two
fingers over it move and zoom the chart as anywhere over it; it is at most 200 px wide and 160 px tall, in the chart's
proportions, drawn from the layout the chart already has, its frame following the view without the chart being drawn
again, and it hides while the whole chart is in view and while what can be seen is too small for it to keep clear of the
middle, where the chart centres on the cell a stage works at, the legend's "?" staying over it; under it the chart's zoom
as a whole percentage, 100% at full size, following the view as the frame does, and in the map's place while the map
hides, so that the owner always reads the zoom, as the owner asked; instead of finding a part of a large chart only by
showing the whole with Space and going back, and instead of the arrows drawn on the map, which at the scale the whole
chart is drawn at run a few pixels between cells and over them, because the owner asked for a small map in the corner of
each chart, to see where the view is on the whole chart and to go to another part of it at once.

### A cell's shape and colour

On a chart a cell's shape and colour say who does its work, and nothing else does (ours): a box is an agent's cell, in
its role's colour (above); a rounded cell is no agent's, the page's neutral grey the engine's own step, a quiet yellow a
tool the engine runs, and orange the owner's; every rule below on a cell's look keeps to this, and a test over the flow
holds that only an agent's cell is a box, instead of a shape or a colour for each kind of cell the flow defines, because
the owner reads a chart by who works at each cell, and read a tool the engine runs, drawn as a box in a colour of its
own, as an agent's cell.

### A chart's tokens

A chart keeps its layout, shapes and lines as the rules of this section draw them (stage boxes, routed arrows with
rounded bends and arches, circles, badges, the halo's three arcs), and only its look follows the rest of this spec: its
text, borders and backgrounds use the functional tokens (`--fgColor-default` and `--fgColor-muted` for labels,
`--borderColor-default` and `--borderColor-muted` for boxes and the rules beside them, `--borderColor-emphasis` for the
arrows, `--borderColor-accent-emphasis` for the main path, and `--borderColor-success-emphasis` and
`--borderColor-danger-emphasis` for the rows into and out of the cell lit, which must read on the chart's inset
background, `--bgColor-default`, `--bgColor-muted` and `--bgColor-inset` for fills), and its text the type steps at the
sizes the owner set for reading it at full size: a cell's title, its code with it, body large semibold
(`--text-body-size-large`, 16px), its notes, the arrows' labels and the nodes beside the cells body medium (14px), a
stage's name title medium (20px semibold), a step over a cell's title, so that a stage's name stands apart from the
titles of the cells in it, which were the same size, codes monospace; text the layout measures is measured in the font
the tokens give, read from the page at run time, instead of a font written in its code.

### A chart's text

A chart's text is two sizes above the page's small text (a cell's title 16 px and semibold, its code beside it in the
page's monospace, its notes, the labels on the arrows and the nodes beside the cells 14 px, the stage names 20 px and
semibold; the corner badges keep their size; each in the font the page's tokens give, which the chart reads from the
page to measure its boxes), and a cell holds its text with little room around it (12 px at the sides, 9 px above and
below), each line of a cell, of a node beside it and of a label placed by its middle so that the room above the first
line is the room below the last, instead of 13 and 12 px in roomy boxes and text set by its baseline, which sat closer
to the top border than to the bottom, because the layout gives each box the size its text takes, so larger boxes still
fit, and the owner reads it at a distance.

### A cell's padding

A cell keeps the tight padding the owner set, 9px above and below its text and `--base-size-12` at its sides, the 9px
named among the chart's values since Primer's scale has 8 and 12, and the corner badges (the effort, the new session,
the rounds) keep their size, their 10px text named there too since Primer's smallest text step is 12px, because the
owner asked for larger text in the boxes and less room round it, and the chart's conversion must not undo either.

### Legend and panels are ordinary parts

A chart's legend (its "?") and every panel that opens from it (a cell, a round, a rule, a store, a comparison) are
ordinary page parts and use the components above, because the owner reads them as the rest of the page and must not
learn a second look.

### The "?"

A chart's "?" stays in the chart's bottom left corner whether its legend is open or not (ours): a round surface of the
controls that float over a chart (above), drawn as the option picked (`--bgColor-accent-muted` edged
`--borderColor-accent-muted`) while the legend is open; the legend opens above it, its left edge with the "?"'s and
`--base-size-8` over it, on the same surface, its words small text, no taller than the chart's room between the "?" and
the flow's tools over its top left corner, what does not fit scrolling inside it; a press of the "?" opens and closes
it, a press anywhere but on the legend or the "?" closes it, as Primer's overlays take a press outside them
(`useOnOutsideClick`), and goes on to do what it does there (picks what is under it on the chart, moves the chart), and
Escape closes it and leaves open what is open in the window floating over the chart.

Instead of the "?" as the head of the legend it opened into, at the legend's top, because the owner asked that the "?"
not move away from where it was pressed, and that a second press of it or a click anywhere else close the legend; and
instead of Primer's `AnchoredOverlay`, which floats over the whole page, so that the chart's room above the "?" would
have to be measured by script, and moves an overlay that does not fit above its anchor to another side of it, off the
"?".

### A chart's category colours

A chart's category colours come from Primer's data visualisation and display scales instead of hand-picked hex values:
an agent's cell its role's colour, the display colour the flow gives its line (above), beside the owner's
`--data-orange-*`, a tool's quiet yellow and the engine's neutral grey, the stage boxes `--display-gray-bgColor-muted`
with `--display-gray-borderColor-muted`, light grey, as the owner asked instead of the palest warm display colour
(`--display-brown-*`), since only the cells have colours, which stand out on it, the effort badges, as the owner chose
them, `--bgColor-success-emphasis`, `--bgColor-accent-emphasis`, `--data-orange-color-emphasis`,
`--bgColor-danger-emphasis` and `--bgColor-done-emphasis` from low to max, each lettered in `--fgColor-onEmphasis` but
high, lettered in `--fgColor-default`, which white does not read on in the light theme, high a plain orange since a
darker one (`--bgColor-severe-emphasis`) is too close to xhigh's red for the owner to tell apart and a pale one
(`--display-orange-bgColor-muted`) hardly shows, and the running cell's halo, which the owner chose and which stays the
chart's only motion, in the page's red, orange and blue (`--borderColor-danger-emphasis`,
`--borderColor-attention-emphasis` and `--borderColor-accent-emphasis`).

### A cell's edge

Every cell's edge is solid, and who does a cell's work shows in its colour alone: an agent's cell with no colour of its
own (on no line, or on a line the flow no longer has) is filled as any agent's cell is, in the page's neutral grey, the
engine's own step in the same grey (above), a tool the engine runs rounded as the engine's own step is, in a quiet
yellow, `--data-yellow-color-emphasis` and its `-muted` lightened a little, three quarters of each mixed into
`--bgColor-default`, told from the engine's step, which waits on nothing outside, can fail only by a fault and leaves no
output, yet standing back from the cells an agent or the owner works, and only an agent's cell a box, instead of a tool
in a lemon box, which the owner read as an agent's cell; that an agent's cell starts a new session every time it runs,
which only a cell on no line does, is a badge over its top left corner after its effort badge (ours, as the effort badge
below is), of an effort badge's size and text, saying `new session`, on `--bgColor-default` with a
`--borderColor-emphasis` edge and `--fgColor-default` letters, outlined instead of filled so that it reads as no effort
level, instead of a cell on no line drawn on the page's background with a dotted edge and an engine's step and a tool
dashed, a tool in grey, because the owner could not make out the dotted and dashed edges, the white cell looked empty
and the grey one disabled, as a cell a work has not reached looks, the colours already tell an agent's cell from the
engine's and a tool's, and dashes are kept for marks that matter more.

### Only a cell has colours

Only a cell, a place a work stands at, has colours on a chart, its session's, an agent's its role's, the engine's
neutral grey, a tool's quiet yellow or the owner's orange (above); a circle that names a cell takes that cell's, from
the one rule of the cell's kind, which the map's mark of the cell takes too; every other shape (the actions on a line or
every time a cell returns, a rule, the interjection, a row's label, a store, and a store beside a cell, rounded, its
text body small, the part of the flow the store is kept for in `--fgColor-muted` before its title, its line dashed for a
read and solid for a write) is `--data-gray-color-muted` edged `--borderColor-emphasis`, and its arrow the arrows' grey,
instead of a purple hexagon, a blue rule, an orange flag, a teal store and circles of their own colours, because the
owner found the chart too many-coloured and asked that only the cells a work goes through carry colour, one to a
session.

### Line weights

A cell (an agent's, the engine's, the owner's or a tool's, each named by its code) is edged `--chart-line-cell` and
titled semibold, its code with it, the text under it normal, a rounded cell's corners no rounder than the half-circle
ends of a cell of one line, so that every line of its text lies inside it, which a test over the flow holds for every
cell, and every other shape on a chart (a stage's box, a circle, the actions on a line, a rule, the interjection, a
store, a mark or a badge) is edged `--chart-line-thin`, its text normal; the arrows are `--borderColor-emphasis` at
`--chart-line-arrow`, the main path at the same width in `--borderColor-accent-emphasis`; the rows into the cell lit are
`--borderColor-success-emphasis` and those out of it `--borderColor-danger-emphasis`, at `--chart-line-arrow` with heads
of an arrow's size, their labels filled in the same colours (`--bgColor-success-emphasis` and
`--bgColor-danger-emphasis`) under semibold `--fgColor-onEmphasis` letters; the arrows the round chosen came in by and
left by take the same two colours at the same width, flowing; a row the work took stays `--data-blue-color-emphasis`,
flowing, at `--chart-line-arrow` too, as the owner asked; the row pointed at, on the chart or by its line in a panel's
From or To (below), alone is heavier, at `--chart-line-marked`, its label semibold, and keeps its colour, its dashes and
their flow as it is drawn without the pointer; no other mark makes a line heavier.

Instead of a cell edged about as heavy as the shapes round it, a taller rounded cell's ends drawn as half-circles, which
cut into its lines, the engine's step lighter still, the main path a heavier `--fgColor-muted`, and the round chosen in
a pink of its own, because the owner found the cells too like what is round them and asked them heavier and their titles
bold, and saw the text of a tall engine step run out past its ends, asked the main path blue instead of heavier, and
asked a cell's rows in green and out red, one colour to a meaning: the main path is drawn where no work's rows are (a
work's chart draws the rows it took and mutes the rest), the in and out colours are the page's success and danger
colours, light since the page has its own palette (above), where they were the data visualisation's green and red,
lighter than Primer's success and danger colours, which the owner found too dark, and still green and red on the inset
background and a stage's box in either theme, a lit row's label is filled in its colour, white on it, as the owner
asked, and a lit row's dashes flow the way the row goes, as a row taken flows in blue, as the owner asked, so that the
ways in and out of the cell picked move as the way a work went does; the row pointed at was as thin as any arrow until
the owner asked its arrows bold, to find on the chart a row read in a panel's From or To, and then orange until the
owner asked it only heavier, keeping its colour and its flow, so that a row lit green or red stays green or red.

### A chart's named values

A chart value Primer has no token for (an arrow's stroke width, an arrowhead's size, a halo arc's width and speed, a
badge's size in the chart's own units) is named once, with its reason, in the chart's part of the page's token file
(`client/src/styles/tokens.css`), which the check lets write raw values, and a size the layout computes with once as a
constant of the chart's code, instead of numbers written through the chart's styles, because a value written twice
drifts.

### Arrows

An arrow on a chart is thin and grey, the main path as thin in blue instead of heavy black or a darker grey a little
heavier, as the owner asked, the main path being a cell's own answers alone, never a move such as a search drawn as a
line, though its row is marked main, since a move is on no cell's way through the flow, its head small and of one size,
on a last straight piece at least as long as the head and pointing along it; an arrow's label names its rows, with no
numbers, on every chart, as a cell's panel names them, instead of thick black lines, heads that grew with the line and
sat beside it where its last piece was short, and a badge with each row's number, which the owner found meant nothing, a
cell's panel showing no numbers, because the owner follows a row across the chart by eye and reads it by its name.

### An arrow pointed at

Hovering an arrow or its label lights up that arrow, heavier than every other arrow and keeping its colour (above),
which the owner first asked be no heavier and then asked bold, to find on the chart a row read in a panel's From or To,
and its label, semibold, and, when the arrow is one half of a row drawn through a circle beside the cell it leaves and
one beside the cell it enters, its other half into the same cell, and nothing else, so that a row drawn into several
cells, such as one going back to whichever cell asked, lights only where it is pointed at; a click on an arrow or its
label keeps it lit so, until it or the chart's background is clicked again, and nothing keeps it past the page, because
the owner follows an arrow far across the chart, where the pointer cannot stay on it.

### An arrow's label

Every label sits on its own line, near where its arrow leaves its cell, the first place along the arrow clear of every
node, of a stage's name and of the labels placed before it, the labels with the least room placed first, so that the
owner, who reads a cell's ways out at the cell, finds them there however far the arrow runs, instead of in the arrow's
middle, often out of view, as the owner asked, and never over a stage's name, and the arrow out of the actions on a line
carries no label, the arrows into them naming their rows, hovering one lighting the way on, and hovering it every way
in.

### A cell's rows lit

The cell picked, its panel open (a round's cell among them), lights its rows on every chart: each row into it green and
each out of it red, at an arrow's width, its label filled in its colour with white letters, as the owner asked, who
found the first green and red too dark and the lines too heavy, a row drawn through the actions on a line or a circle of
another part or graph lit whole, while the arrows of a rule, of the interjection and to what the cell does every time,
which are no row, are not; hovering a cell lights its rows the same way until the pointer leaves it, hovering the cell
picked changes nothing, and hovering another cell shows its rows alone, the picked cell's coming back as the pointer
leaves, so that two cells' rows never mix; a click on the chart's background leaves the cell picked as it is, since its
panel stays open (the window floating over a chart, above), instead of letting go of it as the panel's close did,
because the owner asked that the panel not close; the row pointed at is drawn over them, because the owner follows a
cell's ways in and out across the chart, and asked for the way in green and the way out red, lit while picked or
hovered.

### A cell and what is beside it

A cell and what the chart draws beside it are laid out together, the same way for every cell, and the layout places the
whole as one node: the circles of rows into the cell from another part in a row just above it, each with its row's name
on its outer side and a short line down into the cell, a single one left of the cell's middle where the cell is wide
enough; its rules on its left, the interjection under them, and what it does every time on its right, each joined to it
by a short level line; the circles of its rows to another part in a row just below it, each under the actions on its
row, if any, its row's name on its line; and a row from the cell back to itself as a small loop on its right; the arrows
to and from other cells meet the cell itself, in the middle of its top and bottom edges, between the lines of its
circles, and every line and arrow meets a cell away from its corners, where a rounded cell's edge is straight; a row to
a node above its cell leaves the cell's top, and one from below comes into its bottom, and runs only up or only down,
over no piece of another row, the layout given a row that runs up the chart already turned round, instead of a row the
layout turned round itself to break a loop, which it drew out of the cell's bottom, round the cell and up past the cells
between in a tangle of turns back that the owner found at H8, a test over the flow holding every row to it; the layout
draws its layers as close together as their arrows let them and gives a cell with many rows out, such as the step that
routes a matter, room for what they lead to beside it; and a stage's name is drawn over any arrow that passes under it,
on its box's colour; instead of each of these as a node of its own that the layout put wherever it liked, because the
layout pulled a cell's own circles far from it, such as T4's circle for A2, and drew long level arrows across the middle
of the chart to reach them; a test lays out the real flow and measures the arrows' total length, their longest level
run, their bends and how far what is beside a cell is from it, against limits a worse layout breaks.

### The effort badge

Every agent's cell, a gate's included, carries over its top left corner a small badge of the reasoning effort the flow
says it runs at, on every chart that draws cells (the flow page's, a work's and an architect's): the level's word,
`medium` shortened to `med`, the whole word on hovering, every badge of one size, each level in its own colour, green
for low, blue for medium, orange for high, red for xhigh and purple for max, lettered in white but high, lettered in the
text's colour, from Primer's emphasis backgrounds, which follow the light and the dark page, and the levels the chart
draws named in its "?" from low to high; an engine's, a tool's and the owner's cell, which run at no effort, have none,
and a work's round count stays at the top right, instead of a cell's effort read only in the flow's file and in the run
facts of each of its rounds, because the owner asked to see each cell's level on the chart at a glance, and instead of a
dark orange for high, which the owner found too close to xhigh's red to tell apart, or a pale one, which the owner found
too light.

Rejected:

- Steps of one violet with dark letters, which read as an order and share their colour with no other badge on the chart
  (the rounds' blue, the amber of returns sent back, the red of failed runs): rejected because the owner found them
  murky.

### Moves, actions and stores

What an agent's cell may do of its own besides its own answers (run tools, search, ask, pause, answer an interjection,
redirect, and the engine's going back to understanding when a session starts afresh in the middle of a work of the
architect) is drawn by how many of the cells that could do it do, worked out from the compiled flow for each version:
when every one does, the legend alone says so; when more than half do, the legend says every one but those named, and
each of them carries a small mark ("no tools") saying why on hovering, the engine's own move named in the legend without
marks; when half or fewer do, a line from each cell that does to where it goes, such as the architect's searches to
`X2`; a move of one line counts that line's cells, and the legend names the line. An asking cell's answer back to
whichever cell asked is drawn only into the cells whose own answers asked it: a cell that asked by a move the legend
names comes back as from any move, as from the tools or a pause, which is drawn for no cell, and its panel lists that
way back under Back here after a move.

What the engine does on a row is a node on its line just above the cell the line goes into, in that cell's box, on the
way in, which the lines with the same actions into the same cell share (`D8`'s two ways and `D9`'s two into `I1` through
one {open session, lock acceptance, post decision card}), and the lines of a cell back into itself share whatever their
actions, as they share its loop, an action only some of them do naming them (`M1`'s waits, "post flow card (on
flow_redrawn)"), and on a row to another part, just above the circle it leaves by; what a cell does every time it
returns is a node beside it on a dashed line, and what every agent cell does, or every cell of a line of more than one
cell, in the legend, saying which line, as for a move, instead of the same action beside each of the line's cells, which
the owner found made the chart too busy and told nothing new; under a step the engine decides, one line says what the
values it decides by read.

The stores the engine keeps what it writes in are drawn beside the graphs, between the first two, in three boxes by what
they belong to (the project, an item, a work and its rounds), a computed one dashed; pointing at a cell, the actions on
a line or a store, or opening one, marks the stores it reads or writes; beside each cell, on its left under its rules,
each store the cell reads or writes (but one every cell of a line of more than one cell reads or writes alike, which the
legend names once for the line, as it does an action) is a small light grey rounded box of its own saying the part of
the flow the store is kept for (project, item or work) and its title, a read's line dashed into the cell, a write's
solid out to the box (what the cell's step, what it does every time and the actions on its rows write), the lines
entering the cell's edge below the rules', which then end no lower than its middle, so that no line runs behind a rule
or another box, as the owner saw one do, a test over the flow holding every line of a rule, the interjection or a store
clear of every node but its ends, and a click on it goes to the store's box among the stores, as a circle goes to the
cell it names, instead of curved lines from the stores across the chart to the cells, which the owner found only made
the chart messy; a store and the actions on a line open panels of their own that name, in each graph, what writes and
reads the store. The engine's fallback is not drawn.

Instead of one circle beside every agent cell for the rows they all share, a circle from the owner's cell into every
architect cell that may ask the owner, actions only in the cell's panel, actions in the middle of a line outside the
boxes, which the layout put past the cell they lead into and on another cell's line (`R1`'s {open session, record place}
under `R2`, on the way to stage D), a node for each set of actions on a cell's lines back into itself, beside which the
arrows round `M1` turned back, and no stores, because the owner could not see from the chart what the engine does
between the cells and where what it writes is kept, a circle on every cell said nothing one cell did not, the way back
from a move is the legend's as the move is, actions away from their cell were read as part of another line, and lines
from every cell to everything they read would cover the chart.

### The flow page's graphs

The flow page draws every graph of the flow on one chart, side by side, each in a dashed frame round its drawing with
its name on the frame at its top left, moved and zoomed as one: a cell of either opens in the window floating over the
chart, and the choice of version and the comparison apply to both, the cells added and changed marked in each, instead
of a choice of one graph at a time, because the owner approved the two graphs side by side, and a change to one is read
beside the other.

### Comparing flows

The flow page compares two flows: two stored versions, a stored version and the flow as `flow/flow.json` is at a git ref
of a project's checkout, which the interface reads with git and compiles with the flow's compiler without writing
anything, or a stored version and the flow a decision card carries, read from the database; the chart of the newer is
drawn with the cells added and changed marked, and a panel lists what changed in each cell, a return field's change
saying of each side what earlier returns may hold there as the pattern does, because a proposed change to the flow is
reviewed against the flow it changes, on the page the flow is read on.

When the newer flow is read at a git ref, which no work runs on, the panel opens with a section above what changed,
Works going on: what installing that flow would do with each work that is neither ended nor done, as the engine judges
it (`GET /api/v1/flows/cost`, the judgment `wf-agents flow cost` prints for the same file), under a line saying which
flow version the flow would be. The works are in runs by what happens to them, in the order the engine gives: those that
cannot be worked out, those that go back through a cell and do work again, those stuck, those that end, those that move
where they stand, and those already on the version. Each run is a `Fold` headed by how many works it holds and what
happens to them, open for the first four and folded for the last two, since the owner reads the works that cost
something and has nothing to read in those that go on as they are. Each work is a row: its name, a link to its page,
the item (by its number and title) or the graph it works on, the flow version and cell it stands at and who paused it,
muted, and under it, set in by a line, what happens to it: the cell it goes back through and the cell it goes to, the
round it does again, or the cell it is stuck at and what the owner can do, or why it ends, with the engine's own words.
A flow this engine cannot judge, because it does not compile against the engine's database definition or does not fit
the configuration, shows the reason in a warning in place of the runs, and says what judges it instead, because a cost
that is missing must not read as no cost. The section is read once as the comparison opens, as the works stand then, and
no event of a work changing keeps it current, so that the list the owner reads does not move under the owner's eyes as
works go on; it is worked out again only when the page reads everything it shows again (the event stream
resynchronising, the engine restarting), which is when a new flow has often just been installed. The owner rules a change to the flow with its cost before it, which only the command line
gave, and an owner who must ask for a command does not see it. A comparison of two stored versions, or with a decision
card's flow, has no such section: what it would cost to install is asked of a flow that is not installed.

### The policy's stage on the architect's chart

The architect's chart draws the check of the policy in force, the writing of a new one and its taking effect as a stage
of its graph, the policy's box (`P1`, `P2`, `P3`), each entered from the router (`N2`) and going back to the batch
(`B1`), as every other stage, and no circle to or from another graph, instead of a circle beside the actions that
started the setting leading to `P2` of a graph of its own and one above `B2` from each cell of that graph that queued
the matter of its end, because the setting is the architect's own work, which the owner reads on its one chart and in
its one conversation, and a work of the policy's graph is drawn on the chart of the version it ran on.

### A work's flow version

A work is drawn on the state machine of the flow version it is pinned to, with its rounds placed on their cells (the
current cell highlighted, each visited cell with its round count, rounds sent back or failed marked, the transitions
taken emphasised), so that a round leads to its cell and a cell to its rounds, instead of on the current flow, because a
work runs only on its pinned version, and the current flow may no longer have its cells.

### The path a work took

On a work's chart the rows the work took are drawn with dashes flowing the way it went, as thin as any arrow, as the
owner asked, and every motion on the charts (the halo, the flowing dashes) moves whatever the browser's setting for less
motion, since the owner wants them to move, because the owner follows the path a work took; on a work's chart what the
work never went by is muted alike: a cell it never reached, an arrow and the label of a row it never took, and an
actions, rule or circle node none of whose arrows it took, or, beside one cell, whose cell it never reached, instead of
the cells alone, which left bright nodes and bold labels in a part never run.

### Centring a work's chart

A work's chart opens at full size centred on the cell the work is at, and the arrows into and out of that cell end at
the ring around it, because the owner opens a work to read where it is, and a chart opened too small to read hides it.
When a stage starts to work at another cell than the one the chart last centred on, the chart centres on that cell at
the zoom the owner has, and on no other occasion: the page reading the same work again, or the engine's status changing
while the same stage works, does not move it, because the owner watching a work follows where it works.

### The halo

While a stage of the work works, its cell, and only that cell, wears a halo that moves (three arcs of colour running
round the cell) and stays on that cell for as long as the stage works; when the work goes on to the next stage, the halo
goes to that cell, with no gap and never on two cells at once. A stage works where a round of it runs, the engine taking
its step, or a tool of it is under way (a tool cell such as the full CI runs for minutes), or where the work goes on and
its next step is due and the engine takes it (`course`'s `working`, components.tsx), the one fact the halo, the ring of
the cell, the map's mark, the centring of the chart, the running mark of the work's head and the icon of its state all
take, so that they agree. It is a fact of the work and the engine's status as the page has them, never of when a round
came to the page: a round that comes in and is over in milliseconds, as a tool's or an engine step's, draws nothing of
its own, and reading the work's rounds again, or the status changing while the same stage works, does not make the halo
go out or begin again. The halo is not drawn while the work waits (for the owner, the architect, other items, a free
slot, or a failed round to be run again), is held back, the engine has halted every work, the page cannot reach the
engine, and not for the engine's step of a waiting work that only looks whether what it waits for came, which runs
whenever another work moves and writes no round. While the architect waits for the owner's next words, its cell, a still
tag and no halo, the owner's words being what it waits for, not work. While no stage works at the cell the work is at, a
still tag beside it says in a few words why the work waits there (waiting for Step, paused, waiting for you, waiting for
the architect, waiting for other items, waiting to retry, waiting for a free slot, halted, paused by the architect, as a
work's states are worded) and in full on hovering, instead of two bars, as a pause is drawn, saying why only on
hovering, and no mark for a work that waits for something, which left the owner looking at a work standing on a tool
cell for half a minute with no sign of why; the last round that ended needs no mark, being the arrow taken into that
cell; both marks are in the chart's "?", because the owner looks for where the work works and where it stands still,
and why.

Instead of a halo that stayed 1.5 seconds on the cell of each round the page saw come in since it last read the work's
rounds, or, through several that came in together, half a second on each in turn (so that a round over in milliseconds,
as a tool's or an engine step's, was seen), which jumped from cell to cell through a batch of engine rounds that arrived
together and again through the rounds a window of the architect's page read late, and went out and came again with the
engine's steps of a waiting architect, because the owner said on 2 November 2026: "The mark itself going round is
right: it tells me which stage the work is at. While a stage works it goes round, and when it is done it goes on to the
next stage." A round too short to be seen is shown by the arrow the work took and the count of rounds at its cell, not
by a halo. Earlier instead of a ring that breathed, which the owner found too faint to notice, and a glow and a
spreading ring besides the arcs, which the owner found too much.

### A round's arrows

A round chosen, in the rounds list or by any link, on a work's chart and on the architect's alike, lights the arrows it
came in by and left by (the rows the round before it and the round itself took), the way in green and the way out red,
as a cell's rows in and out are lit, at an arrow's width with dashes flowing the way the work went, and only the way in
while it has taken no row; this mark is in the "?" too, because the arrows a work took again and again are many and
alike, and the owner could not tell which of them a round went by.

## The three voices

### Three voices

Every text on the owner's page is in one of three voices, and the page shows which at a glance, in one look for each
wherever the text appears (below): the page's own words, written for the owner (its headings, names, counts, links and
facts, and the flow's notes on what the engine does, such as a check, an action, a row's note or the engine's fallback,
which no agent is sent); what is sent to an agent (a cell's task, the descriptions of its inputs and of the fields of
its return, a rule, a format, a round's message and every input as the message gives it, among them what a tool
returned, which the round after gives the agent that asked, word for word); and what an agent handed back (a round's
return, and what the page shows of it elsewhere: the item's understanding and design, a decision card's and an ask's
fields, a progress note, what an agent told the owner). The page's own words are never inside the other two, and inside
them the page adds only names, numbers and controls, never a sentence of its own.

Instead of the descriptions written for agents set in the page's own style among its labels, chips and "used by" lines,
the page's notes inside the box of what was handed back, and the message and the return in two boxes told apart only by
their labels, because the owner could not tell the words written for the owner from those sent to an agent, and read
instructions to an agent as the page explaining itself.

### Their looks

Each of the three voices has one look wherever the page shows it. The page's own words are text on the page's background
in the type steps above, with no frame and no fill. What is sent to an agent is a sheet (ours): `--bgColor-muted`, a
`--borderWidth-thicker` `--borderColor-default` edge at its left and no other border, `--borderRadius-medium`,
`--base-size-8` `--base-size-12` inside, its prose in body medium, and a message as sent, a format, data as given and
what a tool printed in `--fontStack-monospace` at `--text-codeBlock-size`. What an agent handed back is the box:
`--borderWidth-thin` `--borderColor-default` all round, `--borderRadius-medium`, `--bgColor-default`, `--base-size-8`
`--base-size-12` inside, its prose as markdown in the page's font and its code and data in monospace, the rows of a list
in one box parted by `--borderColor-muted` lines. The box holds no round's message, which is a sheet: an agent's text
reads inside the box wherever it stands among the page's own words (a round's parts handed back, a message), while a
document that fills a section or a tab by itself is that section's body text with no box, as a document's rule says
(above), as the owner decided.

Instead of one box for what an agent got and what it gave, told apart by its label, an agent's words on the item page in
the page's own style, and a colour of its own for each voice (the accent for what was sent and success for what came
back, as the round panel had), because a filled sheet with a thick left edge and an outlined box differ in shape before
a word is read, in either theme and for an eye that does not tell colours apart, the accent and success colours already
mean a link, the place chosen and done, and both shapes are GitHub's: a quoted text is set off at its left, and what
someone wrote is framed.

### No sentence of the page's in a box

Inside a sheet or a box the page adds only names, numbers and controls, never a sentence of its own: a field's or a
value's name in body small `--fgColor-muted`, a row's number in `--fgColor-muted`, a value the schema fixes (a kind) as
a secondary `Label`, what each round fills in as its name in body small `--fgColor-muted` inside a `--borderWidth-thin`
dashed `--borderColor-default` outline on `--bgColor-default` with `--borderRadius-small` (ours), and the control that
opens a fold; what the page says of the text (what it is, where it came from, why it is there) is over the sheet or the
box, as a sub-heading or small muted text, because a sentence of the page's inside an agent's words is read as the
agent's.

### A long text folds

A text in a sheet or a box of more than 1,000 characters shows its first three lines and, under them inside the sheet or
the box, a link-styled button in body small `--fgColor-accent` with a `ChevronDownIcon`, saying what it opens ("Show all
10,489 characters, 85% of the message"); a list of more than three rows in a box shows three and a last row "Show all
15"; a shorter text is never folded.

Instead of a right-pointing or down-pointing triangle character before a label over a preview that fades out, because
Primer's fold marker is the chevron (above), and the owner reads the size before opening.

## A cell's and a round's panels

### Four sections

A cell's panel and a round's panel answer the same four questions, in the same order, from the same parts, drawn as a
page's sections are, each saying in small muted text after its count what it holds: In, what goes in; Out, what comes
out; From, where it comes from; To, where it goes. On a cell, In and Out are the pattern of what goes in and comes out,
From and To its transition tables; on a round, In and Out are what actually went in and came out, From and To the cell's
same tables, with the rows the round actually came in by and left by marked and every other row greyed. A cell's panel
is one page of these four sections; on a work's chart its rounds there, and in a comparison what changed, stay tabs
beside it, named The cell. Under its title, its code and title and, in parentheses, its stage as the box around it on
the chart names it (`R2 Whether it stands (Stage R: research)`), its line too when the chart draws that line in no
colour of its own. All four are folded at first, From and To too, as the owner asked, who found the tables, open, ran
long down the panel, those the owner opens or folds staying so from cell to cell, as the tab chosen does, and from round
to round.

A round's panel opens with a short head of label and value pairs, Ran and Cost, the value's second line in body small
`--fgColor-muted`, and why a return was sent back or a run ended without one as a `Flash`; its In is what the round was
given (the inputs new this round and those already in its session, and the message as sent; for the tools, each tool's
question; for an engine's step, what it read), and the cells it names are those of the flow version its work ran on,
whichever version the chart beside it shows. In: the task, sent every round, in the sheet of what is sent to an agent,
folded to its first lines with its size, and under it, folded, the whole message in the order sent (its heading, why the
round happens, what is given, the task, what to work from by each input's number and addresses, the review table, what
to hand in) with what each round fills in; the inputs in the order the message numbers them, a row each with its name,
its addresses, where it comes from and "if any", each opening to what the agent is told it is and to what it holds as
far as the page can know outside a round (the flow's own texts in full, a setting as each project's configuration has
it, the project guide as each project's main has it now, at the commit named, short and with when it was committed, or
in the place of both, why its main could not be read, what else the engine computes as the flow says it, a table or view as its definition describes
it); and the rules and formats, a row each with its size, opening to their text.

Out: the pattern of its return as JSON, in the sheet of what is sent to an agent, since the agent is sent it as its
format: the forms it may hand in, its own answer first, one of them in the field that holds one, each its fields with
their kinds or fixed values, beside the fields every return holds, a key saying on hovering what the agent is told it is
and what reads it; then the checks on the whole return, each with what it checks. From: every row that leads here, a
line each with the cell it is of, its name and its condition, a move many cells make once with all of them, and the
engine's fallback, which brings the cell back to itself, one line. To: every row of its own, a line each with its name,
its condition, the cell it leads to and what the row does (its actions, what it records and counts, whether it waits or
pauses the work), its own answers first, the main path first and marked, then its moves, then the engine's fallback,
folded under one line; under it, a move most cells make that it lacks. A condition says what of the return it is worked
out from, as the return holds it ("`outcome.take.way` is `do_now`"), or the engine's value it reads, whole. An engine's,
a tool's or the owner's cell answers the same with what its step reads, what it comes to (an owner's cell that rules by
buttons each button over the line under it, as its ask says them) and what the engine does every time.

Instead of an overview of what the cell is given, hands back and is entered from over three sections, What it is given,
What it hands back (its own answers each with where it leads, the fields as a tree of titles each opening to what the
agent is told, the forms every agent cell may hand in, the fields every return holds, the checks and the fallback) and
Where it comes from, each row's condition only where it said more than its name, and of a round's Ran, Why, Led to and
Cost over the sections What it was given and What it handed back, because the owner found "what it was given" and "what
it handed back" strange to read, and the way on mixed into what came out ("which took done", with `done` said nowhere,
neither in the return nor in its JSON), and asked for in, out, from and to on a cell and on a round alike: the
transition table kept apart from what comes out, which otherwise cannot be read, and one table for from and to with what
actually came and went highlighted.

Rejected:

- Three tabs, Inputs, Returns and From and to: rejected because they are long stacks of notes.

### A round's panel

A round's panel is the four sections of a cell's, about the round. Under its title (the round at its cell, as the page
writes one everywhere, `R7-4 (1 ...)` with the cell's title, the cell a link that shows it on the chart), a short head
of two label and value rows: Ran, who did its work (the agent's line, its model, level and effort, and whether it went
on in its session or started one and why; or the tools, the engine's step or the owner), how long it took and when it
ended; Cost, what it cost and the shares read again from the cache, newly cached and output, with its steps and its
context at its start and its end; and a return sent back, a run that failed or a round cut off said in a Flash with the
engine's reason, a return sent back for several problems with each on a line of its own, since the Flash would run
them together; a return taken with problems set aside (what only checks the work says it changes found, which the
engine did not send back, until the change to its checks is installed, and the reviews judge) said in the same warning
Flash as a return sent back, each problem on a line of its own after its number in the work (S1, S2, ...), since it is
what the checks would have sent the return back with and the owner learns that look once, and a reader who follows the
number finds the review's answer to it; the reason's text carries the `reason` class (`white-space: pre-line`), which the reason has wherever it
is shown whole, also in the notes under a line of the item page's checklist, while a list of rounds that keeps a round
to one line (above) lets the lines run on: the work page's table cuts them and says them whole on hovering, and the cell
panel's list wraps them, so it shows them whole. In: the inputs new this round, a row each with its name, its address,
its size and the round that made it, leading to its place in the message; the inputs already in its session, folded
under their count, each with the round that gave it and its size, opening to what the session held; and the message as
sent, exactly, in one sheet, every heading and line as it is, a part over 1,000 characters and the task folded to their
first lines with their size and their share of the message, the task saying it is the cell's, the same every round.

Out: the return as handed back, as JSON, in the box of what an agent handed back, every key, value and line of it,
nothing renamed, merged or left out, a key saying on hovering what the flow says of its place; what the agent was told
each field is, is not repeated here; the cell's panel gives it. From and To: the cell's two tables, as the flow version
the round's work ran on has them, whichever version the chart shows: the row the round came in by (the row the round
before it in the work left by) and the row it left by marked, with the round on the other side beside each, every other
row greyed; a return sent back marks the fallback's row for it, a round cut off the fallback's row for that; where no
row of a table is the one, a line over it says that the round started the work or where it came from, or that it runs.
In and Out are folded at first, From and To open, each kept as the owner leaves it from round to round. A round's panel
keeps one rule: what the agent got or gave is shown exactly, in its voice's look, and what the page adds is outside it.

Instead of an overview of Ran, Why, Led to and Cost over the sections What it was given and What it handed back, the
return laid out field by field under each field's title, its JSON folded at the bottom, because the owner found the two
titles strange to read, the field-by-field explanation of no use and harder to read than the JSON, whose layout it lost,
and the row the round took ("which took done") said nowhere, neither in the return nor in its JSON.

Rejected:

- The conditional inputs in one list, those sent in bold and those held greyed: rejected because the owner cannot tell
  from it what the agent got this round from what its session already held.

### A round of the tools or of an engine's step

A round of the tools has the same four sections. In: each tool's question, one tool under another, its number, the tool,
and how long it ran when a second or more, and what it was asked, as the agent that asked handed it back, in the agent's
box; above them, when the tools run again after the architect fixed something, the architect's reply, in the box of what
an agent handed back. Out: what they returned, as JSON, in the sheet of what is sent to an agent, since the round after
gives it to the agent that asked, word for word, and the files they kept. What the step read for itself, the work's
state and its local commits, is the engine's own and is not shown. A round of an engine's step: In, what it read, in the
page's own words, as labelled values, quoting in a box only what an agent handed back; Out, what it came to, as JSON on
the page, then the tools it ran, each question with its answer, and what it asked the owner.

Instead of each tool's question and answer side by side in one section headed by the round that asked and the round
given the results, and what a step came to in labelled values with its JSON folded, because the owner asked for In and
Out on every round, the Out as JSON, and the rounds on either side are in From and To.

Rejected:

- What the tools were given in one box and what they returned in another, and the work's machine, worktree and pushed
  head shown as what the step read: rejected because they tell the owner nothing (the page shows the owner only what the
  owner uses, above).

### An input the session held

A conditional input of a round that its session already held opens, from its greyed row, to what it held, read by the
hash the round's record names it with (`/api/v1/works/5/contents/<hash>`): the text the round of the work that gave it
gave, or the value the session's own return produced at the cell and path its address names; only a round recorded
before its inputs had hashes is read by the address, instead of by the address alone, because an address in a round
recorded before rounds were named by their work names a round by a number no round has now, and the owner reads old
rounds as well as new ones.

### What comes out of a cell

What comes out of a cell is shown as JSON (ours, in Primer's tokens and GitHub's syntax colours): pretty-printed in
`--fontStack-monospace` at `--text-codeBlock-size`, each object and list one `--base-size-16` step in; keys in
`--prettylights-syntax-entity-tag`, texts in `--prettylights-syntax-string`, numbers, booleans and null in
`--prettylights-syntax-constant`, the punctuation in the colour of the text round it; a text's line breaks shown as
breaks and every line wrapped, a text of more than 1,000 characters folded as a text in a box is; each object and list
folding at its opening line by a `ChevronDownIcon` or `ChevronRightIcon` in `--fgColor-muted` in the gutter at its left,
folded on one line with how many it holds as a name inside a box is written; a key the flow says something of underlined
with `--borderColor-default` dots and saying on hovering its title and its description, nothing more.

On a round it is the return as handed back, in the box of what an agent handed back (the tools' results in the sheet of
what is sent to the agent that asked, an engine's step's on the page); on a cell it is the pattern of the return in the
same look, in the sheet of what is sent to an agent, since the agent is sent it as its format (an engine's step's on the
page): what each round fills in as its placeholder (a kind, such as "text"), a field of fixed values its values joined
by " | ", each saying on hovering what it means, the forms an agent's cell may hand in one under another after "one of"
in the field that holds one, as the shapes a field may take are, a field that may be left out followed by "optional", a
field that a return handed in on an earlier flow version may lack or hold otherwise followed by what it may ("earlier
returns may lack it", "earlier returns may hold text"), since a work carrying such a return moves to the version all the
same and the owner rules on that with the flow, a key saying on hovering what the agent is told it is and what reads it;
open at first to the fields of each form and one level inside them, and the checks the engine makes on a return listed
under it.

Instead of each field of the return under its title, its value in a box of its own, with the JSON folded at the bottom,
and a cell's fields as a tree of titles opening to what the agent is told, because the owner found the field-by-field
explanation of no use and harder to read than the JSON, whose layout it lost, and asked for the JSON made readable,
nothing more.

### A cell's rows

A cell's rows are two tables as a table is drawn above (ours): From, every row that leads into the cell, by the cell it
is of (by its code and title, "itself", or a move many cells make once, with all their codes), the row's name and its
condition, the engine's fallback, which brings the cell back to itself, one line; To, every row of the cell, by its
name, with the main path's `Label`, its condition, and the cell it leads to with what the row does under it (its
actions, what it records and counts, whether it waits or pauses the work) in body small `--fgColor-muted`, its own
answers first, the main path first among them, then its moves, then the engine's fallback, the same for every agent
cell, folded under one line saying how many rows it holds; a row's name breaks only after an underscore, and its note
says itself on hovering the name. A condition is written whole, as what of the return it is worked out from, named as
the return holds it ("`outcome.take.way` is `do_now`": the field of an agent's own answer or form inside the field that
holds one), or as the engine's value it reads.

On a round the two tables are the cell's same ones with a fourth column, Round: the row the round came in by and the row
it left by on `--bgColor-accent-muted` as the row picked, the round on the other side in that row (a round at a cell, as
written above, where the row names no one cell), every other row in `--fgColor-muted`, its links too; where none of a
table's rows is the one, a line over it says where the round came from or what it led to.

Instead of where a cell leads in its overview, its rows as label and value pairs in the section of what it hands back,
where it comes from by kind in a section of its own, and on a round a Why and a Led to ("done", the right arrow, "B1"),
because the owner asked for the transition table apart from what comes out, which otherwise cannot be read, the same
table on a cell and on a round, with what actually came and went highlighted and the rest greyed, and a row's condition
saying which branch of the return makes it.

### Rows and arrows

Beside a chart, a line of From or To, on a cell's panel and a round's alike, is tied to the chart's arrows (ours, one
part for both panels): while the pointer is on it, on `--bgColor-neutral-muted` as a row under the pointer, its rows
light on the chart as pointing at their arrows does, every arrow that draws each row, a line of From only on its way
into the cell; a click picks it, on `--bgColor-accent-muted` as the row picked, under the pointer too, and keeps its
rows lit until it is clicked again or the panel shows another cell or round, several lines picked at once, each on its
own, and a line pointed at lit with them; a link in a line does what it does, and a drag over its text selects the text
and picks nothing.

Instead of tables that tell the chart nothing, because the owner reads a row in the tables and follows its arrows across
the chart, and asked that a line hovered make its arrows bold, and that lines be picked and let go by a click, to keep
their arrows in sight while reading the chart.

## The system view

### The view

A view of its own, System, shows the state of the whole system in five parts, each saying on its title what the owner
does with it: Now, whether the owner needs to step in (the asks to answer, the decision cards to rule on and the faults
of wf-agents unread, each with how long the oldest has waited; the works by where they stand, each work in one place:
running, ready (a work that waits to run a failed round again stands here, since it waits for nothing of the owner's or
of other works'), waiting for the owner, for the architect or for other items, paused, or halted by the engine; a work
held only by Pause all stands where it would without the pause, because a work's place answers where it is stuck and who
is to step in, and the bar already says that every work is paused, while the works' lists and marks say it of each work;
each
project's architect, working, paused or idle, never by its works, with the matters in its queue and how long the oldest
has waited, leading to its one-to-one; and the rounds running now, each with how large its context is and what it added
and spent, the last two against the line the run guard judges them by, the numbers kept current by the event stream, not
read again, instead of tiles of totals, because whether a round needs a look shows only against its own line); Output, what got done (the items merged, made and cancelled, the pull requests
open now, and how long a merged work took from its start, on average); Spend, where the money goes (the engineers' works
and the architects' works apart, everything spent per item merged with what the merged works themselves cost beside it
in the same tile, by line and model, by item, and the ten costliest cells); Waste and efficiency, where to save (the
share of the agents' input read from the prompt cache and the rounds that missed it, the rounds sent back and those
failed or cut off with what they cost, how long a usage limit, being offline or being unable to run halted every work,
and what the keep-warm pings cost); and Anomalies, what went wrong (rows counting a flow anomaly, counters past their
project's limit, works the engine stopped, tools that failed after the engine's retries, checks red on main after a
merge, and each signal the run guard saw past its line, with its value, its line, how many rounds gave the line and what
came of it, leading to its round). Every part but Now counts over a period the owner picks, which the address keeps: the week under way, the last
four weeks, or all time, a week running from Monday at midnight in the owner's time zone. Each part sums up first, in
tiles, and each number leads to the list it counts, folded under the tiles or on the view that holds it, a tile of
nothing muted; over more than one week, what got done shows week by week, and the spend as columns, the engineers' and
the architects' stacked, to one scale. The view takes the whole width of the window, its tiles and tables spread across
it. Everything is computed from what the database keeps, on one read connection. The view leaves out the engine's state
and the slots in use, which the bar beside it shows, and how long slots stood empty while items could have started,
which nothing kept tells, since neither the engine's pauses nor the slots held over time are kept. A work at the cell
that asks the architect says it waits for the architect's reply, in the list of works as here. The Spend view's address
leads to it.

Instead of the Spend view, which showed what the agents spent by line and model and by day and nothing else of the
system, and instead of a tile of what a merged work cost under Output beside the cost per item merged under Spend, two
costs of a merge in two parts that read as one figure disagreeing with itself, because the owner asked to see the state
of the whole system, every side of it, to know where to step in, what the work came to, and where the money and the time
went.

## The settings

### The view

A Settings view holds a section for the installation and one for each project. The owner changes there the owner's
language, each tier's model (one the tier's CLI takes), the highest tier an item may be at, the architect's tier, the
size past which a line's session is replaced, each machine's slots, and of each project the limits of its counters, the
tags its observations may carry and whether its agents hand in flow feedback; it says that the advisor runs one tier
above the engineer and what keep-warm does, which is not a setting. It shows what is changed only in the files: the gh
commands agents may run, each CLI's arguments and the limits of a query, and of each project its repository, checkout,
main branch, design and test paths, design check, CI and the checks a merge needs, the path of its constitution (whose
text, the project's own, the panel beside the one-to-one with its architect shows) and, folded, the text of each rule
its agents are given. Saving writes the configuration's files in the data directory, the installation's and the
projects' own, with every other line and comment as it was, and commits it there saying what the owner changed, a value
of the wrong kind or out of its range, and one the configuration would refuse, being refused before anything is written,
saying why.

The login accounts the agents may run under (the CLI's configuration directories beside the one in use, each signed in
once by hand) are listed with the account each is signed in as, read from it and nothing else of it; one click, after a
warning that every line starts a new session at its next round, switches the agents to another by writing its
`config_dir` and committing it; a new one is signed in by a command the page shows to copy, and shows once signed in;
and the head of a work's group lets the owner put the work's item at another tier, offering none above the highest, the
change the owner's in the item's history, instead of editing the configuration by hand and the database by command,
because the owner sets these by watching what the agents spend, and each cell's effort stays in the flow, changed by a
flow upgrade.

### Its sections

The settings are a section for the installation and one for each project, each holding first what the owner changes on
the page and then, under a sub-heading naming the file ("Changed only in install.toml"), what is changed only in the
configuration's files, as label and value pairs, a path, a command or a name of the configuration written as a code; a
rule text a project's agents are given is a fold one step down, its title the rule's name, holding the text as a sheet
(what is sent to an agent); a project's constitution is named by its path, its text being the project's own; the login
accounts the agents may run under are rows, each the account's email, its directory's name as a code after it and the
one in use marked by its state's chip (`StateChip`, accent, as what runs), the others signed in a small `Button` Use;
one Save, the primary `Button`, writes the rest, instead of a page of the tiers alone, because the owner keeps the whole
configuration from one place and must see at a glance what the page can change and what only the file can.

### Its form

A setting in a form that one Save writes (the settings) is a Primer `FormControl` holding its label, its control and
what is wrong with it as `FormControl.Validation` (ours, after Primer's forms): a `TextInput` for words and numbers, a
`Select` for a choice from a fixed list, a `Checkbox` for on or off; a list the owner edits row by row (a project's
observation tags) is a table of small `TextInput`s, each row with an invisible small `IconButton` with an `XIcon` that
takes it out, and a small `Button` with a `PlusIcon` under it that adds one, instead of a `ToggleSwitch`, which acts at
the click, because nothing in the form takes effect before Save, and a list of pairs reads as a table.

### A command to copy

A command the owner runs by hand outside the page (signing a new login account in) is a code block as the markdown draws
one (`--bgColor-neutral-muted`, `--borderRadius-medium`, `--fontStack-monospace` at `--text-codeBlock-size`) with an
invisible small `IconButton` with a `CopyIcon` beside it that copies it, and what to do with it in small
`--fgColor-muted` text over it, instead of a sign-in flow on the page, because the CLI signs in only in its own
terminal, and a command copied whole is not mistyped.
