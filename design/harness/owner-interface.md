Decision: The owner works through a web page, instead of the command line, because the owner must see each work's state as it changes and answer a card with its options and recommendation in view, while the command line shows a snapshot and needs exact command syntax for every answer.

Decision: The page is written in TypeScript over an interface that wf-agents exposes, instead of pages rendered inside the engine, because the interface is the one way in for every client, and the page can change without touching the engine.

Rejected:
- Command line only: rejected because watching works means running commands again and again, and answering a card means retyping its options as flags.
