import * as Slot from '@rn-primitives/slot';
import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';
import { Pressable, Text } from 'react-native';

import { cn } from '../../lib/utils';

const buttonVariants = cva(
  'group flex items-center justify-center rounded-md web:ring-offset-background web:transition-colors web:focus-visible:outline-none web:focus-visible:ring-2 web:focus-visible:ring-ring web:focus-visible:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'bg-primary web:hover:opacity-90 active:opacity-90',
        destructive: 'bg-destructive web:hover:opacity-90 active:opacity-90',
        outline:
          'border border-input bg-background web:hover:bg-accent web:hover:text-accent-foreground active:bg-accent',
        secondary: 'bg-secondary web:hover:opacity-80 active:opacity-80',
        ghost: 'web:hover:bg-accent web:hover:text-accent-foreground active:bg-accent',
        link: 'web:underline-offset-4 web:hover:underline web:focus:underline',
      },
      size: {
        default: 'h-10 px-4 py-2 native:h-12 native:px-5 native:py-3',
        sm: 'h-9 rounded-md px-3',
        lg: 'h-11 rounded-md px-8 native:h-14',
        icon: 'h-10 w-10',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
);

const buttonTextVariants = cva(
  'web:whitespace-nowrap text-sm native:text-base font-medium text-foreground web:transition-colors',
  {
    variants: {
      variant: {
        default: 'text-primary-foreground',
        destructive: 'text-destructive-foreground',
        outline: 'group-active:text-accent-foreground',
        secondary: 'text-secondary-foreground group-active:text-secondary-foreground',
        ghost: 'group-active:text-accent-foreground',
        link: 'text-primary group-active:underline',
      },
      size: {
        default: '',
        sm: 'text-sm',
        lg: 'text-lg native:text-xl',
        icon: '',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
);

type ButtonProps = ComponentPropsWithoutRef<typeof Pressable> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean;
  };

const Button = forwardRef<ComponentRef<typeof Pressable>, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot.Pressable : Pressable;
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        role="button"
        {...props}
      />
    );
  },
);

type ButtonTextProps = ComponentPropsWithoutRef<typeof Text> &
  VariantProps<typeof buttonTextVariants>;

const ButtonText = forwardRef<ComponentRef<typeof Text>, ButtonTextProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <Text className={cn(buttonTextVariants({ variant, size, className }))} ref={ref} {...props} />
    );
  },
);

export type { ButtonProps, ButtonTextProps };
export { Button, ButtonText, buttonTextVariants, buttonVariants };
