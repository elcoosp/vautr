import { Users } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import {
  acceptGroupItem,
  addGroupMember,
  createGroup,
  getGroupInbox,
  getGroupItems,
  getGroupKey,
  revokeGroupItem,
  unwrapGroupKey,
} from '../lib/client';

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

/**
 * Group sharing management (sharing-pki.md §6). Zero-knowledge: the Group SIK is
 * wrapped per member and items are encrypted once under it; the server only
 * stores ciphertext. Members unwrap the Group SIK locally and decrypt items.
 */
export function GroupsView() {
  const [groups, setGroups] = useState<GroupInboxEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');

  const [addFor, setAddFor] = useState<string | null>(null);
  const [memberId, setMemberId] = useState('');

  const [itemsFor, setItemsFor] = useState<Record<string, GroupItem[]>>({});
  const [decrypted, setDecrypted] = useState<Record<string, string>>({});

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const inbox = await getGroupInbox();
      setGroups(inbox);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load groups.');
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleCreate(): Promise<void> {
    if (!newName) return;
    setBusy(true);
    setError(null);
    try {
      await createGroup(newName);
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
      const groupJson = await getGroupKey(groupId);
      if (!groupJson) {
        throw new Error('admin group key not found locally; only the admin can add members');
      }
      await addGroupMember(groupJson, memberId);
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
      await unwrapGroupKey(entry);
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
      const items = await getGroupItems(groupId);
      setItemsFor((prev) => ({ ...prev, [groupId]: items }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to list group items.');
    }
  }

  async function handleDecrypt(groupId: string, item: GroupItem): Promise<void> {
    setError(null);
    try {
      const groupJson = await getGroupKey(groupId);
      if (!groupJson) {
        throw new Error('group key not available locally; unwrap it first');
      }
      const pt = await acceptGroupItem(groupJson, item.item_uuid, item.payload);
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
      await revokeGroupItem(groupId, itemUuid);
      await handleListItems(groupId);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke group item.');
    }
  }

  return (
    <div className="mx-auto max-w-3xl space-y-4 p-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-text">Groups</h2>
        <button
          type="button"
          onClick={() => setShowCreate((v) => !v)}
          className="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-surface-raised"
        >
          New group
        </button>
      </div>

      {error ? (
        <p className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {error}
        </p>
      ) : null}

      {showCreate ? (
        <div className="space-y-2 rounded-md border border-border p-3">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Group name"
            className="w-full rounded border border-border bg-bg px-2 py-1 text-sm"
          />
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => setShowCreate(false)}
              className="rounded border border-border px-3 py-1 text-sm"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void handleCreate()}
              disabled={busy}
              className="rounded bg-accent px-3 py-1 text-sm font-medium text-accent-ink disabled:opacity-50"
            >
              Create
            </button>
          </div>
        </div>
      ) : null}

      {groups.length === 0 ? (
        <p className="text-sm text-text-muted">No groups yet.</p>
      ) : (
        <ul className="space-y-3">
          {groups.map((g) => (
            <li key={g.group_id} className="rounded-md border border-border p-3">
              <div className="flex items-center justify-between">
                <span className="font-medium text-text">{g.name}</span>
                <span className="text-xs text-text-muted">{g.group_id.slice(0, 8)}</span>
              </div>
              <p className="text-xs text-text-muted">Admin: {g.admin_uuid.slice(0, 8)}</p>

              <div className="mt-3 flex flex-wrap gap-2">
                {g.wrapped_sik ? (
                  <button
                    type="button"
                    onClick={() => void handleUnwrap(g)}
                    disabled={busy}
                    className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-raised disabled:opacity-50"
                  >
                    Unwrap key
                  </button>
                ) : (
                  <span className="rounded bg-surface-raised px-2 py-1 text-xs text-text-muted">
                    admin
                  </span>
                )}
                <button
                  type="button"
                  onClick={() => void handleListItems(g.group_id)}
                  className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-raised"
                >
                  Items
                </button>
                <button
                  type="button"
                  onClick={() => setAddFor(addFor === g.group_id ? null : g.group_id)}
                  disabled={busy}
                  className="rounded border border-border px-3 py-1 text-sm hover:bg-surface-raised disabled:opacity-50"
                >
                  Add member
                </button>
              </div>

              {addFor === g.group_id ? (
                <div className="mt-2 flex gap-2">
                  <input
                    value={memberId}
                    onChange={(e) => setMemberId(e.target.value)}
                    placeholder="member user id"
                    className="flex-1 rounded border border-border bg-bg px-2 py-1 text-sm"
                  />
                  <button
                    type="button"
                    onClick={() => void handleAddMember(g.group_id)}
                    disabled={busy}
                    className="rounded bg-accent px-3 py-1 text-sm font-medium text-accent-ink disabled:opacity-50"
                  >
                    Add
                  </button>
                </div>
              ) : null}

              {itemsFor[g.group_id]
                ? (() => {
                    const items = itemsFor[g.group_id] ?? [];
                    return (
                      <ul className="mt-2 space-y-1">
                        {items.map((it) => {
                          const key = `${g.group_id}:${it.item_uuid}`;
                          return (
                            <li
                              key={it.item_uuid}
                              className="rounded border border-border bg-bg p-2 text-sm"
                            >
                              <div className="flex items-center justify-between">
                                <span className="text-text">Item {it.item_uuid.slice(0, 8)}</span>
                                <div className="flex gap-1">
                                  <button
                                    type="button"
                                    onClick={() => void handleDecrypt(g.group_id, it)}
                                    className="rounded border border-border px-2 py-0.5 text-xs hover:bg-surface-raised"
                                  >
                                    Decrypt
                                  </button>
                                  <button
                                    type="button"
                                    onClick={() => void handleRevokeItem(g.group_id, it.item_uuid)}
                                    className="rounded px-2 py-0.5 text-xs text-destructive hover:bg-destructive/10"
                                  >
                                    Revoke
                                  </button>
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
                    );
                  })()
                : null}
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
