// The settings, a reading page of the UI spec: its header, then a section for the installation and one for each
// project. Each holds first what the owner changes there, as form controls, then what is changed only in the
// configuration's files, as label and value pairs, and for a project the rule texts its agents are given, folded, each
// a sheet; its constitution is named by its path, its text being the project's own. Save, the one action the form
// exists for, writes what changed into the configuration's files, their comments kept, and commits it; the engine
// takes it from its next round. A value the configuration would refuse is refused before anything is written: here
// what the page can tell, and on saving what only the configuration can. The agents' login account, under the
// installation, is switched by itself, after a warning, and a new one is signed in by hand with the command the page
// shows, the page reading the accounts again until it is.

import { CopyIcon, PlusIcon, XIcon } from '@primer/octicons-react';
import {
  Button,
  Checkbox,
  ConfirmationDialog,
  Flash,
  FormControl,
  IconButton,
  PageHeader,
  Select,
  TextInput,
} from '@primer/react';
import { type ReactNode, useEffect, useState } from 'react';

import type {
  Account,
  AccountChoice,
  CliAccounts,
  ProjectChange,
  ProjectSettings,
  Settings as SettingsData,
  SettingsChange,
  Tag,
} from '../api';
import { Loaded, StateChip } from '../components';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { Markdown, MarkdownInline } from '../markdown';
import { HeadingLevel, Section, SubHeading } from '../parts';
import { tokens } from '../round';
import { t, tn } from '../text';
import { Code, FoldPart, Name, Overview, Sheet } from '../voices';

/// A project's values as the owner edits them, every number as typed.
type ProjectDraft = { limits: Record<string, string>; tags: Tag[]; feedback: boolean };

/// The form's values as the owner edits them, every number as typed.
export type Draft = {
  models: Record<string, string>;
  highest: string;
  architect: string;
  limit: string;
  language: string;
  slots: Record<string, string>;
  projects: Record<string, ProjectDraft>;
};

/// The form's values at first: the settings as they are.
export function draftOf(s: SettingsData): Draft {
  return {
    models: Object.fromEntries(s.tiers.map((x) => [x.name, x.model])),
    highest: s.highest_level ?? '',
    architect: s.architect,
    limit: String(s.context_limit),
    language: s.language,
    slots: Object.fromEntries(s.hosts.map((h) => [h.name, String(h.slots)])),
    projects: Object.fromEntries(
      s.projects.map((p) => [
        p.name,
        {
          limits: Object.fromEntries(p.limits.map((l) => [l.counter, String(l.limit)])),
          tags: p.observation_tags.map((x) => ({ ...x })),
          feedback: p.flow_feedback,
        },
      ]),
    ),
  };
}

/// Whether a text is a whole number from `least` to `most`.
const whole = (text: string, least: number, most: number): boolean =>
  /^\d+$/.test(text.trim()) && Number(text) >= least && Number(text) <= most;

/// What the page refuses before anything is sent, each once: a tier with no model, a number of the wrong kind or out
/// of its range, a language that is no name on one line, a tag with no name, a name with spaces, a name twice or a
/// tag that does not say what it covers.
export function settingsProblems(d: Draft): string[] {
  const out: string[] = [];
  for (const [tier, model] of Object.entries(d.models)) {
    if (model.trim() === '') out.push(t('settings_no_model', { tier }));
  }
  if (!whole(d.limit, 50_000, 1_000_000)) out.push(t('settings_bad_limit'));
  const language = d.language.trim();
  if (language === '' || /[\n\r]/.test(language) || [...language].length > 40) out.push(t('settings_bad_language'));
  for (const [host, n] of Object.entries(d.slots)) {
    if (!whole(n, 0, 32)) out.push(t('settings_bad_slots', { host }));
  }
  for (const [project, p] of Object.entries(d.projects)) {
    for (const [counter, n] of Object.entries(p.limits)) {
      if (!whole(n, 1, 1_000_000)) out.push(t('settings_bad_counter', { project, counter }));
    }
    const seen = new Set<string>();
    for (const tag of p.tags) {
      const name = tag.name.trim();
      if (name === '' || /\s/.test(name)) out.push(t('settings_bad_tag', { project }));
      else if (seen.has(name)) out.push(t('settings_tag_twice', { project, tag: name }));
      else if (tag.desc.trim() === '') out.push(t('settings_tag_no_desc', { project, tag: name }));
      seen.add(name);
    }
  }
  return [...new Set(out)];
}

