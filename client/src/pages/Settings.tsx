// The settings the owner keeps on the page: each tier's model, the highest tier an item may be at, the architect's
// tier, and the session manager's size limit; saving writes the installation's file in the data directory and commits
// it, and the engine takes it from its next round. A value the configuration would refuse is refused before anything
// is written: here what the page can tell, and on saving what only the configuration can.

import { useState } from 'react';

import type { Settings as SettingsData, SettingsChange } from '../api';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { tokens } from '../round';
import { t } from '../text';

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

function Form({ s }: { s: SettingsData }) {
  const [models, setModels] = useState<Record<string, string>>(Object.fromEntries(s.tiers.map((x) => [x.name, x.model])));
  const [highest, setHighest] = useState(s.highest_level ?? '');
  const [architect, setArchitect] = useState(s.architect);
  const [limit, setLimit] = useState(String(s.context_limit));
  const [said, setSaid] = useState<{ ok: boolean; text: string } | null>(null);
  const problems = settingsProblems(models, limit);
  const save = () => {
    postJson('/api/v1/settings', settingsChange(s, models, highest, architect, limit)).then(
      () => {
        setSaid({ ok: true, text: t('settings_saved') });
        reread('settings');
      },
      (e: unknown) => setSaid({ ok: false, text: e instanceof Error ? e.message : String(e) }),
    );
  };
  return (
    <div className="settings">
      <section className="us-sec">
        <h4>{t('settings_tiers')}</h4>
        <p className="us-note">{t('settings_advisor_note')}</p>
        <table className="us-table">
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
                <td>
                  <b>{x.name}</b>
                </td>
                <td>
                  <input
                    value={models[x.name] ?? ''}
                    onChange={(e) => setModels({ ...models, [x.name]: e.target.value })}
                    aria-label={t('settings_col_model')}
                  />
                </td>
                <td className="muted">{x.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <label className="settings-field">
          {t('settings_highest')}{' '}
          <select value={highest} onChange={(e) => setHighest(e.target.value)}>
            {highest === '' ? <option value="">{t('settings_any_tier')}</option> : null}
            {s.tiers.map((x) => (
              <option key={x.name} value={x.name}>
                {x.name}
              </option>
            ))}
          </select>
        </label>
        <label className="settings-field">
          {t('settings_architect')}{' '}
          <select value={architect} onChange={(e) => setArchitect(e.target.value)}>
            {s.tiers.map((x) => (
              <option key={x.name} value={x.name}>
                {x.name}
              </option>
            ))}
          </select>
        </label>
      </section>
      <section className="us-sec">
        <h4>{t('settings_sessions')}</h4>
        <label className="settings-field">
          {t('settings_limit')} <input value={limit} onChange={(e) => setLimit(e.target.value)} inputMode="numeric" />{' '}
          <span className="muted">{/^\d+$/.test(limit) ? tokens(Number(limit)) : ''}</span>
        </label>
        <p className="us-note">{t('settings_keep_warm', { minutes: s.keep_warm_minutes, most: s.keep_warm_most })}</p>
        <p className="us-note">{t('settings_model_note')}</p>
      </section>
      {problems.map((p) => (
        <p key={p} className="bad">
          {p}
        </p>
      ))}
      <button type="button" disabled={problems.length > 0} onClick={save}>
        {t('settings_save')}
      </button>{' '}
      {said !== null ? <span className={said.ok ? 'muted' : 'bad'}>{said.text}</span> : null}
      <p className="us-note">{t('settings_commit', { commit: s.commit.slice(0, 7) })}</p>
    </div>
  );
}

export function Settings() {
  const r = useResource<SettingsData>('settings', '/api/v1/settings');
  if (r.error !== null) return <p className="bad page">{r.error}</p>;
  if (r.data === undefined) return null;
  return (
    <div className="page us">
      <Form key={r.data.commit} s={r.data} />
    </div>
  );
}
