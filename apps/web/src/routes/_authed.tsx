import { createFileRoute, Link, Outlet, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';
import {
  Bot,
  CircleCheck,
  Eye,
  Folder,
  Globe,
  HardDrive,
  LayoutDashboard,
  Lock,
  LogOut,
  Replace,
  ScrollText,
  Settings,
  Settings2,
} from 'lucide-react';
import { useEffect } from 'react';
import { logout } from '@/lib/client';
import { cn } from '@/lib/utils';

export const Route = createFileRoute('/_authed')({
  component: AuthedLayout,
});

const NAV = [
  { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/projects', label: 'Projects', icon: Folder },
  { to: '/vault', label: 'Vault', icon: Eye },
  { to: '/generator', label: 'Generator', icon: Settings2 },
  { to: '/secrets', label: 'Secrets', icon: HardDrive },
  { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
  { to: '/tokens', label: 'Tokens', icon: Globe },
  { to: '/mfa', label: 'MFA & security', icon: CircleCheck },
  { to: '/audit', label: 'Security log', icon: ScrollText },
  { to: '/import-export', label: 'Import / export', icon: Replace },
  { to: '/settings', label: 'Settings', icon: Settings },
] as const;

// Maps nav routes to the `data-tour` anchors the feature tour highlights (VTR-077).
const TOUR_ANCHORS: Record<string, string | undefined> = {
  '/vault': 'vault',
  '/mfa': 'emergency-kit',
  '/audit': 'audit',
};

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
              data-tour={TOUR_ANCHORS[to]}
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
