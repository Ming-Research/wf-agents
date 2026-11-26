// How a project's red main stands, in words, for the System view's part of what holds now. The owner needs it to know
// that the project's works wait on a test that fails on main's head, which no change of theirs fixes. The engine works
// the fact out from the checks it ran on main's head (`crates/driver`, module `red_main`), so the view works out none of
// its own, and shows a project whose main is not known to be red no row.

import { clock } from '../works/words';
import type { RedMain } from './api';

/// How many of the tests that fail a row names; the rest are counted.
const TESTS = 5;

/// The tests that fail on main's head, since when, and how many works wait for it to pass.
export function redMainWords(red: RedMain): string {
  const named = red.tests.slice(0, TESTS).map((test) => test.name);
  const more = red.tests.length > TESTS ? ` and ${red.tests.length - TESTS} more` : '';
  const heads = [...new Set(red.tests.map((test) => test.head.slice(0, 12)))].join(', ');
  const waiting =
    red.waiting.length === 0
      ? 'no work waits for it'
      : `${red.waiting.length} ${red.waiting.length === 1 ? 'work waits' : 'works wait'} for it to pass`;
  return `${named.join(', ')}${more} ${red.tests.length === 1 ? 'fails' : 'fail'} on its head ${heads}, since ${clock(red.since)}; ${waiting}`;
}
