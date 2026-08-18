import * as DialogPrimitive from '@rn-primitives/dialog';
import { X } from 'lucide-react-native';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';
import { View } from 'react-native';
import { cn } from '../../lib/utils';
import { Button, ButtonText } from './button';

const Sheet = DialogPrimitive.Root;
const SheetTrigger = DialogPrimitive.Trigger;
const SheetClose = DialogPrimitive.Close;
const SheetPortal = DialogPrimitive.Portal;

const SheetOverlay = forwardRef<
  ComponentRef<typeof DialogPrimitive.Overlay>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    className={cn('bg-black/50 absolute inset-0 z-50 flex items-center justify-end p-2', className)}
    {...props}
    ref={ref}
  />
));
SheetOverlay.displayName = DialogPrimitive.Overlay.displayName;

const SheetContent = forwardRef<
  ComponentRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <SheetPortal>
    <SheetOverlay />
    <DialogPrimitive.Content
      ref={ref}
      className={cn(
        'fixed native:absolute left-0 right-0 top-0 z-50 border-b border-border bg-card web:animate-in web:slide-in-from-top',
        className,
      )}
      {...props}
    >
      <View className="mt-4 h-1.5 w-12 self-center rounded-full bg-muted" />
      <View className="p-6">{children}</View>
      <DialogPrimitive.Close asChild>
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-4 top-4 web:top-3"
          accessibilityLabel="Close"
        >
          <ButtonText className="h-4 w-4 text-muted-foreground">
            <X size={20} className="text-foreground" />
          </ButtonText>
        </Button>
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </SheetPortal>
));
SheetContent.displayName = DialogPrimitive.Content.displayName;

const SheetHeader = forwardRef<ComponentRef<typeof View>, ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => (
    <View
      ref={ref}
      className={cn('flex flex-col space-y-1.5 text-center sm:text-left', className)}
      {...props}
    />
  ),
);
SheetHeader.displayName = 'SheetHeader';

const SheetFooter = forwardRef<ComponentRef<typeof View>, ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => (
    <View
      ref={ref}
      className={cn('flex flex-row justify-end space-x-2 pt-4', className)}
      {...props}
    />
  ),
);
SheetFooter.displayName = 'SheetFooter';

const SheetTitle = forwardRef<
  ComponentRef<typeof DialogPrimitive.Title>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn('text-lg font-semibold text-foreground', className)}
    {...props}
  />
));
SheetTitle.displayName = DialogPrimitive.Title.displayName;

const SheetDescription = forwardRef<
  ComponentRef<typeof DialogPrimitive.Description>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn('text-sm text-muted-foreground', className)}
    {...props}
  />
));
SheetDescription.displayName = DialogPrimitive.Description.displayName;

export {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetOverlay,
  SheetPortal,
  SheetTitle,
  SheetTrigger,
};
