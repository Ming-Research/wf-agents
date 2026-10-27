// A round in full, for diagnosis: for an agent's round, each material of its message by address, given or referred
// to, the message exactly as sent and how the command line was called; for another step, each input with where it
// came from; the return, the row it took or why it was sent back, the tools it ran with the files they kept, and what
// it cost.

import { useEffect, useRef, useState } from 'react';

import type { RoundDetail, ToolRun } from '../api';
import { Facts, Json, Loaded, OutcomeBadge, Section } from '../components';
import { cost, duration, time } from '../format';
import { getText } from '../http';
import { useResource } from '../live';
import { srcText, useT } from '../text';

/// A file a tool kept, read when asked for.
function KeptFile({ file, path }: { file: string; path: string }) {
  const [text, setText] = useState<string | null>(null);
  const open = () => {
    if (text !== null) {
      setText(null);
      return;
    }
    const [dir, ...name] = file.split('/');
    getText(`/api/v1/files/${encodeURIComponent(dir)}/${encodeURIComponent(name.join('/'))}`).then(setText, (e: unknown) =>
      setText(String(e)),
    );
  };
  return (
    <div className="kept">
      <button className="link" onClick={open} title={path}>
        {file}
      </button>
      {text !== null ? <pre className="j-text file-text">{text}</pre> : null}
    </div>
  );
}

function Tool({ run }: { run: ToolRun }) {
  const t = useT();
  return (
    <div className="card-box">
      <div>
        <strong>{run.tool}</strong> <span className="muted">{t('attempt', { n: run.attempt })}</span>{' '}
        <span className="muted">
          {time(run.started_at ?? run.queued_at)}
          {run.ended_at !== null && run.started_at !== null ? ` · ${duration(run.ended_at - run.started_at)}` : ''}
        </span>
      </div>
      <Facts
        rows={[
          [t('args'), <Json key="a" value={run.args} />],
          [t('result'), <Json key="r" value={run.result} />],
          run.files.length > 0 && [
            t('files'),
            <div key="f">
              {run.files.map((f) => (
                <KeptFile key={f.file} file={f.file} path={f.path} />
              ))}
            </div>,
          ],
        ]}
      />
    </div>
  );
}

function Detail({ d, onClose }: { d: RoundDetail; onClose: () => void }) {
  const t = useT();
  const r = d.round;
  const top = useRef<HTMLDivElement>(null);
  // A round picked further down the timeline is shown above it: bring it into view.
  useEffect(() => {
    top.current?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }, [r.id]);
  return (
    <div className="round" ref={top}>
      <div className="round-head">
        <h2>
          {t('round_title', { id: r.id })} <span className="cell-id">{r.cell ?? '–'}</span>{' '}
          <OutcomeBadge outcome={r.outcome} />
        </h2>
        <button className="link" onClick={onClose}>
          {t('close')}
        </button>
      </div>
      <Facts
        rows={[
          [t('round_line'), r.line],
          r.transition !== null && [t('round_transition'), <strong key="t">{r.transition}</strong>],
          r.why !== null && [t('round_why'), <pre key="w" className="j-text why">{r.why}</pre>],
          [t('round_started'), time(r.started_at)],
          [t('round_ended'), time(r.ended_at)],
          r.ended_at !== null && [t('round_duration'), duration(r.ended_at - r.started_at)],
          [t('round_model'), r.model],
          [t('round_cost'), cost(r.cost_usd)],
          r.compactions > 0 && [t('round_compactions'), r.compactions],
          d.session !== null && [t('round_session'), `${d.session.line} · ${d.session.cli} · ${d.session.cli_session}`],
          [t('round_config'), <code key="c">{d.config_commit}</code>],
        ]}
      />
      {d.materials.length > 0 ? (
        <Section title={`${t('materials')} (${d.materials.length})`}>
          <table className="list defs">
            <tbody>
              {d.materials.map((m) => (
                <tr key={m.address}>
                  <td>
                    <code>{m.address}</code>
                  </td>
                  <td>{m.given ? t('material_given') : <span className="muted">{t('material_referred')}</span>}</td>
                  <td className="muted" title={m.hash}>
                    <code>{m.hash.slice(0, 12)}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      ) : null}
      {d.inputs.length > 0 ? (
        <Section title={`${t('inputs')} (${d.inputs.length})`}>
          {d.inputs.map((x, i) => (
            <details key={i} className="input">
              <summary>
                <span className="input-what">{x.what}</span>
              </summary>
              {x.from.length > 0 ? (
                <div className="muted">
                  {t('input_from')}: {x.from.map(srcText).join('; ')}
                </div>
              ) : null}
              <Json value={x.value} />
            </details>
          ))}
        </Section>
      ) : null}
      <Section title={t('message')} open={false}>
        {d.message !== null ? (
          <pre className="j-text prompt">{d.message}</pre>
        ) : (
          <p className="muted">{t('no_message')}</p>
        )}
      </Section>
      {d.call !== null ? (
        <Section title={t('call')} open={false}>
          <Json value={d.call} />
        </Section>
      ) : null}
      {d.output !== null ? (
        <Section title={t('output')}>
          <Json value={d.output} />
        </Section>
      ) : null}
      {d.rejected !== null ? (
        <Section title={t('rejected')}>
          <Json value={d.rejected} />
        </Section>
      ) : null}
      {d.tool_runs.length > 0 ? (
        <Section title={`${t('tool_runs')} (${d.tool_runs.length})`}>
          {d.tool_runs.map((run) => (
            <Tool key={run.id} run={run} />
          ))}
        </Section>
      ) : null}
      {d.interjections.length > 0 ? (
        <Section title={t('interjections')}>
          {d.interjections.map((x) => (
            <div key={x.id} className="card-box">
              <div className="muted">
                {x.speaker} → {x.line}
              </div>
              <pre className="j-text">{x.text}</pre>
            </div>
          ))}
        </Section>
      ) : null}
      {d.cards.length > 0 ? (
        <Section title={t('cards')}>
          {d.cards.map((c) => (
            <div key={c.id} className="card-box">
              <div>
                #{c.id} {c.kind} · {c.status}
              </div>
              <Json value={c.body} />
            </div>
          ))}
        </Section>
      ) : null}
      {d.usage !== null ? (
        <Section title={t('usage')} open={false}>
          <Json value={d.usage} />
        </Section>
      ) : null}
      <Section title={t('engine_input')} open={false}>
        <Json value={d.input} />
      </Section>
    </div>
  );
}

export function RoundView({ id, onClose }: { id: number; onClose: () => void }) {
  const round = useResource<RoundDetail>(`round:${id}`, `/api/v1/rounds/${id}`);
  return <Loaded r={round}>{(d) => <Detail d={d} onClose={onClose} />}</Loaded>;
}
