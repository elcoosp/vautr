import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Button, ButtonText } from '../../components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Text } from '../../components/ui/text';
import { useToast } from '../../components/ui/toast';
import type { ProjectType } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/projects/new')({
  component: NewProjectScreen,
});

function NewProjectScreen() {
  const router = useRouter();
  const toast = useToast();
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [type, setType] = useState<ProjectType>('personal');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const create = async () => {
    if (!name.trim()) {
      setError('Project name is required.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const project = await services.api.createProject({
        name: name.trim(),
        description: description.trim() || undefined,
        type,
      });
      toast.show({ title: 'Project created', description: project.name });
      router.navigate({ to: '/projects/$projectId', params: { projectId: project.uuid } });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create project.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <Card>
        <CardHeader>
          <CardTitle>
            <Text variant="h3">New project</Text>
          </CardTitle>
          <CardDescription>Create a space to store passwords and secrets.</CardDescription>
        </CardHeader>
        <CardContent className="gap-4">
          <View className="gap-1.5">
            <Label htmlFor="project-name">Name</Label>
            <Input
              id="project-name"
              value={name}
              onChangeText={setName}
              placeholder="Engineering"
            />
          </View>
          <View className="gap-1.5">
            <Label htmlFor="project-desc">Description</Label>
            <Input
              id="project-desc"
              value={description}
              onChangeText={setDescription}
              placeholder="Optional"
            />
          </View>
          <View className="gap-1.5">
            <Label>Type</Label>
            <View className="flex-row gap-2">
              {(['personal', 'shared'] as const).map((value) => {
                const active = type === value;
                return (
                  <Button
                    key={value}
                    variant={active ? 'default' : 'outline'}
                    className="flex-1 capitalize"
                    onPress={() => setType(value)}
                  >
                    <ButtonText className="capitalize">{value}</ButtonText>
                  </Button>
                );
              })}
            </View>
          </View>

          {error ? (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}
        </CardContent>
        <CardFooter className="gap-2">
          <Button disabled={busy} onPress={() => void create()}>
            <ButtonText>{busy ? 'Creating…' : 'Create project'}</ButtonText>
          </Button>
          <Button variant="ghost" disabled={busy} onPress={() => router.navigate({ to: '/' })}>
            <ButtonText>Cancel</ButtonText>
          </Button>
        </CardFooter>
      </Card>
    </View>
  );
}
