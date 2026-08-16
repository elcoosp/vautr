import * as React from 'react';
import { View } from 'react-native';

import { cn } from '../../lib/utils';

type SeparatorProps = React.ComponentPropsWithoutRef<typeof View> & {
  orientation?: 'horizontal' | 'vertical';
  decorative?: boolean;
};

const Separator = React.forwardRef<View, SeparatorProps>(
  ({ className, orientation = 'horizontal', decorative = true, ...props }, ref) => (
    <View
      ref={ref}
      className={cn(
        'bg-border',
        orientation === 'horizontal' ? 'h-[1px] w-full' : 'h-full w-[1px]',
        className,
      )}
      {...props}
    />
  ),
);
Separator.displayName = 'Separator';

export type { SeparatorProps };
export { Separator };
