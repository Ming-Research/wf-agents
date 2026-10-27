// The page: the bar, and the view the address names.

import { useEffect } from 'react';

import { connect } from './live';
import { Flow } from './pages/Flow';
import { Item } from './pages/Item';
import { Items } from './pages/Items';
import { Work } from './pages/Work';
import { Works } from './pages/Works';
import { Link, useRoute } from './route';
import { useT } from './text';
import { TopBar } from './TopBar';

export function App() {
  const route = useRoute();
  const t = useT();
  useEffect(() => connect(), []);
  return (
    <div className="app">
      <TopBar route={route} />
      <main>
        {route.page === 'works' ? <Works /> : null}
        {route.page === 'work' ? <Work id={route.id} round={route.round} cell={route.cell} /> : null}
        {route.page === 'items' ? <Items /> : null}
        {route.page === 'item' ? <Item id={route.id} /> : null}
        {route.page === 'flow' ? <Flow version={route.version} cell={route.cell} /> : null}
        {route.page === 'missing' ? (
          <div className="page">
            <Link to="/works">{t('nav_works')}</Link>
          </div>
        ) : null}
      </main>
    </div>
  );
}
