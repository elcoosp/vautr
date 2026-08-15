import React from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster } from '@/components/ui/sonner';
import { OnboardingFlow } from '../onboarding/OnboardingFlow';
import { App } from './App';
import '@/styles/globals.css';

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <React.StrictMode>
      <OnboardingFlow>
        <App />
        <Toaster position="bottom-center" />
      </OnboardingFlow>
    </React.StrictMode>,
  );
}
