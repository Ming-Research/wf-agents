// What the Observations view shows of a project's observations: the tags that sort them, the buttons over them that
// pick by tag and by state, those the picks let show, and how many each button would show under the other picks.

import type { Observation } from './api';

/// The button of the observations that carry none of the project's tags: none at all, or only tags the project no
/// longer has. No tag is named so, since a tag is one word.
export const NO_TAG = '';

/// Whether an observation is handled: an item took it in, or it was closed.
export const handled = (o: Observation): boolean => o.folded_into !== null || o.closed !== null;

/// Where an observation stands, as its button over the list names it: open, or handled.
export type ObservationState = 'open' | 'handled';

/// The states the buttons pick from, in their order.
export const OBSERVATION_STATES: readonly ObservationState[] = ['open', 'handled'];

export const stateOf = (o: Observation): ObservationState => (handled(o) ? 'handled' : 'open');

/// What an observation's row leads with: its title, or, for one recorded before observations had titles, what it is,
/// which the row cuts to the words it starts with.
export const rowLine = (o: Observation): string => o.title ?? o.what;

/// The tags of a project's that an observation carries, each once; `NO_TAG` alone when it carries none of them.
export function tagsOf(o: Observation, tags: readonly string[]): string[] {
  const of = tags.filter((x) => o.tags.includes(x));
  return of.length > 0 ? of : [NO_TAG];
}

/// What the owner picked over the list, one choice for every project: tags, and states. Nothing picked in a group holds
/// every observation.
export type Picks = { tags: readonly string[]; states: readonly ObservationState[] };

/// Whether an observation carries one of the tags picked, or nothing is picked by tag.
const byTags = (o: Observation, tags: readonly string[], picked: readonly string[]): boolean =>
  picked.length === 0 || tagsOf(o, tags).some((x) => picked.includes(x));

/// Whether an observation is in one of the states picked, or nothing is picked by state.
const byState = (o: Observation, picked: readonly ObservationState[]): boolean =>
  picked.length === 0 || picked.includes(stateOf(o));

/// What a project's list shows, in the order read (the open ones first, each newest first): with nothing picked every
/// observation; else those that match any pick within each group and every group with a pick; and the one open
/// whatever is picked, so that what is open stays in the list.
export function shownObservations(
  all: readonly Observation[],
  tags: readonly string[],
  picks: Picks,
  open: number | null,
): Observation[] {
  return all.filter((o) => o.id === open || (byTags(o, tags, picks.tags) && byState(o, picks.states)));
}

/// The buttons over a project's list and what each would show under the picks of the other group: in `order`, each of
/// the project's tags that any of its observations carries, in the project's order, then a tag picked that the project
/// lacks, then none of its tags when any carries none of them; open and handled, each where any is so or it is picked;
/// so that a pick always has its button to take it back. In `counts`, one for each tag a row carries among the rows the
/// state picks let show, and one for each row's state among the rows the tag picks let show.
export function filters(
  all: readonly Observation[],
  tags: readonly string[],
  picks: Picks,
): {
  tags: { order: string[]; counts: string[] };
  states: { order: ObservationState[]; counts: ObservationState[] };
} {
  const carried = new Set(all.flatMap((o) => tagsOf(o, tags)));
  const tagOrder = [...tags, ...picks.tags.filter((x) => x !== NO_TAG && !tags.includes(x)), NO_TAG].filter(
    (x) => carried.has(x) || picks.tags.includes(x),
  );
  const states = new Set(all.map(stateOf));
  return {
    tags: {
      order: tagOrder,
      counts: all.filter((o) => byState(o, picks.states)).flatMap((o) => tagsOf(o, tags)),
    },
    states: {
      order: OBSERVATION_STATES.filter((x) => states.has(x) || picks.states.includes(x)),
      counts: all.filter((o) => byTags(o, tags, picks.tags)).map(stateOf),
    },
  };
}
