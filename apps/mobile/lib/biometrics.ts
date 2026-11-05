import * as LocalAuthentication from 'expo-local-authentication';

/** Result of attempting a biometric gate. */
export interface BiometricGate {
  success: boolean;
  /** Reason when `success` is false. */
  reason?: 'no_hardware' | 'cancelled' | 'failed' | 'not_enrolled';
}

/**
 * Gate an action (e.g. revealing a secret) behind the device biometrics.
 * Uses `expo-local-authentication`. Fail-closed (VTRFIX-SEC-C02): on devices
 * without biometric hardware the gate DENIES instead of passing through, so a
 * simulator or stripped-down device can never silently bypass a protected
 * action. Callers should surface `reason` as a friendly message.
 */
export async function requireBiometric(promptMessage = 'Vautr unlock'): Promise<BiometricGate> {
  const hasHardware = await LocalAuthentication.hasHardwareAsync();
  if (!hasHardware) {
    return { success: false, reason: 'no_hardware' };
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
    (await LocalAuthentication.hasHardwareAsync()) && (await LocalAuthentication.isEnrolledAsync())
  );
}

/**
 * Friendly, user-facing message for a denied gate. Callers pass `gate.reason`
 * from `requireBiometric`; the wording explains WHY the gate failed instead of
 * a generic "authentication failed" (VTRFIX-SEC-C02 fail-closed follow-up).
 */
export function describeBiometricDenial(
  reason: BiometricGate['reason'],
  subject = 'this action',
): string {
  switch (reason) {
    case 'no_hardware':
      return `This device has no biometric hardware, so ${subject} is unavailable.`;
    case 'not_enrolled':
      return 'No biometrics are enrolled on this device. Enroll them in system settings first.';
    case 'cancelled':
      return 'Authentication cancelled.';
    default:
      return 'Authentication failed.';
  }
}
