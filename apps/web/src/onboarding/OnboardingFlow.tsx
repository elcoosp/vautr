import { OnboardingProvider, useOnboarding } from '@onboardjs/react';
import { useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardFooter } from '@/components/ui/card';
import { useSession } from '@/lib/useSession';
import { onboardingComponents, onboardingSteps } from './steps';

const STORAGE_KEY = 'vautr_onboarding_v1';
export const ONBOARDING_REPLAY_EVENT = 'vautr:replay-onboarding';

/** Clear persisted progress + ask the provider to restart the flow. */
export function triggerReplayOnboarding() {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    /* ignore */
  }
  window.dispatchEvent(new CustomEvent(ONBOARDING_REPLAY_EVENT));
}

/**
 * Renders the current onboarding step as a centered modal overlay. The host app
 * is rendered behind it (via children). When the flow is complete (or skipped
 * to the end), nothing is shown.
 */
function OnboardingOverlay({ children }: { children: React.ReactNode }) {
  const { state, next, previous, skip, reset, renderStep } = useOnboarding();

  useEffect(() => {
    const onReplay = () => {
      void reset();
    };
    window.addEventListener(ONBOARDING_REPLAY_EVENT, onReplay);
    return () => window.removeEventListener(ONBOARDING_REPLAY_EVENT, onReplay);
  }, [reset]);

  if (!state || state.isCompleted || !state.currentStep) return <>{children}</>;

  return (
    <>
      {children}
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
        <Card className="w-full max-w-md">
          <CardContent className="pt-6">
            <div className="mb-3 text-xs text-text-muted">
              Step {state.currentStepNumber} of {state.totalSteps}
            </div>
            {renderStep()}
          </CardContent>
          <CardFooter className="flex items-center justify-between gap-2">
            <Button variant="ghost" onClick={() => previous()} disabled={!state.canGoPrevious}>
              Back
            </Button>
            <div className="flex gap-2">
              {state.isSkippable ? (
                <Button variant="ghost" onClick={() => skip()}>
                  Skip
                </Button>
              ) : null}
              <Button onClick={() => next()} disabled={!state.canGoNext}>
                {state.isLastStep ? 'Finish' : 'Next'}
              </Button>
            </div>
          </CardFooter>
        </Card>
      </div>
    </>
  );
}

/**
 * Wraps the app with the OnboardingProvider. The flow shows once (localStorage
 * persistence) after the user is logged in, and never blocks the dashboard on
 * later loads. Crucially, it only mounts when a session exists: onboarding's
 * first step creates a vault via the API, which requires an authenticated
 * session, and the app cannot be navigated until login anyway.
 */
export function OnboardingFlow({ children }: { children: React.ReactNode }) {
  const authed = useSession();

  if (!authed) return <>{children}</>;

  return (
    <OnboardingProvider
      flowId="vautr-setup"
      flowName="Vautr first-run setup"
      flowVersion="1.0.0"
      steps={onboardingSteps}
      componentRegistry={onboardingComponents}
      localStoragePersistence={{ key: STORAGE_KEY }}
      onFlowComplete={() => {
        /* persistence already records completion */
      }}
    >
      <OnboardingOverlay>{children}</OnboardingOverlay>
    </OnboardingProvider>
  );
}
