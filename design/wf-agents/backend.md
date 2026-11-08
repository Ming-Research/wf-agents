- The engine, the HTTP interface and the serving of the page run in one process. An owner's action, such as answering a
  question, must wake the engine at once rather than wait for it to poll the database; live events come straight from
  the engine; the engine's state in memory needs no table to report it; and there is one thing to start and to upgrade.
  A separate process for the page would have to go the long way round for all of these.

- The built page is part of the program itself, not files installed beside it. The page and the interface it calls then
  always come from one build, and installing or upgrading is still one file.

- The page's types are generated from the interface's own types, and CI holds the generated file to them. The page and
  the interface are one build (backend.md, "The built page is part of the program itself"); written by hand on both
  sides, their types drift apart, and the error shows only when the page runs.

- The page and its interface are served on the loopback address only: the page has no sign-in, yet it starts works and
  spends money.

- What the assistant reads from the system comes in looks as small as they can be, with nothing that matters lost: each
  command's output has a limit held by a test, says what it left out and how to get it, and is never raw JSON. The
  assistant's context must stay on what it discusses with the owner (flow/asking.md, "The assistant is the owner's other
  self"), which a look that brings more than it needs fills (Article 3), and a look that leaves out what matters leaves
  it guessing (Article 2).
