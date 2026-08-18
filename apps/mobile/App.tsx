import { RouterProvider } from '@tanstack/react-router';
import { useEffect } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { SafeAreaProvider } from 'react-native-safe-area-context';

import './global.css';

import { services } from './lib/client';
import { useSession } from './lib/session';
import { OnboardingFlow } from './src/onboarding/OnboardingFlow';
import { router } from './src/router';
import { TourOverlay } from './src/tour/TourOverlay';

/**
 * App root. Boots by restoring any persisted OPAQUE session token onto the API
 * client, then hands off to the TanStack Router. Unauthenticated users are
 * redirected to /login; authenticated users stay in the /_app shell.
 */
export function App() {
  const authenticated = useSession((s) => s.authenticated);
  const booting = useSession((s) => s.booting);
  const setBooting = useSession((s) => s.setBooting);
  const setAuthenticated = useSession((s) => s.setAuthenticated);
  const setUsername = useSession((s) => s.setUsername);

  // Restore persisted session on first mount.
  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const hasSession = await services.auth.hasSession();
        if (active && hasSession) {
          const ok = await services.auth.restore(services.api);
          if (ok) {
            const username = await services.auth.getUsername();
            if (active) {
              setAuthenticated(true);
              setUsername(username);
            }
          }
        }
      } finally {
        if (active) setBooting(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [setAuthenticated, setUsername, setBooting]);

  // Route redirect based on session state.
  useEffect(() => {
    if (booting) return;
    const current = router.state.location.pathname;
    if (!authenticated && current !== '/login') {
      void router.navigate({ to: '/login' });
    } else if (authenticated && current === '/login') {
      void router.navigate({ to: '/' });
    }
  }, [authenticated, booting]);

  if (booting) {
    return (
      <View className="flex-1 items-center justify-center bg-background">
        <ActivityIndicator size="large" color="#42b59a" />
        <Text className="mt-4 text-sm text-muted-foreground">Restoring secure session…</Text>
      </View>
    );
  }

  return (
    <SafeAreaProvider>
      <OnboardingFlow>
        <RouterProvider router={router} />
      </OnboardingFlow>
      <TourOverlay />
    </SafeAreaProvider>
  );
}
