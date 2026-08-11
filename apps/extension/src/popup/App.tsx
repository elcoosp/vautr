import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import type { VautrMlpClient } from '@vautr/client-sdk';
import { usePopupStore } from './store';
import { AuthView } from './components/AuthView';
import { VaultTab } from './components/VaultTab';
import { ProjectsTab } from './components/ProjectsTab';
import { SecretsTab } from './components/SecretsTab';
import { GeneratorTab } from './components/GeneratorTab';
import { MfaTab } from './components/MfaTab';
import { disposePopupClient, getPopupMlpClient, getPopupClient } from './popupClient';

const TABS = [
  { id: 'vault', label: 'Vault' },
  { id: 'projects', label: 'Projects' },
  { id: 'secrets', label: 'Secrets' },
  { id: 'generator', label: 'Generator' },
  { id: 'mfa', label: 'MFA' },
];

export function App() {
  const status = usePopupStore((s) => s.status);
  const username = usePopupStore((s) => s.username);
  const error = usePopupStore((s) => s.error);
  const setUsername = usePopupStore((s) => s.setUsername);
  const setItems = usePopupStore((s) => s.setItems);
  const upsertItem = usePopupStore((s) => s.upsertItem);
  const removeItem = usePopupStore((s) => s.removeItem);
  const setError = usePopupStore((s) => s.setError);
  const setStatus = usePopupStore((s) => s.setStatus);
  const reset = usePopupStore((s) => s.reset);
  const activeTab = usePopupStore((s) => s.activeTab);
  const setActiveTab = usePopupStore((s) => s.setActiveTab);

  const clientRef = useRef<VautrWebClient | null>(null);
  const mlpRef = useRef<VautrMlpClient | null>(null);

  // Graceful shutdown.
  useEffect(() => {
    const shutdown = (): void => {
      void disposePopupClient();
    };
    window.addEventListener('pagehide', shutdown);
    window.addEventListener('beforeunload', shutdown);
    return () => {
      window.removeEventListener('pagehide', shutdown);
      window.removeEventListener('beforeunload', shutdown);
    };
  }, []);

  const onVaultUpdate = useCallback(
    (update: { type: string; overview?: { uuid: string } ; uuid?: string }) => {
      if (update.type === 'OverviewUpserted' && update.overview) {
        upsertItem(update.overview as never);
      } else if (update.type === 'OverviewDeleted' && update.uuid) {
        removeItem(update.uuid);
      }
    },
    [upsertItem, removeItem],
  );

  async function handleAuthenticated(user: string): Promise<void> {
    setUsername(user);
    setStatus('unlocked');
    const client = await getPopupClient();
    clientRef.current = client;
    client.subscribe(onVaultUpdate as never);
    await refreshAll();
  }

  async function refreshAll(): Promise<void> {
    try {
      const client = clientRef.current;
      if (client) {
        await client.sync();
      }
      const mlp = await getPopupMlpClient();
      mlpRef.current = mlp;
      const items = client ? await client.sync() : undefined;
      void items;
      const projects = await mlp.listProjects();
      const projectStore = usePopupStore.getState();
      projectStore.setProjects(projects.projects);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleLock(): Promise<void> {
    const client = clientRef.current;
    if (client) {
      await client.lock();
    }
    reset();
    clientRef.current = null;
    mlpRef.current = null;
    await disposePopupClient();
  }

  if (status === 'locked') {
    return <AuthView onAuthenticated={(u) => void handleAuthenticated(u)} />;
  }

  const client = clientRef.current;
  const mlp = mlpRef.current;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-base font-semibold">Vautr</span>
          {username ? <span className="text-xs text-muted-foreground">{username}</span> : null}
        </div>
        <Button size="sm" variant="outline" onClick={() => void handleLock()}>
          Lock
        </Button>
      </header>

      <Tabs value={activeTab} onValueChange={setActiveTab} className="flex flex-1 flex-col">
        <TabsList className="mx-4 mt-3 grid w-auto grid-cols-5">
          {TABS.map((t) => (
            <TabsTrigger key={t.id} value={t.id} className="text-xs">
              {t.label}
            </TabsTrigger>
          ))}
        </TabsList>
        <div className="flex-1 overflow-y-auto p-4">
          {error ? (
            <div className="mb-3 rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {error}
            </div>
          ) : null}
          <TabsContent value="vault" className="mt-0">
            {client ? <VaultTab client={client} /> : null}
          </TabsContent>
          <TabsContent value="projects" className="mt-0">
            {mlp ? <ProjectsTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="secrets" className="mt-0">
            {mlp ? <SecretsTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="generator" className="mt-0">
            <GeneratorTab />
          </TabsContent>
          <TabsContent value="mfa" className="mt-0">
            {mlp ? <MfaTab mlp={mlp} /> : null}
          </TabsContent>
        </div>
      </Tabs>
      <Separator />
    </div>
  );
}
