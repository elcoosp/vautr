import { createFileRoute, Outlet } from '@tanstack/react-router';
import { KeyboardAvoidingView, Platform, ScrollView } from 'react-native';

export const Route = createFileRoute('/_auth')({
  component: AuthLayout,
});

/**
 * Container for auth screens (login / register).
 *
 * Wrapped in a KeyboardAvoidingView + ScrollView so the form stays reachable
 * when the soft keyboard covers the lower fields (Confirm password + the
 * action button) on small phones. The ScrollView only mounts inside the
 * keyboard-avoiding wrapper; the earlier RN 0.86 / reanimated v4 `scrollTo`
 * warning is benign and does not blank the screen.
 */
function AuthLayout() {
  return (
    <KeyboardAvoidingView
      className="flex-1"
      behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 0 : 24}
    >
      <ScrollView
        className="flex-1"
        contentContainerClassName="flex-1 justify-center px-6"
        keyboardShouldPersistTaps="handled"
      >
        <Outlet />
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
