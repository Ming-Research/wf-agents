- The engine, the HTTP interface and the serving of the page run in one process. An owner's action, such as answering a
  question, must wake the engine at once rather than wait for it to poll the database; live events come straight from
  the engine; the engine's state in memory needs no table to report it; and there is one thing to start and to upgrade.
  A separate process for the page would have to go the long way round for all of these.

- The built page is part of the program itself, not files installed beside it. The page and the interface it calls then
  always come from one build, and installing or upgrading is still one file.
