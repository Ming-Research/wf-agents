// The observations as the Observations view reads them, as `crates/web/src/observations.rs` gives them: the types here
// follow those, field for field. Every time is in milliseconds since the start of 1970.
//
// The routes:
// - `GET  /api/v1/observations`: every observation not deleted, the newest first (`ObservationsPage`).
// - `POST /api/v1/observations/{number}/read`: the owner read the observation; answers with when it was read (`Read`),
//   which a reading after the first leaves as it was, or with why it was refused.

import { read } from './api';
import { send } from './works-api';

/// What an agent came across beyond its work, as a return of its run said it and the run kept it as it finished, or
/// what an architect added: its number, written On; what it says; its title and tags, where it has them; the project it
/// is of and the project that reported it (none where a run kept it for an item kept before items had projects); where
/// a run kept it, none for one an architect added; when it was kept; when the owner read it, none while it is unread;
/// the item that took it in; and whether it is handled, which is where an item whose work is not cancelled took it in.
export interface Observation {
  number: number;
  text: string;
  title: string | null;
  tags: string[];
  project: string | null;
  reportedBy: string | null;
  round: Round | null;
  kept: number;
  read: number | null;
  takenInBy: number | null;
  handled: boolean;
}

/// Where a run kept an observation: the work whose run's return carried it, the work's item with its title as it
/// stands, and the cell, the run and the round whose return carried it.
export interface Round {
  work: number;
  item: number;
  itemTitle: string;
  cell: string;
  run: number;
  round: number;
}

export interface ObservationsPage {
  observations: Observation[];
}

export interface Read {
  read: number;
}

export const loadObservations = () => read<ObservationsPage>('/api/v1/observations');
export const readObservation = (number: number) => send<Read>(`/api/v1/observations/${number}/read`, {});