/// What changed of a project: none when nothing did.
function projectChange(p: ProjectSettings, d: ProjectDraft | undefined): ProjectChange | null {
  if (d === undefined) return null;
  const limits: Record<string, number> = {};
  for (const l of p.limits) {
    const n = Number(d.limits[l.counter] ?? l.limit);
    if (n !== l.limit) limits[l.counter] = n;
  }
  const tags = d.tags.map((x) => ({ name: x.name.trim(), desc: x.desc.trim() }));
  const tagsChanged =
    tags.length !== p.observation_tags.length ||
    tags.some((x, i) => x.name !== p.observation_tags[i]?.name || x.desc !== p.observation_tags[i]?.desc);
  const feedback = d.feedback !== p.flow_feedback ? d.feedback : null;
  if (Object.keys(limits).length === 0 && !tagsChanged && feedback === null) return null;
  return { limits, observation_tags: tagsChanged ? tags : null, flow_feedback: feedback };
}

/// The change to send: only what differs from the settings as they are.
export function settingsChange(s: SettingsData, d: Draft): SettingsChange {
  const models: Record<string, string> = {};
  for (const tier of s.tiers) {
    const m = (d.models[tier.name] ?? tier.model).trim();
    if (m !== tier.model) models[tier.name] = m;
  }
  const slots: Record<string, number> = {};
  for (const h of s.hosts) {
    const n = Number(d.slots[h.name] ?? h.slots);
    if (n !== h.slots) slots[h.name] = n;
  }
  const projects: Record<string, ProjectChange> = {};
  for (const p of s.projects) {
    const c = projectChange(p, d.projects[p.name]);
    if (c !== null) projects[p.name] = c;
  }
  const language = d.language.trim();
  return {
    models,
    highest_level: d.highest !== (s.highest_level ?? '') && d.highest !== '' ? d.highest : null,
    architect: d.architect !== s.architect ? d.architect : null,
    context_limit: Number(d.limit) !== s.context_limit ? Number(d.limit) : null,
    language: language !== s.language ? language : null,
    slots,
    projects,
  };
}

/// Whether a change changes nothing.
export function unchanged(c: SettingsChange): boolean {
  return (
    Object.keys(c.models).length === 0 &&
    c.highest_level === null &&
    c.architect === null &&
    c.context_limit === null &&
    c.language === null &&
    Object.keys(c.slots).length === 0 &&
    Object.keys(c.projects).length === 0
  );
}

