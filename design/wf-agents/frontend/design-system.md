- The page follows GitHub's Primer design system: colours, type sizes, spacing and the like are its tokens, a few
  colours keeping the token's name with another value, and controls and recurring parts are its React components. A look
  the page made up view by view had no rule to hold the next change to, and small grey bold labels came to head a page.
  Primer's CSS was not taken, since the React components carry behaviour such as focus, keyboard and overflow menus.

- The page's styles write no raw value for colour, size or spacing, only those of the design system, and CI checks that.
  A change then cannot get round it.

- Where Primer has no token, the page decides in Primer's spirit and names the value once, beside its reason, every use
  taking it by that name, since a value written in two places drifts.

- The page follows the system's light or dark appearance with Primer's two themes, with no switch of its own: the owner
  sets the appearance once in the system, and one source of the mode keeps the colours, the components and what floats
  over them in step.

- The palette is bright: one colour to a meaning, of middle weight, with little grey, and the states can be told apart.
  Primer's dark saturated blocks were too heavy, a greyer version looked dull, and several dark colours side by side
  could not be told apart.

- A fill, a mark or a line in a role's colour is the colour itself in both themes, so that a role's colour is one colour
  in either; only words in it take a darker step of the same hue in the light theme, where the colour itself is too
  light to read on white.

- A coloured button or label has white letters on a solid ground, with no border of another colour and no mixing with
  outlined ones; a button that is on or off is filled in its colour while on and in its tint while off, so that on and
  off differ by the fill alone, and an uncoloured button held pressed is drawn as the option picked. The owner does not
  like a border differing from its ground, and found the filled and the outlined mixed; outlined while off and filled
  while on was the path that failed.

- A state is a coloured label, the same colour everywhere; tags and categories are outlined and unfilled. Links, text
  and boxed states mixed in a line, in sizes big and small, grey and black, read strangely.

- A state's colour is that of its meaning: going on blue; done, confirmed and merged green; waiting on someone or
  something, paused and sent back yellow; failed, cut off, halted, cancelled and disagreed red; new, ready, ended and
  every state with no meaning of its own grey, filled as every state's chip is and not outlined as a tag. The owner
  learns a colour once and reads it on every view.

- A coloured and an uncoloured chip have one shape, height and corner, a rounded rectangle and not a pill, their words
  the size of the line they stand in. How a chip looks is the owner's taste, by which what is good is judged
  (wf-agents.md, "What is good is judged by the owner's taste and use").

- The page's own parts take Primer's corners, line widths and shadows, and the page casts no shadow except under what
  floats over it: Primer's components already draw with these, and the page's parts beside them must not look like
  another kit.

- One line of text keeps to one size as far as it can. Big and small in a line looks bad.

- A title stands out, not smaller and paler than the text, and small grey text is not bold. A title is meant to show,
  and small grey and bold contradict each other.

- The page's type has six steps and no others, from Primer's text tokens: the page's title, a section's heading, a
  sub-heading, body text, small text and code; a chart's text, read at a distance, takes sizes of its own. A few steps
  far enough apart read as an order at a glance, and Primer's are those GitHub's readers already know.

- Weight marks structure only: semibold for headings, a card's head, who says something at the head of what they wrote,
  what an agent set strong in its text and the one current choice; everything else is normal, but for the chart's own
  marks (frontend/charts.md, "The numbered cells the flow really reaches"). Weight spent on decoration stops marking
  what is a heading.

- What agents and the owner wrote reads in the page's font, and only a message as sent, a file and what a tool printed
  keep a fixed width: that text is prose, and a fixed width makes prose slower to read.

- What the page writes for the owner, the text sent to an agent, and what an agent hands back each have one fixed look;
  sent text is shown as it is, in order, with no heading or note of the page's inside. The owner once took the first
  line sent to an agent for a page title, and could not tell what was sent from what was for the owner. What is sent to
  an agent is a filled sheet with a thick edge at its left and what came back an outlined box, so that they differ in
  shape before a word is read, in either theme and for an eye that does not tell colours apart; a colour for each voice
  was rejected, since the accent and success colours already mean a link, the place chosen and done. A blue line beside
  the text meant little.

- What is wholly one document has no frame and is laid out like body text under a title, one document one look; a frame
  is used only where an agent's words mix with the page's text. A frame around what an agent wrote entirely divides
  nothing and only adds a line and space, and one document both framed and not looks odd.

- A heading an agent wrote is drawn as a sub-heading whatever its level, since an agent's headings must not outrank the
  page's own.

- The markdown agents write is drawn by a small renderer of the page's own, covering only the forms they use, and a
  paragraph that is plainly code stays code. Raw markdown is hard to read, the page needs only these few forms, and
  nothing an agent writes may become markup the page did not mean; a markdown library was rejected for that.

