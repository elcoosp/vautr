import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { Users } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { EmptyState } from '@/popup/components/EmptyState';

interface GroupInboxEntry {
  group_id: string;
  name: string;
  admin_uuid: string;
  wrapped_sik: string | null;
  ephemeral_public_key: string | null;
}

interface GroupItem {
  group_id: string;
  item_uuid: string;
  payload: string;
}

interface Props {
  client: VautrWebClient;
  mlp: VautrMlpClient;
}

/**
 * Group sharing management (sharing-pki.md §6). Zero-knowledge: the Group SIK is
 * wrapped per member and the item payload is encrypted once under it; the server
 * only stores ciphertext. This tab lets the admin create groups + add members,
 * and any member unwrap the Group SIK and read group items.
 */
export function GroupsTab({ client, mlp }: Props) {
  const [groups, setGroups] = useState<GroupInboxEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Create-group form
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');

  // Add-member form (per group_id)
  const [addFor, setAddFor] = useState<string | null>(null);
  const [memberId, setMemberId] = useState('');

  // Group items (per group_id)
  const [itemsFor, setItemsFor] = useState<Record<string, GroupItem[]>>({});
  const [decrypted, setDecrypted] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const inbox = await client.getGroupInbox(mlp);
      setGroups(inbox);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load groups.');
    }
  }, [client, mlp]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleCreate(): Promise<void> {
    if (!newName) return;
    setBusy(true);
    setError(null);
    try {
      await client.createGroup(mlp, newName);
      setNewName('');
      setShowCreate(false);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create group.');
    } finally {
      setBusy(false);
    }
  }

  async function handleAddMember(groupId: string): Promise<void> {
    if (!memberId) return;
    setBusy(true);
    setError(null);
    try {
      const groupJson = await client.getGroupKey(mlp, groupId);
      if (!groupJson) {
        throw new Error('admin group key not found locally; only the admin can add members');
      }
      await client.addGroupMember(mlp, groupJson, memberId);
      setMemberId('');
      setAddFor(null);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add member.');
    } finally {
      setBusy(false);
    }
  }

  async function handleUnwrap(entry: GroupInboxEntry): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      await client.unwrapGroupKey(mlp, entry);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to unwrap group key.');
    } finally {
      setBusy(false);
    }
  }

  async function handleListItems(groupId: string): Promise<void> {
    setError(null);
    try {
      const items = await client.getGroupItems(mlp, groupId);
      setItemsFor((prev) => ({ ...prev, [groupId]: items }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to list group items.');
    }
  }

  async function handleDecrypt(groupId: string, item: GroupItem): Promise<void> {
    setError(null);
    try {
      const groupJson = await client.getGroupKey(mlp, groupId);
      if (!groupJson) {
        throw new Error('group key not available locally; unwrap it first');
      }
      const pt = await client.acceptGroupItem(groupJson, item.item_uuid, item.payload);
      setDecrypted((prev) => ({
        ...prev,
        [`${groupId}:${item.item_uuid}`]: new TextDecoder().decode(pt),
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to decrypt group item.');
    }
  }

  async function handleRevokeItem(groupId: string, itemUuid: string): Promise<void> {
    setError(null);
    try {
      await client.revokeGroupItem(mlp, groupId, itemUuid);
      await handleListItems(groupId);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke group item.');
    }
  }

  return (
    <div className="space-y-3 p-3 text-sm text-text">
      <div className="flex items-center justify-between">
        <span className="font-medium">Groups</span>
        <Button variant="outline" size="sm" onClick={() => setShowCreate((v) => !v)}>
          New group
        </Button>
      </div>

      {error ? <p className="text-xs text-destructive">{error}</p> : null}

      {showCreate ? (
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm">New group</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Group name"
              className="w-full rounded border border-border bg-bg px-2 py-1"
            />
            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
              <Button size="sm" onClick={() => void handleCreate()} disabled={busy}>
                Create
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      {groups.length === 0 ? (
        <EmptyState variant="inline" icon={Users} title="No groups yet." />
      ) : (
        <ul className="space-y-2">
          {groups.map((g) => (
            <li key={g.group_id} className="rounded-md border border-border p-2">
              <div className="flex items-center justify-between">
                <span className="font-medium text-text">{g.name}</span>
                <span className="text-xs text-text-muted">{g.group_id.slice(0, 8)}</span>
              </div>
              <p className="text-xs text-text-muted">Admin: {g.admin_uuid.slice(0, 8)}</p>

              <div className="mt-2 flex flex-wrap gap-2">
                {g.wrapped_sik ? (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => void handleUnwrap(g)}
                    disabled={busy}
                  >
                    Unwrap key
                  </Button>
                ) : (
                  <Badge variant="secondary">admin</Badge>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void handleListItems(g.group_id)}
                >
                  Items
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setAddFor(addFor === g.group_id ? null : g.group_id)}
                  disabled={busy}
                >
                  Add member
                </Button>
              </div>

              {addFor === g.group_id ? (
                <div className="mt-2 flex gap-2">
                  <input
                    value={memberId}
                    onChange={(e) => setMemberId(e.target.value)}
                    placeholder="member user id"
                    className="flex-1 rounded border border-border bg-bg px-2 py-1 text-xs"
                  />
                  <Button
                    size="sm"
                    onClick={() => void handleAddMember(g.group_id)}
                    disabled={busy}
                  >
                    Add
                  </Button>
                </div>
              ) : null}

              {itemsFor[g.group_id] ? (
                <ul className="mt-2 space-y-1">
                  {itemsFor[g.group_id]?.map((it) => {
                    const key = `${g.group_id}:${it.item_uuid}`;
                    return (
                      <li
                        key={it.item_uuid}
                        className="rounded border border-border bg-bg p-1 text-xs"
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-text">Item {it.item_uuid.slice(0, 8)}</span>
                          <div className="flex gap-1">
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-6 px-1.5 text-xs"
                              onClick={() => void handleDecrypt(g.group_id, it)}
                            >
                              Decrypt
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-6 px-1.5 text-xs text-destructive"
                              onClick={() => void handleRevokeItem(g.group_id, it.item_uuid)}
                            >
                              Revoke
                            </Button>
                          </div>
                        </div>
                        {decrypted[key] ? (
                          <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap text-text">
                            {decrypted[key]}
                          </pre>
                        ) : null}
                      </li>
                    );
                  })}
                </ul>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      <p className="text-xs text-text-muted">
        <Users className="mr-1 inline size-3" aria-hidden="true" />
        Group payloads are decrypted locally; the server only stores ciphertext.
      </p>
    </div>
  );
}
