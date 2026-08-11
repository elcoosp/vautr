import { useVaultActions, vaultEventBus } from '@vautr/ui-logic';
import { useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { getClient } from '../lib/client';
import { makeDemoKey, secureEnclaveBridge } from '../native';

/**
 * Lock screen. Unlocks via OS-keystore biometrics (SVK) when an SVK is stored,
 * or via the master password otherwise. Follows the web UnlockScreen flow:
 * unlock -> list -> hydrate the store -> transition to the vault.
 */
export function LockScreen() {
  const { unlock } = useVaultActions();
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hasBio, setHasBio] = useState(false);

  useEffect(() => {
    let active = true;
    void secureEnclaveBridge
      .hasSvk()
      .then((present) => {
        if (active) setHasBio(present);
      })
      .catch(() => {
        if (active) setHasBio(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const enterVault = async (rawKey: Uint8Array) => {
    setBusy(true);
    setError(null);
    try {
      const client = await getClient();
      // JSI call (Result errors surface as .message on throw).
      await client.unlock(rawKey, 1);
      const overviews = await client.listOverviews();
      for (const overview of overviews) {
        vaultEventBus.emit({ type: 'OverviewUpserted', overview });
      }
      unlock();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unlock failed.');
    } finally {
      setBusy(false);
    }
  };

  const onBiometric = async () => {
    if (busy) return;
    const svk = await secureEnclaveBridge.loadSvk();
    if (!svk) {
      setError('No vault key stored. Use your master password instead.');
      return;
    }
    await enterVault(svk);
  };

  const onMasterPassword = () => {
    if (!password) {
      setError('Enter your master password.');
      return;
    }
    void enterVault(makeDemoKey(password));
  };

  return (
    <View style={styles.container}>
      <Text style={styles.brand}>Vautr</Text>
      <Text style={styles.subtitle}>Unlock your vault to view saved items.</Text>

      {hasBio ? (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Unlock with biometrics"
          onPress={() => void onBiometric()}
          disabled={busy}
          style={({ pressed }) => [styles.bioButton, pressed && styles.pressed]}
        >
          {busy ? (
            <ActivityIndicator color="#ffffff" />
          ) : (
            <Text style={styles.bioButtonText}>Unlock with Face ID / Touch ID</Text>
          )}
        </Pressable>
      ) : null}

      {hasBio ? <View style={styles.divider} /> : null}

      <Text style={styles.label}>Master password</Text>
      <TextInput
        value={password}
        onChangeText={setPassword}
        secureTextEntry
        autoComplete="current-password"
        placeholder="••••••••"
        placeholderTextColor="#6b7280"
        style={styles.input}
      />

      {error ? (
        <Text accessibilityRole="alert" style={styles.error}>
          {error}
        </Text>
      ) : null}

      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Unlock vault"
        onPress={onMasterPassword}
        disabled={busy}
        style={({ pressed }) => [styles.unlockButton, pressed && styles.pressed]}
      >
        <Text style={styles.unlockButtonText}>{busy ? 'Unlocking…' : 'Unlock vault'}</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    justifyContent: 'center',
    padding: 24,
    backgroundColor: '#0d0f14',
  },
  brand: {
    fontSize: 30,
    fontWeight: '700',
    color: '#f5f7fa',
    textAlign: 'center',
  },
  subtitle: {
    marginTop: 6,
    marginBottom: 28,
    fontSize: 14,
    color: '#9aa3b2',
    textAlign: 'center',
  },
  bioButton: {
    backgroundColor: '#2f6fed',
    borderRadius: 12,
    paddingVertical: 14,
    alignItems: 'center',
  },
  bioButtonText: {
    color: '#ffffff',
    fontSize: 15,
    fontWeight: '600',
  },
  divider: {
    height: 1,
    backgroundColor: '#232936',
    marginVertical: 20,
  },
  label: {
    fontSize: 13,
    color: '#9aa3b2',
    marginBottom: 6,
  },
  input: {
    backgroundColor: '#151a24',
    borderColor: '#2a3140',
    borderWidth: 1,
    borderRadius: 12,
    paddingHorizontal: 14,
    paddingVertical: 12,
    color: '#f5f7fa',
    fontSize: 15,
    marginBottom: 12,
  },
  error: {
    color: '#f85149',
    fontSize: 13,
    marginBottom: 12,
  },
  unlockButton: {
    backgroundColor: '#2f6fed',
    borderRadius: 12,
    paddingVertical: 14,
    alignItems: 'center',
  },
  unlockButtonText: {
    color: '#ffffff',
    fontSize: 15,
    fontWeight: '600',
  },
  pressed: {
    opacity: 0.85,
  },
});
