import { Crown, Eye, Pencil, ShieldCheck } from 'lucide-react';
import { Badge } from '@/components/ui/badge';

const PERMISSION_META: Record<string, { label: string; icon: typeof Eye }> = {
  can_view: { label: 'Can View', icon: Eye },
  can_edit: { label: 'Can Edit', icon: Pencil },
  can_manage: { label: 'Can Manage', icon: ShieldCheck },
};

const ROLE_META: Record<string, { label: string; icon: typeof Crown }> = {
  owner: { label: 'Owner', icon: Crown },
  admin: { label: 'Admin', icon: ShieldCheck },
  member: { label: 'Member', icon: Eye },
};

export function PermissionBadge({ permission }: { permission?: string | null }) {
  if (!permission) return null;
  const meta = PERMISSION_META[permission];
  if (!meta) return <Badge>{permission}</Badge>;
  const Icon = meta.icon;
  return (
    <Badge variant="secondary" className="gap-1">
      <Icon className="size-3" aria-hidden="true" />
      {meta.label}
    </Badge>
  );
}

export function RoleBadge({ role }: { role?: string | null }) {
  if (!role) return null;
  const meta = ROLE_META[role];
  if (!meta) return <Badge>{role}</Badge>;
  const Icon = meta.icon;
  return (
    <Badge variant="outline" className="gap-1">
      <Icon className="size-3" aria-hidden="true" />
      {meta.label}
    </Badge>
  );
}
