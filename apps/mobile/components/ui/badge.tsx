import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';
import { Text } from 'react-native';

import { cn } from '../../lib/utils';

const badgeVariants = cva(
  'web:inline-flex items-center rounded-full border border-border px-2.5 py-0.5 web:transition-colors web:focus:outline-none web:focus:ring-2 web:focus:ring-ring web:focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-primary text-primary-foreground web:hover:bg-primary/80',
        secondary: 'border-transparent bg-secondary text-secondary-foreground',
        destructive:
          'border-transparent bg-destructive text-destructive-foreground web:hover:bg-destructive/80',
        outline: 'text-foreground',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  },
);

const badgeTextVariants = cva('text-xs font-semibold native:text-sm', {
  variants: {
    variant: {
      default: 'text-primary-foreground',
      secondary: 'text-secondary-foreground',
      destructive: 'text-destructive-foreground',
      outline: 'text-foreground',
    },
  },
  defaultVariants: {
    variant: 'default',
  },
});

type BadgeProps = ComponentPropsWithoutRef<typeof Text> & VariantProps<typeof badgeVariants>;

const Badge = forwardRef<ComponentRef<typeof Text>, BadgeProps>(
  ({ className, variant, children, ...props }, ref) => {
    return (
      <Text ref={ref} className={cn(badgeVariants({ variant }), className)} {...props}>
        <Text className={cn(badgeTextVariants({ variant }))}>{children}</Text>
      </Text>
    );
  },
);

export type { BadgeProps };
export { Badge, badgeTextVariants, badgeVariants };
