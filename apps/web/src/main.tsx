import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { OnboardingFlow } from './onboarding/OnboardingFlow';
import { router } from './router';
import './index.css';

const root = document.getElementById('root');
if (!root) {
  throw new Error('#root element not found');
}

createRoot(root).render(
  <StrictMode>
    <OnboardingFlow>
      <RouterProvider router={router} />
    </OnboardingFlow>
  </StrictMode>,
);
