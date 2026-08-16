import { cva, type VariantProps } from 'class-variance-authority';
import * as React from 'react';
import { Text as RNText, type TextProps as RNTextProps } from 'react-native';

import { cn } from '../../lib/utils';

const textVariants = cva('text-foreground', {
  variants: {
    variant: {
      h1: 'text-3xl font-bold tracking-tight',
      h2: 'text-2xl font-semibold tracking-tight',
      h3: 'text-xl font-semibold tracking-tight',
      h4: 'text-lg font-semibold tracking-tight',
      p: 'text-base leading-relaxed',
      large: 'text-lg font-medium',
      lead: 'text-lg text-muted-foreground leading-relaxed',
      small: 'text-sm',
      muted: 'text-sm text-muted-foreground',
      tiny: 'text-xs text-muted-foreground',
      label: 'text-sm font-medium',
      link: 'text-sm text-primary native:underline web:underline-offset-4 web:hover:underline',
    },
  },
  defaultVariants: {
    variant: 'p',
  },
});

type TextProps = RNTextProps & VariantProps<typeof textVariants>;

const Text = React.forwardRef<RNText, TextProps>(({ className, variant, ...props }, ref) => (
  <RNText ref={ref} className={cn(textVariants({ variant }), className)} {...props} />
));
Text.displayName = 'Text';

export type { TextProps };
export { Text, textVariants };
