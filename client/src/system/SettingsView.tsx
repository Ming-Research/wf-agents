// The Settings view: the installation's settings as their file writes them, key by key. First what the page changes
// (the language the owner reads, each level's model, the highest level an item may be at, the context past which a
// line begins a new session, and how many rounds run at once), each as a box; then the accounts the agents run under,
// one of which the owner takes, or the engine moving on, at once, apart from what Save writes; then what only the file
// changes, table by table, each key with its value as written. Beside a value the engine runs at
// another, which it does until it starts again, since it reads the file as it starts, is the value it runs with.
// Save, the one action the view is for, writes what changed into the file, its comments and layout kept, and commits
// it where the file's directory keeps it in git; what the engine would refuse is refused before anything is written,
// in the engine's words. What the owner typed and did not save is kept in the browser, key by key, through a reload, a
// read of the file anew and a save refused. Last, once the file is read or could not be, the accent the page is drawn
// in, which is no setting of the file: kept in this browser as light or dark is, and taken as it is picked.

import { type CSSProperties, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { accentNow, type Meaning, pickAccent, PRESETS, type Refusal, tones, useAccent, VIOLET } from '../accent';
import { Button, PanelPart, Switch } from '../parts';
import { useReading } from '../works/source';
import { clock, fullTime } from '../works/words';
import {
  type AccountsChange,
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
        </PanelPart>
        <PanelPart label="Accounts the agents run under (agents.accounts)">
          <Accounts reread={reread} />
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

/// The accounts the agents' program may run under, in the owner's order: each directory by its name, the account it is
/// signed in as, when its usage limit resets where it is at one, and which is in use; another signed in is taken in one
/// click, and the words under them say what that costs; the switch that lets the engine move on at the week's limit of
/// the one in use; the accounts taken, the latest first; and a new account's directory, signed in by hand with the
/// command the view gives and then added to the file, the accounts read again every few seconds meanwhile.
function Accounts({ reread }: { reread: () => void }) {
  const source = useSystemSource();
  const load = useCallback(() => source.loadAccounts(), [source]);
  const { value: page, failed, read } = useReading(load);
  // What the owner did last, refused or added, said under the part until the next.
  const [said, setSaid] = useState<Said>(null);
  // Whether what the owner did is on its way: nothing is sent again meanwhile.
  const [sending, setSending] = useState(false);
  const [name, setName] = useState(keptName);
  const typed = name.trim();
  const named = NAME.test(typed);
  useEffect(() => {
    if (!named) return;
    const every = window.setInterval(read, 3000);
    return () => window.clearInterval(every);
  }, [named, read]);
  if (page === null) {
    return failed ? <p className="failed">The accounts could not be read: {failed}</p> : <p className="muted">Reading the accounts</p>;
  }
  const send = (change: AccountsChange, then?: () => void) => {
    if (sending) return;
    setSending(true);
    setSaid(null);
    source
      .changeAccounts(change)
      .then(
        (saved) => {
          if (saved !== null) setSaid({ ok: true, saved });
          then?.();
        },
        (e: unknown) => setSaid({ ok: false, why: message(e) }),
      )
      .finally(() => {
        setSending(false);
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
  const command = (path: string) => `${page.variable}=${path} ${page.command}`;
  const newPath = page.dir === null ? null : `${page.dir}/${page.stem}-${typed}`;
  const running = page.accounts.filter((a) => a.running);
  const pressing = sending ? 'What you did is on its way' : undefined;
  return (
    <>
      {page.accounts.length === 0 ? (
        <p className="muted">
          <span className="mono">agents.accounts</span> lists no account: the agents' program runs with its own directory.
        </p>
      ) : (
        <ViewTable heads={['Directory', 'Signed in as', 'Usage', '']} label="The accounts">
          {page.accounts.map((a) => (
            <tr key={a.path}>
              <td className="mono" title={a.path}>
                {a.name}
              </td>
              <td>{a.signedInAs ?? <span className="muted">not signed in</span>}</td>
              <td className={a.limited ? 'warn-text' : 'muted'} title={a.resets === null ? undefined : fullTime(a.resets)}>
                {a.limited ? (a.resets === null ? 'at its limit' : `at its limit until ${clock(a.resets)}`) : ''}
              </td>
              <td>
                {a.inUse ? (
                  <span className="tag account-in-use">in use</span>
                ) : !a.running ? (
                  <span className="muted small">taken as the engine starts again</span>
                ) : a.signedInAs === null ? (
                  <span className="muted small" title={`Run ${command(a.path)}, type ${page.signIn} there, and sign in`}>
                    sign it in first
                  </span>
                ) : (
                  <Button
                    disabled={sending}
                    title={pressing ?? `Run the agents under ${a.name}: each line's next round begins a new session under it`}
                    onClick={() => send({ take: { path: a.path } })}
                  >
                    Use
                  </Button>
                )}
              </td>
            </tr>
          ))}
        </ViewTable>
      )}
      <p className="small muted">
        Another account is taken at once: each line's next round begins a new session under it, which reads its
        context again.
      </p>
      <div className="account-line">
        <span className="small">Move on at the week's limit</span>
        <Switch<'off' | 'on'>
          label="Move on at the week's limit"
          value={page.movingOn ? 'on' : 'off'}
          onChange={(v) => send({ movingOn: { on: v === 'on' } })}
          choices={[
            {
              value: 'off',
              name: 'Off',
              title: "At the week's limit of the account in use, the works wait for its reset or for you to take another",
            },
            {
              value: 'on',
              name: 'On',
              title:
                running.length < 2
                  ? 'With one account listed there is none to move on to'
                  : "At the week's limit of the account in use, the engine takes the next one in this order that has usage left",
            },
          ]}
        />
      </div>
      {page.taken.length > 0 && (
        <>
          <h4 className="view-sub label">Taken</h4>
          <ul className="account-taken small">
            {page.taken.map((t) => (
              <li key={`${t.at}-${t.path}`}>
                <span className="mono" title={fullTime(t.at)}>
                  {clock(t.at)}
                </span>{' '}
                <span className="mono">{t.name}</span>
                {t.by === 'owner' ? (
                  <span className="muted"> by you</span>
                ) : (
                  <span className="muted">
                    {' '}
                    by the engine: <span className="mono">{t.left ?? 'the one before'}</span> at its limit
                    {t.resets === null ? '' : ` until ${clock(t.resets)}`}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {newPath !== null && (
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
          {typed !== '' && !named && <span className="failed small">A name is small letters, digits and dashes</span>}
          {named && (
            <Button
              disabled={sending}
              title={pressing ?? `Add ${newPath} to agents.accounts once it is signed in: the engine takes it as it starts again`}
              onClick={() => send({ add: { base: page.base, name: typed } }, () => typeName(''))}
            >
              Add
            </Button>
          )}
        </div>
      )}
      {named && newPath !== null && (
        <p className="small">
          Run <code className="mono command">{command(newPath)}</code>, type <code className="mono">{page.signIn}</code>{' '}
          there, and sign in; then add it.
        </p>
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
    </>
  );
}

/// The colours of the meanings the accent keeps clear of, as the page names them.
const MEANING_WORDS: Record<Meaning, string> = {
  red: 'the red of a problem',
  orange: "the orange of a block's meaning changed",
  yellow: 'the yellow of a call under way',
  green: 'the green of a work done',
  tools: "the teal of the tools' time",
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
  const inForce = accent.colour ?? VIOLET;
  const own = accent.colour !== null && !PRESETS.some((p) => p.colour === accent.colour);
  const marks = useMemo(() => PRESETS.map((p) => tones(p.colour)), []);
  const pick = useCallback((colour: string) => {
    const why = pickAccent(colour);
    setRefused(why === null ? null : { colour, why });
    // A colour refused does not stay in the box: it shows the accent in force.
    if (why !== null && box.current !== null) box.current.value = accentNow() ?? VIOLET;
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
                    { '--accent-mark': marks[i].light.accent, '--accent-mark-dark': marks[i].dark.accent } as CSSProperties
                  }
                />
                {p.name}
              </>
            ),
            title:
              p.colour === VIOLET
                ? 'Draw the page in its own violet, kept in this browser'
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
          {refusedWords(refused.colour, refused.why)}: the accent stays as it was, and takes a colour from blue to purple.
        </p>
      ) : (
        accent.refused !== null && (
          <p className="failed small" role="alert">
            The colour kept in this browser is refused now: {refusedWords(accent.refused.colour, accent.refused.why)}.
            The page is drawn in its own violet until another is picked.
          </p>
        )
      )}
    </PanelPart>
  );
}
