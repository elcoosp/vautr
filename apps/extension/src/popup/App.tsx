import type { VautrMlpClient } from '@vautr/client-sdk';
import { createClipboardHandler } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import {
  ArrowLeftRight,
  Bot,
  CircleCheck,
  Eye,
  Folder,
  HardDrive,
  Inbox,
  Lock,
  Settings2,
  Ticket,
} from 'lucide-react';
import { useCallback, useEffect, useRef } from 'react';
import * as browser from 'webextension-polyfill';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { getApiUrl } from '../lib/apiUrl';
import { AuthView } from './components/AuthView';
import { ConflictModal } from './components/ConflictModal';
import { GeneratorTab } from './components/GeneratorTab';
import { ImportExportTab } from './components/ImportExportTab';
import { InboxTab } from './components/InboxTab';
import { MachineAccountsTab } from './components/MachineAccountsTab';
import { MfaTab } from './components/MfaTab';
import { ProjectsTab } from './components/ProjectsTab';
import { SecretsTab } from './components/SecretsTab';
import { TokensTab } from './components/TokensTab';
import { VaultTab } from './components/VaultTab';
import { disposePopupClient, getPopupClient, getPopupMlpClient } from './popupClient';
import { usePopupStore } from './store';

const TABS = [
  { id: 'projects', label: 'Projects', icon: Folder },
  { id: 'vault', label: 'Vault', icon: Eye },
  { id: 'generator', label: 'Generator', icon: Settings2 },
  { id: 'secrets', label: 'Secrets', icon: HardDrive },
  { id: 'mfa', label: 'MFA & security', icon: CircleCheck },
  { id: 'import-export', label: 'Backup', icon: ArrowLeftRight },
  { id: 'machine-accounts', label: 'Machines', icon: Bot },
  { id: 'tokens', label: 'Tokens', icon: Ticket },
  { id: 'inbox', label: 'Inbox', icon: Inbox },
];

export function App() {
  const status = usePopupStore((s) => s.status);
  const username = usePopupStore((s) => s.username);
  const error = usePopupStore((s) => s.error);
  const setUsername = usePopupStore((s) => s.setUsername);
  const _setItems = usePopupStore((s) => s.setItems);
  const upsertItem = usePopupStore((s) => s.upsertItem);
  const removeItem = usePopupStore((s) => s.removeItem);
  const setError = usePopupStore((s) => s.setError);
  const setStatus = usePopupStore((s) => s.setStatus);
  const reset = usePopupStore((s) => s.reset);
  const activeTab = usePopupStore((s) => s.activeTab);
  const setActiveTab = usePopupStore((s) => s.setActiveTab);
  const pushConflict = usePopupStore((s) => s.pushConflict);

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
    (update: {
      type: string;
      overview?: { uuid: string };
      uuid?: string;
      event?: { uuid: string; localVersion: string; serverVersion: string; isToxic: boolean };
    }) => {
      if (update.type === 'OverviewUpserted' && update.overview) {
        upsertItem(update.overview as never);
      } else if (update.type === 'OverviewDeleted' && update.uuid) {
        removeItem(update.uuid);
      } else if (update.type === 'ConflictDetected' && update.event) {
        pushConflict(update.event);
      }
    },
    [upsertItem, removeItem, pushConflict],
  );

  async function handleAuthenticated(user: string): Promise<void> {
    setUsername(user);
    setStatus('unlocked');
    const client = await getPopupClient();
    clientRef.current = client;
    client.setClipboardHandler(createClipboardHandler());
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
    <div className="flex h-full flex-col dark">
      <header className="flex items-center justify-between border-b px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="grid size-8 place-items-center rounded-lg bg-accent/15 text-accent">
            <Lock className="size-4" aria-hidden="true" />
          </span>
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
            <TabsTrigger
              key={t.id}
              value={t.id}
              className="flex flex-col items-center gap-0.5 py-1.5 text-[10px]"
            >
              <t.icon className="size-4" aria-hidden="true" />
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
            {client && mlp ? <VaultTab client={client} mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="projects" className="mt-0">
            {mlp ? <ProjectsTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="secrets" className="mt-0">
            {mlp && client ? <SecretsTab mlp={mlp} client={client} /> : null}
          </TabsContent>
          <TabsContent value="generator" className="mt-0">
            <GeneratorTab />
          </TabsContent>
          <TabsContent value="mfa" className="mt-0">
            {mlp && client ? <MfaTab mlp={mlp} client={client} /> : null}
          </TabsContent>
          <TabsContent value="import-export" className="mt-0">
            {mlp ? <ImportExportTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="machine-accounts" className="mt-0">
            {mlp ? <MachineAccountsTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="tokens" className="mt-0">
            {mlp ? <TokensTab mlp={mlp} /> : null}
          </TabsContent>
          <TabsContent value="inbox" className="mt-0">
            {mlp && client ? <InboxTab mlp={mlp} client={client} /> : null}
          </TabsContent>
        </div>
      </Tabs>
      {client ? <ConflictModal client={client} /> : null}
      <footer className="flex items-center justify-between border-t px-4 py-2 text-xs">
        <span className="text-muted-foreground">Vault on this device</span>
        <button
          type="button"
          className="rounded text-accent underline-offset-2 hover:underline focus-visible:outline-1 focus-visible:outline-ring"
          onClick={async () => {
            const url = await getApiUrl();
            await browser.tabs.create({ url });
          }}
        >
          Open full app
        </button>
      </footer>
    </div>
  );
}
