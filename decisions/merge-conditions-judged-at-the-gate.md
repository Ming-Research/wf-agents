# A merge condition is judged at the merge gate, one at a time

## What is done now

The owner judges a merge condition at the merge gate (cell `M1`) and nowhere else. The gate's value `gates` has four levels,
in this order: items, set_aside, conditions, owner. While a condition of the item stands open, the row `owner_judges` raises
a blocker the owner holds and asks of the oldest one, with the options met and withdrawn, the work waiting and holding no
slot; once it is judged the gate asks of the next. The ask gives what judging takes: the sentence, the words it rests on and
who said them, a link to the record they are in, who recorded it and when, the engineer's latest report or that there is
none yet, and the pull request. Words with no option judge nothing: the row `condition_answered` gives them to the engineer,
the work chooses the way on at cell `A4`, and the gate asks again with what the engineer reports next.

Where a condition stands is not stored: the view `item_merge_conditions` works it out from the answers. No agent has an
action that changes it. The implementation and the fix are given each condition open and report on it (`owner_conditions`):
done, saying what was done and where the owner sees it, or not_done, saying why, which hands it back to the owner. A
condition is not tied to a head: judged met, it stays met through a later fix.

Conditions come before the owner's approval of a change, so that the owner does not approve a change a condition then sends
back to be fixed and approved again; and after the findings set aside, as `decisions/engine-checks-a-work-changes.md`
requires.

## Why

The owner judges where the report and the pull request are in front of them, at the moment the work is to merge, by the same
ask, blocker and relay to the engineer as the owner's approval at the same gate: one more row on the chart (Article 7), and
waiting only where an item has a condition (Article 3). A condition a work cannot meet goes the same way: its report says why,
and the owner decides.

## Options not chosen

- A card for each condition in the work's group, judged at any time with buttons and words, pinned once the work reaches the
  gate. The owner could withdraw one sooner, and would see it was recorded; but it is a second kind of card to rule on with an
  interface of its own, the report has to be added to it, and a condition could be judged before the engineer reported.

## Challenges considered and turned down

None yet.

## How it was decided

The owner's assistant kept this option at the review of the design of item #261, on the owner's behalf: "P1、P2 维持上次的选择。"
(the earlier words: "P2 选「M1 的 ask，一次一条」"). A different choice by the owner replaces this file.
