import * as React from 'react';
import { Text, View } from 'react-native';

import { cn } from '../../lib/utils';

const Avatar = React.forwardRef<
  View,
  React.ComponentPropsWithoutRef<typeof View> & {
    size?: number;
  }
>(({ className, size = 40, style, ...props }, ref) => (
  <View
    ref={ref}
    className={cn('items-center justify-center rounded-full bg-secondary', className)}
    style={[{ width: size, height: size, borderRadius: size / 2 }, style]}
    {...props}
  />
));
Avatar.displayName = 'Avatar';

const AvatarFallbackText = React.forwardRef<
  React.ComponentRef<typeof Text>,
  React.ComponentPropsWithoutRef<typeof Text>
>(({ className, ...props }, ref) => (
  <Text
    ref={ref}
    className={cn('text-sm font-semibold text-secondary-foreground', className)}
    {...props}
  />
));
AvatarFallbackText.displayName = 'AvatarFallbackText';

export { Avatar, AvatarFallbackText };
