import * as React from 'react';
import { useEffect } from 'react';
import { View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withTiming,
} from 'react-native-reanimated';

import { cn } from '../../lib/utils';

const Skeleton = React.forwardRef<View, React.ComponentPropsWithoutRef<typeof View>>(
  ({ className, ...props }, ref) => {
    const pulse = useSharedValue(0.4);

    useEffect(() => {
      pulse.value = withRepeat(withTiming(1, { duration: 1100 }), -1, true);
      return () => {
        pulse.value = 0.4;
      };
    }, [pulse]);

    const animatedStyle = useAnimatedStyle(() => ({
      opacity: pulse.value,
    }));

    return (
      <Animated.View
        ref={ref}
        style={animatedStyle}
        className={cn('rounded-md bg-skeleton-base', className)}
        {...props}
      />
    );
  },
);
Skeleton.displayName = 'Skeleton';

export { Skeleton };