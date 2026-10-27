// The page: the bar, and the view the address names.

import { useEffect, useState } from 'react';

import { BarSlot } from './bar';
import { connect } from './live';
import { Flow } from './pages/Flow';
import { Item } from './pages/Item';
import { Items } from './pages/Items';
import { Messages } from './pages/Messages';
import { Work } from './pages/Work';
import { Works } from './pages/Works';
import { Link, useRoute } from './route';
import { t } from './text';
import { TopBar } from './TopBar';

export function App() {
  const route = useRoute();
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  useEffect(() => connect(), []);
  return (
    <BarSlot.Provider value={slot}>
      <div className={route.page === 'flow' || route.page === 'work' ? 'app app-screen' : 'app'}>
        <TopBar route={route} slot={setSlot} />
        <main>
          {route.page === 'works' ? <Works /> : null}
          {route.page === 'work' ? <Work id={route.id} round={route.round} cell={route.cell} /> : null}
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
    </BarSlot.Provider>
  );
}
