// The Settings view: the installation's settings as their file writes them, key by key. First what the page changes
// (the language the owner reads, each level's model, the highest level an item may be at, the context past which a
// line begins a new session, how many rounds run at once, and the account the agents run under), each as a box; then
// what only the file changes, table by table, each key with its value as written. Beside a value the engine runs at
// another, which it does until it starts again, since it reads the file as it starts, is the value it runs with.
// Save, the one action the view is for, writes what changed into the file, its comments and layout kept, and commits
// it where the file's directory keeps it in git; what the engine would refuse is refused before anything is written,
// in the engine's words. What the owner typed and did not save is kept in the browser, key by key, through a reload, a
// read of the file anew and a save refused.

import { useCallback, useEffect, useMemo, useState } from 'react';

import { Button, PanelPart } from '../parts';
import { useReading } from '../works/source';
import {
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
type Said = { ok: true; saved: Saved } | { ok: false; why: string } | null;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function SettingsView() {
  const source = useSystemSource();
  const load = useCallback(() => source.loadSettings(), [source]);
  const { value: page, failed, read } = useReading(load);
  const [said, setSaid] = useState<Said>(null);
  if (page === null) {
    return failed ? (
      <p className="message">The page could not read the settings: {failed}</p>
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
          onSaid({ ok: true, saved });
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
                  title="The engine reads the file as it starts, and runs with what it read until it starts again"
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
                {said.saved.committed ? ', and committed' : ''}. The engine takes it as it starts again.
              </p>
            ) : (
              <p className="failed" role="alert">
                Refused: {said.why}
              </p>
            ))}
          <h4 className="view-sub label">The account the agents run under (agents.config_dir)</h4>
          <Accounts onSaid={onSaid} reread={reread} />
        </PanelPart>
        <PanelPart label={`Changed in ${page.file.split('/').pop() ?? page.file} only`}>
          {fileOnly.map((t) => (
            <FileTable key={t.name} table={t} />
          ))}
        </PanelPart>
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

/// A name the owner gives a new account's directory, after the program's own name and a dash.
const NAME = /^[a-z0-9][a-z0-9-]*$/;

const ACCOUNT_DRAFT = 'settings-account-draft';

function keptName(): string {
  try {
    return localStorage.getItem(ACCOUNT_DRAFT) ?? '';
  } catch {
    return '';
  }
}

/// The accounts the agents' program may run under: each directory by its name, the account it is signed in as, and
/// the one the file names; another signed in is used once the owner says yes beside it; a new one is signed in by
/// hand with the command the view gives, the accounts read again every few seconds until it shows.
function Accounts({ onSaid, reread }: { onSaid: (said: Said) => void; reread: () => void }) {
  const source = useSystemSource();
  const load = useCallback(() => source.loadAccounts(), [source]);
  const { value: page, failed, read } = useReading(load);
  const [asking, setAsking] = useState<string | null>(null);
  // Whether the choice of an account is on its way: it is not sent again meanwhile.
  const [choosing, setChoosing] = useState(false);
  const [name, setName] = useState(keptName);
  const typed = name.trim();
  const ok = NAME.test(typed) && !(page?.accounts ?? []).some((a) => a.name === `${page?.stem}-${typed}` && a.signedInAs !== null);
  useEffect(() => {
    if (!ok) return;
    const every = window.setInterval(read, 3000);
    return () => window.clearInterval(every);
  }, [ok, read]);
  if (page === null) {
    return failed ? <p className="failed">The accounts could not be read: {failed}</p> : <p className="muted">Reading the accounts</p>;
  }
  if (page.dir === null) {
    return (
      <p className="muted">
        <span className="mono">agents.config_dir</span> names no directory: the agents' program runs with its own, and has no
        accounts beside it.
      </p>
    );
  }
  const use = (account: string) => {
    if (choosing) return;
    setChoosing(true);
    onSaid(null);
    source
      .chooseAccount({ base: page.base, name: account })
      .then(
        (saved) => onSaid({ ok: true, saved }),
        (e: unknown) => onSaid({ ok: false, why: message(e) }),
      )
      .finally(() => {
        setChoosing(false);
        setAsking(null);
        read();
        reread();
      });
  };
  const typeName = (v: string) => {
    setName(v);
    try {
      if (v === '') localStorage.removeItem(ACCOUNT_DRAFT);
      else localStorage.setItem(ACCOUNT_DRAFT, v);
    } catch {
      // Kept for this page only.
    }
  };
  const command = `${page.variable}=${page.dir}/${page.stem}-${typed} ${page.command}`;
  return (
    <>
      <ViewTable heads={['Directory', 'Signed in as', '']} label="The accounts">
        {page.accounts.map((a) => (
          <tr key={a.name}>
            <td className="mono" title={a.path}>
              {a.name}
            </td>
            <td>{a.signedInAs ?? <span className="muted">not signed in</span>}</td>
            <td>
              {a.named ? (
                <span className="tag account-named">named</span>
              ) : a.signedInAs === null ? null : asking === a.name ? (
                <span className="ask-line">
                  The agents run under {a.signedInAs} once the engine starts again, each line in a new session.{' '}
                  <Button
                    disabled={choosing}
                    title={choosing ? 'The choice is on its way' : `Write ${a.path} as agents.config_dir`}
                    onClick={() => use(a.name)}
                  >
                    Use it
                  </Button>{' '}
                  <Button onClick={() => setAsking(null)}>Cancel</Button>
                </span>
              ) : (
                <Button title={`Write ${a.path} as agents.config_dir`} onClick={() => setAsking(a.name)}>
                  Use
                </Button>
              )}
            </td>
          </tr>
        ))}
      </ViewTable>
      <div className="account-add">
        <label className="small muted" htmlFor="account-name">
          A new account's directory: {page.stem}-
        </label>
        <input
          id="account-name"
          className="field mono"
          value={name}
          placeholder="name"
          onChange={(e) => typeName(e.target.value)}
        />
        {typed !== '' && !ok && <span className="failed small">A name is small letters, digits and dashes, and not one signed in</span>}
      </div>
      {ok && (
        <p className="small">
          Run <code className="mono command">{command}</code>, type <code className="mono">{page.signIn}</code> there, and
          sign in: the account shows here once it is.
        </p>
      )}
    </>
  );
}
