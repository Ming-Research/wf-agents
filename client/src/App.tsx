// The page: the bar, and the view the address names.

import { useEffect } from 'react';

import { connect } from './live';
import { Architect } from './pages/Architect';
import { Flow } from './pages/Flow';
import { Items } from './pages/Items';
import { Messages } from './pages/Messages';
import { Settings } from './pages/Settings';
import { System } from './pages/System';
import { Works } from './pages/Works';
import { href, Link, navigate, useRoute } from './route';
import { t } from './text';
import { TopBar } from './TopBar';

export function App() {
  const route = useRoute();
  useEffect(() => connect(), []);
  // The Spend view's old address goes on to the System view that took its place.
  useEffect(() => {
    if (route.page === 'system' && location.pathname === '/spend') navigate(href(route), true);
  }, [route]);
  const work = route.page === 'work' ? route : null;
  // A view that fills the window and scrolls inside its parts; the items scroll the page, their tree's pane staying
  // under the bar as Primer's PageLayout keeps a sticky pane.
  const screen = ['flow', 'work', 'works', 'architect', 'messages'].includes(route.page);
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
        {route.page === 'system' ? <System period={route.period} /> : null}
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
