import { useVirtualizer } from '@tanstack/react-virtual';
import type { DecryptedOverview } from '@vautr/ui-logic';
import { useOverviews } from '@vautr/ui-logic';
import { KeyRound } from 'lucide-react';
import { useRef } from 'react';
import { Favicon } from './Favicon';

interface VaultListProps {
  selectedUuid: string | null;
  onSelect: (uuid: string) => void;
}

export function VaultList({ selectedUuid, onSelect }: VaultListProps) {
  const overviews = useOverviews();
  const sorted = [...overviews].sort((a, b) => b.updatedAt - a.updatedAt);
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: sorted.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 64,
    overscan: 8,
  });

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const idx = sorted.findIndex((item) => item.uuid === selectedUuid);
      const next =
        event.key === 'ArrowDown' ? Math.min(idx + 1, sorted.length - 1) : Math.max(idx - 1, 0);
      const item = sorted[next];
      if (item) {
        onSelect(item.uuid);
      }
    }
  };

  return (
    <div
      ref={parentRef}
      role="listbox"
      aria-label="Vault items"
      aria-multiselectable="false"
      tabIndex={0}
      onKeyDown={onKeyDown}
      className="h-full overflow-auto bg-surface"
    >
      {sorted.length === 0 ? (
        <p className="p-6 text-center text-sm text-text-muted">No items yet.</p>
      ) : (
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const item = sorted[virtualRow.index] as DecryptedOverview;
            const isSelected = item.uuid === selectedUuid;
            const faviconUrl = item.urls?.[0];
            return (
              <div
                key={item.uuid}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: virtualRow.size,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <button
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  data-uuid={item.uuid}
                  onClick={() => onSelect(item.uuid)}
                  className={`flex w-full items-center gap-3 border-b border-border px-4 text-left transition-colors focus-visible:outline focus-visible:outline-accent ${
                    isSelected ? 'bg-surface-raised' : 'bg-surface hover:bg-surface-raised'
                  }`}
                  style={{ height: virtualRow.size - 1 }}
                >
                  <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/15 text-accent">
                    {faviconUrl ? (
                      <Favicon url={faviconUrl} size={20} />
                    ) : (
                      <KeyRound className="size-4" aria-hidden="true" />
                    )}
                  </span>
                  <span className="min-w-0">
                    <span className="block truncate font-medium text-text">{item.title}</span>
                    <span className="block truncate text-sm text-text-muted">{item.subtitle}</span>
                  </span>
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
