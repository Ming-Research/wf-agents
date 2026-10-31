// The observations: what each project's architect recorded on the project's behalf, a problem, an idea or feedback
// not yet worth an item, the project's pool of what is not an item yet. A view of its own beside the items, since a
// project's observations far outnumber its items. On the left, each project's under its name, as the list of
// conversations groups its rows, with the buttons that show or hide those with each of the project's tags and, off at
// first, those handled (taken in by an item or closed), then a row for each, which opens it; on the right, the one
// open, in full, where the owner closes it, saying why. While none is open the list fills the view; the split is the
// one the items take (`ListSplit`), for one look and one code.

import { XIcon } from '@primer/octicons-react';
import { IconButton, NavList, PageHeader } from '@primer/react';
import { Fragment, type MouseEvent, useState } from 'react';

import type { Items as ItemsData, Observation, ProjectObservations } from '../api';
import { Loaded, StateChip, StateFilter, TagChip, useShownStates } from '../components';
import { time } from '../format';
import { postJson } from '../http';
import { words } from '../labelled';
import { reread, useResource, useShared } from '../live';
import { firstWords, Markdown, MarkdownInline, MarkdownLine } from '../markdown';
import { handled, NO_TAG, rowLine, shownObservations, tagCounts } from '../observations';
import { Fold, HeadingLevel, ItemRef, ListSplit, SendForm, SubHeading } from '../parts';
import { Link, navigate, observationsHref, type ObservationsRoute } from '../route';
import { t, tn } from '../text';
import { factsAfter, joinedWords } from '../voices';

/// The one choice of the handled button, for every project, kept in this browser: off until the owner turns it on.
const HANDLED = ['handled'] as const;

/// An observation's facts, as words: how many times it was reported, when it was recorded, and where it came from.
function facts(o: Observation): Array<string | null> {
  return [
    tn('observation_reported', o.times_reported),
    o.created !== null ? t('observation_at', { at: time(o.created) }) : null,
    o.source.trim() !== '' ? o.source : null,
  ];
}

/// How an observation was handled, as chips: the item that took it in, and closed, in a word, why on hovering.
function HandledChips({ o }: { o: Observation }) {
  return (
    <>
      {o.folded_into !== null ? <StateChip tone="neutral">{t('observation_folded', { item: o.folded_into })}</StateChip> : null}
      {o.closed !== null ? (
        <StateChip tone="neutral" title={o.closed_why ?? undefined}>
          {t('observation_closed', { why: words(o.closed) })}
        </StateChip>
      ) : null}
    </>
  );
}

/// One observation's row: its title, or for one recorded before observations had titles the words it starts with, cut
/// to one line and whole on hovering; under them its tags, how it was handled, and its facts, cut to one line and whole
/// on hovering. A click opens it beside the list.
function Row({ o, project, open }: { o: Observation; project: string | null; open: number | null }) {
  const to = observationsHref(project, o.id);
  const said = joinedWords(facts(o));
  const line = rowLine(o);
  return (
    <NavList.Item
      href={to}
      aria-current={o.id === open ? 'page' : undefined}
      className={handled(o) ? 'obs-row obs-handled' : 'obs-row'}
      onClick={(e: MouseEvent) => {
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
        e.preventDefault();
        navigate(to);
      }}
    >
      <span className="obs-what" title={firstWords(line)}>
        <MarkdownLine text={line} />
      </span>
      <NavList.Description variant="block" className="obs-facts">
        <span className="obs-facts-line" title={said}>
          {o.tags.map((x) => (
            <TagChip key={x}>{x}</TagChip>
          ))}
          <HandledChips o={o} />
          <span>{said}</span>
        </span>
      </NavList.Description>
    </NavList.Item>
  );
}

