import { createFileRoute, Outlet } from '@tanstack/react-router';
import { KeyboardAvoidingView, Platform, View } from 'react-native';

export const Route = createFileRoute('/_auth')({
  component: AuthLayout,
});

/**
 * Container for auth screens (login / register).
 *
 * Uses `KeyboardAvoidingView` (no nested `ScrollView`) so the form stays above
 * the soft keyboard on small phones. A nested `ScrollView` previously triggered
 * a reanimated v4 / RN 0.86 new-arch `ReferenceError: Property 'scrollTo'
 * doesn't exist` at runtime, so we avoid it — `KeyboardAvoidingView` alone
 * provides the keyboard-reachability behavior without the crash.
 */
function AuthLayout() {
  return (
    <KeyboardAvoidingView
      className="flex-1"
      behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 0 : 24}
    >
      <View className="flex-1 justify-center px-6">
        <Outlet />
      </View>
    </KeyboardAvoidingView>
  );
}
