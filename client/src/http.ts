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

async function fetchBody(path: string, signal?: AbortSignal): Promise<string> {
  const r = await fetch(path, { headers: { Accept: 'application/json' }, signal });
  if (!r.ok) throw await failure(r);
  return r.text();
}

/// A stored flow version and what the interface says of it, read once and shared by every part of the page that shows
/// it, such as a work's chart and the panel of a round beside it: a version never changes once stored.
const STORED = /^\/api\/v1\/flows\/\d+(\/view)?$/;
const stored = new Map<string, Promise<string>>();

/// What the interface answers at `path`, as the text of its JSON, for a reader that compares it with the last it got
/// before it parses it (`reader.ts`). `signal` ends the request; a stored flow version is shared by every reader, so no
/// one's `signal` ends it.
export async function getBody(path: string, signal?: AbortSignal): Promise<string> {
  if (!STORED.test(path)) return fetchBody(path, signal);
  let read = stored.get(path);
  if (read === undefined) {
    read = fetchBody(path);
    stored.set(path, read);
    // A read that failed is tried again the next time.
    read.catch(() => stored.delete(path));
  }
  return read;
}

export async function getJson<T>(path: string): Promise<T> {
  return JSON.parse(await getBody(path)) as T;
}

export async function getText(path: string): Promise<string> {
  const r = await fetch(path);
  if (!r.ok) throw await failure(r);
  return r.text();
}

/// Who speaks in every request that carries the owner's words or answers: this page is the owner's own, and a session
/// that speaks for the owner declares `delegate` instead. The engine keeps it with the words; it cannot tell who spoke.
export const SPEAKER = 'owner';

/// A POST that carries JSON, as the interface requires of every POST, refused with the interface's reason.
async function post(path: string, body: unknown): Promise<Response> {
  const r = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw await failure(r);
  return r;
}

/// What the owner asks of the engine: a POST that carries JSON, as the interface requires of every POST.
export async function postJson(path: string, body: unknown): Promise<void> {
  await post(path, body);
}

/// What the owner asks of the engine, where the interface answers with what came of it: the JSON it gave.
export async function postJsonFor<T>(path: string, body: unknown): Promise<T> {
  return JSON.parse(await (await post(path, body)).text()) as T;
}
