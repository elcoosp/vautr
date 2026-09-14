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
  Share2,
} from 'lucide-react';
import { useEffect } from 'react';
import { logout } from '@/lib/client';

export const Route = createFileRoute('/_authed')({
  component: AuthedLayout,
});

const NAV_SECTIONS = [
  {
    label: 'Overview',
    items: [{ to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard }],
  },
  {
    label: 'Vault',
    items: [
      { to: '/projects', label: 'Projects', icon: Folder },
      { to: '/vault', label: 'Vault', icon: Eye },
      { to: '/secrets', label: 'Secrets', icon: HardDrive },
      { to: '/shares', label: 'Shares', icon: Share2 },
    ],
  },
  {
    label: 'Tools',
    items: [
      { to: '/generator', label: 'Generator', icon: Settings2 },
      { to: '/mfa', label: 'MFA & security', icon: CircleCheck },
    ],
  },
  {
    label: 'Administration',
    items: [
      { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
      { to: '/tokens', label: 'Tokens', icon: Globe },
      { to: '/audit', label: 'Security log', icon: ScrollText },
      { to: '/import-export', label: 'Import / export', icon: Replace },
      { to: '/settings', label: 'Settings', icon: Settings },
    ],
  },
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
        <div className="flex items-center gap-2.5 border-b border-border px-4 py-4">
          <span className="grid size-8 place-items-center rounded-lg bg-accent/15 text-accent transition-colors duration-200">
            <Lock className="size-4" aria-hidden="true" />
          </span>
          <span className="text-base font-semibold tracking-tight text-text">Vautr</span>
        </div>

        <nav className="flex-1 overflow-y-auto px-2 pt-3" aria-label="Main navigation">
          {NAV_SECTIONS.map((section, si) => (
            <div key={section.label} className={si > 0 ? 'mt-4' : ''}>
              <span className="mb-1 block px-3 text-[10px] font-semibold uppercase tracking-widest text-text-muted/60">
                {section.label}
              </span>
              <div className="space-y-0.5">
                {section.items.map(({ to, label, icon: Icon }) => (
                  <Link
                    key={to}
                    to={to}
                    data-tour={TOUR_ANCHORS[to]}
                    activeOptions={{ exact: to === '/dashboard' }}
                    className="vault-nav-item vault-active-indicator flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted hover:bg-[var(--sidebar-item-hover)] hover:text-text data-[status=active]:bg-[var(--sidebar-item-active)] data-[status=active]:text-text"
                  >
                    <Icon className="size-4 shrink-0" aria-hidden="true" />
                    {label}
                  </Link>
                ))}
              </div>
            </div>
          ))}
        </nav>

        <div className="border-t border-border p-2">
          <button
            type="button"
            onClick={() => void logout()}
            className="vault-nav-item flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted hover:bg-[var(--sidebar-item-hover)] hover:text-text"
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
