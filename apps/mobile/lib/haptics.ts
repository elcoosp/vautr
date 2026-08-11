import { useCallback } from 'react';
import * as Haptics from 'expo-haptics';

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
