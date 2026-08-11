import * as LocalAuthentication from 'expo-local-authentication';

/** Result of attempting a biometric gate. */
export interface BiometricGate {
  success: boolean;
  /** Reason when `success` is false. */
  reason?: 'no_hardware' | 'cancelled' | 'failed' | 'not_enrolled';
}

/**
 * Gate an action (e.g. revealing a secret) behind the device biometrics.
 * Uses `expo-local-authentication`; on devices without hardware the gate passes
 * through (so the app remains usable on simulators).
 */
export async function requireBiometric(promptMessage = 'Vautr unlock'): Promise<BiometricGate> {
  const hasHardware = await LocalAuthentication.hasHardwareAsync();
  if (!hasHardware) {
    return { success: true, reason: 'no_hardware' };
  }
  const enrolled = await LocalAuthentication.isEnrolledAsync();
  if (!enrolled) {
    return { success: false, reason: 'not_enrolled' };
  }
  const result = await LocalAuthentication.authenticateAsync({
    promptMessage,
    cancelLabel: 'Cancel',
    disableDeviceFallback: false,
  });
  return { success: result.success, reason: result.success ? undefined : 'cancelled' };
}

/** Whether the device supports biometric authentication. */
export async function canUseBiometrics(): Promise<boolean> {
  return (
    (await LocalAuthentication.hasHardwareAsync()) &&
    (await LocalAuthentication.isEnrolledAsync())
  );
}
