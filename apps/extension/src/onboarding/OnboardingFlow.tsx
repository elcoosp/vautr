import { OnboardingProvider, useOnboarding } from '@onboardjs/react';
import { useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardFooter } from '@/components/ui/card';
import { onboardingSteps } from './steps';

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
 * Renders the current onboarding step as a centered overlay above the popup. The
 * host app is rendered behind it (via children). When the flow is complete (or
 * skipped to the end), nothing is shown.
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
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-3">
        <Card className="w-full max-w-sm">
          <CardContent className="pt-5">
            <div className="mb-2 text-xs text-muted-foreground">
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
 * Wraps the popup with the OnboardingProvider. The flow shows once (localStorage
 * persistence) after first unlock and never blocks the popup on later loads.
 */
export function OnboardingFlow({ children }: { children: React.ReactNode }) {
  return (
    <OnboardingProvider
      flowId="vautr-setup"
      flowName="Vautr first-run setup"
      flowVersion="1.0.0"
      steps={onboardingSteps}
      localStoragePersistence={{ key: STORAGE_KEY }}
      onFlowComplete={() => {
        /* persistence already records completion */
      }}
    >
      <OnboardingOverlay>{children}</OnboardingOverlay>
    </OnboardingProvider>
  );
}
