import * as ToastPrimitive from '@rn-primitives/toast';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import {
  createContext,
  forwardRef,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Pressable, Text, View } from 'react-native';
import { X } from 'lucide-react-native';

import { cn } from '../../lib/utils';

type ToastVariant = 'default' | 'destructive';

interface ToastItem {
  id: number;
  title: string;
  description?: string;
  variant?: ToastVariant;
}

interface ToastContextValue {
  show: (toast: Omit<ToastItem, 'id'> & { id?: number }) => number;
  dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const TOAST_AUTO_DISMISS_MS = 4000;

/** Provider managing an in-memory list of toasts (sonner-equivalent). */
export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((t) => t.id !== id));
  }, []);

  const show = useCallback((toast: Omit<ToastItem, 'id'> & { id?: number }) => {
    const id = toast.id ?? nextId.current++;
    setToasts((current) => [...current.filter((t) => t.id !== id), { ...toast, id }]);
    return id;
  }, []);

  const value = useMemo(() => ({ show, dismiss }), [show, dismiss]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

function useToastContext(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast must be used within a ToastProvider');
  }
  return ctx;
}

/** Imperative hook: `const toast = useToast(); toast.show({ title: '...' })`. */
export function useToast(): ToastContextValue {
  return useToastContext();
}

function ToastViewport({
  toasts,
  onDismiss,
}: {
  toasts: ToastItem[];
  onDismiss: (id: number) => void;
}) {
  return (
    <View
      pointerEvents="box-none"
      className="pointer-events-none absolute inset-x-0 top-6 z-50 flex flex-col items-center gap-2 px-4"
    >
      {toasts.map((toast) => (
        <Toast key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </View>
  );
}

function Toast({ toast, onDismiss }: { toast: ToastItem; onDismiss: (id: number) => void }) {
  const [open, setOpen] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => {
      setOpen(false);
      onDismiss(toast.id);
    }, TOAST_AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast.id]);

  return (
    <ToastPrimitive.Root
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) onDismiss(toast.id);
      }}
      type="foreground"
      className={cn(
        'w-full flex-row items-start rounded-lg border bg-card p-4 shadow-md',
        toast.variant === 'destructive' ? 'border-destructive' : 'border-border',
      )}
    >
      <View className="flex-1 gap-1">
        <ToastPrimitive.Title className="text-sm font-semibold text-foreground">
          {toast.title}
        </ToastPrimitive.Title>
        {toast.description ? (
          <ToastPrimitive.Description className="text-sm text-muted-foreground">
            {toast.description}
          </ToastPrimitive.Description>
        ) : null}
      </View>
      <ToastPrimitive.Close asChild onPress={() => onDismiss(toast.id)}>
        <Pressable accessibilityLabel="Dismiss toast">
          <X size={16} className="text-muted-foreground" />
        </Pressable>
      </ToastPrimitive.Close>
    </ToastPrimitive.Root>
  );
}

// Re-export low-level toast parts for composition.
const ToastTitle = forwardRef<
  ComponentRef<typeof ToastPrimitive.Title>,
  ComponentPropsWithoutRef<typeof ToastPrimitive.Title>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Title
    ref={ref}
    className={cn('text-sm font-semibold text-foreground', className)}
    {...props}
  />
));
const ToastDescription = forwardRef<
  ComponentRef<typeof ToastPrimitive.Description>,
  ComponentPropsWithoutRef<typeof ToastPrimitive.Description>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Description
    ref={ref}
    className={cn('text-sm text-muted-foreground', className)}
    {...props}
  />
));

export { ToastDescription, ToastTitle };
