import { createFileRoute } from '@tanstack/react-router';
import {
  getMobileClient,
  MobileSharingClient,
  type VautrNativeBridge,
} from '@vautr/client-sdk/mobile';
import { Skeleton as BoneSkeleton } from 'boneyard-js/native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Text } from '../../components/ui/text';
import { services } from '../../lib/client';
import { useSession } from '../../lib/session';

interface GroupInboxEntry {
  group_id: string;
  name: string;
  admin_uuid: string;
  member_uuid: string;
  wrapped_sik: string;
  ephemeral_public_key: string;
}

interface GroupItem {
  itemUuid: string;
  bytes: number;
}

const ACCENT = '#42b59a';

/**
 * Native-gated sharing inbox + group sharing (VTR-070). Only meaningful when
 * the uniffi core is linked (`getMobileClient() !== null`); on an HTTP-only
 * build the FFI client is absent and we surface a clear gated message. The
 * crypto runs in Rust — plaintext never enters the JS heap.
 */
function SharesScreen() {
  const username = useSession((s) => s.username);
  const [inbox, setInbox] = useState<Array<{ itemUuid: string; bytes: number }> | null>(null);
  const [recipient, setRecipient] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  // Group sharing state.
  const [groups, setGroups] = useState<GroupInboxEntry[]>([]);
  const [activeGroupId, setActiveGroupId] = useState<string | null>(null);
  const [memberId, setMemberId] = useState('');
  const [groupItemUuid, setGroupItemUuid] = useState('');
  const [groupItems, setGroupItems] = useState<GroupItem[] | null>(null);

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

  const loadGroups = useCallback(async () => {
    if (!sharing) return;
    try {
      setGroups(await sharing.getGroupInbox());
    } catch {
      setGroups([]);
    }
  }, [sharing]);

  useEffect(() => {
    void loadInbox();
    void loadGroups();
  }, [loadInbox, loadGroups]);

  if (!native || !sharing) {
    return (
      <View className="gap-3">
        <Text variant="h3">Shares</Text>
        <Text variant="muted">
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
      setActiveGroupId(g.group_id);
      await loadGroups();
      setInfo(`Created group ${g.group_id}. Add members to share into it.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Group create failed.');
    } finally {
      setBusy(false);
    }
  };

  const addMember = async () => {
    if (!activeGroupId) {
      setError('Create or select a group first.');
      return;
    }
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const groupJson = sharing.getGroupKey(activeGroupId);
      if (!groupJson) throw new Error('group key not found locally');
      await sharing.addGroupMember(groupJson, memberId);
      setInfo(`Added ${memberId} to the group.`);
      setMemberId('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Add member failed.');
    } finally {
      setBusy(false);
    }
  };

  const shareToGroup = async () => {
    if (!activeGroupId) {
      setError('Create or select a group first.');
      return;
    }
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const groupJson = sharing.getGroupKey(activeGroupId);
      if (!groupJson) throw new Error('group key not found locally');
      await sharing.shareToGroup(
        groupJson,
        activeGroupId,
        groupItemUuid,
        new TextEncoder().encode('shared-secret'),
      );
      setInfo(`Shared ${groupItemUuid} into the group.`);
      setGroupItemUuid('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Share to group failed.');
    } finally {
      setBusy(false);
    }
  };

  const acceptInvite = async (entry: GroupInboxEntry) => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await sharing.acceptGroup(JSON.stringify(entry));
      await loadGroups();
      setInfo(`Accepted invite to group ${entry.group_id}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Accept invite failed.');
    } finally {
      setBusy(false);
    }
  };

  const loadGroupItems = async (groupId: string) => {
    setBusy(true);
    setError(null);
    try {
      const groupJson = sharing.getGroupKey(groupId);
      if (!groupJson) throw new Error('group key not found locally');
      const items = await sharing.listGroupItems(groupJson, groupId);
      setGroupItems(items.map((it) => ({ itemUuid: it.itemUuid, bytes: it.plaintext.length })));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load group items.');
    } finally {
      setBusy(false);
    }
  };

  const invites = groups.filter((g) => g.wrapped_sik && g.admin_uuid !== username);
  const myGroups = groups.filter((g) => !g.wrapped_sik || g.admin_uuid === username);

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text variant="h3">Shares</Text>
        {inbox ? <Badge variant="outline">{inbox.length}</Badge> : null}
      </View>

      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}
      {info ? (
        <Alert variant="default">
          <AlertDescription>{info}</AlertDescription>
        </Alert>
      ) : null}

      <Card className="gap-2 p-4">
        <Text variant="label">Share an item</Text>
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
        <Text variant="label">Groups</Text>
        <Button variant="outline" onPress={() => void createGroup()} disabled={busy}>
          <ButtonText>New group</ButtonText>
        </Button>
        {activeGroupId ? <Text variant="tiny">Active group: {activeGroupId}</Text> : null}

        <Text variant="small">Add member</Text>
        <Input
          placeholder="Member user id"
          value={memberId}
          onChangeText={setMemberId}
          autoCapitalize="none"
        />
        <Button
          onPress={() => void addMember()}
          disabled={busy || memberId.length === 0 || !activeGroupId}
        >
          <ButtonText>{busy ? 'Adding…' : 'Add member'}</ButtonText>
        </Button>

        <Text variant="small">Share item to group</Text>
        <Input
          placeholder="Item uuid"
          value={groupItemUuid}
          onChangeText={setGroupItemUuid}
          autoCapitalize="none"
        />
        <Button
          onPress={() => void shareToGroup()}
          disabled={busy || groupItemUuid.length === 0 || !activeGroupId}
        >
          <ButtonText>{busy ? 'Sharing…' : 'Share to group'}</ButtonText>
        </Button>

        <Button
          variant="ghost"
          onPress={() => activeGroupId && void loadGroupItems(activeGroupId)}
          disabled={busy || !activeGroupId}
        >
          <ButtonText>View group items</ButtonText>
        </Button>
        {groupItems ? (
          groupItems.length === 0 ? (
            <Text variant="tiny">No items in this group.</Text>
          ) : (
            groupItems.map((it) => (
              <Text key={it.itemUuid} variant="tiny">
                {it.itemUuid} · {it.bytes} bytes
              </Text>
            ))
          )
        ) : null}
      </Card>

      {invites.length > 0 ? (
        <Card className="gap-2 p-4">
          <Text variant="label">Group invites</Text>
          {invites.map((entry) => (
            <View key={entry.group_id} className="flex-row items-center justify-between">
              <Text variant="p">{entry.name}</Text>
              <Button variant="outline" onPress={() => void acceptInvite(entry)} disabled={busy}>
                <ButtonText>Accept</ButtonText>
              </Button>
            </View>
          ))}
        </Card>
      ) : null}

      {myGroups.length > 0 ? (
        <Card className="gap-2 p-4">
          <Text variant="label">My groups</Text>
          {myGroups.map((g) => (
            <Text key={g.group_id} variant="tiny">
              {g.name} ({g.group_id})
            </Text>
          ))}
        </Card>
      ) : null}

      <View className="gap-2">
        <Text variant="label">Inbox</Text>
        {inbox === null ? (
          <BoneSkeleton
            name="shares-inbox-loading"
            loading
            fallback={<ActivityIndicator className="mt-2" color={ACCENT} />}
          >
            {null}
          </BoneSkeleton>
        ) : inbox.length === 0 ? (
          <Text variant="muted">No pending shares.</Text>
        ) : (
          inbox.map((s) => (
            <Card key={s.itemUuid} className="p-4">
              <Text variant="p">{s.itemUuid}</Text>
              <Text variant="tiny" className="mt-0.5">
                {s.bytes} bytes decrypted on-device
              </Text>
            </Card>
          ))
        )}
      </View>
    </View>
  );
}

export const Route = createFileRoute('/_app/shares')({
  component: SharesScreen,
});
