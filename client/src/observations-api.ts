// The observations as the Observations view reads them, as `crates/web/src/observations.rs` gives them: the types here
// follow those, field for field. Every time is in milliseconds since the start of 1970.
//
// The routes:
// - `GET  /api/v1/observations`: every observation, the newest first (`ObservationsPage`).
// - `POST /api/v1/observations/{number}/read`: the owner read the observation; answers with when it was read (`Read`),
//   which a reading after the first leaves as it was, or with why it was refused.

import { read } from './api';
import { send } from './works-api';

/// What an agent came across beyond its work, as a return of its run said it and the run kept it as it finished: its
/// number, written On; what it says; the work, its item with its title as it stands, the cell, the run and the round
/// whose return carried it; when it was kept; and when the owner read it, none while it is unread.
export interface Observation {
  number: number;
  text: string;
  work: number;
  item: number;
  title: string;
  cell: string;
  run: number;
  round: number;
  kept: number;
  read: number | null;
}

export interface ObservationsPage {
  observations: Observation[];
}

export interface Read {
  read: number;
}

export const loadObservations = () => read<ObservationsPage>('/api/v1/observations');
export const readObservation = (number: number) => send<Read>(`/api/v1/observations/${number}/read`, {});
