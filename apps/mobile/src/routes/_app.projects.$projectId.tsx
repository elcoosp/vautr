import { createFileRoute, useRouter } from '@tanstack/react-router';
import { Folder, KeyRound, Plus, Users } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { View } from 'react-native';
import { PermissionBadge, RoleBadge } from '../../components/PermissionBadge';
import { SecretOverlay } from '../../components/SecretOverlay';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Avatar, AvatarFallbackText } from '../../components/ui/avatar';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { EmptyState } from '../../components/ui/empty-state';
import { ThemedIcon } from '../../components/ui/icon';
import { Skeleton } from '../../components/ui/skeleton';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../../components/ui/table';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import { Text } from '../../components/ui/text';
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
    return (
      <View className="gap-4">
        <Skeleton className="h-12 w-2/3" />
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-24 w-full" />
      </View>
    );
  }

  if (!project) {
    return (
      <View className="gap-3">
        <Alert variant="destructive">
          <AlertDescription>{error ?? 'Project not found.'}</AlertDescription>
        </Alert>
        <Button onPress={() => void load()}>
          <ButtonText>Retry</ButtonText>
        </Button>
      </View>
    );
  }

  return (
    <View className="gap-4">
      <View className="flex-row items-start justify-between">
        <View className="flex-1 flex-row items-center gap-3">
          <Avatar size={44} className="bg-primary/15">
            <AvatarFallbackText>
              <ThemedIcon icon={Folder} size={20} tone="primary" />
            </AvatarFallbackText>
          </Avatar>
          <View className="flex-1 gap-1">
            <Text variant="h4">{project.name}</Text>
            <View className="flex-row flex-wrap items-center gap-2">
              <Badge variant="outline">{project.type}</Badge>
              <RoleBadge role={project.role} />
              <PermissionBadge permission={project.permission} />
            </View>
            {project.description ? (
              <Text variant="muted" className="mt-1">
                {project.description}
              </Text>
            ) : null}
          </View>
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
              <Text variant="muted">{secrets.length} secret(s)</Text>
              <Button
                size="sm"
                onPress={() =>
                  router.navigate({ to: '/projects/$projectId/secrets/new', params: { projectId } })
                }
              >
                <ThemedIcon icon={Plus} size={16} tone="primaryForeground" />
                <ButtonText className="ml-1">New</ButtonText>
              </Button>
            </View>
            {secrets.length === 0 ? (
              <EmptyState
                variant="inline"
                icon={KeyRound}
                title="No secrets in this project yet."
              />
            ) : (
              secrets.map((secret) => (
                <Card key={secret.uuid} className="p-4">
                  <View className="flex-row items-center justify-between">
                    <View className="flex-1 gap-0.5">
                      <Text variant="p" className="font-medium">
                        {secret.key}
                      </Text>
                      <Text variant="tiny">
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
          {members.length === 0 ? (
            <EmptyState
              icon={Users}
              title="No members yet."
              description="This project has no collaborators. Add a teammate or a machine account to share access."
            />
          ) : (
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
                    <TableCell className="flex-1">
                      <RoleBadge role={member.role} />
                    </TableCell>
                    <TableCell className="flex-1">
                      <PermissionBadge permission={member.permission} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </TabsContent>
      </Tabs>
    </View>
  );
}