/// One project's observations: its name, the buttons that show or hide those with each of its tags, in its order, and
/// those carrying none of them, each with how many there are among those shown (`StateFilter`, the part the items' tree
/// and the list of conversations show or hide their rows with), and the button of those handled, off at first; then
/// its rows. The owner's choice of tags is one for every project, as the items' states are, and kept in this browser.
function Project({ p, project, open, hidden, onTag, withHandled, onHandled }: {
  p: ProjectObservations;
  /// The project the address names, which the rows' addresses keep.
  project: string | null;
  open: number | null;
  hidden: readonly string[];
  onTag: (tag: string) => void;
  withHandled: boolean;
  onHandled: () => void;
}) {
  const tags = p.tags.map((x) => x.name);
  const order = [...tags, NO_TAG];
  const covers = new Map(p.tags.map((x) => [x.name, x.desc]));
  const counts = tagCounts(p.observations, tags, withHandled);
  const rows = shownObservations(p.observations, tags, hidden, withHandled, open);
  return (
    <NavList.Group title={p.project}>
      <li className="obs-filters">
        <StateFilter
          order={order}
          states={counts.tags}
          shown={order.filter((x) => !hidden.includes(x))}
          tone="neutral"
          words={(x) => (x === NO_TAG ? t('observation_no_tag') : x)}
          title={(x, n, on) =>
            x === NO_TAG
              ? tn(on ? 'observations_hide_untagged' : 'observations_show_untagged', n)
              : `${tn(on ? 'observations_hide_tag' : 'observations_show_tag', n, { tag: x })} ${covers.get(x) ?? ''}`.trim()
          }
          onShown={onTag}
        />
        <StateFilter
          order={HANDLED}
          states={Array.from({ length: counts.handled }, () => 'handled' as const)}
          shown={withHandled ? HANDLED : []}
          tone="neutral"
          words={() => t('observation_handled')}
          title={(_, n, on) => tn(on ? 'observations_hide_handled' : 'observations_show_handled', n)}
          onShown={onHandled}
        />
      </li>
      {rows.length === 0 ? <li className="obs-none muted">{t('observations_none_shown')}</li> : null}
      {rows.map((o) => (
        <Row key={o.id} o={o} project={project} open={open} />
      ))}
    </NavList.Group>
  );
}

/// Every project's observations, or the one project's the address names, with the way back to every project's.
function List({ all, route }: { all: ProjectObservations[]; route: ObservationsRoute }) {
  const names = [...new Set(all.flatMap((p) => [...p.tags.map((x) => x.name), ...p.observations.flatMap((o) => o.tags)]))];
  const [hidden, toggleTag] = useShownStates('observations-hidden-tags', [...names, NO_TAG], []);
  const [withHandled, toggleHandled] = useShownStates('observations-handled', HANDLED, []);
  const projects = route.project !== null ? all.filter((p) => p.project === route.project) : all;
  return (
    <>
      {route.project !== null ? (
        <p className="obs-only muted">
          {t('observations_only', { project: route.project })} <Link to={observationsHref()}>{t('observations_every')}</Link>
        </p>
      ) : null}
      {projects.length === 0 ? <p className="obs-none muted">{t('observations_none')}</p> : null}
      {projects.length > 0 ? (
        <NavList aria-label={t('nav_observations')} className="obs-list">
          {projects.map((p) => (
            <Project
              key={p.project}
              p={p}
              project={route.project}
              open={route.open}
              hidden={hidden}
              onTag={toggleTag}
              withHandled={withHandled.length > 0}
              onHandled={() => toggleHandled('handled')}
            />
          ))}
        </NavList>
      ) : null}
    </>
  );
}

/// The owner's close of an observation still open, folded at first: the words saying why, which it cannot be closed
/// without, and the button that closes it.
function Close({ o }: { o: Observation }) {
  const [why, setWhy] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    setBusy(true);
    setError(null);
    postJson(`/api/v1/observations/${o.id}/close`, { why: why.trim() }).then(
      () => {
        setBusy(false);
        setWhy('');
        reread('observations');
      },
      (x: unknown) => {
        setBusy(false);
        setError(x instanceof Error ? x.message : String(x));
      },
    );
  };
  return (
    <Fold summary={t('observation_close_fold')} className="obs-close">
      <SendForm
        words={{ text: why, set: setWhy, placeholder: t('observation_close_why'), rows: 2 }}
        label={t('observation_close')}
        ready={why.trim() !== ''}
        busy={busy}
        why={t('observation_close_needs_why')}
        onSend={close}
        error={error}
      />
    </Fold>
  );
}

