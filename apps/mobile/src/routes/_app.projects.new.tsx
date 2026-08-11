import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { Text, View } from 'react-native';

import { services } from '../../lib/client';
import type { ProjectType } from '../../lib/api';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../components/ui/select';
import { useToast } from '../../components/ui/toast';

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
      <Text className="text-lg font-semibold text-foreground">New project</Text>
      <Card className="p-4 gap-4">
        <View className="gap-1.5">
          <Label htmlFor="project-name">Name</Label>
          <Input id="project-name" value={name} onChangeText={setName} placeholder="Engineering" />
        </View>
        <View className="gap-1.5">
          <Label htmlFor="project-desc">Description</Label>
          <Input id="project-desc" value={description} onChangeText={setDescription} placeholder="Optional" />
        </View>
        <View className="gap-1.5">
          <Label>Type</Label>
          <Select
            value={{ value: type, label: type }}
            onValueChange={(option) => {
              if (option) setType(option.value as ProjectType);
            }}
          >
            <SelectTrigger>
              <SelectValue placeholder="Select a type" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="personal" label="personal">
                Personal
              </SelectItem>
              <SelectItem value="shared" label="shared">
                Shared
              </SelectItem>
            </SelectContent>
          </Select>
        </View>

        {error ? (
          <Text accessibilityRole="alert" className="text-sm text-destructive">
            {error}
          </Text>
        ) : null}

        <Button disabled={busy} onPress={() => void create()}>
          <ButtonText>{busy ? 'Creating…' : 'Create project'}</ButtonText>
        </Button>
      </Card>
    </View>
  );
}
