import { getMobileClient } from '@vautr/client-sdk/mobile';
import { Eye, EyeOff } from 'lucide-react-native';
import { useEffect, useRef, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { SecretHandleScope } from '../src/lib/secretLifecycle';
import { ThemedIcon } from './ui/icon';

interface SecretOverlayProps {
  /** The vault item uuid whose secret should be revealed in the native overlay. */
  uuid: string;
  /** Label shown next to the reveal toggle. */
  label: string;
}

/**
 * Reveal control for a vault secret. Tapping "Reveal" asks the native overlay to
 * render the plaintext (VTR-048, ADR-003): JS only ever forwards the opaque
 * handle to `renderInOverlay` — the secret string is delivered to the native
 * overlay by the Rust core and never enters the JS heap. On unmount the handle
 * is released (zeroized). When the uniffi native client is not linked (the
 * HTTP-only build), it degrades to a locked state.
 */
export function SecretOverlay({ uuid, label }: SecretOverlayProps) {
  const [revealed, setRevealed] = useState(false);
  const scopeRef = useRef<SecretHandleScope | null>(null);

  // Release the handle on unmount (zeroization) — TDD3 / acceptance criterion.
  useEffect(() => {
    return () => {
      void scopeRef.current?.dispose();
      scopeRef.current = null;
    };
  }, []);

  const toggle = async () => {
    const client = getMobileClient();
    if (!client) {
      // HTTP-only build: no local decryption; show the locked state.
      setRevealed((v) => !v);
      return;
    }
    if (scopeRef.current) {
      await scopeRef.current.dispose();
      scopeRef.current = null;
      setRevealed(false);
      return;
    }
    // Reveal returns an opaque handle (u64 as string) — never the plaintext.
    const handle = await client.reveal(uuid);
    const scope = new SecretHandleScope(client);
    scope.set(handle);
    // Delegate rendering to the native overlay; JS keeps only the handle.
    await scope.view();
    scopeRef.current = scope;
    setRevealed(true);
  };

  return (
    <View className="flex-row items-center justify-between">
      <Text className="flex-1 pr-2 text-base font-medium text-foreground">{label}</Text>
      <Pressable onPress={() => void toggle()} accessibilityRole="button">
        {revealed ? (
          <ThemedIcon icon={EyeOff} size={18} tone="muted" />
        ) : (
          <ThemedIcon icon={Eye} size={18} tone="muted" />
        )}
      </Pressable>
    </View>
  );
}
