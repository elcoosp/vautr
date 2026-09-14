import { createFileRoute, Link } from '@tanstack/react-router';
import type {
  AccessToken,
  BackupStatus,
  MachineAccount,
  MfaStatus,
  Project,
} from '@vautr/api-contract';
import { ArrowRight, Bot, Database, FolderKanban, ShieldCheck, Ticket } from 'lucide-react';
import { PermissionBadge } from '@/components/PermissionBadge';
import { useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/dashboard')({
  component: DashboardPage,
});

interface Overview {
  projects: Project[];
  machineAccounts: MachineAccount[];
  tokens: AccessToken[];
  mfa: MfaStatus | null;
  backup: BackupStatus | null;
}

function DashboardPage() {
  const [data, setData] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const [projects, machineAccounts, tokens, mfa, backup] = await Promise.all([
          mlp.listProjects(),
          mlp.listMachineAccounts(),
          mlp.listTokens(),
          mlp.mfaStatus().catch(() => null),
          mlp.backupStatus().catch(() => null),
        ]);
        if (active)
          setData({
            projects: projects.projects,
            machineAccounts: machineAccounts.machine_accounts,
            tokens: tokens.tokens,
            mfa,
            backup,
          });
      } catch (err) {
        if (active) setError(err instanceof MlpApiError ? err.message : String(err));
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="animate-vault-enter space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-text">Dashboard</h1>
          <p className="text-sm text-text-muted">
            Overview of your organization&apos;s vaults and secrets.
          </p>
        </div>
        <Link to="/projects" search={{ create: true }}>
          <Button>New project</Button>
        </Link>
      </div>

      {error ? <p className="text-sm text-danger">Failed to load overview: {error}</p> : null}

      {data ? (
        <>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <StatCard icon={FolderKanban} label="Projects" value={data.projects.length} />
            <StatCard icon={Bot} label="Machine accounts" value={data.machineAccounts.length} />
            <StatCard icon={Ticket} label="Access tokens" value={data.tokens.length} />
            <StatCard
              icon={ShieldCheck}
              label="MFA"
              value={data.mfa ? data.mfa.configured_methods.length : 0}
              hint={data.mfa?.required ? 'required' : undefined}
            />
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Recent projects</CardTitle>
              <CardDescription>
                {data.projects.length === 0
                  ? 'No projects yet. Create one to get started.'
                  : `You can access ${data.projects.length} project(s).`}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-2">
              {data.projects.slice(0, 5).map((p) => (
                <Link
                  key={p.uuid}
                  to="/projects/$uuid"
                  params={{ uuid: p.uuid }}
                  className="vault-card-hover vault-btn-press flex items-center justify-between rounded-md border border-border bg-surface-raised px-4 py-3 transition-colors hover:bg-border/40"
                >
                  <div className="flex items-center gap-3">
                    <FolderKanban className="size-4 text-accent" aria-hidden="true" />
                    <span className="font-medium text-text">{p.name}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary">{p.type}</Badge>
                    <PermissionBadge permission={p.permission} />
                    <ArrowRight className="size-4 text-text-muted" aria-hidden="true" />
                  </div>
                </Link>
              ))}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Backup status</CardTitle>
            </CardHeader>
            <CardContent className="flex items-center gap-2 text-sm">
              <Database className="size-4 text-text-muted" aria-hidden="true" />
              {data.backup
                ? data.backup.enabled
                  ? `Enabled · last backup ${data.backup.last_backup_at ? new Date(data.backup.last_backup_at).toLocaleString() : 'n/a'}`
                  : 'Not configured'
                : 'Backup API unavailable'}
            </CardContent>
          </Card>
        </>
      ) : (
        !error && (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {[1, 2, 3, 4].map((i) => (
              <Card key={i}>
                <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
                  <div className="h-3 w-20 animate-shimmer rounded" />
                  <div className="size-8 rounded-lg animate-shimmer" />
                </CardHeader>
                <CardContent>
                  <div className="h-8 w-12 animate-shimmer rounded" />
                </CardContent>
              </Card>
            ))}
          </div>
        )
      )}
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  hint,
}: {
  icon: typeof FolderKanban;
  label: string;
  value: number;
  hint?: string;
}) {
  return (
    <Card className="vault-card-hover">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardDescription>{label}</CardDescription>
        <span className="grid size-8 place-items-center rounded-lg bg-accent/15">
          <Icon className="size-4 text-accent" aria-hidden="true" />
        </span>
      </CardHeader>
      <CardContent>
        <div className="text-3xl font-semibold tracking-tight text-text">{value}</div>
        {hint ? <Badge className="mt-1.5" variant="secondary">{hint}</Badge> : null}
      </CardContent>
    </Card>
  );
}
