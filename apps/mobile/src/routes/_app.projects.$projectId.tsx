import { createFileRoute, useRouter } from '@tanstack/react-router';
import { Plus } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { SecretOverlay } from '../../components/SecretOverlay';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../components/ui/table';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import type { Project, ProjectMember, Secret } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/projects/$projectId')({
  component: ProjectDetailScreen,
});

function ProjectDetailScreen() {
  const { projectId } = Route.useParams();
  const router = useRouter();

  const [project, setProject] = useState<Project | null>(null);
  const [members, setMembers] = useState<ProjectMember[]>([]);
  const [secrets, setSecrets] = useState<Secret[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [tab, setTab] = useState<'secrets' | 'members'>('secrets');

  const load = useCallback(async () => {
    setError(null);
    try {
      const [proj, memberList, secretList] = await Promise.all([
        services.api.getProject(projectId),
        services.api.listProjectMembers(projectId),
        services.api.listProjectSecrets(projectId),
      ]);
      setProject(proj);
      setMembers(memberList);
      setSecrets(secretList);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load project.');
    } finally {
      setLoaded(true);
    }
  }, [projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!loaded) {
    return <ActivityIndicator className="mt-8" color="#42b59a" />;
  }

  if (!project) {
    return (
      <View className="gap-3">
        <Text accessibilityRole="alert" className="text-sm text-destructive">
          {error ?? 'Project not found.'}
        </Text>
        <Button onPress={() => void load()}>
          <ButtonText>Retry</ButtonText>
        </Button>
      </View>
    );
  }

  return (
    <View className="gap-4">
      <View className="flex-row items-start justify-between">
        <View className="flex-1 gap-1">
          <Text className="text-lg font-semibold text-foreground">{project.name}</Text>
          <View className="flex-row gap-2">
            <Badge variant="outline">{project.type}</Badge>
            <Badge variant="secondary">{project.permission ?? 'n/a'}</Badge>
          </View>
          {project.description ? (
            <Text className="mt-1 text-sm text-muted-foreground">{project.description}</Text>
          ) : null}
        </View>
      </View>

      <Tabs value={tab} onValueChange={(value) => setTab(value as 'secrets' | 'members')}>
        <TabsList className="flex-row">
          <TabsTrigger value="secrets" active={tab === 'secrets'} className="flex-1">
            Secrets
          </TabsTrigger>
          <TabsTrigger value="members" active={tab === 'members'} className="flex-1">
            Members
          </TabsTrigger>
        </TabsList>

        <TabsContent value="secrets">
          <View className="gap-3">
            <View className="flex-row items-center justify-between">
              <Text className="text-sm text-muted-foreground">{secrets.length} secret(s)</Text>
              <Button
                size="sm"
                onPress={() =>
                  router.navigate({ to: '/projects/$projectId/secrets/new', params: { projectId } })
                }
              >
                <Plus size={16} className="text-primary-foreground" />
                <ButtonText className="ml-1">New</ButtonText>
              </Button>
            </View>
            {secrets.length === 0 ? (
              <Text className="text-sm text-muted-foreground">No secrets in this project yet.</Text>
            ) : (
              secrets.map((secret) => (
                <Card key={secret.uuid} className="p-4">
                  <View className="flex-row items-center justify-between">
                    <View className="flex-1 gap-0.5">
                      <Text className="text-base font-medium text-foreground">{secret.key}</Text>
                      <Text className="text-xs text-muted-foreground">
                        v{secret.version} · {new Date(secret.updated_at).toLocaleString()}
                      </Text>
                    </View>
                    {/* VTR-048/056: reveal via the opaque-handle native overlay when
                        the FFI core is linked; degrades to a locked state when HTTP-only. */}
                    <SecretOverlay uuid={secret.uuid} label={secret.key} />
                  </View>
                </Card>
              ))
            )}
          </View>
        </TabsContent>

        <TabsContent value="members">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="flex-[2]">User</TableHead>
                <TableHead className="flex-1">Role</TableHead>
                <TableHead className="flex-1">Permission</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {members.map((member) => (
                <TableRow key={member.user_uuid}>
                  <TableCell className="flex-[2]">
                    {member.display_name ?? member.user_uuid}
                  </TableCell>
                  <TableCell className="flex-1">{member.role}</TableCell>
                  <TableCell className="flex-1">{member.permission}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TabsContent>
      </Tabs>
    </View>
  );
}
