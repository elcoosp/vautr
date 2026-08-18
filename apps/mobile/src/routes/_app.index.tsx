import { createFileRoute, useRouter } from '@tanstack/react-router';
import { Folder, Plus } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { Pressable, View } from 'react-native';
import { Alert, AlertDescription, AlertTitle } from '../../components/ui/alert';
import { Avatar, AvatarFallbackText } from '../../components/ui/avatar';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Separator } from '../../components/ui/separator';
import { Skeleton } from '../../components/ui/skeleton';
import { Text } from '../../components/ui/text';
import type { Project } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/')({
  component: ProjectsListScreen,
});

function ProjectCard({ project, onPress }: { project: Project; onPress: () => void }) {
  const initial = (project.name[0] ?? '?').toUpperCase();
  return (
    <Pressable onPress={onPress} className="active:opacity-90">
      <Card className="flex-row items-center gap-4 p-4">
        <Avatar size={44}>
          <AvatarFallbackText>{initial}</AvatarFallbackText>
        </Avatar>
        <View className="flex-1 gap-1">
          <View className="flex-row items-center justify-between">
            <Text variant="h4">{project.name}</Text>
            <Badge variant={project.type === 'personal' ? 'secondary' : 'default'}>
              {project.type}
            </Badge>
          </View>
          {project.description ? (
            <Text variant="muted" numberOfLines={1}>
              {project.description}
            </Text>
          ) : null}
          <Text variant="tiny">
            Role: {project.role}
            {project.permission ? ` · ${project.permission}` : ''}
          </Text>
        </View>
        <Folder size={18} className="text-muted-foreground" />
      </Card>
    </Pressable>
  );
}

function ProjectsListScreen() {
  const router = useRouter();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const list = await services.api.listProjects();
      setProjects(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load projects.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <View className="gap-5">
      <View className="flex-row items-center justify-between">
        <View className="gap-0.5">
          <Text variant="h2">Projects</Text>
          <Text variant="muted">
            {projects
              ? `${projects.length} vault ${projects.length === 1 ? 'space' : 'spaces'}`
              : 'Loading…'}
          </Text>
        </View>
        <Button
          size="sm"
          className="gap-1.5"
          onPress={() => router.navigate({ to: '/projects/new' })}
        >
          <Plus size={16} className="text-primary-foreground" />
          <ButtonText>New</ButtonText>
        </Button>
      </View>

      <Separator />

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Couldn’t load projects</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
          <Button variant="outline" className="mt-3 self-start" onPress={() => void load()}>
            <ButtonText>Retry</ButtonText>
          </Button>
        </Alert>
      ) : projects === null ? (
        <View className="gap-3">
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-[76px] w-full" />
          ))}
        </View>
      ) : projects.length === 0 ? (
        <Alert variant="info">
          <AlertTitle>No projects yet</AlertTitle>
          <AlertDescription>
            Create a vault space to start storing passwords and secrets.
          </AlertDescription>
          <Button
            className="mt-3 self-start"
            onPress={() => router.navigate({ to: '/projects/new' })}
          >
            <ButtonText>Create project</ButtonText>
          </Button>
        </Alert>
      ) : (
        <View className="gap-3">
          {projects.map((project) => (
            <ProjectCard
              key={project.uuid}
              project={project}
              onPress={() =>
                router.navigate({
                  to: '/projects/$projectId',
                  params: { projectId: project.uuid },
                })
              }
            />
          ))}
        </View>
      )}
    </View>
  );
}
