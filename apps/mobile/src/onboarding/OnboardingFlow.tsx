import { OnboardingProvider, useOnboarding } from '@onboardjs/react';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useEffect, useState } from 'react';
import { DeviceEventEmitter, Modal, Text, View } from 'react-native';
import { Button } from '../../components/ui/button';
import { Card, CardFooter } from '../../components/ui/card';
import { useSession } from '../../lib/session';
import { onboardingComponentRegistry, onboardingSteps } from './steps';

const STORAGE_KEY = 'vautr_onboarding_v1';
const DONE_KEY = 'vautr_onboarding_done';
export const ONBOARDING_REPLAY_EVENT = 'vautr:replay-onboarding';

/** Clear persisted progress + ask the provider to restart the flow. */
export function triggerReplayOnboarding() {
  void AsyncStorage.removeItem(STORAGE_KEY);
  void AsyncStorage.removeItem(DONE_KEY);
  DeviceEventEmitter.emit(ONBOARDING_REPLAY_EVENT);
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
    const subscription = DeviceEventEmitter.addListener(ONBOARDING_REPLAY_EVENT, onReplay);
    return () => subscription.remove();
  }, [reset]);

  // The terminal step (DoneStep) renders its own Finish button that calls
  // next(), which completes the engine (done.nextStep === null). onFlowComplete
  // (wired on the provider) then records the `done` marker and clears progress.
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
                {/* The terminal step renders its own Finish button (DoneStep calls
                    next() to complete the engine). The overlay only drives Next
                    for non-terminal steps, since the engine disables canGoNext on
                    the final step (nextStep: null). */}
                {!state?.isLastStep ? (
                  <Button onPress={() => void next()} disabled={!state?.canGoNext}>
                    <Text className="text-primary-foreground">Next</Text>
                  </Button>
                ) : null}
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
 * loads. A `done` marker keeps it dismissed across remounts.
 */
export function OnboardingFlow({ children }: { children: React.ReactNode }) {
  const username = useSession((s) => s.username);
  const [done, setDone] = useState(false);

  useEffect(() => {
    if (!username) {
      setDone(false);
      return;
    }
    let alive = true;
    void AsyncStorage.getItem(DONE_KEY).then((v) => {
      if (alive) setDone(v === '1');
    });
    return () => {
      alive = false;
    };
  }, [username]);

  // Onboarding only mounts once the user is logged in: its first step creates
  // a vault via the API (requires an authenticated session) and the app cannot
  // be navigated before login anyway.
  if (!username || done) return <>{children}</>;

  return (
    <OnboardingProvider
      flowId="vautr-setup"
      flowName="Vautr first-run setup"
      flowVersion="1.0.0"
      steps={onboardingSteps}
      componentRegistry={onboardingComponentRegistry}
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
      onFlowComplete={async () => {
        // Persist the done state so the modal never re-appears on a later mount.
        await AsyncStorage.setItem(DONE_KEY, '1');
        await AsyncStorage.removeItem(STORAGE_KEY);
      }}
    >
      <OnboardingOverlay>{children}</OnboardingOverlay>
    </OnboardingProvider>
  );
}
