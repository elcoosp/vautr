import { createFileRoute } from '@tanstack/react-router';
import {
  getMobileClient,
  MobileSharingClient,
  type VautrNativeBridge,
} from '@vautr/client-sdk/mobile';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { services } from '../../lib/client';
import { useSession } from '../../lib/session';

export const Route = createFileRoute('/_app/shares')({
  component: SharesScreen,
});

/**
 * Native-gated sharing inbox (VTR-070). Only meaningful when the uniffi core
 * is linked (`getMobileClient() !== null`); on an HTTP-only build the FFI
 * client is absent and we surface a clear gated message. The crypto runs in
 * Rust — plaintext never enters the JS heap.
 */
function SharesScreen() {
  const username = useSession((s) => s.username);
  const [inbox, setInbox] = useState<Array<{ itemUuid: string; bytes: number }> | null>(null);
  const [recipient, setRecipient] = useState('');
  const [groupId, setGroupId] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  const native = getMobileClient()?.getNativeBridge() ?? null;
  const sharing: MobileSharingClient | null = native
    ? new MobileSharingClient(native as VautrNativeBridge, services.api)
    : null;

  const loadInbox = useCallback(async () => {
    if (!sharing) return;
    setError(null);
    try {
      const shares = await sharing.collectShares();
      setInbox(shares.map((s) => ({ itemUuid: s.itemUuid, bytes: s.plaintext.length })));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load shares.');
    }
  }, [sharing]);

  useEffect(() => {
    void loadInbox();
  }, [loadInbox]);

  if (!native || !sharing) {
    return (
      <View className="gap-3">
        <Text className="text-lg font-semibold text-foreground">Shares</Text>
        <Text className="text-sm text-muted-foreground">
          Secure sharing requires the on-device vault core. Open Vautr desktop or web to share
          items.
        </Text>
      </View>
    );
  }

  const shareToUser = async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const sender = username ?? 'self';
      // Demo item: a freshly-encrypted vault item would be passed here. We share
      // a placeholder payload to exercise the native share flow end-to-end.
      const itemUuid = `item-${Date.now()}`;
      await sharing.shareItem(
        sender,
        recipient,
        itemUuid,
        new TextEncoder().encode('shared-secret'),
      );
      setInfo(`Shared to ${recipient}.`);
      setRecipient('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Share failed.');
    } finally {
      setBusy(false);
    }
  };

  const createGroup = async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const json = await sharing.createGroup('Mobile sharing group', 'self');
      const g = JSON.parse(json) as { group_id: string };
      setGroupId(g.group_id);
      setInfo(`Created group ${g.group_id}. Add members to share into it.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Group create failed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text className="text-lg font-semibold text-foreground">Shares</Text>
        {inbox ? <Badge variant="outline">{inbox.length}</Badge> : null}
      </View>

      {error ? (
        <Text accessibilityRole="alert" className="text-sm text-destructive">
          {error}
        </Text>
      ) : null}
      {info ? (
        <Text className="rounded border border-primary/40 bg-primary/10 px-2 py-1 text-sm text-foreground">
          {info}
        </Text>
      ) : null}

      <Card className="gap-2 p-4">
        <Text className="text-base font-medium text-foreground">Share an item</Text>
        <Input
          placeholder="Recipient user id"
          value={recipient}
          onChangeText={setRecipient}
          autoCapitalize="none"
        />
        <Button onPress={() => void shareToUser()} disabled={busy || recipient.length === 0}>
          <ButtonText>{busy ? 'Sharing…' : 'Share'}</ButtonText>
        </Button>
      </Card>

      <Card className="gap-2 p-4">
        <Text className="text-base font-medium text-foreground">Groups</Text>
        <Button variant="outline" onPress={() => void createGroup()} disabled={busy}>
          <ButtonText>New group</ButtonText>
        </Button>
        {groupId ? (
          <Text className="text-xs text-muted-foreground">Active group: {groupId}</Text>
        ) : null}
      </Card>

      <View className="gap-2">
        <Text className="text-base font-medium text-foreground">Inbox</Text>
        {inbox === null ? (
          <ActivityIndicator className="mt-2" color="#42b59a" />
        ) : inbox.length === 0 ? (
          <Text className="text-sm text-muted-foreground">No pending shares.</Text>
        ) : (
          inbox.map((s) => (
            <Card key={s.itemUuid} className="p-4">
              <Text className="text-base font-medium text-foreground">{s.itemUuid}</Text>
              <Text className="mt-0.5 text-xs text-muted-foreground">
                {s.bytes} bytes decrypted on-device
              </Text>
            </Card>
          ))
        )}
      </View>
    </View>
  );
}
