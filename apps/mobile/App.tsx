import { PortalHost } from '@rn-primitives/portal';
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
      {/* RN-primitives portal host: required for Dialog/Sheet/Select portals
          (drawer, bottom sheet, dropdowns) to have a render target. Without it
          they mount but render into nothing, so the drawer never appears.
          We also re-declare the design-token CSS variables here: @rn-primitives
          relocates portal children via a zustand store, and on native the
          `:root` variables from global.css don't reliably reach that relocated
          subtree — so text colors (text-foreground, etc.) fall back to RN's
          default black, producing black-on-black drawer/sheet content. Setting
          the vars explicitly on this wrapper makes them inherit into portals. */}
      <View className="[--background:226_18%_11%] [--foreground:220_14%_92%] [--card:226_16%_14%] [--card-foreground:220_14%_92%] [--popover:226_16%_14%] [--popover-foreground:220_14%_92%] [--primary:166_47%_48%] [--primary-foreground:226_18%_11%] [--secondary:226_14%_18%] [--secondary-foreground:220_14%_92%] [--muted:226_14%_18%] [--muted-foreground:224_10%_62%] [--accent:226_14%_18%] [--accent-foreground:220_14%_92%] [--destructive:351_60%_60%] [--destructive-foreground:226_18%_11%] [--border:226_14%_22%] [--input:226_14%_22%] [--ring:166_47%_48%]">
        <PortalHost />
      </View>
    </SafeAreaProvider>
  );
}
