import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { mlp, MlpApiError } from '@/lib/mlp';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { Project } from '@vautr/api-contract';
import { FolderKanban, Plus } from 'lucide-react';

export const Route = createFileRoute('/_authed/projects/')({
  component: ProjectsPage,
  validateSearch: (search: Record<string, unknown>) => ({ create: Boolean(search.create) }),
});

function ProjectsPage() {
  const { create } = Route.useSearch();
  const navigate = useNavigate();
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(create);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [type, setType] = useState<'personal' | 'shared'>('personal');
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const res = await mlp.listProjects();
      setProjects(res.projects);
      setError(null);
    } catch (err) {
      setError(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => setDialogOpen(create), [create]);

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.error('Project name is required');
      return;
    }
    setBusy(true);
    try {
      const project = await mlp.createProject({ name: name.trim(), description: description.trim() || undefined, type });
      toast.success(`Created project "${project.name}"`);
      setDialogOpen(false);
      setName('');
      setDescription('');
      void navigate({ to: '/projects/$uuid', params: { uuid: project.uuid } });
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-text">Projects</h1>
          <p className="text-sm text-text-muted">Projects unify password vaults and secrets.</p>
        </div>
        <Button onClick={() => setDialogOpen(true)}>
          <Plus className="mr-1.5 size-4" aria-hidden="true" />
          New project
        </Button>
      </div>

      {error ? <p className="text-sm text-danger">Failed to load projects: {error}</p> : null}
      {loading ? <p className="text-sm text-text-muted">Loading…</p> : null}

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {projects.map((p) => (
          <Link key={p.uuid} to="/projects/$uuid" params={{ uuid: p.uuid }}>
            <Card className="h-full transition-colors hover:border-accent/60">
              <CardContent className="flex flex-col gap-3 p-5">
                <div className="flex items-center justify-between">
                  <span className="grid size-10 place-items-center rounded-lg bg-accent/15 text-accent">
                    <FolderKanban className="size-5" aria-hidden="true" />
                  </span>
                  <Badge variant="secondary">{p.type}</Badge>
                </div>
                <div>
                  <p className="font-medium text-text">{p.name}</p>
                  {p.description ? (
                    <p className="mt-0.5 text-sm text-text-muted">{p.description}</p>
                  ) : null}
                </div>
                <div className="flex items-center gap-2 text-xs">
                  {p.permission ? <Badge>{p.permission}</Badge> : null}
                  <span className="text-text-muted">{p.role}</span>
                </div>
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>

      {!loading && projects.length === 0 && (
        <p className="py-16 text-center text-sm text-text-muted">
          No projects yet. Create one to organize your vaults and secrets.
        </p>
      )}

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New project</DialogTitle>
              <DialogDescription>Create a personal or shared project.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="project-name">Name</Label>
                <Input id="project-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Engineering vault" />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="project-desc">Description</Label>
                <Input id="project-desc" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Optional description" />
              </div>
              <div className="space-y-1.5">
                <Label>Type</Label>
                <Select value={type} onValueChange={(v) => setType(v as 'personal' | 'shared')}>
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="Type" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="personal">Personal</SelectItem>
                    <SelectItem value="shared">Shared</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setDialogOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                {busy ? 'Creating…' : 'Create project'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
