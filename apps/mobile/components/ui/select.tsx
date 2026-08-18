import * as SelectPrimitive from '@rn-primitives/select';
import { Check, ChevronDown, ChevronUp } from 'lucide-react-native';
import type { ComponentPropsWithoutRef, ComponentRef, ReactNode } from 'react';
import { forwardRef } from 'react';
import { Text, View } from 'react-native';
import { cn } from '../../lib/utils';
import { ThemedIcon } from './icon';

const Select = SelectPrimitive.Root;

const SelectGroup = SelectPrimitive.Group;

const SelectValue = SelectPrimitive.Value;

const SelectTrigger = forwardRef<
  ComponentRef<typeof SelectPrimitive.Trigger>,
  Omit<ComponentPropsWithoutRef<typeof SelectPrimitive.Trigger>, 'children'> & {
    children?: ReactNode;
  }
>(({ className, children, ...props }, ref) => (
  <SelectPrimitive.Trigger
    ref={ref}
    className={cn(
      'native:h-12 h-10 flex flex-row items-center justify-between rounded-md border border-input bg-background px-3 py-2 text-sm',
      className,
    )}
    {...props}
  >
    {children}
    <ThemedIcon icon={ChevronDown} size={16} tone="muted" />
  </SelectPrimitive.Trigger>
));
SelectTrigger.displayName = SelectPrimitive.Trigger.displayName;

const SelectContent = forwardRef<
  ComponentRef<typeof SelectPrimitive.Content>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Content>
>(({ className, children, position = 'popper', ...props }, ref) => (
  <SelectPrimitive.Portal>
    <SelectPrimitive.Overlay className="bg-black/50" />
    <SelectPrimitive.Content
      ref={ref}
      className={cn(
        'relative z-50 max-h-96 min-w-[8rem] rounded-md border border-border bg-popover shadow-md',
        position === 'popper' && 'data-[side=bottom]:translate-y-1 data-[side=top]:-translate-y-1',
        className,
      )}
      position={position}
      {...props}
    >
      <SelectPrimitive.ScrollUpButton>
        <ThemedIcon icon={ChevronUp} size={16} tone="muted" />
      </SelectPrimitive.ScrollUpButton>
      <SelectPrimitive.Viewport
        className={cn(
          'p-1',
          position === 'popper' &&
            'h-[var(--radix-select-trigger-height)] w-full min-w-[var(--radix-select-trigger-width)]',
        )}
      >
        {children}
      </SelectPrimitive.Viewport>
      <SelectPrimitive.ScrollDownButton>
        <ThemedIcon icon={ChevronDown} size={16} tone="muted" />
      </SelectPrimitive.ScrollDownButton>
    </SelectPrimitive.Content>
  </SelectPrimitive.Portal>
));
SelectContent.displayName = SelectPrimitive.Content.displayName;

const SelectLabel = forwardRef<
  ComponentRef<typeof SelectPrimitive.Label>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Label>
>(({ className, ...props }, ref) => (
  <SelectPrimitive.Label
    ref={ref}
    className={cn(
      'native:pb-2 py-1.5 pl-8 pr-2 text-sm font-semibold text-popover-foreground',
      className,
    )}
    {...props}
  />
));
SelectLabel.displayName = SelectPrimitive.Label.displayName;

const SelectItem = forwardRef<
  ComponentRef<typeof SelectPrimitive.Item>,
  Omit<ComponentPropsWithoutRef<typeof SelectPrimitive.Item>, 'children'> & { children?: ReactNode }
>(({ className, children, ...props }, ref) => (
  <SelectPrimitive.Item
    ref={ref}
    className={cn(
      'web:group web:cursor-pointer native:py-2 py-1.5 pl-8 pr-2 text-sm relative flex w-full flex-row items-center rounded-sm web:select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
      className,
    )}
    {...props}
  >
    <View className="absolute left-2 native:left-3.5 flex h-3.5 w-3.5 items-center justify-center">
      <SelectPrimitive.ItemIndicator>
        <ThemedIcon icon={Check} size={14} tone="foreground" />
      </SelectPrimitive.ItemIndicator>
    </View>
    <Text className="pl-8 text-sm text-popover-foreground">{children}</Text>
  </SelectPrimitive.Item>
));
SelectItem.displayName = SelectPrimitive.Item.displayName;

const SelectSeparator = forwardRef<
  ComponentRef<typeof SelectPrimitive.Separator>,
  ComponentPropsWithoutRef<typeof SelectPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <SelectPrimitive.Separator
    ref={ref}
    className={cn('-mx-1 my-1 h-px bg-muted', className)}
    {...props}
  />
));
SelectSeparator.displayName = SelectPrimitive.Separator.displayName;

export {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
};
