Decision loopback-only: The interface listens on the loopback address only and checks the Host and Origin headers of every request, reads and writes alike, because a page on another site could otherwise reach it through the owner's browser, by rebinding a domain name to the loopback address or by sending a cross-site request.

Rests on: C:design-on-merit, F:browser-reaches-loopback

Decision no-login: The interface has no login, because while it listens on the loopback address only the owner's own machine reaches it.

Rests on: C:problem-first, D:loopback-only
