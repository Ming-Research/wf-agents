// The settings the owner keeps on the page: each tier's model, the highest tier an item may be at, the architect's
// tier, and the session manager's size limit; saving writes the installation's file in the data directory and commits
// it, and the engine takes it from its next round. A value the configuration would refuse is refused before anything
// is written: here what the page can tell, and on saving what only the configuration can. Drawn as a reading page of
// the UI spec: its header, a section for the tiers and one for the sessions, each field a Primer form control, and the
// one Save the form exists for.

import { Button, Flash, FormControl, PageHeader, Select, TextInput } from '@primer/react';
import { useState } from 'react';

import type { Settings as SettingsData, SettingsChange } from '../api';
import { Loaded } from '../components';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { tokens } from '../round';
import { t, tn } from '../text';

/// What the page refuses before anything is sent: a tier with no model, a size limit that is not a number of tokens.
export function settingsProblems(models: Record<string, string>, limit: string): string[] {
  const out: string[] = [];
  for (const [tier, model] of Object.entries(models)) {
    if (model.trim() === '') out.push(t('settings_no_model', { tier }));
  }
  if (!/^\d+$/.test(limit.trim()) || Number(limit) <= 0) out.push(t('settings_bad_limit'));
  return out;
}

/// The change to send: only what differs from the settings as they are.
export function settingsChange(
  s: SettingsData,
  models: Record<string, string>,
  highest: string,
  architect: string,
  limit: string,
): SettingsChange {
  const changed: Record<string, string> = {};
  for (const tier of s.tiers) {
    const m = (models[tier.name] ?? tier.model).trim();
    if (m !== tier.model) changed[tier.name] = m;
  }
  return {
    models: changed,
    highest_level: highest !== (s.highest_level ?? '') && highest !== '' ? highest : null,
    architect: architect !== s.architect ? architect : null,
    context_limit: Number(limit) !== s.context_limit ? Number(limit) : null,
  };
}

/// What saving said: that it was saved, or why it was not.
type Said = { ok: boolean; text: string } | null;

function Form({ s, said, onSaid }: { s: SettingsData; said: Said; onSaid: (said: Said) => void }) {
  const [models, setModels] = useState<Record<string, string>>(Object.fromEntries(s.tiers.map((x) => [x.name, x.model])));
  const [highest, setHighest] = useState(s.highest_level ?? '');
  const [architect, setArchitect] = useState(s.architect);
  const [limit, setLimit] = useState(String(s.context_limit));
  const problems = settingsProblems(models, limit);
  const limitBad = !/^\d+$/.test(limit.trim()) || Number(limit) <= 0;
  const save = () => {
    onSaid(null);
    postJson('/api/v1/settings', settingsChange(s, models, highest, architect, limit)).then(
      () => {
        onSaid({ ok: true, text: t('settings_saved') });
        reread('settings');
      },
      (e: unknown) => onSaid({ ok: false, text: e instanceof Error ? e.message : String(e) }),
    );
  };
  return (
    <div className="settings-form">
      <section className="settings-sec">
        <h2>{t('settings_tiers')}</h2>
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
                    value={models[x.name] ?? ''}
                    onChange={(e) => setModels({ ...models, [x.name]: e.target.value })}
                    validationStatus={(models[x.name] ?? '').trim() === '' ? 'error' : undefined}
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
            <Select value={highest} onChange={(e) => setHighest(e.target.value)}>
              {highest === '' ? <Select.Option value="">{t('settings_any_tier')}</Select.Option> : null}
              {s.tiers.map((x) => (
                <Select.Option key={x.name} value={x.name}>
                  {x.name}
                </Select.Option>
              ))}
            </Select>
          </FormControl>
          <FormControl>
            <FormControl.Label>{t('settings_architect')}</FormControl.Label>
            <Select value={architect} onChange={(e) => setArchitect(e.target.value)}>
              {s.tiers.map((x) => (
                <Select.Option key={x.name} value={x.name}>
                  {x.name}
                </Select.Option>
              ))}
            </Select>
          </FormControl>
        </div>
      </section>
      <section className="settings-sec">
        <h2>{t('settings_sessions')}</h2>
        <div className="settings-fields">
          <FormControl>
            <FormControl.Label>{t('settings_limit')}</FormControl.Label>
            <TextInput
              value={limit}
              onChange={(e) => setLimit(e.target.value)}
              inputMode="numeric"
              validationStatus={limitBad ? 'error' : undefined}
            />
            {limitBad ? (
              <FormControl.Validation variant="error">{t('settings_bad_limit')}</FormControl.Validation>
            ) : (
              <FormControl.Caption>{tokens(Number(limit))}</FormControl.Caption>
            )}
          </FormControl>
        </div>
        <p className="settings-note">{t('settings_keep_warm', { after: tn('n_minutes', s.keep_warm_minutes), most: tn('n_times', s.keep_warm_most) })}</p>
        <p className="settings-note">{t('settings_model_note')}</p>
      </section>
      {/* The size limit says what is wrong with it under its field; a tier with no model says it here. */}
      {problems.some((p) => p !== t('settings_bad_limit')) ? (
        <Flash variant="danger">
          {problems
            .filter((p) => p !== t('settings_bad_limit'))
            .map((p) => (
              <div key={p}>{p}</div>
            ))}
        </Flash>
      ) : null}
      <div className="settings-save">
        <Button variant="primary" disabled={problems.length > 0} onClick={save}>
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
  );
}