/// The observation open, as a reading page: its number, with its project, its tags, how it was handled and its facts
/// on the title's line, and the way back to the list alone; what it is, whole, as its architect wrote it; where it
/// came from and what it refers to; the item that took it in, leading to the item; why it was closed; and, while it is
/// open, the owner's close.
function Open({ o, project, route }: { o: Observation; project: string; route: ObservationsRoute }) {
  const items = useShared<ItemsData>('items', '/api/v1/items');
  const item = o.folded_into !== null ? items?.items.find((i) => i.id === o.folded_into) : undefined;
  const head = factsAfter([
    project,
    o.tags.length > 0 ? (
      <span className="obs-head-tags">
        {o.tags.map((x) => (
          <TagChip key={x}>{x}</TagChip>
        ))}
      </span>
    ) : null,
    tn('observation_reported', o.times_reported),
    o.created !== null ? t('observation_at', { at: time(o.created) }) : null,
  ]);
  return (
    <div className="page obs-page">
      <HeadingLevel.Provider value={2}>
        <PageHeader as="header" className="item-head">
          <PageHeader.TitleArea className="item-head-title">
            <PageHeader.Title as="h1">{t('observation_head', { id: o.id })}</PageHeader.Title>
            <span className="item-head-facts muted">
              {head.map((x, k) => (
                <Fragment key={k}>{x}</Fragment>
              ))}
            </span>
          </PageHeader.TitleArea>
          <PageHeader.Actions>
            <IconButton
              icon={XIcon}
              size="small"
              variant="invisible"
              aria-label={t('observation_hide')}
              tooltipDirection="w"
              onClick={() => navigate(observationsHref(route.project))}
            />
          </PageHeader.Actions>
        </PageHeader>
        <div className="obs-body">
          <Markdown text={o.what} />
          {o.source.trim() !== '' ? (
            <div className="field">
              <SubHeading>{t('observation_from')}</SubHeading>
              <Markdown text={o.source} />
            </div>
          ) : null}
          {o.refs.length > 0 ? (
            <div className="field">
              <SubHeading>{t('observation_refs')}</SubHeading>
              <ul className="md-list">
                {o.refs.map((r, k) => (
                  <li key={k}>
                    <MarkdownInline text={r} />
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {o.folded_into !== null ? (
            <div className="field">
              <SubHeading>{t('observation_taken_in')}</SubHeading>
              <p className="obs-item">
                <ItemRef id={o.folded_into} title={item?.title} />
              </p>
            </div>
          ) : null}
          {o.closed !== null ? (
            <div className="field">
              <SubHeading>{t('observation_closed_why', { why: words(o.closed) })}</SubHeading>
              {o.closed_why !== null ? <Markdown text={o.closed_why} /> : null}
            </div>
          ) : null}
          {o.closed === null ? <Close key={o.id} o={o} /> : null}
        </div>
      </HeadingLevel.Provider>
    </div>
  );
}

export function Observations({ route }: { route: ObservationsRoute }) {
  const all = useResource<ProjectObservations[]>('observations', '/api/v1/observations');
  const found =
    route.open !== null && all.data !== undefined
      ? all.data.flatMap((p) => p.observations.map((o) => ({ o, project: p.project }))).find((x) => x.o.id === route.open)
      : undefined;
  const open =
    route.open === null || all.data === undefined ? null : found !== undefined ? (
      <Open o={found.o} project={found.project} route={route} />
    ) : (
      <div className="page obs-page">
        <p className="muted">{t('observation_unknown', { id: route.open })}</p>
      </div>
    );
  return (
    <ListSplit
      label={t('nav_observations')}
      widthKey="wf-agents.observations-pane"
      list={<Loaded r={all}>{(data) => <List all={data} route={route} />}</Loaded>}
      open={open}
    />
  );
}
