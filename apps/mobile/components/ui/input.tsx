import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';
import { TextInput } from 'react-native';

import { cn } from '../../lib/utils';

const Input = forwardRef<ComponentRef<typeof TextInput>, ComponentPropsWithoutRef<typeof TextInput>>(
  ({ className, placeholderTextColor, ...props }, ref) => {
    return (
      <TextInput
        ref={ref}
        placeholderTextColor={placeholderTextColor ?? '#6b7280'}
        className={cn(
          'web:flex h-10 native:h-12 web:w-full rounded-md border border-input bg-background px-3 web:py-2 text-base native:text-lg text-foreground placeholder:text-muted-foreground web:ring-offset-background file:border-0 file:bg-transparent file:font-medium web:focus-visible:outline-none web:focus-visible:ring-2 web:focus-visible:ring-ring web:focus-visible:ring-offset-2 lg:text-sm',
          className,
        )}
        {...props}
      />
    );
  },
);

export { Input };
