import type { LucideIcon } from 'lucide-react-native';
import { View } from 'react-native';
import { Button, ButtonText } from './button';
import { ThemedIcon } from './icon';
import { Text } from './text';

type EmptyStateProps = {
  icon: LucideIcon;
  title: string;
  description?: string;
  action?: { label: string; onPress: () => void };
  variant?: 'card' | 'inline';
  className?: string;
};

/**
 * Canonical centered empty state: icon + title + optional subtitle + optional CTA.
 * Mirrors the desktop `empty_state` helper and the web/extension `EmptyState` so
 * every client renders the same shape for an empty list/section.
 *
 * - `card`  (default): dashed-border card — use for a full empty section/list.
 * - `inline`: compact icon + muted text, no border — use inside an existing card.
 */
export function EmptyState({
  icon,
  title,
  description,
  action,
  variant = 'card',
  className,
}: EmptyStateProps) {
  if (variant === 'inline') {
    return (
      <View className={`items-center gap-2 py-4 ${className ?? ''}`}>
        <ThemedIcon icon={icon} size={18} tone="muted" />
        <Text variant="muted" className="text-center">
          {title}
        </Text>
      </View>
    );
  }
  return (
    <View
      className={`flex-1 items-center justify-center gap-3 rounded-lg border border-dashed border-border px-6 py-10 ${className ?? ''}`}
    >
      <View className="items-center justify-center rounded-full bg-muted/40 p-3">
        <ThemedIcon icon={icon} size={24} tone="muted" />
      </View>
      <Text variant="h4" className="text-center">
        {title}
      </Text>
      {description ? (
        <Text variant="muted" className="max-w-[16rem] text-center">
          {description}
        </Text>
      ) : null}
      {action ? (
        <Button variant="outline" className="mt-2" onPress={action.onPress}>
          <ButtonText>{action.label}</ButtonText>
        </Button>
      ) : null}
    </View>
  );
}
