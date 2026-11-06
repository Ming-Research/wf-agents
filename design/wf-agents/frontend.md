- Everything the owner uses is on the web, not the command line, which the owner does not like to use.

- The page's places and the strength of its marks are scarce and graded: the strongest marks, such as a whole
  background, colour and motion, and the first places go to what matters most; weaker marks and later places to lesser
  things. A strong mark spent on something minor leaves the important nothing to stand out with, and where everything
  is loud nothing is.

- The page follows GitHub's Primer design system: colours, type sizes, spacing and the like are its tokens, a few
  colours keeping the token's name with another value, and controls and recurring parts are its React components. A look
  the page made up view by view had no rule to hold the next change to, and small grey bold labels came to head a page.
  Primer's CSS was not taken, since the React components carry behaviour such as focus, keyboard and overflow menus.

- Every text of the page itself is in English, from one message catalog. The owner reads the interface in English, and
  one catalog keeps the names of the flow's parts the same wherever the page shows them.

- The flow's chart is laid out by elkjs, each node a component. A node carries overlays, such as the cell a work is at
  and how many rounds it ran, and mermaid gives only a fixed picture, so it draws no flow chart.

- The console's tree is drawn in WebGL by PixiJS, and what opens from a node is HTML. Each node moves, scales and
  animates on its own, and many must be drawn at 60 frames a second: on the owner's machine about 300 nodes in view,
  all scaling at once, took 3 to 5 ms a frame whatever the tree's size, from 2,000 to 20,000 items, once only the nodes
  in view were attached and titles came from one shared atlas of glyphs; a texture of its own for each title stalled a
  frame up to 150 ms as nodes came into view. Nodes in HTML were not taken, since scaling each one makes the browser lay
  out and rasterise its text again every frame; three.js, an engine for 3D, is more than a flat tree needs. What opens
  from a node holds text to select and copy, Markdown and answers to type, which only HTML does well.
