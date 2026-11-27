// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`snap.ts`), as they stand, drawn
// again as they change; with `?fixtures` in the page's address, made-up items that show every state the drawing has
// (`fixtures.ts`): those that stand as the boxes over the tree's corner tick, open, done or cancelled, and what stands
// over them. A press on the funnel or the dot of an item's fold folds the items under it away or shows them again, as
// `e` does over its card, except while the owner types in a field or holds Ctrl, Alt or Meta: a choice kept in the
// browser for that item, which wins over the tree's own rule. Under the boxes, a search finds any item (`find.tsx`),
// which `/` gives the keys to as `e` folds; an item picked from it is selected as one the address names, its box ticked
// first where its kind is not. `n` goes to the next of what the cards' red and orange counts count, in the tree's
// order and in the order the window draws them, and `N` to the one before, each selected and brought to the top of the
// window as a pick is. At the tree's bottom left, a question mark shows what the marks and the keys mean
// (`help.tsx`). Beside the tree, a window shows the item selected (`ItemWindow.tsx`). Selecting an item slides the
// window out; letting it go slides the window back, unless the tab stuck to its left edge brought it out, in which case
// it stays until the tab takes it back. The tab takes it back without letting the item go, and the next item selected
// brings it out again. The window coming out over the item selected, or an item selected half under it or half off the
// screen, moves the tree only until the item is wholly within what the window leaves. Escape lets the item go while the
// tree has the keys, and slides the window in while it has them. Each time the interface says something happened, the
// items are read again, once for the whole page, as the bar's count and the list of works read them (works/source.ts),
// at most one read at a time and one more after it, and drawn where they stand, the item selected and the panel as they
// were; when the stream opens again after a cut, everything is. An item the address names is selected once the tree
// shows it, as from a notification of a question it waits on. The page's shell mounts it lazily (`lazy.tsx`).
//
// The window slides over the tree, which keeps its whole width under it (`Drawer`, in the page's parts, which the flow
// view's panel and a work's item on its page are in too).

import './console.css';

import { type CSSProperties, useEffect, useMemo, useRef, useState } from 'react';

import { Drawer } from '../parts';
import { CHOICE_KEYS, read, useKeptFolds, useKeptTicks, write } from '../kept';
import { replaceHash } from '../route';
import { WIDTH_KEYS } from '../sizes';
import type { Items, WorkPage } from '../works-api';
import { useItemsOf } from '../works/source';
import { goalMock } from './mock';
import { fakeShown } from './fake';
import { Find } from './find';
import { ConsoleHelp } from './help';
import {
  type Empty,
  type ItemStanding,
  STANDINGS,
  NO_PROJECT,
  type Shown,
  projectOf,
  projectsIn,
  emptyOf,
  foldable,
  quietFolds,
  shownTree,
  standingOf,
  walkOrder,
} from './items';
import { ItemPanel, type PageState } from './ItemWindow';
import { SnapScene } from './snap';
import { aheadCount, repliesUnreadCount, waitingCount } from '../works/waiting';
import { ProjectList } from './projects';
import { type Heard, type Source, inOrder, pageSource } from './source';

/// How long the plane of the trees takes to move to another project's tree, or to open or close the overview: as its
/// transition in console.css says.
const PLANE_MS = 320;
/// The gap between two projects' trees in the overview's grid, and round the grid, in pixels.
const PLANE_GAP = 48;
/// The trees one behind another while the overview is closed: how much larger each tree in front of the one shown
/// stands, how much smaller each behind it, no smaller than `least`, and how far up to the right each behind it stands,
/// as parts of the room's width and height.
const DEPTH = { nearer: 0.3, further: 0.14, least: 0.2, x: 0.04, y: 0.03 };
const OVERVIEW_PAD = 24;
/// How long the page draws the trees in turn in each frame while the overview shows every tree at once, in
/// milliseconds: at least one tree a frame, and as many more as fit.
const OVERVIEW_DRAWS_MS = 4;
/// What the overview Space opens says it is.
const OVERVIEW_SAYS = "Every project's tree: a click or its key shows it; Space or Escape goes back";

/// Calls `then` once the browser has nothing else to do, or soon where it cannot say when that is.
function idle(then: () => void) {
  if (typeof window.requestIdleCallback === 'function') window.requestIdleCallback(() => then(), { timeout: 2000 });
  else window.setTimeout(then, 50);
}

/// The order of the list of projects as the browser keeps it, or none.
function keptOrder(): string[] {
  try {
    const v: unknown = JSON.parse(read(CHOICE_KEYS.projectOrder) ?? '[]');
    return Array.isArray(v) ? v.filter((p): p is string => typeof p === 'string') : [];
  } catch {
    return [];
  }
}

/// The projects the console lists, in the list's order (`projectsIn`): the items of no project, which were added before
/// items had projects, stand as a project of their own only while the boxes tick a kind some of them are of.
function listedIn(
  data: Items,
  order: readonly string[],
  installed: readonly string[],
  shows: ReadonlySet<ItemStanding>,
): string[] {
  return projectsIn(data, order, installed).filter(
    (p) => p !== NO_PROJECT || data.items.some((i) => i.project === null && shows.has(standingOf(i))),
  );
}

