import { useIsLocked } from '@vautr/ui-logic';
import { useEffect, useState, useTransition } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';

import { getClient } from './src/lib/client';
import { LockScreen } from './src/screens/LockScreen';
import { VaultScreen } from './src/screens/VaultScreen';

/**
 * Boot screen shown while `initializeVautrCore` links the Rust core into the
 * JSI instance (the "allocating" phase of the skill matrix boot pattern).
 */
function BootScreen({ error }: { error: string | null }) {
  return (
    <View style={styles.boot}>
      <ActivityIndicator size="large" color="#2f6fed" />
      <Text style={styles.bootText}>{error ?? 'Initializing secure vault…'}</Text>
      {error ? (
        <Pressable
          accessibilityRole="button"
          onPress={() => {
            // Re-render triggers a fresh boot attempt via App's effect re-run.
            getClient().catch(() => undefined);
          }}
          style={styles.retryButton}
        >
          <Text style={styles.retryText}>Retry</Text>
        </Pressable>
      ) : null}
    </View>
  );
}

export function App() {
  const isLocked = useIsLocked();
  const [initialized, setInitialized] = useState(false);
  const [bootError, setBootError] = useState<string | null>(null);
  const [, startTransition] = useTransition();

  useEffect(() => {
    let active = true;
    // useTransition keeps the JSI allocation off the critical UI path; we await
    // initializeVautrCore() on mount, set `initialized`, and capture the Rust
    // Result error via try/catch (.message) into `bootError`.
    startTransition(() => {
      void getClient()
        .then(() => {
          if (active) setInitialized(true);
        })
        .catch((err: unknown) => {
          if (active) setBootError(err instanceof Error ? err.message : String(err));
        });
    });
    return () => {
      active = false;
    };
  }, [startTransition]);

  return (
    <SafeAreaProvider>
      <SafeAreaView style={styles.safe} edges={['top', 'bottom']}>
        {!initialized ? (
          <BootScreen error={bootError} />
        ) : isLocked ? (
          <LockScreen />
        ) : (
          <VaultScreen />
        )}
      </SafeAreaView>
    </SafeAreaProvider>
  );
}

const styles = StyleSheet.create({
  safe: {
    flex: 1,
    backgroundColor: '#0d0f14',
  },
  boot: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: '#0d0f14',
  },
  bootText: {
    color: '#9aa3b2',
    fontSize: 15,
    marginTop: 16,
    textAlign: 'center',
    paddingHorizontal: 32,
  },
  retryButton: {
    marginTop: 16,
    backgroundColor: '#2f6fed',
    borderRadius: 10,
    paddingHorizontal: 20,
    paddingVertical: 10,
  },
  retryText: {
    color: '#ffffff',
    fontSize: 14,
    fontWeight: '600',
  },
});
