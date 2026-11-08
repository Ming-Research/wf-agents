- The private directory is managed by git as a whole, with a private remote, and the database is not in git; the
  configuration commit recorded with each round is a commit of that repository. SQLite is binary and changes all the
  time, so git means nothing for it.

- No share of usage is reserved for the owner, and when the quota runs short the account is changed. The owner has
  several accounts, and changing one is enough.

- Tags are a fixed set to choose from, so that they do not grow into a mess.


- A field the engine does not know in a project's file is an error, so the engine that knows a field is installed before
  the field is added: a misspelt field silently ignored leaves the owner believing a setting is in force that is not.
