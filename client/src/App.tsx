// The page: the bar, and the view the address names.

import { useEffect } from 'react';

import { connect } from './live';
import { Architect } from './pages/Architect';
import { Flow } from './pages/Flow';
import { Items } from './pages/Items';
import { Messages } from './pages/Messages';
import { Settings } from './pages/Settings';
import { Spend } from './pages/Spend';
import { Works } from './pages/Works';
import { Link, useRoute } from './route';
import { t } from './text';
import { TopBar } from './TopBar';

export function App() {
  const route = useRoute();
  useEffect(() => connect(), []);
  const work = route.page === 'work' ? route : null;
  const screen = ['flow', 'work', 'works', 'architect', 'items', 'item', 'messages'].includes(route.page);
  return (
    <div className={screen ? 'app app-screen' : 'app'}>
      <TopBar route={route} />
      <main>
        {/* One element for the list and a work picked from it, so that picking a work keeps the list as it is. */}
        {route.page === 'works' || work !== null ? (
          <Works work={work} />
        ) : null}
        {/* One element for the tree and the item picked from it, so that picking an item keeps the tree as it is. */}
        {route.page === 'items' || route.page === 'item' ? (
          <Items selected={route.page === 'item' ? route.id : null} />
        ) : null}
        {route.page === 'architect' ? <Architect route={route} /> : null}
        {route.page === 'messages' ? <Messages open={route.open} /> : null}
        {route.page === 'spend' ? <Spend /> : null}
        {route.page === 'settings' ? <Settings /> : null}
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
