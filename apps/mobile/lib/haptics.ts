import * as Haptics from 'expo-haptics';
import { useCallback } from 'react';

/** Small haptics helper for feedback (expo-haptics). */
export function useHaptics() {
  const notifySuccess = useCallback(() => {
    void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success).catch(() => undefined);
  }, []);
  const notifyError = useCallback(() => {
    void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error).catch(() => undefined);
  }, []);
  const lightTap = useCallback(() => {
    void Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => undefined);
  }, []);
  return { notifySuccess, notifyError, lightTap };
}
