- Everything the owner uses is on the web, not the command line, which the owner does not like to use.

- The page follows GitHub's Primer design system: colours, type sizes, spacing and the like are its tokens, a few
  colours keeping the token's name with another value, and controls and recurring parts are its React components. A look
  the page made up view by view had no rule to hold the next change to, and small grey bold labels came to head a page.
  Primer's CSS was not taken, since the React components carry behaviour such as focus, keyboard and overflow menus.

- Every text of the page itself is in English, from one message catalog. The owner reads the interface in English, and
  one catalog keeps the names of the flow's parts the same wherever the page shows them.

- The flow's chart is laid out by elkjs, each node a component. A node carries overlays, such as the cell a work is at
  and how many rounds it ran, and mermaid gives only a fixed picture, so it draws no flow chart.
