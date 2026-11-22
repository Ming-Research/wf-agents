// The Settings view: the installation's settings as their file writes them, key by key. First what the page changes
// (the language the owner reads, each level's model, the highest level an item may be at, the context past which a
// line begins a new session, and how many rounds run at once), each as a box, and the accounts the agents run under,
// which are the engine's own state and change at once; then what only the file changes, table by table, each key with
// its value as written. Beside a value the engine runs at another, which it does until it starts again, since it reads
// the file as it starts, is the value it runs with; but the slots, which a save gives the engine at once.
// Save, the one action the view is for, writes what changed into the file, its comments and layout kept, and commits
// it where the file's directory keeps it in git; what the engine would refuse is refused before anything is written,
// in the engine's words. What the owner typed and did not save is kept in the browser, key by key, through a reload, a
// read of the file anew and a save refused. Last, once the file is read or could not be, the accent the page is drawn
// in, which is no setting of the file: kept in this browser as light or dark is, and taken as it is picked.

import { type CSSProperties, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  accentNow,
  DEFAULT_ACCENT,
  type Meaning,
  pickAccent,
  PRESETS,
  type Refusal,
  tones,
  useAccent,
} from '../accent';
import { Button, PanelPart, Switch } from '../parts';
import { useChanges, useReading } from '../works/source';
import { useNow } from '../works/marks';
import { clock, duration, fullTime } from '../works/words';
import {
  type AccountAsked,
  type AccountDone,
  type AccountsPage,
  type AccountUsage,
  type Change,
  type Saved,
  type SettingsPage,
  type SettingsTable,
  useSystemSource,
} from './api';
import { ViewTable } from './parts';

/// A key the page changes: its tables and itself joined by dots, what it may be, and its value as the box shows it.
interface Changeable {
  key: string;
  change: Change;
  /// The value as written, or as the engine runs with it where the file leaves it out.
  value: string;
  /// The value the engine runs with, where it is not the one written.
  running: string | null;
  /// Whether the file leaves it out.
  leftOut: boolean;
}

/// A value as TOML writes it, as the box shows it: a string without its quotes.
export function unquoted(text: string): string {
  if (text.startsWith('"')) {
    try {
      return JSON.parse(text) as string;
    } catch {
      return text.slice(1, -1);
    }
  }
  if (text.startsWith("'")) return text.slice(1, -1);
  return text;
}

const joined = (table: string, key: string) => (table === '' ? key : `${table}.${key}`);

/// The keys the page changes, in the order the file has them.
export function changeables(page: SettingsPage): Changeable[] {
  return page.tables.flatMap((t) =>
    t.keys.flatMap((k) =>
      k.change === null
        ? []
        : [
            {
              key: joined(t.name, k.key),
              change: k.change,
              value: unquoted(k.written ?? k.running ?? ''),
              running: k.running,
              leftOut: k.written === null,
            },
          ],
    ),
  );
}

/// Why a value typed is not of the key's kind, as the route refuses it; none where it is. What the engine refuses of a
/// value of the right kind, such as no slots at all, its own reading of the file refuses as the change is saved.
export function problem(c: Changeable, value: string, levels: string[]): string | null {
  switch (c.change.kind) {
    case 'name':
      return value.trim() !== '' && !/[\n\r]/.test(value) ? null : `${c.key} is a name on one line`;
    case 'count':
      return /^\d+$/.test(value) ? null : `${c.key} is a whole number`;
    case 'level':
      return levels.includes(value) ? null : `${c.key} is one of the levels: ${levels.join(', ')}`;
  }
}

/// What the owner typed and did not save: each key the owner changed, with its value as typed.
type Draft = Record<string, string>;

const DRAFT = 'settings-draft';

function keptDraft(): Draft {
  try {
    const text = localStorage.getItem(DRAFT);
    if (text === null) return {};
    const draft: unknown = JSON.parse(text);
    const ok =
      typeof draft === 'object' &&
      draft !== null &&
      !Array.isArray(draft) &&
      Object.values(draft).every((v) => typeof v === 'string');
    return ok ? (draft as Draft) : {};
  } catch {
    return {};
  }
}

