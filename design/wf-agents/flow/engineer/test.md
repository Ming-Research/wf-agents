- A reviewer has one chance and lists every problem at once; the re-check after the fix looks only at what was fixed,
  and ends if nothing is wrong. That keeps the rounds of review under control.

- A reviewer's ability and knowledge are no less than the engineer's: it knows the background and the decisions the
  engineer knows. Engineers mostly do what a review says, so a wrong request turns good code bad.

- Code review checks the code against the design, and catches the design changed on one's own while writing: a short
  cut, laziness, another method, something different from the design or more than it. A design only partly done is no
  problem. This is the most important part of such a review, and partly done code is still faithful to the design.

- A migration, or a design done in steps, is not reported by review, including code written on a way about to be
  replaced, provided what departs is kept track of by a comment or a document, so a reviewer can see it is temporary.
  What is removed with no explanation is reported. These do not depart from the design's direction; what is left with no
  comment or document is the problem.

- Review of an implementation looks at problems such as the effect on performance, in general words, with no fixed check
  for one project. wf-agents is not only for itself.

- Each new test says how long it takes and why that is worth it, and the review challenges it. Tests that can each
  justify themselves add little to CI's time, and what still adds up the system watches by itself.

- A fix in a fresh session is given the item, the acceptance, the design and what the implementation handed in: it is
  checked against the acceptance, must not leave the design, and must know how things were done.

- An interjection that asks only for a code change, with the design and the acceptance unchanged, goes straight to the
  fix, which is re-checked as usual; only if the design or the acceptance must really change is the design redone. A
  small fix is tested and reviewed anyway, and needs no rewriting of a whole design and acceptance.
