import { createFileRoute, Outlet, Link, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';
import { cn } from '@/lib/utils';
import {
  LayoutDashboard,
  FolderKanban,
  KeyRound,
  Wrench,
  Bot,
  Ticket,
  ShieldCheck,
  ArrowLeftRight,
  Settings,
  LogOut,
  Lock,
} from 'lucide-react';
import { logout } from '@/lib/client';
import { useEffect } from 'react';

export const Route = createFileRoute('/_authed')({
  component: AuthedLayout,
});

const NAV = [
  { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/projects', label: 'Projects', icon: FolderKanban },
  { to: '/vault', label: 'Vault', icon: KeyRound },
  { to: '/generator', label: 'Generator', icon: Wrench },
  { to: '/secrets', label: 'Secrets', icon: Lock },
  { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
  { to: '/tokens', label: 'Tokens', icon: Ticket },
  { to: '/mfa', label: 'MFA & security', icon: ShieldCheck },
  { to: '/import-export', label: 'Import / export', icon: ArrowLeftRight },
  { to: '/settings', label: 'Settings', icon: Settings },
] as const;

function AuthedLayout() {
  const isLocked = useIsLocked();
  const navigate = useNavigate();

  useEffect(() => {
    if (isLocked) {
      void navigate({ to: '/login' });
    }
  }, [isLocked, navigate]);

  if (isLocked) {
    return null;
  }

  return (
    <div className="flex h-screen bg-bg text-text">
      <aside className="flex w-60 shrink-0 flex-col border-r border-border bg-surface">
        <div className="flex items-center gap-2 border-b border-border px-4 py-4">
          <span className="grid size-8 place-items-center rounded-lg bg-accent/15 text-accent">
            <Lock className="size-4" aria-hidden="true" />
          </span>
          <span className="text-base font-semibold text-text">Vautr</span>
        </div>
        <nav className="flex-1 space-y-0.5 overflow-y-auto p-2" aria-label="Main navigation">
          {NAV.map(({ to, label, icon: Icon }) => (
            <Link
              key={to}
              to={to}
              className={cn(
                'flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted transition-colors hover:bg-surface-raised hover:text-text',
                'data-[status=active]:bg-surface-raised data-[status=active]:text-text',
              )}
              activeOptions={{ exact: to === '/dashboard' }}
            >
              <Icon className="size-4 shrink-0" aria-hidden="true" />
              {label}
            </Link>
          ))}
        </nav>
        <div className="border-t border-border p-2">
          <button
            type="button"
            onClick={() => void logout()}
            className="flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted transition-colors hover:bg-surface-raised hover:text-text"
          >
            <LogOut className="size-4" aria-hidden="true" />
            Log out
          </button>
        </div>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        <Outlet />
      </main>
    </div>
  );
}
