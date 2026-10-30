// Requests to the interface. A failed request says why, from the interface's own error when it gave one.

import type { ApiError } from './api';

export class RequestError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function failure(r: Response): Promise<RequestError> {
  let message = `${r.status} ${r.statusText}`;
  try {
    const body = (await r.json()) as ApiError;
    if (typeof body.error === 'string') message = body.error;
  } catch {
    // Not JSON: the status says enough.
  }
  return new RequestError(r.status, message);
}

async function fetchJson<T>(path: string): Promise<T> {
  const r = await fetch(path, { headers: { Accept: 'application/json' } });
  if (!r.ok) throw await failure(r);
  return (await r.json()) as T;
}

/// A stored flow version and what the interface says of it, read once and shared by every part of the page that shows
/// it, such as a work's chart and the panel of a round beside it: a version never changes once stored.
const STORED = /^\/api\/v1\/flows\/\d+(\/view)?$/;
const stored = new Map<string, Promise<unknown>>();

export async function getJson<T>(path: string): Promise<T> {
  if (!STORED.test(path)) return fetchJson<T>(path);
  let read = stored.get(path);
  if (read === undefined) {
    read = fetchJson<T>(path);
    stored.set(path, read);
    // A read that failed is tried again the next time.
    read.catch(() => stored.delete(path));
  }
  return read as Promise<T>;
}

export async function getText(path: string): Promise<string> {
  const r = await fetch(path);
  if (!r.ok) throw await failure(r);
  return r.text();
}

/// What the owner asks of the engine: a POST that carries JSON, as the interface requires of every POST.
export async function postJson(path: string, body: unknown): Promise<void> {
  const r = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw await failure(r);
}