- Every list in a document the owner reads is drawn alike, numbered, each item's lead set strong and each other part a
  paragraph opening with its name, with no box inside it. The owner read two lists of one shape drawn two ways as
  something set apart, and a box inside what an agent handed back says nothing more.

- A text of more than about a thousand characters in a sheet or a box shows its first lines and a button saying how much
  it opens, a list of more than three rows shows three and the rest a click away, and a shorter text is never folded:
  the owner reads a text's size before opening it.

- A page's sections stand one under another with no box, each a line that folds holding its heading, its count and who
  wrote it, parted from what came before by a line across the page with room on each side, and with no rule under its
  heading; one part draws it on every page and panel. The owner reads a page as one document, and a page of boxes read
  as a heap of separate things; a rule tight under a title did not say where one section ends and the next begins.

- Where text is shown diagrams can be drawn, mermaid and others, not cut off at the right. Diagrams are key to the
  owner's understanding of a design.

- A diagram wider than its box is scaled down to the box, never scrolled sideways, and opens enlarged in a dialog, in
  the viewer the charts are moved and zoomed in, where its words can be read. The owner read a diagram cut at the edge
  as cut off, a scroll bar shown only once used does not say there is more, and a diagram is taken in whole before it is
  read closely. Rejected: only scaling down, which leaves a wide diagram's words too small to read; a new tab, which
  takes the owner away from the words round it; opening in place, where it stays as narrow as the text round it; drawing
  it again narrower or turned, which changes what its author drew and holds for some kinds only.

- The libraries that draw diagrams load only when a diagram first comes near the screen, so that a page that shows no
  diagram does not pay for them.

- A diagram is drawn from what an agent wrote, so it is drawn at the library's strict security level and its drawing
  sanitised, with no script and no HTML of its own: the page's look and security stay closed to the diagram, as to the
  markdown (frontend/design-system.md, "The markdown agents write is drawn by a small renderer").

- A diagram that cannot be drawn shows its source with a note under it, never a blank space, and a drawn one can show
  its source in its place: diagrams are key to the owner's understanding of a design (frontend/design-system.md, "Where
  text is shown diagrams can be drawn"), and shown as its source, nothing of one is lost.

- Numbers are written #n for an item alone, Wn for a work and On for an observation, by the page, agents and engine
  alike; text already in the database stays; a group's title gives the item's number and name and, apart, the work's
  number. One #9 could be an item or an observation, told only from the words before it.

- An item's number in a text an agent or the owner wrote links to the item, with its title on hovering, since items are
  named by their number and the owner reaches an item from wherever a text names it; a number in code, an address or a
  path, joined to a word, after a word that names another numbered thing, or naming no item, stays as written.

- Facts on one line are joined by a vertical bar, which reads as a separator, not by middle dots: on a line of muted
  words and dots the owner could not tell which parts could be clicked.

- A link is in the link colour wherever it stands, never muted, also in small text and among facts; a round's name and
  an item's title are such links, the item's number standing muted before its title. A link is recognised by its colour,
  and a muted link among facts could not be told from them.

- What is drawn as a link that opens something on the page can be selected and copied as text: the owner copies what the
  page shows to say what is wrong with it, and every round and cell in a copied panel was left out.

- A code the flow names a thing by, in a sentence of the page's, is monospace in the colour of the text round it with no
  background, while code an agent wrote keeps the code background: a cell's code beside its title is a name, not code to
  copy.

- A byline is one line of small text: who, semibold, then the round at its cell as a link, then the time, muted. Who
  wrote something is read before when and where, and the rest is checked only now and then.

- A pause someone meant, by the owner, the architect or Pause all, is called paused, with two yellow bars, and says who
  stopped it and why. It is one state with the bars in the list, and a state does not need two words.

- The engine halting a work for a problem is a state of its own: red, with its own icon, looking the same in lists,
  filter buttons (shown by default), heads and stage bubbles. In a list one tells at once what someone meant, in yellow,
  from what went wrong and needs looking at, in red.

- An ended work has a grey stop mark. The owner reads a red mark as broken.

- A work both paused and waiting for the owner says in its head what really holds it. With one saying paused and
  another waiting for the owner, one cannot tell what stopped it.

- A work's state does not say the answer came: that asks nothing of the owner, and the state before it already says
  whether the next step runs.

- A work's state is worked out once, from what the work is doing now, and its icon, its words, the running mark in its
  head and the glow on its chart all take it, never disagreeing; running's is the one state mark that moves, and shows
  only what is at work. An icon worked out apart from the words drew a ring at rest for a work its words said was paused
  by Pause all, and a running mark came on for every engine step that only looked at a waiting work.

