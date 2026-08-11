import { useOverview } from '@vautr/ui-logic';
import { useEffect, useRef, useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { getClient } from '../lib/client';
import { SecretHandleScope } from '../lib/secretLifecycle';

interface DetailScreenProps {
  uuid: string;
  onBack: () => void;
}

/**
 * Item detail. Follows the opaque-handle lifecycle (ui-state-charts §3): the
 * handle lives in a `SecretHandleScope` held in a `useRef` (never global state),
 * reveal happens on mount, and `dispose()` (release_secret) runs on unmount,
 * zeroizing the in-memory secret. The plaintext never enters JS state.
 */
export function DetailScreen({ uuid, onBack }: DetailScreenProps) {
  const overview = useOverview(uuid);
  const scopeRef = useRef<SecretHandleScope | null>(null);
  const [masked, setMasked] = useState(true);
  const [revealError, setRevealError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const client = await getClient();
        const handle = await client.reveal(uuid);
        if (!active) {
          // Unmounted before reveal resolved: release immediately.
          await client.release(handle);
          return;
        }
        const scope = new SecretHandleScope(client);
        scope.set(handle);
        scopeRef.current = scope;
      } catch (err) {
        if (active) {
          setRevealError(err instanceof Error ? err.message : 'Reveal failed.');
        }
      }
    })();
    return () => {
      active = false;
      const scope = scopeRef.current;
      scopeRef.current = null;
      if (scope) void scope.dispose();
    };
  }, [uuid]);

  if (!overview) {
    return (
      <View style={styles.container}>
        <Text style={styles.muted}>Item not found.</Text>
        <Pressable accessibilityRole="button" onPress={onBack} style={styles.linkButton}>
          <Text style={styles.linkText}>Back to vault</Text>
        </Pressable>
      </View>
    );
  }

  return (
    <View style={styles.container}>
      <View style={styles.header}>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Back to vault list"
          onPress={onBack}
          style={styles.backButton}
        >
          <Text style={styles.backText}>‹</Text>
        </Pressable>
        <Text style={styles.title} numberOfLines={1}>
          {overview.title}
        </Text>
      </View>

      <View style={styles.body}>
        <Text style={styles.fieldLabel}>Username</Text>
        <Text style={styles.fieldValue}>{overview.subtitle}</Text>

        <Text style={styles.fieldLabel}>Password</Text>
        <View style={styles.row}>
          <Text style={[styles.masked, styles.fieldValue]}>
            {masked ? '••••••••••' : '••••••••••'}
          </Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={masked ? 'Show password' : 'Hide password'}
            onPress={() => setMasked((value) => !value)}
            style={styles.smallButton}
          >
            <Text style={styles.smallButtonText}>{masked ? 'Show' : 'Hide'}</Text>
          </Pressable>
        </View>

        {revealError ? (
          <Text accessibilityRole="alert" style={styles.error}>
            {revealError}
          </Text>
        ) : null}

        {overview.urls.length > 0 ? (
          <>
            <Text style={styles.fieldLabel}>Website</Text>
            <Text style={styles.linkText}>{overview.urls[0]}</Text>
          </>
        ) : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#0d0f14',
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    borderBottomWidth: 1,
    borderBottomColor: '#232936',
    paddingVertical: 12,
    paddingHorizontal: 16,
  },
  backButton: {
    paddingRight: 12,
    paddingVertical: 4,
  },
  backText: {
    color: '#9aa3b2',
    fontSize: 28,
    lineHeight: 28,
  },
  title: {
    flex: 1,
    fontSize: 18,
    fontWeight: '600',
    color: '#f5f7fa',
  },
  body: {
    padding: 20,
  },
  fieldLabel: {
    fontSize: 12,
    fontWeight: '600',
    textTransform: 'uppercase',
    letterSpacing: 0.6,
    color: '#9aa3b2',
    marginTop: 18,
    marginBottom: 4,
  },
  fieldValue: {
    fontSize: 16,
    color: '#f5f7fa',
  },
  masked: {
    fontVariant: ['tabular-nums'],
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  smallButton: {
    backgroundColor: '#2a3140',
    borderRadius: 8,
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
  smallButtonText: {
    color: '#f5f7fa',
    fontSize: 14,
    fontWeight: '500',
  },
  error: {
    color: '#f85149',
    fontSize: 13,
    marginTop: 12,
  },
  linkButton: {
    marginTop: 8,
  },
  linkText: {
    color: '#2f6fed',
    fontSize: 15,
    textDecorationLine: 'underline',
  },
  muted: {
    color: '#9aa3b2',
    fontSize: 15,
    textAlign: 'center',
    marginTop: 40,
  },
});
