import { RouterProvider } from '@tanstack/react-router';
import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { restoreSession, sync } from './lib/client';
import { OnboardingFlow } from './onboarding/OnboardingFlow';
import { router } from './router';
import { TourOverlay } from './tour/TourOverlay';
import './index.css';
import './bones/registry';

// Dev-only e2e hook: lets the Playwright conflict suite trigger a sync
// (push) without a UI button. Only exposed on the local dev server.
if (location.hostname === 'localhost' || location.hostname === '127.0.0.1') {
  (window as unknown as { __vautrSync?: () => Promise<void> }).__vautrSync = sync;
}

const root = document.getElementById('root');
if (!root) {
  throw new Error('#root element not found');
}

/**
 * Top-level app shell. Restores any persisted session before rendering
 * the router so a page reload doesn't bounce the user to /login.
 *
 * VTR-FIX session-restore: without this, `vaultStore` starts with
 * `isLocked: true` on every page load (see `packages/ui-logic/src/store.ts`
 * initial state), `_authed.tsx`'s useEffect redirects to /login, and the
 * user has to re-authenticate even though their session token is still
 * valid in IndexedDB.
 *
 * The `restoring` flag shows a brief splash while IndexedDB is read and
 * the server validates the token; once `restoreSession()` resolves, the
 * real router renders. If no session exists or the token is expired,
 * `_authed.tsx` will redirect to /login as before.
 */
function App() {
  const [restoring, setRestoring] = useState(true);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        await restoreSession();
      } finally {
        if (active) setRestoring(false);
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  if (restoring) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-bg text-text-muted">
        <p className="text-sm">Restoring session…</p>
      </div>
    );
  }

  return (
    <>
      <OnboardingFlow>
        <RouterProvider router={router} />
      </OnboardingFlow>
      <TourOverlay />
    </>
  );
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
