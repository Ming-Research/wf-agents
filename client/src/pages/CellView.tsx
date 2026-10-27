// A cell's definition in the compiled flow: what it is, its inputs and where each comes from, its rules and prompt,
// the forms and schema of its return, and its rows.

import type { CompiledCell } from '../api';
import { Facts, Json, Section } from '../components';
import { atomText, doText, srcText, useT } from '../text';

export function CellView({ cell }: { cell: CompiledCell }) {
  const t = useT();
  const tools = cell.tools ?? [];
  const rules = cell.rules ?? [];
  const prompt = cell.prompt ?? [];
  const forms = cell.forms ?? [];
  const always = cell.always ?? [];
  const checks = cell.checks ?? [];
  const handoff = cell.handoff ?? [];
  const inenums = Object.entries(cell.inenums ?? {});
  return (
    <div className="cell-view">
      <h3>
        <span className="cell-id">{cell.id}</span> {cell.title}
      </h3>
      <Facts
        rows={[
          [t('cell_kind'), `${cell.kind}${cell.gate ? ` · ${t('cell_gate')}` : ''}`],
          cell.line != null && [t('cell_line'), cell.line],
          cell.stage != null && [t('cell_stage'), cell.stage],
          cell.step != null && [t('cell_step'), cell.step],
          cell.needs != null && [t('cell_needs'), cell.needs],
          tools.length > 0 && [t('cell_tools'), tools.join(', ')],
        ]}
      />
      <Section title={`${t('cell_inputs')} (${cell.inputs.length})`}>
        <table className="list defs">
          <tbody>
            {cell.inputs.map((x, i) => (
              <tr key={i}>
                <td>
                  {x.id != null ? <code>@{x.id} </code> : null}
                  {x.what}
                  {x.maybe ? <span className="muted"> · {t('maybe')}</span> : null}
                </td>
                <td className="muted">{x.from.map(srcText).join('; ')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>
      <Section title={`${t('cell_rows')} (${cell.rows.length})`}>
        <table className="list defs">
          <thead>
            <tr>
              <th>{t('row_name')}</th>
              <th>{t('row_if')}</th>
              <th>{t('row_to')}</th>
              <th>{t('row_do')}</th>
            </tr>
          </thead>
          <tbody>
            {cell.rows.map((r) => (
              <tr key={r.name} className={r.common ? 'row-common' : ''}>
                <td>
                  <strong>{r.name}</strong>
                  <div className="flags">
                    {r.main ? <span className="flag flag-main">{t('flag_main')}</span> : null}
                    {r.waits ? <span className="flag">{t('flag_waits')}</span> : null}
                    {r.interject ? <span className="flag">{t('flag_interject')}</span> : null}
                    {r.common ? <span className="flag">{t('flag_common')}</span> : null}
                    {r.counts != null ? (
                      <span className="flag">
                        {t('row_counts')} {r.counts}
                      </span>
                    ) : null}
                  </div>
                </td>
                <td>
                  <code>{r.if.map(atomText).join(' and ')}</code>
                </td>
                <td>
                  <span className="cell-id">{r.to}</span>
                </td>
                <td>
                  {(r.do ?? []).map(doText).join(', ')}
                  {Object.entries(r.sets ?? {}).map(([k, v]) => (
                    <div key={k} className="muted">
                      {t('row_sets')} {k} = {v}
                    </div>
                  ))}
                  {r.note != null ? <div className="muted">{r.note}</div> : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>
      {rules.length > 0 ? (
        <Section title={t('cell_rules')} open={false}>
          <ul>
            {rules.map((r) => (
              <li key={r.name}>{r.name}</li>
            ))}
          </ul>
        </Section>
      ) : null}
      {prompt.length > 0 || cell.prompt_by != null ? (
        <Section title={t('cell_prompt')} open={false}>
          {prompt.map((p, i) => (
            <p key={i} className="prompt-line">
              {p}
            </p>
          ))}
          {cell.prompt_by != null ? <Json value={cell.prompt_by} /> : null}
        </Section>
      ) : null}
      {forms.length > 0 ? (
        <Section title={t('cell_forms')} open={false}>
          <ul>
            {forms.map((f, i) => (
              <li key={i}>
                <code>{f.field ?? 'own'}</code> {f.say}
              </li>
            ))}
          </ul>
        </Section>
      ) : null}
      {cell.schema != null ? (
        <Section title={t('cell_schema')} open={false}>
          <Json value={cell.schema} />
        </Section>
      ) : null}
      {always.length > 0 || checks.length > 0 || handoff.length > 0 ? (
        <Section title={`${t('cell_always')} · ${t('cell_checks')} · ${t('cell_handoff')}`} open={false}>
          <ul>
            {always.map((a, i) => (
              <li key={`a${i}`}>
                {t('cell_always')}: {a.at} → {a.view ?? a.do}
              </li>
            ))}
            {checks.map((c, i) => (
              <li key={`c${i}`}>
                {t('cell_checks')}: {c.check}
                {c.at != null ? ` @ ${c.at}` : ''}
              </li>
            ))}
            {handoff.map((h) => (
              <li key={`h${h}`}>
                {t('cell_handoff')}: {h}
              </li>
            ))}
          </ul>
        </Section>
      ) : null}
      {inenums.length > 0 ? (
        <Section title={t('cell_inenums')} open={false}>
          {inenums.map(([name, values]) => (
            <div key={name}>
              <code>{name}</code>
              <ul>
                {values.map(([v, means]) => (
                  <li key={v}>
                    <code>{v}</code> {means}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </Section>
      ) : null}
    </div>
  );
}
