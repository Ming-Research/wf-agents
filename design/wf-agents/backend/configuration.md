- The private directory is managed by git as a whole, with a private remote, and the database is not in git; the
  configuration commit recorded with each round is a commit of that repository. SQLite is binary and changes all the
  time, so git means nothing for it.

- No share of usage is reserved for the owner, and when the quota runs short the account is changed. The owner has
  several accounts, and changing one is enough.

- Tags are a fixed set to choose from, so that they do not grow into a mess.

- Whether a work must pass the adversarial test is set by a bar in the project's settings, written once by the owner and
  the assistant, polished and then left, changed only on trouble. A project's bar hardly keeps changing. Written by the
  architect it takes time each time, a later change undoes the polish, and it does not come out clear. A review
  answering whether it could quietly fail, or naming a particular bug, was too broad or a matter of luck; and the policy
  is about what is near, while this is a gate for the whole project.
