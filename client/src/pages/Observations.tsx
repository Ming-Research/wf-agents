// The observations: what each project's architect recorded on the project's behalf, a problem, an idea or feedback
// not yet worth an item, the project's pool of what is not an item yet. A view of its own beside the items, since a
// project's observations far outnumber its items. On the left, each project's under its name, as the list of
// conversations groups its rows, with the buttons that pick them by the project's tags and by state (open or
// handled), nothing picked at first so that every observation shows, and the clear of every pick while something is
// picked; then a row for each, which opens it; on the right, the one open, in full, where the owner closes it, saying
// why. While none is open the list fills the view; the split is the one the items take (`ListSplit`), for one look and
// one code.

import { XIcon } from '@primer/octicons-react';
import { IconButton, NavList, PageHeader } from '@primer/react';
import { Fragment, type MouseEvent, useState } from 'react';

import type { Items as ItemsData, Observation, ProjectObservations } from '../api';
import { ClearPicks, Loaded, StateChip, StateFilter, TagChip, useShownStates } from '../components';
import { time } from '../format';
import { postJson } from '../http';
import { words } from '../labelled';
import { reread, useResource, useShared } from '../live';
import { firstWords, Markdown, MarkdownInline, MarkdownLine } from '../markdown';
import {
  filters,
  handled,
  NO_TAG,
  OBSERVATION_STATES,
  type ObservationState,
  type Picks,
  rowLine,
  shownObservations,
} from '../observations';
import { Fold, HeadingLevel, ItemRef, ListSplit, SendForm, SubHeading } from '../parts';
import { Link, navigate, observationsHref, type ObservationsRoute } from '../route';
import { t, tn } from '../text';
import { factsAfter, joinedWords } from '../voices';

/// What a click on a button over the list does, by whether it is picked and whether its group has another pick: take
/// it back, add it to the group's picks, or show only what it matches.
function clickWords(on: boolean, others: boolean): string {
  if (on) return t('observations_unpick');
  return t(others ? 'observations_pick_too' : 'observations_pick_only');
}

/// What a state's button says its count is.
const stateSaid = (s: ObservationState, n: number): string =>
  s === 'open' ? tn('observations_open', n) : tn('observations_handled', n);

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

/// One observation's row, one block parted from the next by a line: its title, or for one recorded before observations
/// had titles the words it starts with, cut to one line and whole on hovering; under them its tags, how it was handled,
/// and its facts, cut to one line and whole on hovering. A row handled is muted. A click opens it beside the list.
function Row({ o, project, open }: { o: Observation; project: string | null; open: number | null }) {
  const to = observationsHref(project, o.id);
  const said = joinedWords(facts(o));
  const whole = joinedWords([
    o.tags.length > 0 ? o.tags.join(', ') : null,
    o.folded_into !== null ? t('observation_folded', { item: o.folded_into }) : null,
    o.closed !== null ? t('observation_closed', { why: words(o.closed) }) : null,
    said,
  ]);
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
        <span className="obs-facts-line" title={whole}>
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

/// The owner's picks over the list, one choice for every project, kept in this browser: the tags and the states picked,
/// nothing at first, with the way to pick or take back one and to take back every pick.
type Picking = {
  picks: Picks;
  onTag: (tag: string) => void;
  onState: (s: ObservationState) => void;
  onClear: () => void;
};

/// One project's observations: its name; the buttons that pick them by its tags, in its order, then by none of them,
/// and by state, open or handled (`StateFilter` in its `pick` mode, the part the items' tree and the list of
/// conversations show or hide their rows with), each with how many it would show under the picks of the other group,
/// and while anything is picked the clear of every pick; then its rows, every one while nothing is picked.
function Project({ p, project, open, picking }: {
  p: ProjectObservations;
  /// The project the address names, which the rows' addresses keep.
  project: string | null;
  open: number | null;
  picking: Picking;
}) {
  const { picks } = picking;
  const tags = p.tags.map((x) => x.name);
  const covers = new Map(p.tags.map((x) => [x.name, x.desc]));
  const f = filters(p.observations, tags, picks);
  const rows = shownObservations(p.observations, tags, picks, open);
  const picked = picks.tags.length + picks.states.length > 0;
  return (
    <NavList.Group title={p.project}>
      <li className="obs-filters">
        <StateFilter
          pick
          order={f.tags.order}
          states={f.tags.counts}
          shown={picks.tags}
          tone="neutral"
          words={(x) => (x === NO_TAG ? t('observation_no_tag') : x)}
          title={(x, n, on) => {
            const said = x === NO_TAG ? tn('observations_untagged', n) : tn('observations_tagged', n, { tag: x });
            const click = clickWords(on, picks.tags.some((y) => y !== x));
            return [said, click, x !== NO_TAG ? (covers.get(x) ?? '') : ''].join(' ').trim();
          }}
          onShown={picking.onTag}
        />
        <StateFilter
          pick
          order={f.states.order}
          states={f.states.counts}
          shown={picks.states}
          tone="neutral"
          words={(x) => t(x === 'open' ? 'observation_open' : 'observation_handled')}
          title={(x, n, on) => `${stateSaid(x, n)} ${clickWords(on, picks.states.some((y) => y !== x))}`}
          onShown={picking.onState}
        />
        {picked ? <ClearPicks title={t('observations_clear')} onClear={picking.onClear} /> : null}
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
  const [tags, onTag, setTags] = useShownStates('observations-picked-tags', [...names, NO_TAG], []);
  const [states, onState, setStates] = useShownStates('observations-picked-states', OBSERVATION_STATES, []);
  const picking: Picking = {
    picks: { tags, states },
    onTag,
    onState,
    onClear: () => {
      setTags([]);
      setStates([]);
    },
  };
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
              picking={picking}
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
          {/* Below the narrow breakpoint the way back over the observation leads where its close does. */}
          <PageHeader.Actions hidden={{ narrow: true, regular: false }}>
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
      back={observationsHref(route.project)}
      list={<Loaded r={all}>{(data) => <List all={data} route={route} />}</Loaded>}
      open={open}
    />
  );
}
