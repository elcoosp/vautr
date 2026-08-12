import { createFileRoute, Link } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { mlp, MlpApiError } from '@/lib/mlp';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { Project, ProjectMember, Secret, UserGroup } from '@vautr/api-contract';
import { ArrowLeft, Eye, EyeOff, MoreHorizontal, Plus, Trash2, UserPlus } from 'lucide-react';

export const Route = createFileRoute('/_authed/projects/$uuid')({
  component: ProjectDetailPage,
});

const PERMISSIONS = [
  { value: 'can_view', label: 'Can View' },
  { value: 'can_edit', label: 'Can Edit' },
  { value: 'can_manage', label: 'Can Manage' },
];

function b64decode(value: string): string {
  try {
    return atob(value);
  } catch {
    return value;
  }
}

function b64encode(value: string): string {
  return btoa(value);
}

function ProjectDetailPage() {
  const { uuid } = Route.useParams();
  const [project, setProject] = useState<Project | null>(null);
  const [members, setMembers] = useState<ProjectMember[]>([]);
  const [secrets, setSecrets] = useState<Secret[]>([]);
  const [groups, setGroups] = useState<UserGroup[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState('secrets');

  const load = useCallback(async () => {
    try {
      const [p, m, s, g] = await Promise.all([
        mlp.getProject(uuid),
        mlp.listMembers(uuid).catch(() => ({ members: [] as ProjectMember[] })),
        mlp.listSecrets(uuid).catch(() => ({ secrets: [] as Secret[] })),
        mlp.listGroups(uuid).catch(() => ({ groups: [] as UserGroup[] })),
      ]);
      setProject(p);
      setMembers(m.members);
      setSecrets(s.secrets);
      setGroups(g.groups);
      setError(null);
    } catch (err) {
      setError(err instanceof MlpApiError ? err.message : String(err));
    }
  }, [uuid]);

  useEffect(() => {
    void load();
  }, [load]);

  if (error) {
    return (
      <div className="p-6">
        <p className="text-sm text-danger">{error}</p>
        <Link to="/projects" search={{ create: false }} className="text-accent underline">
          Back to projects
        </Link>
      </div>
    );
  }
  if (!project) {
    return <p className="p-6 text-sm text-text-muted">Loading…</p>;
  }

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Link
            to="/projects"
            search={{ create: false }}
            aria-label="Back to projects"
            className="text-text-muted hover:text-text"
          >
            <ArrowLeft className="size-5" aria-hidden="true" />
          </Link>
          <div>
            <h1 className="text-2xl font-semibold text-text">{project.name}</h1>
            <div className="mt-1 flex items-center gap-2">
              <Badge variant="secondary">{project.type}</Badge>
              {project.permission ? <Badge>{project.permission}</Badge> : null}
              <span className="text-xs text-text-muted">{project.role}</span>
            </div>
          </div>
        </div>
        {project.permission === 'can_manage' ||
        project.role === 'owner' ||
        project.role === 'admin' ? (
          <DeleteProjectButton uuid={uuid} />
        ) : null}
      </div>

      <Tabs value={tab} onValueChange={setTab}>
        <TabsList>
          <TabsTrigger value="secrets">Secrets</TabsTrigger>
          <TabsTrigger value="members">Members</TabsTrigger>
          <TabsTrigger value="groups">Groups</TabsTrigger>
        </TabsList>

        <TabsContent value="secrets" className="space-y-4">
          <SecretsTab projectUuid={uuid} secrets={secrets} onChanged={load} />
        </TabsContent>
        <TabsContent value="members" className="space-y-4">
          <MembersTab
            projectUuid={uuid}
            members={members}
            onChanged={load}
            canManage={
              project.permission === 'can_manage' ||
              project.role === 'owner' ||
              project.role === 'admin'
            }
          />
        </TabsContent>
        <TabsContent value="groups" className="space-y-4">
          <GroupsTab projectUuid={uuid} groups={groups} onChanged={load} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function DeleteProjectButton({ uuid }: { uuid: string }) {
  return (
    <AlertDialog>
      <AlertDialogTrigger asChild>
        <Button variant="destructive" size="sm">
          <Trash2 className="mr-1.5 size-4" aria-hidden="true" />
          Delete
        </Button>
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Delete this project?</AlertDialogTitle>
          <AlertDialogDescription>
            This permanently removes the project and all of its secrets.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={() => {
              void mlp.deleteProject(uuid).then(() => {
                toast.success('Project deleted');
                window.location.href = '/projects';
              });
            }}
          >
            Delete
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

// ---------------------------------------------------------------------------
// Secrets tab
// ---------------------------------------------------------------------------

function SecretsTab({
  projectUuid,
  secrets,
  onChanged,
}: {
  projectUuid: string;
  secrets: Secret[];
  onChanged: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [revealError, setRevealError] = useState<Record<string, string>>({});

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!key.trim()) return;
    setBusy(true);
    try {
      await mlp.createSecret({
        project_uuid: projectUuid,
        key: key.trim(),
        value_ciphertext: b64encode(value),
      });
      toast.success('Secret created');
      setOpen(false);
      setKey('');
      setValue('');
      onChanged();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const onReveal = async (secretUuid: string) => {
    if (revealed[secretUuid]) {
      setRevealed((prev) => {
        const next = { ...prev };
        delete next[secretUuid];
        return next;
      });
      return;
    }
    try {
      const res = await mlp.revealSecret(secretUuid);
      setRevealed((prev) => ({ ...prev, [secretUuid]: b64decode(res.value_ciphertext) }));
      setRevealError((prev) => ({ ...prev, [secretUuid]: '' }));
    } catch (err) {
      setRevealError((prev) => ({
        ...prev,
        [secretUuid]: err instanceof MlpApiError ? err.message : String(err),
      }));
    }
  };

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle>Secrets</CardTitle>
        <Button size="sm" onClick={() => setOpen(true)}>
          <Plus className="mr-1.5 size-4" aria-hidden="true" />
          New secret
        </Button>
      </CardHeader>
      <CardContent>
        {secrets.length === 0 ? (
          <p className="py-8 text-center text-sm text-text-muted">No secrets yet.</p>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Key</TableHead>
                <TableHead>Version</TableHead>
                <TableHead>Updated</TableHead>
                <TableHead className="w-24">Value</TableHead>
                <TableHead className="w-12" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {secrets.map((s) => (
                <TableRow key={s.uuid}>
                  <TableCell className="font-mono text-text">{s.key}</TableCell>
                  <TableCell>{s.version}</TableCell>
                  <TableCell className="text-text-muted">
                    {new Date(s.updated_at).toLocaleString()}
                  </TableCell>
                  <TableCell>
                    <Button size="sm" variant="outline" onClick={() => void onReveal(s.uuid)}>
                      {revealed[s.uuid] ? (
                        <EyeOff className="mr-1 size-4" aria-hidden="true" />
                      ) : (
                        <Eye className="mr-1 size-4" aria-hidden="true" />
                      )}
                      {revealed[s.uuid] ? 'Hide' : 'Reveal'}
                    </Button>
                  </TableCell>
                  <TableCell>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon" aria-label="Secret actions">
                          <MoreHorizontal className="size-4" aria-hidden="true" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          className="text-danger"
                          onClick={() => {
                            void mlp.deleteSecret(s.uuid).then(() => {
                              toast.success('Secret deleted');
                              onChanged();
                            });
                          }}
                        >
                          <Trash2 className="mr-2 size-4" aria-hidden="true" />
                          Delete
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}

        {Object.values(revealed).some(Boolean) ? (
          <div className="mt-4 space-y-2">
            {secrets.map((s) =>
              revealed[s.uuid] ? (
                <div
                  key={s.uuid}
                  className="rounded-md border border-accent/40 bg-accent/10 px-3 py-2 font-mono text-sm text-text"
                >
                  <span className="text-text-muted">{s.key}: </span>
                  {revealed[s.uuid]}
                </div>
              ) : null,
            )}
          </div>
        ) : null}

        {Object.values(revealError).some(Boolean) ? (
          <div className="mt-4 space-y-2">
            {secrets.map((s) =>
              revealError[s.uuid] ? (
                <p
                  key={s.uuid}
                  role="alert"
                  className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger"
                >
                  Reveal denied for {s.key}: {revealError[s.uuid]}
                </p>
              ) : null,
            )}
          </div>
        ) : null}
      </CardContent>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New secret</DialogTitle>
              <DialogDescription>Add a key/value secret to this project.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="secret-key">Key</Label>
                <Input
                  id="secret-key"
                  value={key}
                  onChange={(e) => setKey(e.target.value)}
                  placeholder="DATABASE_URL"
                  className="font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="secret-value">Value</Label>
                <Input
                  id="secret-value"
                  type="password"
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                  placeholder="super-secret"
                  className="font-mono"
                />
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                {busy ? 'Creating…' : 'Create secret'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Members tab
// ---------------------------------------------------------------------------

function MembersTab({
  projectUuid,
  members,
  onChanged,
  canManage,
}: {
  projectUuid: string;
  members: ProjectMember[];
  onChanged: () => void;
  canManage: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [userUuid, setUserUuid] = useState('');
  const [permission, setPermission] = useState('can_view');
  const [busy, setBusy] = useState(false);

  const onAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!userUuid.trim()) return;
    setBusy(true);
    try {
      await mlp.addMember(projectUuid, { user_uuid: userUuid.trim(), permission });
      toast.success('Member added');
      setOpen(false);
      setUserUuid('');
      onChanged();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const changePermission = (userUuidValue: string, next: string) => {
    void mlp.updateMember(projectUuid, userUuidValue, { permission: next }).then(() => {
      toast.success('Permission updated');
      onChanged();
    });
  };

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle>Members</CardTitle>
        {canManage ? (
          <Button size="sm" onClick={() => setOpen(true)}>
            <UserPlus className="mr-1.5 size-4" aria-hidden="true" />
            Add member
          </Button>
        ) : null}
      </CardHeader>
      <CardContent>
        {members.length === 0 ? (
          <p className="py-8 text-center text-sm text-text-muted">No members yet.</p>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>User</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Permission</TableHead>
                {canManage ? <TableHead className="w-12" /> : null}
              </TableRow>
            </TableHeader>
            <TableBody>
              {members.map((m) => (
                <TableRow key={m.user_uuid}>
                  <TableCell className="text-text">
                    {m.display_name ?? m.user_uuid}
                    <span className="block font-mono text-xs text-text-muted">{m.user_uuid}</span>
                  </TableCell>
                  <TableCell>
                    <Badge variant="secondary">{m.role}</Badge>
                  </TableCell>
                  <TableCell>
                    {canManage ? (
                      <Select
                        value={m.permission}
                        onValueChange={(v) => changePermission(m.user_uuid, v)}
                      >
                        <SelectTrigger className="w-36">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {PERMISSIONS.map((p) => (
                            <SelectItem key={p.value} value={p.value}>
                              {p.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    ) : (
                      <Badge>{m.permission}</Badge>
                    )}
                  </TableCell>
                  {canManage ? (
                    <TableCell>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="icon" aria-label="Member actions">
                            <MoreHorizontal className="size-4" aria-hidden="true" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            className="text-danger"
                            onClick={() => {
                              void mlp.removeMember(projectUuid, m.user_uuid).then(() => {
                                toast.success('Member removed');
                                onChanged();
                              });
                            }}
                          >
                            <Trash2 className="mr-2 size-4" aria-hidden="true" />
                            Remove
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </TableCell>
                  ) : null}
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onAdd}>
            <DialogHeader>
              <DialogTitle>Add member</DialogTitle>
              <DialogDescription>Grant this user access to the project.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="member-uuid">User UUID</Label>
                <Input
                  id="member-uuid"
                  value={userUuid}
                  onChange={(e) => setUserUuid(e.target.value)}
                  placeholder="00000000-0000-0000-0000-000000000000"
                  className="font-mono"
                />
              </div>
              <div className="space-y-1.5">
                <Label>Permission</Label>
                <Select value={permission} onValueChange={setPermission}>
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PERMISSIONS.map((p) => (
                      <SelectItem key={p.value} value={p.value}>
                        {p.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                {busy ? 'Adding…' : 'Add member'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Groups tab
// ---------------------------------------------------------------------------

function GroupsTab({
  projectUuid,
  groups,
  onChanged,
}: {
  projectUuid: string;
  groups: UserGroup[];
  onChanged: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    try {
      await mlp.createGroup(projectUuid, { name: name.trim() });
      toast.success('Group created');
      setOpen(false);
      setName('');
      onChanged();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle>Groups</CardTitle>
        <Button size="sm" onClick={() => setOpen(true)}>
          <Plus className="mr-1.5 size-4" aria-hidden="true" />
          New group
        </Button>
      </CardHeader>
      <CardContent className="space-y-2">
        {groups.length === 0 ? (
          <p className="py-8 text-center text-sm text-text-muted">No groups yet.</p>
        ) : (
          groups.map((g) => (
            <div
              key={g.id}
              className="flex items-center justify-between rounded-md border border-border bg-surface-raised px-4 py-3"
            >
              <div>
                <p className="font-medium text-text">{g.name}</p>
                {g.description ? <p className="text-sm text-text-muted">{g.description}</p> : null}
                <p className="mt-0.5 text-xs text-text-muted">{g.members.length} member(s)</p>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="text-danger"
                onClick={() => {
                  void mlp.deleteGroup(projectUuid, g.id).then(() => {
                    toast.success('Group deleted');
                    onChanged();
                  });
                }}
              >
                <Trash2 className="mr-1.5 size-4" aria-hidden="true" />
                Delete
              </Button>
            </div>
          ))
        )}
      </CardContent>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New group</DialogTitle>
              <DialogDescription>Groups grant shared access to the project.</DialogDescription>
            </DialogHeader>
            <div className="space-y-1.5 py-4">
              <Label htmlFor="group-name">Name</Label>
              <Input
                id="group-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Engineering"
              />
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                {busy ? 'Creating…' : 'Create group'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
