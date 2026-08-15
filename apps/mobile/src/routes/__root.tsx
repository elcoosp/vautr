import { createRootRoute, Outlet } from '@tanstack/react-router';
import { useEffect } from 'react';
import { View } from 'react-native';
import { SafeAreaProvider } from 'react-native-safe-area-context';

import { ToastProvider } from '../../components/ui/toast';
import { bootVautrCore } from '../../lib/client';
import { TourOverlay } from '../tour/TourOverlay';

export const Route = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  // Boot the local vault core (VTR-048/056). No-op on HTTP-only builds where the
  // uniffi TurboModule is not linked; activates the opaque-handle secure-reveal
  // path once it is.
  useEffect(() => {
    // dbPath is only consumed by the uniffi core when the native module is
    // linked; on HTTP-only builds bootVautrCore is a no-op. RN has no `document`,
    // so a fixed app-data path is used (the native side resolves it per-platform).
    void bootVautrCore('vautr.sqlite');
  }, []);
  return (
    <SafeAreaProvider>
      <ToastProvider>
        <View className="flex-1 bg-background">
          <Outlet />
          {/* Feature-tour overlay (VTR-078): anchored spotlight layer above the app. */}
          <TourOverlay />
        </View>
      </ToastProvider>
    </SafeAreaProvider>
  );
}
