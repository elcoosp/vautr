import { createRootRoute, Outlet } from '@tanstack/react-router';
import { View } from 'react-native';
import { SafeAreaProvider } from 'react-native-safe-area-context';

import { ToastProvider } from '../../components/ui/toast';

export const Route = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  return (
    <SafeAreaProvider>
      <ToastProvider>
        <View className="flex-1 bg-background">
          <Outlet />
        </View>
      </ToastProvider>
    </SafeAreaProvider>
  );
}