- Ready is a work whose next step is due, that waits for nothing, and that the engine does not run, as while it is
  stopped or out of reach, so that a still ring is a true sign the work does not move: a work waiting to run a failed
  round again, or for a free slot, is waiting, not ready, and while the page cannot reach the engine no work is marked
  as working.

- A work whose wait is over, or whose pause was asked to be lifted, while its machine has no slot free, reads as waiting
  for a free slot: the owner must see why a work whose answer came does not move, and a resume that does not show looks
  as if it failed. It has Pause and no Start or Step, since it is on its way to going on and the owner's later word must
  be able to keep it paused; Pause withdraws the request.

- A work whose cancel or start over was asked, and that the engine has not yet stopped, reads as stopping, in place of
  whatever state it was in, a state of its own the filters can show or hide; it shows no round timer and has no Start,
  Pause, Step or Start over, none of which would change it. A work that looks as if it goes on after the owner cancelled
  it reads as a cancel that failed.

- While the engine itself stops, a work reads as running only while a step or a tool of it is under way, since running
  says a round goes on; the rounds list reads a round left open the same way.

- State filter buttons have icons only, with no words such as asking, waiting or paused, which are too long to fit a
  line.

- A list keeps in the browser the states the owner hid, not those shown, so that a state the page comes to have shows
  until the owner hides it.

- Each list's state buttons are one part (Article 8), the list's choice one for all its projects and kept in the
  browser; in a list the owner narrows by picks, as the observations, a picked button stays drawn at 0 so that the pick
  can be taken back.

- A mark is an icon, never a character such as a tick, a triangle or a circle: a character takes its size and weight
  from the font round it, so one mark would differ from text to text.

- What the page or the engine tells the owner about the view and says nowhere else, such as a request that failed or why
  a return was sent back, is a band in the colour of its meaning; a hint beside a control is small muted text, and
  loading is the word Loading. A notice about the page must not look like something an agent wrote.

- A count standing alone is a counter, emphasised only for what waits for the owner, and a count in a sentence stays
  words: a count in a pill is read apart from the words round it, and emphasis is kept for what the owner must act on.

- A number shown on its own is a tile with no border, since a border would make it read as the box of an agent's words
  and an edge at its left as the sheet of what is sent.

- The series of every chart of numbers take one order of hues (Article 8).

- Each agent has an avatar, its line's icon in a circle of its role's colour, telling the members apart at a glance, and
  the owner has none: nobody on the page has a picture, and made-up pictures or initials would read as people the page
  does not have.

- A form or a view has one primary command, the one it exists for, and every other is plain: one primary button tells
  the owner which action a form exists for.

- A command whose effect reaches past what it names, such as Start all, starting a work over, switching the agents'
  login account or removing a dependency, asks first, in a dialog whose title is the question, whose body says what
  follows and whose button is the command's verb: the owner must know what follows before it happens. Start all says
  what goes on and how many works on an older flow version are brought up.

- A choice of what a view shows from a list that grows, such as the flow's stored versions, is a menu whose button names
  what is shown, each entry's facts shown only in the menu and the one in use marked: the button reads as a name, and
  the facts belong to the moment of choosing. A native select carrying the facts as characters was rejected.

- The period a view counts over is one switch, the same on every view, its periods side by side as links, the one
  counted marked and kept in the address: a period is read with the others beside it, and like things look alike
  (Article 8).

- In a form that one Save writes, nothing takes effect before Save, so a setting on or off there is a checkbox, not a
  switch that acts at the click, and a list the owner edits row by row is a table, as a list of pairs reads.

- A box for the owner's words beside its send button is one line at rest, as tall as the button and level with it,
  growing upwards with what is written to a limit and then scrolling, with no handle to drag. The owner found a box
  taller than its button, the button hanging at its corner, ugly; a box two rows tall dragged to a size, with Send at
  its corner, was the path that failed.

- The owner's words take images, picked or pasted, sent with the words as one message and shown with them and in the
  rounds given them: a problem the owner sees can be shown to the agent as it is.

- Images added to the owner's words wait over the words as thumbnails, each removable, with how many against the limit,
  and what the box refuses is said file by file: a thumbnail with its cross is how the owner's chat apps show what waits
  to be sent. A file area of the page's own beside the box, and images sent apart from the words, were rejected.

- An image the owner sent shows as a thumbnail and opens in a new tab, since a tab is the simplest way to see an image
  whole and at its own size, and the browser already zooms and saves it; a dialog holding it was rejected. A diagram
  differs: the page draws it, in its theme, and opens it in a dialog.
