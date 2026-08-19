import { X } from 'lucide-react-native';
import type { ComponentPropsWithoutRef, ReactNode } from 'react';
import { useEffect, useRef, useState } from 'react';
import { Animated, Pressable, Text, View } from 'react-native';
import { cn } from '../../lib/utils';
import { Button, ButtonText } from './button';
import { ThemedIcon } from './icon';

/**
 * Left-anchored navigation drawer implemented WITHOUT @rn-primitives' portal.
 *
 * The portal-based Dialog approach (used previously) renders its children into
 * a <PortalHost /> via a zustand store. On the iOS simulator that relocation
 * produced no visible output, so the drawer never appeared. A plain inline
 * overlay + Animated.View slide-in is self-contained, reliably renders, and
 * gives the native slide animation the design calls for.
 */

const DRAWER_WIDTH = 288; // w-72

function Drawer({
  open,
  onOpenChange,
  children,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}) {
  const translateX = useRef(new Animated.Value(open ? 0 : -DRAWER_WIDTH)).current;
  const [mounted, setMounted] = useState(open);

  useEffect(() => {
    if (open) setMounted(true);
    const anim = Animated.timing(translateX, {
      toValue: open ? 0 : -DRAWER_WIDTH,
      duration: 240,
      useNativeDriver: true,
    });
    anim.start(({ finished }) => {
      if (finished && !open) setMounted(false);
    });
    return () => anim.stop();
  }, [open, translateX]);

  if (!mounted) return null;

  return (
    <View className="absolute inset-0 z-50" pointerEvents={open ? 'auto' : 'none'}>
      <Pressable
        className="absolute inset-0 bg-black/50"
        onPress={() => onOpenChange(false)}
        accessibilityLabel="Close menu"
      />
      <Animated.View
        style={{ transform: [{ translateX }] }}
        className="absolute left-0 top-0 h-full w-72 max-w-[80%] border-r border-border bg-card"
      >
        {children}
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-3 top-3"
          accessibilityLabel="Close"
          onPress={() => onOpenChange(false)}
        >
          <ButtonText className="text-muted-foreground">
            <ThemedIcon icon={X} size={20} tone="muted" />
          </ButtonText>
        </Button>
      </Animated.View>
    </View>
  );
}

const DrawerContent = ({ className, ...props }: ComponentPropsWithoutRef<typeof View>) => (
  <View className={cn('flex h-full flex-col p-6', className)} {...props} />
);

const DrawerHeader = ({ className, ...props }: ComponentPropsWithoutRef<typeof View>) => (
  <View className={cn('flex flex-col space-y-1.5', className)} {...props} />
);

const DrawerTitle = ({ className, ...props }: ComponentPropsWithoutRef<typeof Text>) => (
  <Text className={cn('text-xl font-semibold text-foreground', className)} {...props} />
);

export { Drawer, DrawerContent, DrawerHeader, DrawerTitle };
