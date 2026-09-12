import type { LucideProps } from 'lucide-react-native';
import { Crown, Eye, Pencil, ShieldCheck } from 'lucide-react-native';
import type { ComponentType } from 'react';
import { View } from 'react-native';
import { ThemedIcon } from '../components/ui/icon';
import { Text } from '../components/ui/text';

const PERMISSION_META: Record<string, { label: string; icon: ComponentType<LucideProps> }> = {
  can_view: { label: 'Can View', icon: Eye },
  can_edit: { label: 'Can Edit', icon: Pencil },
  can_manage: { label: 'Can Manage', icon: ShieldCheck },
};

const ROLE_META: Record<string, { label: string; icon: ComponentType<LucideProps> }> = {
  owner: { label: 'Owner', icon: Crown },
  admin: { label: 'Admin', icon: ShieldCheck },
  member: { label: 'Member', icon: Eye },
};

function Pill({
  icon,
  label,
  variant = 'secondary',
}: {
  icon: ComponentType<LucideProps>;
  label: string;
  variant?: 'secondary' | 'outline';
}) {
  return (
    <View
      className={`flex-row items-center gap-1 rounded-full px-2 py-0.5 ${
        variant === 'secondary' ? 'bg-secondary' : 'border border-border'
      }`}
    >
      <ThemedIcon icon={icon} size={12} tone="muted" />
      <Text
        className={`text-xs font-semibold ${variant === 'secondary' ? 'text-secondary-foreground' : 'text-foreground'}`}
      >
        {label}
      </Text>
    </View>
  );
}

export function PermissionBadge({ permission }: { permission?: string | null }) {
  if (!permission) return null;
  const meta = PERMISSION_META[permission];
  if (!meta) {
    return (
      <View className="flex-row items-center rounded-full border border-border px-2 py-0.5">
        <Text className="text-xs font-semibold text-foreground">{permission}</Text>
      </View>
    );
  }
  return <Pill icon={meta.icon} label={meta.label} variant="secondary" />;
}

export function RoleBadge({ role }: { role?: string | null }) {
  if (!role) return null;
  const meta = ROLE_META[role];
  if (!meta) {
    return (
      <View className="flex-row items-center rounded-full border border-border px-2 py-0.5">
        <Text className="text-xs font-semibold text-foreground">{role}</Text>
      </View>
    );
  }
  return <Pill icon={meta.icon} label={meta.label} variant="outline" />;
}
