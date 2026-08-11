import { createMemoryHistory, createRouter } from '@tanstack/react-router';

import { routeTree } from '../routeTree.gen';

// React Native has no browser URL; use an in-memory history.
const history = createMemoryHistory({ initialEntries: ['/'] });

export const router = createRouter({
  routeTree,
  history,
  defaultPreload: false,
});

// Register the router instance for type-safe navigation helpers.
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
