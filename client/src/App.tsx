// The page: the bar, and the view the address names.

import { useEffect } from 'react';

import { connect } from './live';
import { Flow } from './pages/Flow';
import { Item } from './pages/Item';
import { Items } from './pages/Items';
import { Messages } from './pages/Messages';
import { Works } from './pages/Works';
import { Link, useRoute } from './route';
import { t } from './text';
import { TopBar } from './TopBar';

export function App() {
  const route = useRoute();
  useEffect(() => connect(), []);
  const work = route.page === 'work' ? route : null;
  return (
    <div className={route.page === 'flow' || route.page === 'work' || route.page === 'works' ? 'app app-screen' : 'app'}>
      <TopBar route={route} />
      <main>
        {/* One element for the list and a work picked from it, so that picking a work keeps the list as it is. */}
        {route.page === 'works' || work !== null ? (
          <Works selected={work?.id ?? null} round={work?.round ?? null} cell={work?.cell ?? null} />
        ) : null}
        {route.page === 'items' ? <Items /> : null}
        {route.page === 'item' ? <Item id={route.id} /> : null}
        {route.page === 'messages' ? <Messages /> : null}
        {route.page === 'flow' ? <Flow route={route} /> : null}
        {route.page === 'missing' ? (
          <div className="page">
            <Link to="/works">{t('nav_works')}</Link>
          </div>
        ) : null}
      </main>
    </div>
  );
}