function keepDraft(draft: Draft) {
  try {
    if (Object.keys(draft).length === 0) localStorage.removeItem(DRAFT);
    else localStorage.setItem(DRAFT, JSON.stringify(draft));
  } catch {
    // Kept for this page only.
  }
}

/// Of a draft, the keys still changed against the file as it is now: a key the file no longer has, or whose value the
/// file now has, is no longer a change.
function stillChanged(draft: Draft, keys: Changeable[]): Draft {
  return Object.fromEntries(Object.entries(draft).filter(([key, v]) => keys.some((c) => c.key === key && c.value !== v)));
}

/// What a request said: done, with what, or refused, with why.
type Said = { ok: true; saved: Saved; keys: string[] } | { ok: false; why: string } | null;

/// The key whose saved value the engine takes at once (decision the-slots-change-at-once); every other waits for the
/// engine to start again.
const AT_ONCE = 'agents.slots';

/// What a save says of when the engine takes what it saved, by the keys it set.
function takes(keys: string[]): string {
  if (!keys.includes(AT_ONCE)) return 'The engine takes it as it starts again.';
  return keys.length === 1
    ? 'The engine runs with it now.'
    : 'The engine runs with the slots now, and takes the rest as it starts again.';
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function SettingsView() {
  const source = useSystemSource();
  const load = useCallback(() => source.loadSettings(), [source]);
  const { value: page, failed, read } = useReading(load);
  const [said, setSaid] = useState<Said>(null);
  if (page === null) {
    // While the file is read, the accent waits with the rest, so that it does not stand first and then move.
    return failed ? (
      <>
        <p className="message">The page could not read the settings: {failed}</p>
        <main className="view-page">
          <AccentPart />
        </main>
      </>
    ) : (
      <p className="message muted">Reading the settings</p>
    );
  }
  return <SettingsForm page={page} failed={failed} said={said} onSaid={setSaid} reread={read} />;
}

function SettingsForm({ page, failed, said, onSaid, reread }: {
  page: SettingsPage;
  failed: string | null;
  said: Said;
  onSaid: (said: Said) => void;
  reread: () => void;
}) {
  const source = useSystemSource();
  const keys = useMemo(() => changeables(page), [page]);
  // What was typed is kept as it was typed, the file read anew under it: each key it changes stays changed.
  const [typed, setTyped] = useState<Draft>(keptDraft);
  const values = useMemo(() => stillChanged(typed, keys), [typed, keys]);
  const [sending, setSending] = useState(false);
  useEffect(() => keepDraft(values), [values]);
  const valueOf = (c: Changeable) => values[c.key] ?? c.value;
  const set = (key: string, value: string) => {
    const next = stillChanged({ ...values, [key]: value }, keys);
    setTyped(next);
    keepDraft(next);
  };
  const changed = keys.filter((c) => valueOf(c) !== c.value);
  const problems = changed.map((c) => problem(c, valueOf(c), page.levels)).filter((p): p is string => p !== null);
  const notCommitted = page.git !== null && !page.git.committed;
  const why =
    changed.length === 0
      ? 'Nothing is changed yet'
      : problems.length > 0
        ? 'A value is not one the key may have'
        : notCommitted
          ? `${page.file} has changes not committed in its directory: commit them or drop them first`
          : sending
            ? 'Saving'
            : undefined;
  const save = () => {
    if (why !== undefined) return;
    setSending(true);
    onSaid(null);
    source
      .saveSettings({
        base: page.base,
        set: changed.map((c) => ({ key: c.key, value: c.change.kind === 'count' ? Number(valueOf(c)) : valueOf(c) })),
      })
      .then(
        (saved) => {
          const left = Object.fromEntries(Object.entries(values).filter(([key]) => !changed.some((c) => c.key === key)));
          setTyped(left);
          keepDraft(left);
          onSaid({ ok: true, saved, keys: changed.map((c) => c.key) });
        },
        // Refused, what was typed stays, and the file is read anew: where it changed since, the change is made on it.
        (e: unknown) => onSaid({ ok: false, why: message(e) }),
      )
      .finally(() => {
        setSending(false);
        reread();
      });
  };
  const fileOnly = page.tables
    .map((t) => ({ ...t, keys: t.keys.filter((k) => k.change === null) }))
    .filter((t) => t.keys.length > 0);
  return (
    <>
      <div className="view-line">
        <span className="view-name">Settings</span>
        <span className="view-facts mono" title={page.file}>
          {page.file}
        </span>
        <span className="view-end">
          {page.git !== null && (
            <span className={notCommitted ? 'warn-text small' : 'muted small'} title="How the file's directory keeps it in git">
              git {page.git.committed ? 'committed' : 'changes not committed'}
            </span>
          )}
          <Button primary disabled={why !== undefined} title={why ?? 'Write what changed into the file'} onClick={save}>
            Save
          </Button>
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The settings could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main className="view-page">
        {page.unreadable !== null && (
          <p className="failed view-fault" role="alert">
            The file does not read as settings: {page.unreadable}
          </p>
        )}
        <PanelPart label="Changed here">
          <ViewTable heads={['Key', 'Value', 'The engine runs with']} label="The settings the page changes">
            {keys.map((c) => (
              <tr key={c.key}>
                <td className="mono">{c.key}</td>
                <td>
                  <ValueBox c={c} value={valueOf(c)} levels={page.levels} onChange={(v) => set(c.key, v)} />
                </td>
                <td
                  className="mono muted"
                  title={
                    c.key === AT_ONCE
                      ? 'The engine runs with this now: a save here gives it at once'
                      : 'The engine reads the file as it starts, and runs with what it read until it starts again'
                  }
                >
                  {c.running ?? ''}
                  {c.leftOut && ' · left out of the file'}
                </td>
              </tr>
            ))}
          </ViewTable>
          {problems.length > 0 && (
            <ul className="failed" role="alert">
              {problems.map((p) => (
                <li key={p}>{p}</li>
              ))}
            </ul>
          )}
          {said !== null &&
            (said.ok ? (
              <p className="said" role="status">
                Saved: <span className="mono">{said.saved.said}</span>
                {said.saved.committed ? ', and committed' : ''}. {takes(said.keys)}
              </p>
            ) : (
              <p className="failed" role="alert">
                Refused: {said.why}
              </p>
            ))}
          <h4 className="view-sub label">The accounts the agents run under</h4>
          <Accounts />
        </PanelPart>
        <PanelPart label={`Changed in ${page.file.split('/').pop() ?? page.file} only`}>
          {fileOnly.map((t) => (
            <FileTable key={t.name} table={t} />
          ))}
        </PanelPart>
        <AccentPart />
      </main>
    </>
  );
}

/// The box a key's value is typed or picked in.
function ValueBox({ c, value, levels, onChange }: {
  c: Changeable;
  value: string;
  levels: string[];
  onChange: (value: string) => void;
}) {
  if (c.change.kind === 'level') {
    return (
      <select className="select mono" aria-label={c.key} value={value} onChange={(e) => onChange(e.target.value)}>
        {!levels.includes(value) && <option value={value}>{value}</option>}
        {levels.map((l) => (
          <option key={l} value={l}>
            {l}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      className="field mono"
      aria-label={c.key}
      inputMode={c.change.kind === 'count' ? 'numeric' : undefined}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

/// A table of the file, by its header, each key with its value as written; a key the file leaves out has the value
/// the engine runs with, and says it is left out.
function FileTable({ table }: { table: SettingsTable }) {
  return (
    <section className="file-table">
      <h4 className="view-sub mono">
        {table.name === '' ? 'top level' : `[${table.name}]`}
        {table.leftOut && <span className="muted"> left out</span>}
      </h4>
      <dl className="pairs view-pairs">
        {table.keys.map((k) => (
          <div key={k.key} className="pair">
            <dt className="mono">{k.key}</dt>
            <dd className="mono value-written">
              {k.written ?? <span className="muted">left out</span>}
              {k.running !== null && (
                <span
                  className="muted"
                  title="The engine reads the file as it starts, and runs with what it read until it starts again"
                >
                  {' '}
                  · runs with {k.running}
                </span>
              )}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

/// A name the owner gives an account: small letters, digits and dashes, beginning with a letter or a digit.
const NAME = /^[a-z0-9][a-z0-9-]*$/;

const ACCOUNT_DRAFT = 'settings-account-draft';

function keptName(): string {
  try {
    return localStorage.getItem(ACCOUNT_DRAFT) ?? '';
  } catch {
    return '';
  }
}

const DAY = 24 * 60 * 60 * 1000;

/// From how long before its token ends a row says so.
const ENDING = 30 * DAY;

/// When a usage limit resets, from now: how long until it does and the time it does, `resets in 2d 5h (11-22 16:00)`,
/// since the owner chooses the account by how soon its limit frees it; once the time has passed, the time it reset.
function resetsWords(resets: number, now: number): string {
  return resets > now ? `resets in ${duration(resets - now)} (${clock(resets)})` : `reset ${clock(resets)}`;
}

/// What the last act on an account came to, in words: done, or not, as when the engine refused it, in its words.
type AccountSaid = { ok: boolean; words: string } | null;

/// The accounts the agents run under, as the engine holds them: each by its name, and its usage, which the owner
/// chooses the account by ([`UsageCell`]), counted down to its resets as the view stays open; the one in use marked,
/// each other with Use, which takes it at once, and Remove, asked once beside it. An account is added by its name and its token, which the engine checks before it keeps it; the
/// command that makes a token is given to copy. The name box keeps what was typed through a reload; the token box keeps
/// nothing. With no account in use, one line says that no agent's round runs until one is. As the view opens, the
/// engine looks under the accounts it has not heard of lately, and the view reads the accounts again each time the
/// engine says they changed.
function Accounts() {
  const source = useSystemSource();
  const load = useCallback(() => source.loadAccounts(), [source]);
  const { value: page, failed, read } = useReading(load);
  // A look spends a short request under each account not heard of lately; reading the accounts spends nothing. A
  // refused look is said, so that usage not looked at is not taken for the latest.
  const [lookRefused, setLookRefused] = useState<string | null>(null);
  useEffect(() => {
    source.look().then(
      () => setLookRefused(null),
      (e: unknown) => setLookRefused(message(e)),
    );
  }, [source]);
  useChanges((changed) => {
    if (changed.accounts) read();
  }, read);
  // The account whose removal is asked, and what is on its way: an act on an account, by its name.
  const [asking, setAsking] = useState<string | null>(null);
  const [busy, setBusy] = useState<AccountAsked | null>(null);
  const [said, setSaid] = useState<AccountSaid>(null);
  const [name, setName] = useState(keptName);
  // The token is held while the page is open, and written nowhere.
  const [token, setToken] = useState('');
  // The engine's time as it answered, carried on by the browser's clock, so that how long until a limit resets counts
  // down while the view is open.
  const ticking = useNow(page !== null);
  const [answered, setAnswered] = useState({ page, at: Date.now() });
  if (answered.page !== page) setAnswered({ page, at: Date.now() });
  if (page === null) {
    return failed ? (
      <p className="failed">The accounts could not be read: {failed}</p>
    ) : (
      <p className="muted">Reading the accounts</p>
    );
  }
  const now = page.now + Math.max(0, ticking - answered.at);
  const typed = name.trim();
  const named = page.accounts.some((a) => a.name === typed);
  const typeName = (v: string) => {
    setName(v);
    try {
      if (v === '') localStorage.removeItem(ACCOUNT_DRAFT);
      else localStorage.setItem(ACCOUNT_DRAFT, v);
    } catch {
      // Kept for this page only.
    }
  };
  const ask = (asked: AccountAsked, done: (answer: AccountDone) => { ok: boolean; words: string }) => {
    if (busy !== null) return;
    setBusy(asked);
    setSaid(null);
    source
      .account(asked)
      .then(
        (answer) => {
          setSaid(done(answer));
          if (asked.kind === 'add') {
            setToken('');
            typeName('');
          }
        },
        (e: unknown) => setSaid({ ok: false, words: `Refused: ${message(e)}` }),
      )
      .finally(() => {
        setBusy(null);
        setAsking(null);
        read();
      });
  };
  const add = () =>
    ask({ kind: 'add', name: typed, token: token.trim() }, (answer) => {
      if (!answer.atLimit) return { ok: true, words: `Added ${typed}.` };
      const now = page.now + Date.now() - answered.at;
      const resets = answer.resets === null ? '' : `, which ${resetsWords(answer.resets, now)}`;
      return { ok: true, words: `Added ${typed}: its token is at its usage limit${resets}.` };
    });
  const why =
    page.closed !== null
      ? page.closed
      : !NAME.test(typed)
        ? 'A name is small letters, digits and dashes, beginning with a letter or a digit'
        : named
          ? `An account named ${typed} is added already`
          : token.trim() === ''
            ? 'Paste the token the command printed'
            : busy !== null
              ? 'An act on an account is on its way'
              : undefined;
  return (
    <>
      {page.accounts.length > 0 && (
        <ViewTable heads={['Account', 'Usage', '']} label="The accounts">
          {page.accounts.map((a) => (
            <AccountRow
              key={a.name}
              account={a}
              page={page}
              now={now}
              asking={asking === a.name}
              busy={busy !== null}
              onUse={() =>
                ask({ kind: 'take', name: a.name }, () => ({ ok: true, words: `The agents run under ${a.name}.` }))
              }
              onAsk={(yes) => setAsking(yes ? a.name : null)}
              onRemove={() =>
                ask({ kind: 'remove', name: a.name }, (answer) =>
                  answer.notLetGo === null
                    ? { ok: true, words: `Removed ${a.name}.` }
                    : {
                        ok: false,
                        words:
                          `Removed ${a.name}, but a token is still in the keychain, and is let go of again at the ` +
                          `next removal and as the engine starts: ${answer.notLetGo}`,
                      },
                )
              }
            />
          ))}
        </ViewTable>
      )}
      {page.inUse === null && (
        <p className="warn-text small">No account is in use, and no agent's round runs until one is.</p>
      )}
      {lookRefused !== null && (
        <p className="failed small" role="alert">
          The usage was not looked at: {lookRefused}
        </p>
      )}
      <div className="account-add">
        <label className="small muted" htmlFor="account-name">
          Name
        </label>
        <input id="account-name" className="field mono" value={name} onChange={(e) => typeName(e.target.value)} />
        <label className="small muted" htmlFor="account-token">
          Token
        </label>
        <input
          id="account-token"
          className="field mono"
          type="password"
          autoComplete="off"
          value={token}
          onChange={(e) => setToken(e.target.value)}
        />
        <Button disabled={why !== undefined} title={why ?? `Check the token and keep it for ${typed}`} onClick={add}>
          Add
        </Button>
      </div>
      <p className="small">
        Run <code className="mono command">{page.command}</code> while the browser is signed in to that account, and
        paste the token it prints.
      </p>
      {page.closed !== null && (
        <p className="failed small" role="alert">
          No account can be added or taken, and no agent runs: {page.closed}
        </p>
      )}
      {busy?.kind === 'add' && (
        <p className="muted small" role="status">
          Checking the token: this may take up to two minutes.
        </p>
      )}
      {said !== null && (
        <p className={said.ok ? 'said' : 'failed'} role={said.ok ? 'status' : 'alert'}>
          {said.words}
        </p>
      )}
    </>
  );
}

/// An account's row: its name, with when it was added and until when its token lasts on pointing, and in the look of a
/// warning from a month before the token ends, that it does; its usage; and what may be done to it.
function AccountRow({ account: a, page, now, asking, busy, onUse, onAsk, onRemove }: {
  account: AccountsPage['accounts'][number];
  page: AccountsPage;
  now: number;
  asking: boolean;
  busy: boolean;
  onUse: () => void;
  onAsk: (yes: boolean) => void;
  onRemove: () => void;
}) {
  const left = a.lasts - now;
  const ending = left <= ENDING;
  const busyWhy = busy ? 'An act on an account is on its way' : undefined;
  const cache = `${a.name}'s cache`;
  const use = `Run the agents under ${a.name} from now on: each line's next round writes its session to ${cache}`;
  return (
    <tr>
      <td title={`Added ${fullTime(a.added)}; its token lasts until ${fullTime(a.lasts)}.`}>
        <span className="mono">{a.name}</span>
        {ending && (
          <span className="warn-text">{left > 0 ? ` · its token ends in ${duration(left)}` : ' · its token ended'}</span>
        )}
      </td>
      <UsageCell usage={a.usage} limit={a.limit} now={now} />
      <td>
        {page.inUse === a.name ? (
          <span className="tag account-in-use">in use</span>
        ) : asking ? (
          <span className="ask-line">
            Remove {a.name} and its token?{' '}
            <Button
              disabled={busy}
              title={busyWhy ?? `Remove ${a.name} and delete its token from the keychain`}
              onClick={onRemove}
            >
              Remove it
            </Button>{' '}
            <Button onClick={() => onAsk(false)}>Cancel</Button>
          </span>
        ) : (
          <>
            <Button disabled={busy || page.closed !== null} title={busyWhy ?? page.closed ?? use} onClick={onUse}>
              Use
            </Button>{' '}
            <Button disabled={busy} title={busyWhy ?? `Remove ${a.name}`} onClick={() => onAsk(true)}>
              Remove
            </Button>
          </>
        )}
      </td>
    </tr>
  );
}

/// An account's usage cell, on one line as every cell of the table is: the usage limit the engine holds the account's
/// rounds at, as its records say, until its reset has passed, since it is what holds the rounds, or else the limit the
/// service said it met naming no window; each window the service last named under it, by the name the service gives
/// it, with how much of it is used and how long until it resets and when, joined; then "looking" while a look under it is under way, or
/// "not seen" where nothing was heard of it; and "failed" where its latest look failed. A limit, and a window the
/// service lets through only with a warning or not at all, are in the look of a warning, and a failed look in the look
/// of a failure. On pointing, the cell says the whole line, when the service said it, what the warning's colour means,
/// and why the look failed.
function UsageCell({ usage, limit, now }: {
  usage: AccountUsage;
  limit: AccountsPage['accounts'][number]['limit'];
  now: number;
}) {
  const held = limit !== null && (limit.resets === null || limit.resets > now) ? limit : null;
  const atLimit =
    held !== null
      ? held.resets === null
        ? `at its limit since ${clock(held.since)}`
        : `at its limit · ${resetsWords(held.resets, now)}`
      : usage.atLimit !== null
        ? usage.atLimit.resets === null
          ? 'at its limit'
          : `at its limit · ${resetsWords(usage.atLimit.resets, now)}`
        : null;
  const parts: { text: string; look?: string }[] = [];
  if (atLimit !== null) parts.push({ text: atLimit, look: 'warn-text' });
  for (const w of usage.windows) {
    const used = w.utilization === null ? '' : ` ${Math.round(w.utilization * 100)}%`;
    const resets = w.resets === null ? '' : ` · ${resetsWords(w.resets, now)}`;
    parts.push({ text: `${w.name}${used}${resets}`, look: w.allowed ? undefined : 'warn-text' });
  }
  if (usage.looking) parts.push({ text: 'looking', look: 'muted' });
  else if (parts.length === 0 && usage.failed === null) parts.push({ text: 'not seen', look: 'muted' });
  if (usage.failed !== null) parts.push({ text: 'failed', look: 'failed' });
  const title = [
    parts.map((p) => p.text).join(' · '),
    usage.seen === null ? null : `Said by the service ${fullTime(usage.seen)}.`,
    parts.some((p) => p.look === 'warn-text')
      ? 'In the look of a warning: the account at its limit, or a window the service lets its calls through in only ' +
        'with a warning or not at all.'
      : null,
    usage.failed === null ? null : `The latest look under it failed: ${usage.failed}`,
  ]
    .filter((line) => line !== null)
    .join('\n');
  return (
    <td title={title}>
      {parts.map((p, i) => (
        <span key={i}>
          {i > 0 && ' · '}
          <span className={p.look}>{p.text}</span>
        </span>
      ))}
    </td>
  );
}

/// The colours of the meanings the accent keeps clear of, as the page names them.
const MEANING_WORDS: Record<Meaning, string> = {
  red: 'the red of a problem',
  orange: "the orange of a block's meaning changed",
  yellow: 'the yellow of a call under way',
  green: 'the green of a work done',
};

/// Why a colour is not taken as the accent, in a clause, the colour as a value.
function refusedWords(colour: string, why: Refusal): ReactNode {
  return (
    <>
      <span className="mono">{colour}</span>
      {why === 'grey'
        ? ' is too grey to stand apart from the muted words and a pause'
        : ` is too near ${MEANING_WORDS[why]}, which keeps its meaning`}
    </>
  );
}

/// The accent the page is drawn in: the presets as a switch, each with a mark of the accent it gives in light and in
/// dark, and a colour of the owner's own beside them, taken as the browser's picker closes; one that is refused leaves
/// the accent as it was, and the part says why under them, once, until the next colour is picked.
function AccentPart() {
  const accent = useAccent();
  const [refused, setRefused] = useState<{ colour: string; why: Refusal } | null>(null);
  const box = useRef<HTMLInputElement>(null);
  const inForce = accent.colour ?? DEFAULT_ACCENT;
  const own = accent.colour !== null && !PRESETS.some((p) => p.colour === accent.colour);
  const marks = useMemo(() => PRESETS.map((p) => tones(p.colour)), []);
  const pick = useCallback((colour: string) => {
    const why = pickAccent(colour);
    setRefused(why === null ? null : { colour, why });
    // A colour refused does not stay in the box: it shows the accent in force.
    if (why !== null && box.current !== null) box.current.value = accentNow() ?? DEFAULT_ACCENT;
  }, []);
  // On the browser's `change`, which a colour picker sends as it closes, and not on each step of a drag in it, which
  // would draw the whole page and say why anew at every step.
  useEffect(() => {
    const input = box.current;
    if (input === null) return;
    const changed = () => pick(input.value);
    input.addEventListener('change', changed);
    return () => input.removeEventListener('change', changed);
  }, [pick]);
  useEffect(() => {
    if (box.current !== null) box.current.value = inForce;
  }, [inForce]);
  return (
    <PanelPart label="Accent, kept in this browser">
      <div className="accent-pick">
        <Switch
          label="The accent, kept in this browser"
          value={own ? '' : inForce}
          onChange={pick}
          choices={PRESETS.map((p, i) => ({
            value: p.colour,
            name: (
              <>
                <span
                  className="accent-mark"
                  style={
                    {
                      '--accent-mark': marks[i].light.accent,
                      '--accent-mark-dark': marks[i].dark.accent,
                    } as CSSProperties
                  }
                />
                {p.name}
              </>
            ),
            title:
              p.colour === DEFAULT_ACCENT
                ? 'Draw the page in its own blue, kept in this browser'
                : `Draw the page's accent in ${p.name.toLowerCase()}, kept in this browser`,
          }))}
        />
        <label
          className={own ? 'accent-own accent-own-taken' : 'accent-own'}
          title="Draw the page's accent in a colour of your own, kept in this browser: the page takes its hue and strength, and draws each tone as light as it reads on its ground"
        >
          Your own
          <input ref={box} type="color" className="accent-box" defaultValue={inForce} />
        </label>
      </div>
      {refused !== null ? (
        <p className="failed small" role="alert">
          {refusedWords(refused.colour, refused.why)}: the accent stays as it was, and takes a colour from blue to
          purple.
        </p>
      ) : (
        accent.refused !== null && (
          <p className="failed small" role="alert">
            The colour kept in this browser is refused now: {refusedWords(accent.refused.colour, accent.refused.why)}.
            The page is drawn in its own blue until another is picked.
          </p>
        )
      )}
    </PanelPart>
  );
}
