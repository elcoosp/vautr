import * as DialogPrimitive from '@rn-primitives/dialog';
import { X } from 'lucide-react-native';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';
import { View } from 'react-native';
import { cn } from '../../lib/utils';
import { Button, ButtonText } from './button';

/**
 * Left-anchored navigation drawer, built on the same `@rn-primitives/dialog`
 * primitive as `Sheet`. Reusing the existing Dialog avoids pulling a second
 * navigation system alongside TanStack Router. Slide-in is from the left and the
 * content hugs the left edge.
 */

const Drawer = DialogPrimitive.Root;
const DrawerTrigger = DialogPrimitive.Trigger;
const DrawerClose = DialogPrimitive.Close;
const DrawerPortal = DialogPrimitive.Portal;

const DrawerOverlay = forwardRef<
  ComponentRef<typeof DialogPrimitive.Overlay>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    className={cn('bg-black/50 absolute inset-0 z-50 flex items-center justify-start', className)}
    {...props}
    ref={ref}
  />
));
DrawerOverlay.displayName = DialogPrimitive.Overlay.displayName;

const DrawerContent = forwardRef<
  ComponentRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <DrawerPortal>
    <DrawerOverlay />
    <DialogPrimitive.Content
      ref={ref}
      className={cn(
        'fixed left-0 top-0 z-50 h-full w-72 max-w-[80%] border-r border-border bg-card web:animate-in web:slide-in-from-left',
        className,
      )}
      {...props}
    >
      <View className="p-6">{children}</View>
      <DialogPrimitive.Close asChild>
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-3 top-3"
          accessibilityLabel="Close"
        >
          <ButtonText className="text-muted-foreground">
            <X size={20} className="text-foreground" />
          </ButtonText>
        </Button>
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </DrawerPortal>
));
DrawerContent.displayName = DialogPrimitive.Content.displayName;

const DrawerHeader = forwardRef<ComponentRef<typeof View>, ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => (
    <View ref={ref} className={cn('flex flex-col space-y-1.5', className)} {...props} />
  ),
);
DrawerHeader.displayName = 'DrawerHeader';

const DrawerFooter = forwardRef<ComponentRef<typeof View>, ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => (
    <View
      ref={ref}
      className={cn('flex flex-row justify-end space-x-2 pt-4', className)}
      {...props}
    />
  ),
);
DrawerFooter.displayName = 'DrawerFooter';

const DrawerTitle = forwardRef<
  ComponentRef<typeof DialogPrimitive.Title>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn('text-lg font-semibold text-foreground', className)}
    {...props}
  />
));
DrawerTitle.displayName = DialogPrimitive.Title.displayName;

const DrawerDescription = forwardRef<
  ComponentRef<typeof DialogPrimitive.Description>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn('text-sm text-muted-foreground', className)}
    {...props}
  />
));
DrawerDescription.displayName = DialogPrimitive.Description.displayName;

export {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerFooter,
  DrawerHeader,
  DrawerOverlay,
  DrawerPortal,
  DrawerTitle,
  DrawerTrigger,
};
