// What the Observations view shows of a project's observations: the tags that sort them, those it shows with the tags
// the owner shows and the handled ones when the owner asks for them, and how many each button over them stands for.

import type { Observation } from './api';

/// The button of the observations that carry none of the project's tags: none at all, or only tags the project no
/// longer has. No tag is named so, since a tag is one word.
export const NO_TAG = '';

/// Whether an observation is handled: an item took it in, or it was closed.
export const handled = (o: Observation): boolean => o.folded_into !== null || o.closed !== null;

/// What an observation's row leads with: its title, or, for one recorded before observations had titles, what it is,
/// which the row cuts to the words it starts with.
export const rowLine = (o: Observation): string => o.title ?? o.what;

/// The tags of a project's that an observation carries, each once; `NO_TAG` alone when it carries none of them.
export function tagsOf(o: Observation, tags: readonly string[]): string[] {
  const of = tags.filter((x) => o.tags.includes(x));
  return of.length > 0 ? of : [NO_TAG];
}

/// What a project's list shows, in the order read (the open ones first, each newest first): the observations with a tag
/// shown, any of their tags, those handled only with `withHandled`, and the one open whatever the buttons hide, so that
/// what is open stays in the list.
export function shownObservations(
  all: readonly Observation[],
  tags: readonly string[],
  hidden: readonly string[],
  withHandled: boolean,
  open: number | null,
): Observation[] {
  return all.filter(
    (o) => o.id === open || ((withHandled || !handled(o)) && tagsOf(o, tags).some((x) => !hidden.includes(x))),
  );
}

/// What the buttons over a project's list count: for each tag, the observations that carry it among those the handled
/// button lets the list show, one for each of the tags of the project's an observation carries; and how many are
/// handled, whatever their tags.
export function tagCounts(
  all: readonly Observation[],
  tags: readonly string[],
  withHandled: boolean,
): { tags: string[]; handled: number } {
  return {
    tags: all.filter((o) => withHandled || !handled(o)).flatMap((o) => tagsOf(o, tags)),
    handled: all.filter(handled).length,
  };
}
