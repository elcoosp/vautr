import * as SwitchPrimitives from '@rn-primitives/switch';
import type { ComponentPropsWithoutRef, ComponentRef } from 'react';
import { forwardRef } from 'react';

import { cn } from '../../lib/utils';

const Switch = forwardRef<
  ComponentRef<typeof SwitchPrimitives.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitives.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitives.Root
    className={cn(
      'peer web:h-6 web:w-11 native:h-8 native:w-14 shrink-0 flex-row items-center rounded-full border-2 border-transparent bg-muted transition-colors web:cursor-pointer web:select-none web:data-[state=checked]:bg-primary web:data-[state=unchecked]:bg-input native:data-[state=checked]:bg-primary',
      className,
    )}
    {...props}
    ref={ref}
  >
    <SwitchPrimitives.Thumb
      className={cn(
        'native:w-6 native:h-6 web:h-5 web:w-5 rounded-full bg-white shadow-md web:shadow-sm web:transition-transform native:data-[state=checked]:translate-x-7 native:data-[state=unchecked]:translate-x-0 web:data-[state=checked]:translate-x-5 web:data-[state=unchecked]:translate-x-0',
      )}
    />
  </SwitchPrimitives.Root>
));
Switch.displayName = SwitchPrimitives.Root.displayName;

export { Switch };
