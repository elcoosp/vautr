import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type EmptyStateProps = {
  icon: LucideIcon;
  title: string;
  description?: ReactNode;
  action?: { label: string; onClick: () => void };
  variant?: 'card' | 'inline';
  className?: string;
};

/**
 * Canonical centered empty state: icon + title + optional subtitle + optional CTA.
 * Mirrors the desktop `empty_state` helper and the web/mobile `EmptyState` so every
 * client renders the same shape for an empty list/section.
 *
 * - `card`  (default): dashed-border card — use for a full empty section/list.
 * - `inline`: compact icon + muted text, no border — use inside an existing card.
 */
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  variant = 'card',
  className,
}: EmptyStateProps) {
  if (variant === 'inline') {
    return (
      <div
        className={cn(
          'flex flex-col items-center justify-center gap-2 py-6 text-center',
          className,
        )}
      >
        <Icon className="h-5 w-5 text-muted-foreground" />
        <p className="text-xs text-text-muted">{title}</p>
      </div>
    );
  }
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border px-4 py-8 text-center',
        className,
      )}
    >
      <div className="flex h-10 w-10 items-center justify-center rounded-full bg-muted/40">
        <Icon className="h-5 w-5 text-muted-foreground" />
      </div>
      <div className="space-y-1">
        <p className="text-sm font-semibold text-text">{title}</p>
        {description ? <p className="text-xs text-text-muted">{description}</p> : null}
      </div>
      {action ? (
        <Button variant="outline" className="mt-2" onClick={action.onClick}>
          {action.label}
        </Button>
      ) : null}
    </div>
  );
}
