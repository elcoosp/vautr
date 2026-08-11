import { createFileRoute, Outlet } from '@tanstack/react-router';
import { KeyboardAvoidingView, Platform, ScrollView } from 'react-native';

export const Route = createFileRoute('/_auth')({
  component: AuthLayout,
});

/** Centered, scrollable container for auth screens. */
function AuthLayout() {
  return (
    <KeyboardAvoidingView
      className="flex-1"
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
    >
      <ScrollView
        contentContainerClassName="flex-1 justify-center px-6"
        keyboardShouldPersistTaps="handled"
      >
        <Outlet />
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
