import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { sync } from './lib/client';
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

createRoot(root).render(
  <StrictMode>
    <OnboardingFlow>
      <RouterProvider router={router} />
    </OnboardingFlow>
    <TourOverlay />
  </StrictMode>,
);
