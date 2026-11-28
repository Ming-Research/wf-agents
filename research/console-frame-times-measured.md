# How long a frame of the console takes, zoomed out

Measured to choose how the console's tree is drawn while it moves, and how a way's strength and glow are drawn. The
numbers back decision the-tree-is-moved-whole-as-it-was-drawn.

## Conditions

- The made-up States project (`?fixtures&states`, key 1), 137 items, the whole tree fitted to the screen: 12.4% of its
  full size.
- A canvas of 2048 by 1464 pixels: a page 1024 by 732 at a pixel ratio of 2.
- Chromium in the Claude desktop app's browser pane, on the owner's Mac, the engine running beside it.
- Each frame timed by the scene's own benches, from the start of the frame until the GPU has drawn it (`waitForGpu`).
  Scroll: `bench`, a scroll of most of a row every sixth frame. Still: `benchStill`, only the lights move. Hover: the
  pointer put on 12 places down the tree's middle column, 20 frames at each. Zoom out: Space from full size, the
  pointer over the tree, until the tree stands still.

## The three ways measured

1. As it was on main: every lit way built again at each frame the tree moved; a way's strength drawn into its strokes,
   and the glow at rest blurred once onto a texture for each place across the page.
2. The tree drawn for the screen and half a screen round it, and moved whole until the screen leaves that room; the
   strength and the glow as in 1.
3. As 2, with a way's strength the alpha of its strokes and all its glow blurred at each frame.

## Milliseconds a frame, mean and 95th percentile

| | 1 | 2 | 3 |
|---|---|---|---|
| Scroll | 54.9, 97.8 | 5.0, 6.1 | 11.5, 13.2 |
| Still | 11.2, 23.0 | 7.2, 11.5 | 8.2, 13.5 |
| Hover | not measured | 39.9, 52.0 | 14.3, 23.9 |
| Zoom out | not measured | 34.7, 40.6 | 17.0, 42.2 |

In 2, the hover rebuilt the ways in 234 of 240 frames and the zoom out in 24 of 24: the pointer's light easing a way's
strength built every way again. In 3 the hover rebuilt none, and the zoom out rebuilt 4 of 23, where the screen left the
room it was drawn for.

## The cost of blurring the glow at each frame

The same still frame of the same tree, drawn 40 times: 3.98 ms with the glow at rest laid from its textures, and 5.14 ms
with all of it blurred at each frame, 94 ways drawn. At full size on the wf-agents tree, 51 cards drawn, the ways' glow
took 2.7 ms of a 9.5 ms frame. Scrolling costs more in 3 than in 2 for this blur, which 2 did not draw while the light
was elsewhere.
