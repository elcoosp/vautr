import { useEffect, useState } from 'react';
import { Modal, Pressable, Text, View } from 'react-native';
import { Button, ButtonText } from '../../components/ui/button';
import { TOUR_STEPS } from './steps';

export const TOUR_REPLAY_EVENT = 'vautr:replay-tour';

/**
 * Feature-tour overlay (VTR-077, mobile). RN has no element-measurement
 * primitive, so this is a centered-card sequence (not pixel-anchored) — same
 * visual language as first-run onboarding. Anchored tours on RN would require
 * `measure()` and are a follow-up.
 */
export function TourOverlay() {
  const [index, setIndex] = useState<number | null>(null);

  useEffect(() => {
    const onReplay = () => setIndex(0);
    window.addEventListener(TOUR_REPLAY_EVENT, onReplay);
    return () => window.removeEventListener(TOUR_REPLAY_EVENT, onReplay);
  }, []);

  if (index === null) return null;
  const step = TOUR_STEPS[index];
  if (!step) return null;
  const isLast = index + 1 >= TOUR_STEPS.length;
  const finish = () => setIndex(null);
  const next = () => (isLast ? finish() : setIndex(index + 1));
  const back = () => setIndex(Math.max(0, index - 1));

  return (
    <Modal transparent animationType="fade" visible onRequestClose={finish}>
      <Pressable className="flex-1 items-center justify-center bg-black/60" onPress={back}>
        <Pressable className="w-[88%] max-w-[380px] rounded-2xl border border-border bg-surface p-5">
          <Text className="mb-1 text-xs text-text-muted">
            Step {index + 1} of {TOUR_STEPS.length}
          </Text>
          <Text className="mb-1 text-lg font-semibold text-foreground">{step.title}</Text>
          <Text className="mb-4 text-sm text-text-muted">{step.body}</Text>
          <View className="flex-row items-center justify-between">
            <Button variant="ghost" disabled={index === 0} onPress={back}>
              <ButtonText>Back</ButtonText>
            </Button>
            <View className="flex-row gap-2">
              <Button variant="ghost" onPress={finish}>
                <ButtonText>Skip</ButtonText>
              </Button>
              <Button onPress={next}>
                <ButtonText>{isLast ? 'Finish' : 'Next'}</ButtonText>
              </Button>
            </View>
          </View>
        </Pressable>
      </Pressable>
    </Modal>
  );
}

export function triggerReplayTour() {
  window.dispatchEvent(new Event(TOUR_REPLAY_EVENT));
}
