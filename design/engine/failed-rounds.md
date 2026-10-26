Decision: A round whose agent command line fails to hand in a return is not a return sent back and is never counted with returns sent back, because nothing was wrong with what the agent did: it never ran to the end.

Decision: When the account's usage limit is reached or the network cannot be reached, the engine starts no new round for any work, waits until the time the command line gives for the limit to reset or until the network is back, then carries on each work from the cell it recorded, instead of retrying each round, because one cause stops every work alike, and retrying would only use up each work's retries and raise a card per work.

Decision: When the service is overloaded, returns a server error or times out, or the command line crashes, the engine runs the same round again twice, waiting longer each time, and records each attempt on the work; if it still fails, it pauses the work and asks the architect, which while the architect's flow does not run is a card for the owner, because these failures usually pass, as a tool that fails is retried twice before it counts as failed.

Decision: When the login has expired, the command line cannot be found, or the prompt is too long for the model, the engine does not retry: it pauses the work and gives the owner a card at once, because retrying cannot help, and only the owner can log in again or change what is installed.

Rejected:
- Treating a failed run as a return sent back: rejected because it tells the agent its return was wrong when there was none, and after three failures in a row pauses the work for a cause outside it.
- Retrying every failure twice, then asking: rejected because a usage limit or a network outage then uses up every work's retries at once and raises one card per work for a single cause.
