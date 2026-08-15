import { OnboardingProvider, useOnboarding } from '@onboardjs/react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useEffect } from 'react';
import { Modal, Text, View } from 'react-native';
import { Button } from '../../components/ui/button';
import { Card, CardFooter } from '../../components/ui/card';
import { useSession } from '../../lib/session';
import { onboardingSteps } from './steps';

const STORAGE_KEY = 'vautr_onboarding_v1';
export const ONBOARDING_REPLAY_EVENT = 'vautr:replay-onboarding';

/** Clear persisted progress + ask the provider to restart the flow. */
export function triggerReplayOnboarding() {
  void AsyncStorage.removeItem(STORAGE_KEY);
  window.dispatchEvent(new CustomEvent(ONBOARDING_REPLAY_EVENT));
}

/**
 * Renders the current onboarding step as a centered modal above the app. The
 * host app is rendered behind it (via children). When the flow is complete (or
 * skipped to the end), the modal is hidden.
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

  const visible = !!state && !state.isCompleted && !!state.currentStep;

  return (
    <>
      {children}
      <Modal visible={visible} transparent animationType="fade">
        <View className="flex-1 items-center justify-center bg-black/50 px-4">
          <Card className="w-full max-w-md gap-4 p-5">
            <Text className="text-xs text-muted-foreground">
              Step {state?.currentStepNumber} of {state?.totalSteps}
            </Text>
            {renderStep()}
            <CardFooter className="flex flex-row items-center justify-between gap-2 p-0 pt-2">
              <Button variant="ghost" onPress={() => previous()} disabled={!state?.canGoPrevious}>
                <Text className="text-foreground">Back</Text>
              </Button>
              <View className="flex flex-row gap-2">
                {state?.isSkippable ? (
                  <Button variant="ghost" onPress={() => void skip()}>
                    <Text className="text-foreground">Skip</Text>
                  </Button>
                ) : null}
                <Button onPress={() => void next()} disabled={!state?.canGoNext}>
                  <Text className="text-primary-foreground">
                    {state?.isLastStep ? 'Finish' : 'Next'}
                  </Text>
                </Button>
              </View>
            </CardFooter>
          </Card>
        </View>
      </Modal>
    </>
  );
}

/**
 * Wraps the app with the OnboardingProvider. The flow shows once (AsyncStorage
 * persistence) after first register/login and never blocks the app on later
 * loads.
 */
export function OnboardingFlow({ children }: { children: React.ReactNode }) {
  const username = useSession((s) => s.username);

  // Onboarding only mounts once the user is logged in: its first step creates
  // a vault via the API (requires an authenticated session) and the app cannot
  // be navigated before login anyway.
  if (!username) return <>{children}</>;

  return (
    <OnboardingProvider
      flowId="vautr-setup"
      flowName="Vautr first-run setup"
      flowVersion="1.0.0"
      steps={onboardingSteps}
      customOnDataLoad={async () => {
        const raw = await AsyncStorage.getItem(STORAGE_KEY);
        return raw ? JSON.parse(raw) : null;
      }}
      customOnDataPersist={async (ctx: unknown) => {
        await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(ctx));
      }}
      customOnClearPersistedData={async () => {
        await AsyncStorage.removeItem(STORAGE_KEY);
      }}
    >
      <OnboardingOverlay>{children}</OnboardingOverlay>
    </OnboardingProvider>
  );
}
