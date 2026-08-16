import { createFileRoute, Outlet } from '@tanstack/react-router';
import { View } from 'react-native';

export const Route = createFileRoute('/_auth')({
  component: AuthLayout,
});

/**
 * Centered container for auth screens. Uses a plain View (no ScrollView /
 * KeyboardAvoidingView): the auth forms are short and vertically centered.
 * Avoids mounting a ScrollView on the launch path, which triggers a benign
 * RN 0.86 / reanimated v4 new-architecture `scrollTo` warning.
 */
function AuthLayout() {
  return (
    <View className="flex-1 justify-center px-6">
      <Outlet />
    </View>
  );
}
