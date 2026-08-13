import * as TabsPrimitive from '@rn-primitives/tabs';
import type { ComponentPropsWithoutRef, ComponentRef, ReactNode } from 'react';
import { forwardRef } from 'react';
import { Text } from 'react-native';

import { cn } from '../../lib/utils';

const Tabs = TabsPrimitive.Root;

const TabsList = forwardRef<
  ComponentRef<typeof TabsPrimitive.List>,
  ComponentPropsWithoutRef<typeof TabsPrimitive.List>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.List
    ref={ref}
    className={cn(
      'native:h-12 web:inline-flex h-10 items-center justify-center rounded-md bg-muted p-1 web:w-full',
      className,
    )}
    {...props}
  />
));
TabsList.displayName = TabsPrimitive.List.displayName;

const TabsTrigger = forwardRef<
  ComponentRef<typeof TabsPrimitive.Trigger>,
  Omit<ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>, 'children'> & {
    children?: ReactNode;
  }
>(({ className, children, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(
      'group inline-flex items-center justify-center rounded-sm px-3 py-1.5 text-sm font-medium web:transition-all data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=inactive]:text-muted-foreground web:data-[state=active]:shadow-sm native:rounded-md native:px-4 native:py-2',
      className,
    )}
    {...props}
  >
    <Text className="text-sm font-medium text-foreground data-[slot=trigger]:group-data-[state=inactive]:text-muted-foreground">
      {children}
    </Text>
  </TabsPrimitive.Trigger>
));
TabsTrigger.displayName = TabsPrimitive.Trigger.displayName;

const TabsContent = forwardRef<
  ComponentRef<typeof TabsPrimitive.Content>,
  ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn('web:ring-offset-background mt-2 web:focus-visible:outline-none', className)}
    {...props}
  />
));
TabsContent.displayName = TabsPrimitive.Content.displayName;

export { Tabs, TabsContent, TabsList, TabsTrigger };
