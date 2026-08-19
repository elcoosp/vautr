import type { Project, ProjectType } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import { FolderKanban } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { EmptyState } from '@/popup/components/EmptyState';
import { usePopupStore } from '../store';

interface ProjectsTabProps {
  mlp: VautrMlpClient;
}

export function ProjectsTab({ mlp }: ProjectsTabProps) {
  const projects = usePopupStore((s) => s.projects);
  const setProjects = usePopupStore((s) => s.setProjects);
  const removeProject = usePopupStore((s) => s.removeProject);
  const setError = usePopupStore((s) => s.setError);

  const [showCreate, setShowCreate] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [type, setType] = useState<ProjectType>('personal');

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const res = await mlp.listProjects();
      setProjects(res.projects);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [mlp, setProjects, setError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleCreate(): Promise<void> {
    if (!name) {
      setError('Project name is required.');
      return;
    }
    try {
      const project = await mlp.createProject({ name, description, type });
      setProjects([...projects, project]);
      setName('');
      setDescription('');
      setType('personal');
      setShowCreate(false);
      toast.success(`Created project "${project.name}".`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function confirmDelete(): Promise<void> {
    const target = deleteTarget;
    if (!target) return;
    try {
      await mlp.deleteProject(target.uuid);
      removeProject(target.uuid);
      toast.success(`Deleted project "${target.name}".`);
      setDeleteTarget(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setDeleteTarget(null);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Projects</h3>
          <p className="text-xs text-muted-foreground">{projects.length} total</p>
        </div>
        <Button size="sm" onClick={() => setShowCreate(true)}>
          New
        </Button>
      </div>

      {projects.length === 0 ? (
        <EmptyState
          icon={FolderKanban}
          title="No projects yet."
          description="Create one to organize items."
          action={{ label: 'Create project', onClick: () => setShowCreate(true) }}
        />
      ) : (
        <div className="space-y-2">
          {projects.map((p) => (
            <div key={p.uuid} className="flex items-center justify-between rounded-lg border p-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium">{p.name}</span>
                  <Badge variant={p.type === 'shared' ? 'secondary' : 'outline'}>{p.type}</Badge>
                </div>
                {p.description ? (
                  <p className="truncate text-xs text-muted-foreground">{p.description}</p>
                ) : null}
                <p className="text-xs text-muted-foreground">Role: {p.role}</p>
              </div>
              <Button
                size="sm"
                variant="ghost"
                className="text-destructive"
                onClick={() => setDeleteTarget(p)}
              >
                Delete
              </Button>
            </div>
          ))}
        </div>
      )}

      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create project</DialogTitle>
            <DialogDescription>Projects group logins and secrets together.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>Name</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>Description (optional)</Label>
              <Input value={description} onChange={(e) => setDescription(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>Type</Label>
              <Select value={type} onValueChange={(v) => setType(v as ProjectType)}>
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="personal">Personal</SelectItem>
                  <SelectItem value="shared">Shared</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => void handleCreate()}>Create</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteTarget !== null} onOpenChange={(o) => !o && setDeleteTarget(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete project</DialogTitle>
            <DialogDescription>
              Permanently delete project &quot;{deleteTarget?.name}&quot;? This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={() => void confirmDelete()}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
