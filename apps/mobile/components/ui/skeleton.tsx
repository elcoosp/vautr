import * as React from 'react';
import { View } from 'react-native';

import { cn } from '../../lib/utils';

const Skeleton = React.forwardRef<View, React.ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => (
    <View ref={ref} className={cn('web:animate-pulse rounded-md bg-muted', className)} {...props} />
  ),
);
Skeleton.displayName = 'Skeleton';

export { Skeleton };
