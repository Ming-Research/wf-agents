- The database has one definition, one SQL file: the program builds from it, and changing a table means changing it,
  with the migration in the same pull request; each table, field and view has a comment saying what it is, what it
  solves and who reads it. Views are SQL anyway, as is what the database takes and agents read, and without this source
  every change would start over. Another format generating SQL would be one more layer of translation, with views as
  strings.

- The database is made by one command alone, and every other command, the engine included, fails when there is none, so
  that a wrong data directory is an error and not an empty view.

- Every change to the database's state is kept as a record of that change. A work's opening step brings its history,
  the paths dropped and the decisions overturned, from the records (flow/history.md, "Every work opens with a step that
  brings its history"), and the architect's notebook can be turned back to earlier versions
  (frontend/conversations/architect.md, "The architect's notebook can be turned back"); neither can without each change
  recorded.

- What is stored is controlled, with no large repeats, or the size soon explodes.

- Text written for people is only in the flow's version, each round recording only values, and the page looks it up in
  the work's version. That does not store the same words again, and agents do not get them.

- A round's identity is its work's number and its count in that work, with no global number, and tables pointing at
  rounds use that pair. A global number interleaves across parallel works.

- A work's counts are recorded at the end of each round and then read directly, by the flow of that time, unchanged by
  upgrades of the flow. A view recomputing them each time is too slow.
