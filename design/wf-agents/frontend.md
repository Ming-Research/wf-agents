- Everything the owner uses is on the web, not the command line, which the owner does not like to use.

- The page's places and the strength of its marks are scarce and graded: the strongest marks, such as a whole
  background, colour and motion, and the first places go to what matters most; weaker marks and later places to lesser
  things. A strong mark spent on something minor leaves the important nothing to stand out with, and where everything
  is loud nothing is.

- Every text of the page itself is in English, from one message catalog. The owner reads the interface in English, and
  one catalog keeps the names of the flow's parts the same wherever the page shows them.

- The console's tree is drawn in WebGL by PixiJS, and what opens from a node is HTML. Each node moves, scales and
  animates on its own, and many must be drawn at 60 frames a second: on the owner's machine about 300 nodes in view,
  all scaling at once, took 3 to 5 ms a frame whatever the tree's size, from 2,000 to 20,000 items, once only the nodes
  in view were attached and titles came from one shared atlas of glyphs; a texture of its own for each title stalled a
  frame up to 150 ms as nodes came into view. Nodes in HTML were not taken, since scaling each one makes the browser lay
  out and rasterise its text again every frame; three.js, an engine for 3D, is more than a flat tree needs. What opens
  from a node holds text to select and copy, Markdown and answers to type, which only HTML does well.

- Every box of the page that takes words keeps what was typed and not sent, across leaving the page, opening something
  else, a reload or the browser dropping a tab, written at every change with no timer, since a browser that drops a tab
  does not promise to run anything on leaving. Losing what the owner typed is a serious fault. Kept only while the box
  was shown, the words were lost when it went away, and keeping every page shown is what the page was made lighter to
  stop doing.

- A draft is kept under what its box belongs to, never under where it stands or a title, so that one box never shows
  another's words. What is not words, such as the option picked or whom a reply answers, is not kept, since kept it
  could turn into something else unseen. When the browser is full, images give way to words, the oldest draft's first,
  since images could otherwise crowd out the words of any box.