/// A word of a command line as a shell reads it: as it is when it holds nothing the shell takes apart, else quoted.
export function shellWord(w: string): string {
  return /^[\w@%+=:,./-]+$/.test(w) ? w : `'${w.replaceAll("'", `'\\''`)}'`;
}

/// What a request said: that it was done, or why not.
type Said = { ok: boolean; text: string } | null;

const failed = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// A part of a section: its sub-heading over what it holds.
function Part({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="settings-part">
      <SubHeading>{title}</SubHeading>
      {children}
    </div>
  );
}

/// Codes one after another, as a list of paths or commands is written in a sentence of the page; none, the word.
function Codes({ list }: { list: string[] }) {
  if (list.length === 0) return <>{t('settings_none')}</>;
  return (
    <>
      {list.map((x, i) => (
        <span key={i}>
          {i > 0 ? ', ' : null}
          <Name>{x}</Name>
        </span>
      ))}
    </>
  );
}

function Install({ s, d, set }: { s: SettingsData; d: Draft; set: (d: Draft) => void }) {
  const limitBad = !whole(d.limit, 50_000, 1_000_000);
  return (
    <>
      <FormControl>
        <FormControl.Label>{t('settings_language')}</FormControl.Label>
        <TextInput value={d.language} onChange={(e) => set({ ...d, language: e.target.value })} />
      </FormControl>
      <Part title={t('settings_tiers')}>
        <p className="settings-note">{t('settings_advisor_note')}</p>
        <table className="list settings-tiers">
          <thead>
            <tr>
              <th>{t('settings_col_tier')}</th>
              <th>{t('settings_col_model')}</th>
              <th>{t('settings_col_for')}</th>
            </tr>
          </thead>
          <tbody>
            {s.tiers.map((x) => (
              <tr key={x.name}>
                <td className="nowrap">{x.name}</td>
                <td>
                  <TextInput
                    block
                    size="small"
                    value={d.models[x.name] ?? ''}
                    onChange={(e) => set({ ...d, models: { ...d.models, [x.name]: e.target.value } })}
                    validationStatus={(d.models[x.name] ?? '').trim() === '' ? 'error' : undefined}
                    aria-label={t('settings_model_of', { tier: x.name })}
                  />
                </td>
                <td className="muted">{x.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="settings-fields">
          <FormControl>
            <FormControl.Label>{t('settings_highest')}</FormControl.Label>
            <Select value={d.highest} onChange={(e) => set({ ...d, highest: e.target.value })}>
              {d.highest === '' ? <Select.Option value="">{t('settings_any_tier')}</Select.Option> : null}
              {s.tiers.map((x) => (
                <Select.Option key={x.name} value={x.name}>
                  {x.name}
                </Select.Option>
              ))}
            </Select>
          </FormControl>
          <FormControl>
            <FormControl.Label>{t('settings_architect')}</FormControl.Label>
            <Select value={d.architect} onChange={(e) => set({ ...d, architect: e.target.value })}>
              {s.tiers.map((x) => (
                <Select.Option key={x.name} value={x.name}>
                  {x.name}
                </Select.Option>
              ))}
            </Select>
          </FormControl>
        </div>
      </Part>
      <Part title={t('settings_sessions')}>
        <div className="settings-fields">
          <FormControl>
            <FormControl.Label>{t('settings_limit')}</FormControl.Label>
            <TextInput
              value={d.limit}
              onChange={(e) => set({ ...d, limit: e.target.value })}
              inputMode="numeric"
              validationStatus={limitBad ? 'error' : undefined}
            />
            {limitBad ? (
              <FormControl.Validation variant="error">{t('settings_bad_limit')}</FormControl.Validation>
            ) : (
              <FormControl.Caption>{tokens(Number(d.limit))}</FormControl.Caption>
            )}
          </FormControl>
        </div>
        <p className="settings-note">
          {t('settings_keep_warm', { after: tn('n_minutes', s.keep_warm_minutes), most: tn('n_times', s.keep_warm_most) })}
        </p>
        <p className="settings-note">{t('settings_model_note')}</p>
      </Part>
      <Part title={t('settings_machines')}>
        <div className="settings-fields">
          {s.hosts.map((h) => (
            <FormControl key={h.name}>
              <FormControl.Label>{t('settings_slots', { host: h.name })}</FormControl.Label>
              <TextInput
                value={d.slots[h.name] ?? ''}
                onChange={(e) => set({ ...d, slots: { ...d.slots, [h.name]: e.target.value } })}
                inputMode="numeric"
                validationStatus={whole(d.slots[h.name] ?? '', 0, 32) ? undefined : 'error'}
              />
            </FormControl>
          ))}
        </div>
      </Part>
      <Part title={t('settings_account')}>
        <Accounts />
      </Part>
      <Part title={t('settings_file_only', { file: 'install.toml' })}>
        <Overview
          rows={[
            { label: t('settings_gh'), value: <Codes list={s.gh_allow} /> },
            ...s.clis.map((c) => ({
              label: t('settings_cli_args', { cli: c.name }),
              value: <Name>{[c.command, ...c.args].map(shellWord).join(' ')}</Name>,
            })),
            {
              label: t('settings_query'),
              value: t('settings_query_value', { rows: s.query_max_rows, ms: s.query_timeout_ms }),
            },
          ]}
        />
      </Part>
    </>
  );
}

function Project({
  p,
  rules,
  d,
  set,
}: {
  p: ProjectSettings;
  rules: Record<string, string>;
  d: ProjectDraft;
  set: (d: ProjectDraft) => void;
}) {
  const setTag = (i: number, tag: Partial<Tag>) => set({ ...d, tags: d.tags.map((x, k) => (k === i ? { ...x, ...tag } : x)) });
  return (
    <>
      {p.limits.length > 0 ? (
        <Part title={t('settings_limits')}>
          <p className="settings-note">{t('settings_limits_note')}</p>
          <table className="list settings-limits">
            <thead>
              <tr>
                <th>{t('settings_col_counter')}</th>
                <th>{t('settings_col_limit')}</th>
                <th>{t('settings_col_counts')}</th>
              </tr>
            </thead>
            <tbody>
              {p.limits.map((l) => (
                <tr key={l.counter}>
                  <td className="nowrap">
                    <Name>{l.counter}</Name>
                  </td>
                  <td>
                    <TextInput
                      size="small"
                      value={d.limits[l.counter] ?? ''}
                      onChange={(e) => set({ ...d, limits: { ...d.limits, [l.counter]: e.target.value } })}
                      inputMode="numeric"
                      validationStatus={whole(d.limits[l.counter] ?? '', 1, 1_000_000) ? undefined : 'error'}
                      aria-label={t('settings_limit_of', { counter: l.counter })}
                    />
                  </td>
                  <td className="muted">
                    <MarkdownInline text={l.desc} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Part>
      ) : null}
      <Part title={t('settings_tags')}>
        <p className="settings-note">{t('settings_tags_note')}</p>
        {d.tags.length > 0 ? (
          <table className="list settings-tags">
            <thead>
              <tr>
                <th>{t('settings_tag_name')}</th>
                <th>{t('settings_tag_desc')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {d.tags.map((tag, i) => (
                <tr key={i}>
                  <td>
                    <TextInput
                      block
                      size="small"
                      value={tag.name}
                      onChange={(e) => setTag(i, { name: e.target.value })}
                      aria-label={t('settings_tag_name')}
                    />
                  </td>
                  <td>
                    <TextInput
                      block
                      size="small"
                      value={tag.desc}
                      onChange={(e) => setTag(i, { desc: e.target.value })}
                      aria-label={t('settings_tag_desc')}
                    />
                  </td>
                  <td>
                    <IconButton
                      icon={XIcon}
                      size="small"
                      variant="invisible"
                      aria-label={t('settings_tag_remove', { tag: tag.name })}
                      onClick={() => set({ ...d, tags: d.tags.filter((_, k) => k !== i) })}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
        <div>
          <Button size="small" leadingVisual={PlusIcon} onClick={() => set({ ...d, tags: [...d.tags, { name: '', desc: '' }] })}>
            {t('settings_tag_add')}
          </Button>
        </div>
      </Part>
      <FormControl>
        <Checkbox checked={d.feedback} onChange={(e) => set({ ...d, feedback: e.target.checked })} />
        <FormControl.Label>{t('settings_feedback')}</FormControl.Label>
        <FormControl.Caption>{t('settings_feedback_note')}</FormControl.Caption>
      </FormControl>
      <Part title={t('settings_file_only', { file: `projects/${p.name}.toml` })}>
        <Overview
          rows={[
            { label: t('settings_repo'), value: <Name>{p.repo}</Name> },
            { label: t('settings_checkout'), value: <Name>{p.checkout}</Name> },
            { label: t('settings_main'), value: <Name>{p.main}</Name> },
            { label: t('settings_design_paths'), value: <Codes list={p.design_paths} /> },
            { label: t('settings_test_paths'), value: <Codes list={p.test_paths} /> },
            {
              label: t('settings_design_check'),
              value: p.design_check !== null ? <Name>{p.design_check}</Name> : t('settings_none'),
            },
            { label: t('settings_ci_fast'), value: <Name>{p.ci_fast}</Name> },
            { label: t('settings_ci_full'), value: <Codes list={p.ci_full} /> },
            { label: t('settings_merge_checks'), value: <Codes list={p.merge_checks} /> },
            {
              label: t('settings_constitution'),
              value: p.constitution !== null ? <Name>{p.constitution}</Name> : t('settings_none'),
              sub: p.constitution !== null ? t('settings_constitution_where') : undefined,
            },
          ]}
        />
      </Part>
      {p.rules.map((r) => (
        <FoldPart key={r} title={<>{t('settings_rule')} <Name>{r}</Name></>} facts={t('settings_rule_facts', { name: r })}>
          <Sheet>
            <Markdown text={rules[r] ?? ''} />
          </Sheet>
        </FoldPart>
      ))}
    </>
  );
}

/// The command that signs a new account's directory in, by the name the owner gives it.
export function signInCommand(c: CliAccounts, name: string): string {
  return `${c.env}=${shellWord(`${c.dir}/${c.cli}-${name}`)} ${shellWord(c.command)}`;
}

/// Where the owner signs a new account in, by hand: the name its directory takes, and the command to run with it.
function AddAccount({ c, onDone }: { c: CliAccounts; onDone: () => void }) {
  const [name, setName] = useState('');
  const n = name.trim();
  // A directory there already that is not signed in may be signed in; one signed in is taken.
  const ok = /^[a-z0-9][a-z0-9-]*$/.test(n) && !c.accounts.some((a) => a.name === `${c.cli}-${n}` && a.email !== null);
  const command = signInCommand(c, n);
  return (
    <div className="settings-add">
      <FormControl>
        <FormControl.Label>{t('settings_add_name')}</FormControl.Label>
        <TextInput
          leadingVisual={`${c.cli}-`}
          value={name}
          onChange={(e) => setName(e.target.value)}
          validationStatus={n !== '' && !ok ? 'error' : undefined}
        />
        {n !== '' && !ok ? (
          <FormControl.Validation variant="error">{t('settings_add_bad_name')}</FormControl.Validation>
        ) : null}
      </FormControl>
      {ok ? (
        <>
          <p className="settings-note">{t('settings_add_run', { login: c.login })}</p>
          <div className="settings-command">
            <code>{command}</code>
            <IconButton
              icon={CopyIcon}
              size="small"
              variant="invisible"
              aria-label={t('settings_copy')}
              onClick={() => void navigator.clipboard.writeText(command)}
            />
          </div>
        </>
      ) : null}
      <div>
        <Button size="small" onClick={onDone}>
          {t('settings_add_done')}
        </Button>
      </div>
    </div>
  );
}

/// The login accounts of one CLI: each by the account it is signed in as, its directory's name after it, and the one
/// in use marked; another signed in is used by one click after a warning; a new one is signed in by hand.
function CliAccountList({ c }: { c: CliAccounts }) {
  const [asking, setAsking] = useState<Account | null>(null);
  // The accounts signed in when the owner began to add one; none while the owner adds none.
  const [addingFrom, setAddingFrom] = useState<string[] | null>(null);
  const [said, setSaid] = useState<Said>(null);
  const signedIn = c.accounts.filter((a) => a.email !== null).map((a) => a.name);
  // Adding is done once another account is signed in: it shows in the list.
  const adding = addingFrom !== null && signedIn.every((n) => addingFrom.includes(n));
  // While the owner signs a new account in, the accounts are read again every few seconds, so that it shows once
  // signed in.
  useEffect(() => {
    if (!adding) return;
    const every = window.setInterval(() => reread('accounts'), 3000);
    return () => window.clearInterval(every);
  }, [adding]);
  const use = (a: Account) => {
    setSaid(null);
    const choice: AccountChoice = { cli: c.cli, name: a.name };
    postJson('/api/v1/settings/accounts', choice).then(
      () => {
        setSaid({ ok: true, text: t('settings_switched') });
        reread('accounts');
      },
      (e: unknown) => setSaid({ ok: false, text: failed(e) }),
    );
  };
  return (
    <>
      <p className="settings-note">{t('settings_account_note', { cli: c.cli, dir: c.dir })}</p>
      <div className="settings-accounts">
        {c.accounts.map((a) => (
          <div key={a.name} className="settings-account">
            <span>{a.email ?? t('settings_not_signed_in')}</span>
            <Code>{a.name}</Code>
            {a.used ? (
              <StateChip tone="accent">{t('settings_in_use')}</StateChip>
            ) : a.email !== null ? (
              <Button size="small" title={t('settings_use_hint')} onClick={() => setAsking(a)}>
                {t('settings_use')}
              </Button>
            ) : null}
          </div>
        ))}
      </div>
      {adding ? (
        <AddAccount c={c} onDone={() => setAddingFrom(null)} />
      ) : (
        <div>
          <Button size="small" leadingVisual={PlusIcon} onClick={() => setAddingFrom(signedIn)}>
            {t('settings_add_account')}
          </Button>
        </div>
      )}
      {said !== null ? <Flash variant={said.ok ? 'success' : 'danger'}>{said.text}</Flash> : null}
      {asking !== null ? (
        <ConfirmationDialog
          title={t('settings_switch_title', { email: asking.email ?? asking.name })}
          confirmButtonContent={t('settings_switch_confirm')}
          confirmButtonType="primary"
          onClose={(gesture) => {
            const a = asking;
            setAsking(null);
            if (gesture === 'confirm') use(a);
          }}
        >
          {t('settings_switch_body')}
        </ConfirmationDialog>
      ) : null}
    </>
  );
}

function Accounts() {
  const r = useResource<CliAccounts[]>('accounts', '/api/v1/settings/accounts');
  return (
    <Loaded r={r}>
      {(all) => (
        <>
          {all.map((c) => (
            <CliAccountList key={c.cli} c={c} />
          ))}
        </>
      )}
    </Loaded>
  );
}

function Form({ s, said, onSaid }: { s: SettingsData; said: Said; onSaid: (said: Said) => void }) {
  const [d, setD] = useState<Draft>(() => draftOf(s));
  const problems = settingsProblems(d);
  const change = settingsChange(s, d);
  const save = () => {
    onSaid(null);
    postJson('/api/v1/settings', change).then(
      () => {
        onSaid({ ok: true, text: t('settings_saved') });
        reread('settings');
      },
      (e: unknown) => onSaid({ ok: false, text: failed(e) }),
    );
  };
  return (
    <div className="settings-form">
      <Section title={t('settings_install')} open>
        <Install s={s} d={d} set={setD} />
      </Section>
      {s.projects.map((p) => {
        const pd = d.projects[p.name];
        return pd === undefined ? null : (
          <Section key={p.name} title={t('settings_project', { name: p.name })} open>
            <Project p={p} rules={s.rules} d={pd} set={(x) => setD({ ...d, projects: { ...d.projects, [p.name]: x } })} />
          </Section>
        );
      })}
      {problems.length > 0 ? (
        <Flash variant="danger">
          {problems.map((p) => (
            <div key={p}>{p}</div>
          ))}
        </Flash>
      ) : null}
      <div className="settings-save">
        <Button variant="primary" disabled={problems.length > 0 || unchanged(change)} onClick={save}>
          {t('settings_save')}
        </Button>
      </div>
      {said !== null ? <Flash variant={said.ok ? 'success' : 'danger'}>{said.text}</Flash> : null}
    </div>
  );
}

export function Settings() {
  const r = useResource<SettingsData>('settings', '/api/v1/settings');
  // What saving said outlives the form, which starts again from the commit the save made.
  const [said, setSaid] = useState<Said>(null);
  return (
    <HeadingLevel.Provider value={2}>
      <div className="settings-page">
        <PageHeader as="header" className="settings-head">
          <PageHeader.TitleArea>
            <PageHeader.Title as="h1">{t('nav_settings')}</PageHeader.Title>
          </PageHeader.TitleArea>
          {r.data !== undefined ? (
            <PageHeader.Description className="settings-commit">
              {t('settings_commit', { commit: r.data.commit.slice(0, 7) })}
            </PageHeader.Description>
          ) : null}
        </PageHeader>
        <Loaded r={r}>{(s) => <Form key={s.commit} s={s} said={said} onSaid={setSaid} />}</Loaded>
      </div>
    </HeadingLevel.Provider>
  );
}
