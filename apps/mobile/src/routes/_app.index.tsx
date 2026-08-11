import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { Plus } from 'lucide-react-native';

import { services } from '../../lib/client';
import type { Project } from '../../lib/api';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';

export const Route = createFileRoute('/_app/')({
  component: ProjectsListScreen,
});

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

  if (error) {
    return (
      <View className="gap-3">
        <Text accessibilityRole="alert" className="text-sm text-destructive">
          {error}
        </Text>
        <Button onPress={() => void load()}>
          <ButtonText>Retry</ButtonText>
        </Button>
      </View>
    );
  }

  if (projects === null) {
    return <ActivityIndicator className="mt-8" color="#2bba99" />;
  }

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text className="text-lg font-semibold text-foreground">Projects</Text>
        <Button size="sm" onPress={() => router.navigate({ to: '/projects/new' })}>
          <Plus size={16} className="text-primary-foreground" />
          <ButtonText className="ml-1">New</ButtonText>
        </Button>
      </View>

      {projects.length === 0 ? (
        <Text className="text-sm text-muted-foreground">
          No projects yet. Create one to store passwords and secrets.
        </Text>
      ) : (
        projects.map((project) => (
          <Pressable
            key={project.uuid}
            onPress={() => router.navigate({ to: '/projects/$projectId', params: { projectId: project.uuid } })}
          >
            <Card className="p-4">
              <View className="flex-row items-center justify-between">
                <Text className="text-base font-medium text-foreground">{project.name}</Text>
                <Badge variant={project.type === 'personal' ? 'secondary' : 'default'}>
                  {project.type}
                </Badge>
              </View>
              {project.description ? (
                <Text className="mt-1 text-sm text-muted-foreground">{project.description}</Text>
              ) : null}
              <Text className="mt-2 text-xs text-muted-foreground">
                Role: {project.role}
                {project.permission ? ` · ${project.permission}` : ''}
              </Text>
            </Card>
          </Pressable>
        ))
      )}
    </View>
  );
}