/// Each kind of item as its box over the tree names it, and what pointing at the box says.
const SHOWS: Record<ItemStanding, { name: string; title: string }> = {
  open: { name: 'Open', title: 'The items not done or cancelled: running, paused, waiting or not started' },
  done: { name: 'Done', title: 'The items whose work merged' },
  cancelled: { name: 'Cancelled', title: 'The items whose work was cancelled' },
};

/// What the console says where the tree of the project shown has no item to draw: that the project has none, or that
/// the boxes hide them all, and which boxes to tick to see them.
export function emptyWords(empty: Empty): string {
  if (empty.kind === 'none') return 'No items in this project yet';
  const names = empty.unticked.map((s) => SHOWS[s].name);
  const boxes = names.length < 2 ? names.join('') : `${names.slice(0, -1).join(', ')} or ${names[names.length - 1]}`;
  return `${empty.count === 1 ? 'The item is' : `All ${empty.count} items are`} hidden by the boxes: tick ${boxes}`;
}

export function ConsolePage({
  source: given,
  named,
  wait,
  active = true,
}: {
  source?: Source;
  named?: number;
  wait?: string;
  /// Whether the console is the view shown: kept while another is, it stops drawing and takes no keys.
  active?: boolean;
} = {}) {
  const source = useMemo(() => given ?? pageSource(), [given]);
  const panel = useRef<HTMLElement>(null);
  const host = useRef<HTMLDivElement>(null);
  // The scene of the project shown, which the page's acts on the tree reach; and each project's scene and what holds
  // it, by the project's name.
  const scene = useRef<SnapScene | null>(null);
  const scenes = useRef(new Map<string, SnapScene>());
  const holders = useRef(new Map<string, HTMLDivElement>());
  // The items, read once for every part of the page that reads them from the same feed: on the interface, the bar's
  // count and the list of works share this read.
  const items = useItemsOf(source.items);
  const data = items.value ?? undefined;
  const failed = items.failed;
  const [undrawn, setUndrawn] = useState<string | null>(null);
  const [selected, setSelected] = useState<Shown | null>(null);
  // The panel: whether it is out, and whether its handle brought it out, which keeps it out once the item is let go.
  const [win, setWin] = useState({ open: false, byHand: false });
  const open = win.open;
  // Which kinds of item the tree shows, open, done and cancelled each ticked or not, and what stands over them.
  const [shows, tick] = useKeptTicks(CHOICE_KEYS.consoleShows, ['open'], STANDINGS);
  const showsNow = useRef(shows);
  const tickNow = useRef(tick);
  useEffect(() => {
    tickNow.current = tick;
  }, [tick]);
  // Which items the owner folded and which he unfolded, as he last chose: the others fold as the tree's rule says.
  const [folds, turnFold, turnFolds] = useKeptFolds(CHOICE_KEYS.consoleFolds);
  const foldsNow = useRef(folds);
  const turnFoldNow = useRef(turnFold);
  const turnFoldsNow = useRef(turnFolds);
  // The projects whose trees `o` or `O` turned, each tree to be brought back to where its links put it once it is drawn
  // with those folds.
  const realignAsked = useRef(new Set<string>());
  useEffect(() => {
    turnFoldNow.current = turnFold;
    turnFoldsNow.current = turnFolds;
  }, [turnFold, turnFolds]);
  // The projects in the order the owner put them in, dragging them in the list under the map, and the one whose tree
  // the console shows, each kept in the browser: the keys 1 to 9 show the project of that place in the list.
  const [order, setOrder] = useState<string[]>(keptOrder);
  const orderNow = useRef(order);
  // The projects the installation names, read once: a project with no item yet has a tree in the list too.
  const [installed, setInstalled] = useState<string[]>([]);
  const installedNow = useRef(installed);
  useEffect(() => {
    let alive = true;
    source.projects?.().then(
      (names) => {
        if (!alive) return;
        installedNow.current = names;
        setInstalled(names);
      },
      // Where they cannot be read, the list has the projects of the items.
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, [source]);
  const [kept, setKept] = useState<string | null>(() => read(CHOICE_KEYS.consoleProject));
  const projects = useMemo(
    () => (data === undefined ? [] : listedIn(data, order, installed, shows)),
    [data, order, installed, shows],
  );
  const project = kept !== null && projects.includes(kept) ? kept : (projects[0] ?? null);
  const projectNow = useRef<string | null>(null);
  // Why the tree of the project shown has nothing to draw, where it has not.
  const empty = useMemo(
    () => (data === undefined || project === null ? null : emptyOf(data, shows, project)),
    [data, shows, project],
  );
  // Whether the overview Space opens is open: every project's tree on the screen at once.
  const [overview, setOverview] = useState(false);
  const overviewNow = useRef(false);
  // How wide the map the scene shown draws is, which the boxes stand past.
  const [mapWidth, setMapWidth] = useState(0);
  // The scenes made and ready to draw, by their project.
  const ready = useRef(new Set<string>());
  // The item the panel last showed, of whichever project's tree.
  const wasSelected = useRef<number | null>(null);
  // How large the tree's room is, which the overview's stack is laid out in.
  const [area, setArea] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const measure = () => setArea({ width: el.clientWidth, height: el.clientHeight });
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  // An item picked as from the search, as the latest render picks it, for the scenes' handlers made once.
  const pickNow = useRef<(id: number) => void>(() => undefined);

  // What happened, told to the reading of the work of the item selected.
  const heard = useRef<(h: Heard) => void>(() => undefined);
  useEffect(() => source.changes((h) => heard.current(h)), [source]);

  // The items as last read, for a scene made after they came.
  const latestData = useRef<Items | undefined>(undefined);
  // The tree of the project `name` as the console shows it.
  const treeOf = (d: Items, name: string) => shownTree(d, showsNow.current, foldsNow.current, name);

  // Each project in the list has a scene of its own, made once what holds it is there, its tree drawn in it; a project
  // gone from the list loses its scene. Only the scene shown tells the page what is selected and how wide its map is.
  const made = projects.join('\n');
  useEffect(() => {
    for (const [name, s] of scenes.current)
      if (!projects.includes(name)) {
        scenes.current.delete(name);
        ready.current.delete(name);
        if (scene.current === s) scene.current = null;
        s.destroy();
      }
    for (const name of projects) {
      const el = holders.current.get(name);
      if (el === undefined || scenes.current.has(name)) continue;
      const s = new SnapScene();
      scenes.current.set(name, s);
      // A scene goes on being made however the list changes meanwhile, as long as it is still its project's.
      const still = () => scenes.current.get(name) === s;
      s.mount(el).then(
        () => {
          if (!still()) return;
          s.onSelect = (item) => {
            if (scene.current !== s) return;
            setSelected(item);
            // The scene tells the item again each time it draws new data; only another item, in this tree or in the
            // tree of the project shown after it, moves the panel.
            const id = item?.id ?? null;
            if (id === wasSelected.current) return;
            wasSelected.current = id;
            // The address says the item selected while the console is shown, so that its tab leads back to it
            // (decision a-views-choices-are-kept-in-the-browser); an address of one of its waits stands while it is
            // selected.
            if (activeNow.current) {
              const at = id === null ? '#/console' : `#/console/items/${id}`;
              if (!(id !== null && window.location.hash.startsWith(`${at}/waits/`))) replaceHash(at);
            }
            setWin((w) => {
              if (id !== null) return w.open ? w : { open: true, byHand: false };
              return w.byHand ? w : { open: false, byHand: false };
            });
          };
          // A press on the funnel or the dot of an item's fold is the owner's choice for that item, kept, and every
          // tree is drawn again.
          s.onFold = (id, folded) => turnFoldNow.current(id, folded);
          // A click on the shadow of an item of another project shows that project's tree, and selects the item there.
          s.onAway = (id) => pickNow.current(id);
          s.onMapWidth = (w) => {
            if (scene.current === s) setMapWidth(w);
          };
          ready.current.add(name);
          if (latestData.current !== undefined) s.show(treeOf(latestData.current, name));
          s.showMap(name === projectNow.current && !overviewNow.current && moving.current === null);
          stale.current.add(name);
          paintStale();
          if (name === projectNow.current) take(name);
          showScenes();
          // Every project's scene, by its name, as `consoleScene` is the one shown.
          if (import.meta.env.DEV) Object.assign(window, { consoleScenes: scenes.current, consoleFake: fakeShown });
        },
        // A browser that cannot draw the tree (no WebGL) says so in its place.
        (e: unknown) => still() && setUndrawn(e instanceof Error ? e.message : String(e)),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [made]);
  useEffect(
    () => () => {
      for (const s of scenes.current.values()) s.destroy();
      scenes.current.clear();
      ready.current.clear();
      scene.current = null;
    },
    [],
  );

  // Makes the scene of `name` the one the page acts on: what is selected in it and how wide its map is come with it.
  const take = (name: string) => {
    const s = scenes.current.get(name) ?? null;
    scene.current = s;
    if (s === null) return;
    // To try the drawing from the browser's console on the dev server, and for the tests: `consoleScene` is the scene
    // shown, and `consoleScene.show(consoleFake(2000))` draws a made-up tree in its place.
    if (import.meta.env.DEV) Object.assign(window, { consoleScene: s });
    s.setDetail(overviewNow.current ? smallNow.current : 1);
    setMapWidth(s.mapWidth());
    s.select(s.selectedId());
    selectNamed();
  };
  // While the trees move: till then the maps stay away.
  const moving = useRef<number | null>(null);
  // How small each tree stands while the overview is open, and the project of the tree just left, which stays seen as
  // it was drawn while it goes, as the latest render has them.
  const smallNow = useRef(1);
  const leftNow = useRef<string | null>(null);
  // A tree not shown is also drawn once each time it changes, one at a time while the browser has nothing else to do,
  // so that as the overview opens every tree has its first frame drawn already.
  const stale = useRef(new Set<string>());
  const painting = useRef(false);
  const paintStale = () => {
    if (painting.current) return;
    painting.current = true;
    const next = () => {
      const name = [...stale.current].find((n) => n !== projectNow.current);
      if (name === undefined) {
        painting.current = false;
        return;
      }
      stale.current.delete(name);
      const s = scenes.current.get(name);
      // A tree not shown draws no map.
      s?.showMap(false);
      s?.paintOnce();
      idle(next);
    };
    idle(next);
  };
  // While the overview is closed, only the tree shown draws its frames, as finely as the screen. While it is open, no
  // tree draws frames of its own: the page draws them in turn (below). Every tree but the one filling the room, and the
  // one going while it goes, is drawn as finely as it stands small in the overview, so that drawing it there costs no
  // more than what of it is seen. The tree shown comes in from nothing whenever its detail changes, and is drawn by
  // its next frame; another is drawn again while the browser has nothing else to do.
  const showScenes = () => {
    const all = overviewNow.current;
    for (const [name, s] of scenes.current) {
      const shown = name === projectNow.current;
      s.setShown(activeNow.current && shown && !all);
      if (shown && !all) s.setDetail(1);
      else if (name !== leftNow.current && s.setDetail(smallNow.current) && !shown) stale.current.add(name);
    }
    paintStale();
  };
  // The tree just left, by its place in the list, which stays seen while it goes; -1 for none.
  const [left, setLeft] = useState(-1);
  // While the overview opens or closes, the trees fade rather than move: opening, every tree stands at once in its
  // place in the grid and fades in; closing, the tree shown stands at once filling the room and fades in, and every
  // other fades out where it stood in the grid. The tree shown, as it stood, fades out over them all.
  const [fade, setFade] = useState<'in' | 'out' | null>(null);
  // What holds the tree shown as it stood, fading out, while the overview opens or closes.
  const ghosts = useRef<HTMLDivElement>(null);
  // What holds the picture of the map of the tree shown while the trees move.
  const held = useRef<HTMLDivElement>(null);
  // The map each tree draws at its top left is no part of the trees' motion: while they move, the map of the tree shown
  // stands still in its place, as the list and the boxes do, a picture of it over the trees and none in the tree; once
  // they stand, the tree draws it again. None while the overview is open.
  const holdMap = () => {
    const s = scene.current;
    const el = held.current;
    el?.replaceChildren();
    if (s === null) return;
    const p = overviewNow.current ? null : s.mapPicture();
    // The tree shown draws its next frame with no map, as it comes in from nothing.
    s.showMap(false);
    if (p === null || el === null) return;
    Object.assign(p.canvas.style, { position: 'absolute', left: `${p.x}px`, top: `${p.y}px` });
    el.append(p.canvas);
  };
  const letMapGo = () => {
    scene.current?.showMap(true);
    scene.current?.paintOnce();
    held.current?.replaceChildren();
  };
  // The tree of `name` as it stands now, with no map, where it stands, to fade out over the trees.
  const fadeAway = (name: string) => {
    const el = ghosts.current;
    const s = scenes.current.get(name);
    const at = holders.current.get(name);
    if (el === null || s === undefined || at === undefined) return;
    s.showMap(false);
    const c = s.picture();
    if (c === null) return;
    c.className = 'console-ghost';
    c.style.transform = at.style.transform;
    el.replaceChildren(c);
  };
  // The plane moves, to another tree or into or out of the overview: only the tree shown draws while it moves.
  const planeMoves = () => {
    holdMap();
    if (moving.current !== null) window.clearTimeout(moving.current);
    moving.current = window.setTimeout(() => {
      moving.current = null;
      setLeft(-1);
      leftNow.current = null;
      setFade(null);
      ghosts.current?.replaceChildren();
      showScenes();
      if (!overviewNow.current) letMapGo();
    }, PLANE_MS);
    showScenes();
  };
  // Makes `name` the project shown, and says whether it was another: the tree left keeps no map, which is no part of
  // any motion.
  const choose = (name: string) => {
    setKept(name);
    write(CHOICE_KEYS.consoleProject, name);
    if (name === projectNow.current) return false;
    const leaving = scene.current;
    leaving?.showMap(false);
    leaving?.paintOnce();
    projectNow.current = name;
    take(name);
    return true;
  };
  // Shows the tree of `name`: the plane all the trees stand on moves to it, or, while the overview is open, the overview
  // closes onto it.
  const showProject = (name: string) => {
    if (overviewNow.current) {
      setOverviewTo(false, name);
      return;
    }
    const was = projectNow.current;
    if (!choose(name)) return;
    setLeft(was === null ? -1 : projects.indexOf(was));
    leftNow.current = was;
    planeMoves();
  };
  const showProjectNow = useRef(showProject);
  useEffect(() => {
    showProjectNow.current = showProject;
  });
  // Opens the overview, or closes it onto the tree of `to`, or of the project shown where none is given.
  const setOverviewTo = (on: boolean, to?: string) => {
    const was = projectNow.current;
    overviewNow.current = on;
    setOverview(on);
    setFade(on ? 'in' : 'out');
    // Closing onto another tree, the tree that was shown fades out in the grid as the others do.
    if (!(to !== undefined && choose(to)) && was !== null) fadeAway(was);
    planeMoves();
  };
  const setOverviewNow = useRef(setOverviewTo);
  useEffect(() => {
    setOverviewNow.current = setOverviewTo;
  });
  // The list dragged into a new order, kept.
  const reorder = (names: string[]) => {
    orderNow.current = names;
    setOrder(names);
    write(CHOICE_KEYS.projectOrder, JSON.stringify(names));
  };
  // The item the address names, until the tree shows it and it is selected.
  const toSelect = useRef<number | undefined>(named);
  // Selects the item the address names once the tree shows it; where its kind is not ticked, the box is ticked first
  // and the item is selected as the tree is drawn again, as for an item picked from the search; where it is of another
  // project, that project's tree is shown first.
  const selectNamed = () => {
    const id = toSelect.current;
    if (id === undefined) return;
    const found = latestData.current?.items.find((i) => i.number === id);
    if (found === undefined) return;
    if (projectOf(found) !== projectNow.current) showProjectNow.current(projectOf(found));
    // Until the scene of its project is made and draws its tree, which then looks for the item again.
    if (scene.current === null || projectNow.current === null || !ready.current.has(projectNow.current)) return;
    const standing = standingOf(found);
    if (!showsNow.current.has(standing)) {
      tickNow.current(standing, true);
      return;
    }
    toSelect.current = undefined;
    scene.current.select(id);
  };
  useEffect(() => {
    latestData.current = data;
    showsNow.current = shows;
    foldsNow.current = folds;
    // The first project shown; another where the one shown is gone; and the one kept once it is listed, as a project
    // with no item yet is once the installation's projects are read.
    if (projectNow.current === null || !projects.includes(projectNow.current) || project === kept) {
      if (project !== projectNow.current) {
        projectNow.current = project;
        if (project !== null) take(project);
        showScenes();
      }
    }
    if (data !== undefined)
      for (const [name, s] of scenes.current)
        if (ready.current.has(name)) {
          s.show(treeOf(data, name));
          if (realignAsked.current.has(name)) s.realign();
          if (name !== projectNow.current) stale.current.add(name);
        }
    realignAsked.current.clear();
    paintStale();
    selectNamed();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, shows, folds, project, kept]);
  useEffect(() => {
    toSelect.current = named;
    selectNamed();
  }, [named]);
  // Whether the search is open: `/` opens it, and it closes itself. An item picked from it is selected as one the
  // address names: the folds over it open, and where its kind is not ticked, its box is ticked first, so that the tree
  // shows it.
  const [finding, setFinding] = useState(false);
  const pick = (id: number) => {
    if (latestData.current?.items.some((i) => i.number === id) !== true) return;
    toSelect.current = id;
    selectNamed();
  };
  useEffect(() => {
    pickNow.current = pick;
  });

  // Each project's tree is a layer as large as the room. While the overview is closed, the trees stand one behind
  // another in the list's order, the first in front: the tree shown fills the room, those behind it stand smaller and
  // a little up to the right, and those in front of it larger, all of them but the tree shown unseen; so going to the
  // next tree it grows out from behind while the one left grows past and fades, and back the other way. While the
  // overview is open, every tree stands small in a grid as near square as lets each be as large as can be, filled a
  // row at a time in the list's order, with a gap between, the grid in the room's middle; as it opens and closes, the
  // trees fade where they go rather than move there (`fade`).
  // What waits for the owner in each project, and the replies of its works not read, counted as the bar counts them
  // over every project.
  const counts = useMemo(() => {
    const of = new Map<string, { waiting: number; ahead: number; unread: number }>();
    for (const name of projects) {
      const works = (data?.items ?? []).filter((i) => projectOf(i) === name).map((i) => i.work);
      of.set(name, { waiting: waitingCount(works), ahead: aheadCount(works), unread: repliesUnreadCount(works) });
    }
    return of;
  }, [data, projects]);
  const current = project === null ? 0 : Math.max(0, projects.indexOf(project));
  const across = Math.max(1, Math.ceil(Math.sqrt(projects.length)));
  const down = Math.max(1, Math.ceil(projects.length / across));
  const cell = (k: number) => ({
    x: (k % across) * (area.width + PLANE_GAP),
    y: Math.floor(k / across) * (area.height + PLANE_GAP),
  });
  const wide = across * area.width + (across - 1) * PLANE_GAP;
  const high = down * area.height + (down - 1) * PLANE_GAP;
  const small =
    wide > 0 && high > 0
      ? Math.min(1, (area.width - 2 * OVERVIEW_PAD) / wide, (area.height - 2 * OVERVIEW_PAD) / high)
      : 1;
  const origin = { x: (area.width - wide * small) / 2, y: (area.height - high * small) / 2 };
  useEffect(() => {
    smallNow.current = small;
    showScenes();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [small]);
  // While the overview is open, the page draws every tree, each as it stands at that moment, in turn: in each frame of
  // the browser's, for `OVERVIEW_DRAWS_MS` and at least one tree, going on in the next from where it stopped, so that
  // each shows how its project stands within a moment and drawing them all never holds up the page.
  useEffect(() => {
    if (!overview || !active) return;
    // The tree shown first, which comes in from nothing as the overview opens.
    let next = Math.max(0, [...scenes.current.keys()].indexOf(projectNow.current ?? ''));
    let asked = 0;
    const draw = () => {
      const all = [...scenes.current.values()];
      const began = performance.now();
      for (let k = 0; k < all.length; k += 1) {
        all[next % all.length].drawNow();
        next = (next + 1) % all.length;
        if (performance.now() - began >= OVERVIEW_DRAWS_MS) break;
      }
      asked = window.requestAnimationFrame(draw);
    };
    asked = window.requestAnimationFrame(draw);
    return () => window.cancelAnimationFrame(asked);
  }, [overview, active]);
  const layer = (k: number): CSSProperties => {
    const { width: w, height: h } = area;
    // In the grid while the overview is open, and while it closes every tree but the one shown, fading out there.
    if (overview || (fade === 'out' && k !== current)) {
      const c = cell(k);
      return {
        transform: `translate(${origin.x + c.x * small}px, ${origin.y + c.y * small}px) scale(${small})`,
        opacity: overview ? 1 : 0,
        zIndex: 100,
        pointerEvents: overview ? undefined : 'none',
      };
    }
    const d = k - current;
    const s = d === 0 ? 1 : d < 0 ? 1 + DEPTH.nearer * -d : Math.max(DEPTH.least, 1 - DEPTH.further * d);
    // About the room's middle, and those behind a little up to the right.
    const x = ((1 - s) * w) / 2 + Math.max(0, d) * DEPTH.x * w;
    const y = ((1 - s) * h) / 2 - Math.max(0, d) * DEPTH.y * h;
    return {
      transform: `translate(${x}px, ${y}px) scale(${s})`,
      opacity: d === 0 ? 1 : 0,
      zIndex: 100 - d,
      pointerEvents: d === 0 ? undefined : 'none',
      // A tree neither shown nor just left takes no room on the screen at all.
      visibility: d === 0 || k === left ? undefined : 'hidden',
    };
  };
  // Where each tree stands on the screen while the overview is open.
  const spot = (k: number) => ({
    left: origin.x + cell(k).x * small,
    top: origin.y + cell(k).y * small,
    width: area.width * small,
    height: area.height * small,
  });

  // The walk of the stops by n and N, and by the button under each stop: the item the walk last went to and the place
  // among its stops, -1 for its last; and the stop the window is asked to bring to the top of what it shows. The walk
  // goes in the tree's order, each item before the items under it (`walkOrder`), and from the last stop to the first,
  // and back. How many stops an item has is what its counts say, which the window draws only once it has read the work.
  const walk = useRef<{ item: number; at: number } | null>(null);
  const [stopAsked, setStopAsked] = useState<{ item: number; at: number; seq: number } | null>(null);
  // The item selected, as the scene holds it at this moment, which the page's own state follows a render later.
  const selectedNow = () => scene.current?.selectedId() ?? null;
  const goStop = (item: number, at: number) => {
    walk.current = { item, at };
    setStopAsked((was) => ({ item, at, seq: (was?.seq ?? 0) + 1 }));
    if (item !== selectedNow()) pick(item);
  };
  const walkOn = (way: 1 | -1) => {
    const d = latestData.current;
    if (d === undefined) return;
    // Over every project, as over every kind of item: a pick ticks the kind of the item it goes to.
    const order = projectsIn(d, orderNow.current, installedNow.current).flatMap((p) =>
      walkOrder(shownTree(d, new Set(STANDINGS), foldsNow.current, p)),
    );
    const begin = way > 0 ? 0 : -1;
    const stopsOf = (id: number) => order.find((o) => o.id === id)?.stops ?? 0;
    // The next item with stops from the item `from`, `way` on in the tree's order, round from the end to the start.
    const after = (from: number) => {
      const at = order.findIndex((o) => o.id === from);
      for (let k = 1; k <= order.length; k += 1) {
        const o = order[(((at + way * k) % order.length) + order.length) % order.length];
        if (o.stops > 0) return o.id;
      }
      return null;
    };
    const w = walk.current;
    const sel = selectedNow();
    if (w !== null && w.item === sel) {
      const count = stopsOf(w.item);
      const at = (w.at < 0 ? count - 1 : w.at) + way;
      if (at >= 0 && at < count) return goStop(w.item, at);
      const next = after(w.item);
      if (next !== null) goStop(next, begin);
      return;
    }
    // A walk begins at the item selected: at its own stops where it has any, else at the next item that has.
    if (sel !== null && stopsOf(sel) > 0) return goStop(sel, begin);
    const from = sel ?? (way > 0 ? order[order.length - 1]?.id : order[0]?.id);
    const next = from === undefined ? null : after(from);
    if (next !== null) goStop(next, begin);
  };
  const walkOnNow = useRef(walkOn);
  useEffect(() => {
    walkOnNow.current = walkOn;
  });

  // The walk of the replies not read by m and M: the item it last went to, and the item and the way of the window's
  // latest ask to bring one into view. The window decides which reply of an item is next, as it draws them and knows which
  // are read; an item where it finds none sends the walk on to the next item that has any, in the tree's order, round from
  // the end to the start. `hops` bounds how far one press goes on, so that counts that run ahead of what the window draws
  // do not walk on for ever.
  const replyWalk = useRef<number | null>(null);
  const hops = useRef(0);
  const [replyAsked, setReplyAsked] = useState<{ item: number; way: 1 | -1; begin: boolean; seq: number } | null>(null);
  const goReply = (item: number, way: 1 | -1, begin: boolean) => {
    replyWalk.current = item;
    setReplyAsked((was) => ({ item, way, begin, seq: (was?.seq ?? 0) + 1 }));
    if (item !== selectedNow()) pick(item);
  };
  // The items in the tree's order with how many replies each has not read, over every project.
  const replyOrder = () => {
    const d = latestData.current;
    if (d === undefined) return [];
    return projectsIn(d, orderNow.current, installedNow.current).flatMap((p) =>
      walkOrder(shownTree(d, new Set(STANDINGS), foldsNow.current, p), (s) => s.repliesUnread),
    );
  };
  // The next item with replies not read from the item `from`, `way` on, round from the end to the start.
  const replyAfter = (order: { id: number; stops: number }[], from: number | null, way: 1 | -1): number | null => {
    const at = order.findIndex((o) => o.id === from);
    const start = at < 0 ? (way > 0 ? -1 : order.length) : at;
    for (let k = 1; k <= order.length; k += 1) {
      const o = order[(((start + way * k) % order.length) + order.length) % order.length];
      if (o.stops > 0) return o.id;
    }
    return null;
  };
  const replyWalkOn = (way: 1 | -1) => {
    const order = replyOrder();
    const sel = selectedNow();
    hops.current = 0;
    // The walk goes on in the item it is in: the window brings its next reply not read, or says it has none.
    if (sel !== null && replyWalk.current === sel) return goReply(sel, way, false);
    // A walk begins at the item selected where it has replies not read, else at the next item that has.
    if (sel !== null && (order.find((o) => o.id === sel)?.stops ?? 0) > 0) return goReply(sel, way, true);
    const next = replyAfter(order, sel, way);
    if (next !== null) goReply(next, way, true);
  };
  const replyWalkOnNow = useRef(replyWalkOn);
  const repliesAsked = (found: boolean) => {
    if (found || replyAsked === null) return;
    hops.current += 1;
    const order = replyOrder();
    if (hops.current > order.length) return;
    const next = replyAfter(order, replyAsked.item, replyAsked.way);
    if (next !== null) goReply(next, replyAsked.way, true);
  };
  useEffect(() => {
    replyWalkOnNow.current = replyWalkOn;
  });

  /// The width of the tree that the window leaves to be seen: all of it while the window is in.
  const room = (out: boolean) => {
    const tree = host.current?.clientWidth ?? 0;
    return out ? tree - (panel.current?.offsetWidth ?? 0) : tree;
  };
  const selectedId = selected?.id ?? null;
  useEffect(() => {
    if (open && selectedId !== null) scene.current?.reveal(selectedId, room(true));
  }, [open, selectedId]);

  // Whether the console is shown, for the scene's handler, made once.
  const activeNow = useRef(active);
  useEffect(() => {
    activeNow.current = active;
    showScenes();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      // A digit from 1 to 9 shows the tree of the project of that place in the list under the map; not while the owner
      // types in a field, nor with Ctrl, Alt or Meta held.
      if (/^[1-9]$/.test(e.key)) {
        const to = e.target as HTMLElement | null;
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(to.tagName))) return;
        const d = latestData.current;
        const listed = d === undefined ? [] : listedIn(d, orderNow.current, installedNow.current, showsNow.current);
        const name = listed[Number(e.key) - 1];
        if (name !== undefined) {
          e.preventDefault();
          showProjectNow.current(name);
        }
        return;
      }
      // Space opens the overview of every project's tree, and closes it again; as the digits, not in a field.
      if (e.key === ' ') {
        const to = e.target as HTMLElement | null;
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(to.tagName))) return;
        e.preventDefault();
        setOverviewNow.current(!overviewNow.current);
        return;
      }
      if (e.key === 'Escape' && overviewNow.current) {
        setOverviewNow.current(false);
        return;
      }
      // `o` folds every item of the trees shown under which no work is under way, at every level (`quietFolds`), and
      // leaves every other fold as it is; `O` unfolds every item of them. The trees shown are the project's, or every
      // project's in the overview. Each keeps its choices as the owner's own, at once for them all; as the other keys,
      // not while the owner types in a field, nor with Ctrl, Alt or Meta held. Each tree, drawn with its folds, then
      // comes back to where its links put it from the column of its roots, so that no column stays off the screen
      // where it had been scrolled (`realign`); a tree with nothing to turn comes back at once.
      if (e.key === 'o' || e.key === 'O') {
        const to = e.target as HTMLElement | null;
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(to.tagName))) return;
        const d = latestData.current;
        if (d === undefined) return;
        e.preventDefault();
        const names = overviewNow.current
          ? listedIn(d, orderNow.current, installedNow.current, showsNow.current)
          : projectNow.current === null
            ? []
            : [projectNow.current];
        const roots = names.flatMap((name) => shownTree(d, showsNow.current, foldsNow.current, name));
        const fold = e.key === 'o';
        const ids = fold ? quietFolds(roots) : foldable(roots);
        if (ids.length > 0) {
          for (const name of names) realignAsked.current.add(name);
          turnFoldsNow.current(new Map(ids.map((id): [number, boolean] => [id, fold])));
        } else for (const name of names) scenes.current.get(name)?.realign();
        return;
      }
      // `e` folds the items under the card the pointer is over away, or shows them again, as a click on its funnel or
      // its dot does; `/` opens the search; `n` goes to the next stop and `N` to the one before; `m` goes to the next
      // reply not read and `M` to the one before. None while the owner types in a field, nor with Ctrl, Alt or Meta
      // held.
      if (e.key === 'e' || e.key === '/' || e.key === 'n' || e.key === 'N' || e.key === 'm' || e.key === 'M') {
        const to = e.target as HTMLElement | null;
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(to.tagName))) return;
        if (e.key === '/') {
          e.preventDefault();
          setFinding(true);
        } else if (e.key === 'n' || e.key === 'N') {
          e.preventDefault();
          walkOnNow.current(e.key === 'n' ? 1 : -1);
        } else if (e.key === 'm' || e.key === 'M') {
          e.preventDefault();
          replyWalkOnNow.current(e.key === 'm' ? 1 : -1);
        } else if (scene.current?.foldPointedCard() === true) e.preventDefault();
        return;
      }
      // Escape answers a question the page asks, and nothing else.
      if (e.key !== 'Escape' || document.querySelector('dialog[open]') !== null) return;
      const inside = panel.current?.contains(document.activeElement) === true;
      if (open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        setWin({ open: false, byHand: false });
      } else scene.current?.select(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [open, active]);

  // The item selected as the interface gives it, and its work's page, read when another is selected and again when
  // something happened to that work; an answer older than the one shown is dropped, and a read that failed says so.
  const item = selectedId === null ? undefined : data?.items.find((i) => i.number === selectedId);
  const work = item?.work.number;
  const [page, setPage] = useState<{ work: number; state: PageState } | null>(null);
  const takePage = useMemo(() => inOrder(), []);
  useEffect(() => {
    if (work === undefined) return;
    const read = () =>
      takePage(
        source.loadWork(work),
        (p: WorkPage) => setPage({ work, state: { page: p } }),
        (why) => setPage({ work, state: { failed: why } }),
      );
    read();
    heard.current = (h) => {
      if (h === 'everything' || h.works.includes(work)) read();
    };
    return () => {
      heard.current = () => undefined;
    };
  }, [work, source, takePage]);
  const shownPage = page !== null && page.work === work ? page.state : null;

  const fold = () => setWin((w) => (w.open ? { open: false, byHand: false } : { open: true, byHand: true }));

  return (
    <div className="console">
      {/* A press on the tree gives it the keys back from the window. */}
      <div
        className="console-canvas"
        ref={host}
        onPointerDown={() => (document.activeElement as HTMLElement | null)?.blur()}
      >
        {/* Every project's tree, one behind another; while the overview is open, a press on a tree shows it. */}
        <div className={fade === null ? 'console-layers' : 'console-layers fading'}>
          {projects.map((name, k) => (
            <div
              key={name}
              className={
                fade === 'in' && overview
                  ? 'console-tree fading-in'
                  : fade === 'out' && k === current
                    ? 'console-tree fading-back'
                    : 'console-tree'
              }
              style={{ ...layer(k), width: area.width || '100%', height: area.height || '100%' }}
              ref={(el) => {
                if (el !== null) holders.current.set(name, el);
                else holders.current.delete(name);
              }}
            />
          ))}
          <div ref={ghosts} />
        </div>
        {/* The frames over the trees in the grid, which fade out with the trees as the overview closes. */}
        {(overview || fade === 'out') && (
          <div
            className={overview ? 'console-overview' : 'console-overview closing'}
            role="dialog"
            aria-label={OVERVIEW_SAYS}
            onClick={() => setOverviewTo(false)}
          >
            {projects.map((name, k) => (
              <button
                key={name}
                type="button"
                className={k === current ? 'console-overview-tree shown' : 'console-overview-tree'}
                style={spot(k)}
                onClick={(e) => {
                  e.stopPropagation();
                  showProject(name);
                }}
              >
                <span className="console-overview-name">
                  {k < 9 && <kbd className="key">{k + 1}</kbd>}
                  {name}
                </span>
              </button>
            ))}
          </div>
        )}
        {/* The list of projects, the boxes beside the map and the map held while the trees move, which go at once while
            the overview shows every tree small, and come back at once as it closes. */}
        <div className={overview ? 'console-chrome away' : 'console-chrome'}>
          <div className="console-map-held" ref={held} />
          <ProjectList
            projects={projects}
            counts={counts}
            shown={project}
            onShow={showProject}
            onOrder={reorder}
          />
          <div className="console-over" style={{ left: `calc(var(--space-12) + ${mapWidth}px + var(--space-8))` }}>
            <div className="console-shows" role="group" aria-label="Which items the tree shows">
              {STANDINGS.map((s) => (
                <label key={s} className="view-toggle" title={SHOWS[s].title}>
                  <input type="checkbox" checked={shows.has(s)} onChange={(e) => tick(s, e.target.checked)} />
                  {SHOWS[s].name}
                </label>
              ))}
            </div>
            {empty !== null && <p className="console-empty">{emptyWords(empty)}</p>}
            {failed !== null && <p className="console-message">The page could not read the items: {failed}</p>}
            {undrawn !== null && <p className="console-message">The browser could not draw the tree: {undrawn}</p>}
          </div>
        </div>
        <ConsoleHelp />
      </div>
      <Drawer
        open={open}
        onToggle={fold}
        panel={panel}
        label="The item selected"
        hide="Put the item away"
        show="Show the item selected"
        widthKey={WIDTH_KEYS.itemWindow}
      >
        {item === undefined ? (
          <p className="cw-empty">Click an item on the left to see it here.</p>
        ) : (
          <ItemPanel
            item={item}
            page={shownPage}
            act={source.act}
            loadVersion={source.loadVersion}
            wait={wait}
            goal={selected !== null && goalMock(selected.work !== null, selected.children.length)}
            open={open && active}
            onRead={() =>
              latestData.current !== undefined &&
              projectNow.current !== null &&
              scene.current?.show(treeOf(latestData.current, projectNow.current))
            }
            onTitle={() => {
              scene.current?.reveal(item.number, room(open));
              scene.current?.flash(item.number);
            }}
            stop={stopAsked !== null && stopAsked.item === item.number ? stopAsked : undefined}
            reply={replyAsked !== null && replyAsked.item === item.number ? replyAsked : undefined}
            onReply={repliesAsked}
            onStopChosen={(at) => {
              walk.current = { item: item.number, at };
            }}
            onNextStop={(at) => {
              walk.current = { item: item.number, at };
              walkOn(1);
            }}
          />
        )}
      </Drawer>
      {finding && (
        <Find
          data={data}
          onPick={pick}
          onClose={() => setFinding(false)}
          onFound={(ids) => scene.current?.find(ids)}
        />
      )}
    </div>
  );
}
