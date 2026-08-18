import * as React from 'react';
import { Text, View } from 'react-native';

import { cn } from '../../lib/utils';

const alertVariants = {
  default: 'border-border bg-card',
  destructive: 'border-destructive/40 bg-destructive/10',
  warning: 'border-yellow-500/40 bg-yellow-500/10',
  info: 'border-primary/40 bg-primary/10',
} as const;

type AlertVariant = keyof typeof alertVariants;

type AlertProps = React.ComponentPropsWithoutRef<typeof View> & {
  variant?: AlertVariant;
};

const Alert = React.forwardRef<View, AlertProps>(
  ({ className, variant = 'default', ...props }, ref) => (
    <View
      ref={ref}
      role="alert"
      accessibilityRole="alert"
      className={cn('flex flex-col gap-1 rounded-lg border p-4', alertVariants[variant], className)}
      {...props}
    />
  ),
);
Alert.displayName = 'Alert';

const AlertTitle = React.forwardRef<
  React.ComponentRef<typeof View>,
  React.ComponentPropsWithoutRef<typeof View>
>(({ className, children, ...props }, ref) => (
  // RN requires text in a <Text>; wrap bare strings while leaving
  // already-<Text>-wrapped element children untouched.
  <View ref={ref} className={cn('text-sm font-semibold text-foreground', className)} {...props}>
    {typeof children === 'string' ? (
      <Text className="text-sm font-semibold text-foreground">{children}</Text>
    ) : (
      children
    )}
  </View>
));
AlertTitle.displayName = 'AlertTitle';

const AlertDescription = React.forwardRef<
  React.ComponentRef<typeof View>,
  React.ComponentPropsWithoutRef<typeof View>
>(({ className, children, ...props }, ref) => (
  // RN requires text in a <Text>; wrap bare strings (e.g. error messages)
  // while leaving element children (already-<Text>-wrapped) untouched.
  <View ref={ref} className={cn('text-sm text-muted-foreground', className)} {...props}>
    {typeof children === 'string' ? (
      <Text className="text-sm text-muted-foreground">{children}</Text>
    ) : (
      children
    )}
  </View>
));
AlertDescription.displayName = 'AlertDescription';

export type { AlertProps, AlertVariant };
export { Alert, AlertDescription, AlertTitle };
