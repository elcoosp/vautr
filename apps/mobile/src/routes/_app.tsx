import { createFileRoute, Outlet, useRouter } from '@tanstack/react-router';
import {
  Bot,
  CircleCheck,
  Fingerprint,
  Folder,
  Globe,
  HardDrive,
  LayoutDashboard,
  List,
  Lock,
  LogOut,
  Menu,
  Replace,
  Settings,
  Settings2,
  Share2,
} from 'lucide-react-native';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Button, ButtonText } from '../../components/ui/button';
import { Drawer, DrawerContent, DrawerHeader, DrawerTitle } from '../../components/ui/drawer';
import { ThemedIcon } from '../../components/ui/icon';
import { useToast } from '../../components/ui/toast';
import { requireBiometric } from '../../lib/biometrics';
import { isLocalVaultActive, services } from '../../lib/client';
import { useSession } from '../../lib/session';

export const Route = createFileRoute('/_app')({
  component: AppShell,
});

const DRAWER_ITEMS = [
  { to: '/', label: 'Projects', icon: Folder },
  { to: '/secrets', label: 'Secrets', icon: HardDrive },
  { to: '/generator', label: 'Generator', icon: Settings2 },
  { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/settings', label: 'Settings', icon: Settings },
  { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
  { to: '/mfa', label: 'MFA & security', icon: CircleCheck },
  { to: '/tokens', label: 'Tokens', icon: Globe },
  { to: '/import-export', label: 'Import / export', icon: Replace },
  ...(isLocalVaultActive() ? [{ to: '/shares', label: 'Shares', icon: Share2 } as const] : []),
];

function AppShell() {
  const username = useSession((s) => s.username);
  const [locked, setLocked] = useState(false);
  const router = useRouter();
  const toast = useToast();
  const [drawerOpen, setDrawerOpen] = useState(false);

  // Native-gated sharing tab: only when the uniffi core is linked (VTR-070).
  const PRIMARY = [
    { to: '/secrets', label: 'Secrets', icon: HardDrive },
    { to: '/generator', label: 'Generator', icon: Settings2 },
    { to: '/settings', label: 'Settings', icon: Settings },
    ...(isLocalVaultActive() ? [{ to: '/shares', label: 'Shares', icon: Share2 } as const] : []),
  ];

  const go = (to: string) => () => {
    setDrawerOpen(false);
    router.navigate({ to: to as '/' });
  };

  const unlock = async () => {
    const gate = await requireBiometric('Vautr unlock');
    if (gate.success) {
      setLocked(false);
      toast.show({ title: 'Unlocked', description: 'Vault unlocked with biometrics.' });
    } else {
      toast.show({
        title: 'Authentication cancelled',
        variant: 'destructive',
      });
    }
  };

  const logout = async () => {
    await services.auth.logout();
    services.reset();
    useSession.getState().signOut();
    router.navigate({ to: '/login' });
  };

  return (
    <SafeAreaView className="flex-1 bg-background">
      {/* Brand header — canonical: hamburger (left) + size-8 accent tile + wordmark + username. */}
      <View className="flex-row items-center justify-between border-b border-border px-3 py-3">
        <View className="flex-row items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            accessibilityLabel="Open menu"
            onPress={() => setDrawerOpen(true)}
          >
            <ButtonText className="text-foreground">
              <Menu size={22} className="text-foreground" />
            </ButtonText>
          </Button>
          <View className="size-8 items-center justify-center rounded-lg bg-primary/15">
            <Lock size={16} className="text-primary" />
          </View>
          <Text className="text-xl font-semibold text-foreground">Vautr</Text>
        </View>
        <View className="flex-row items-center gap-2">
          <Button variant="ghost" size="sm" onPress={() => void unlock()}>
            <Fingerprint size={18} className="text-primary" />
          </Button>
          <Button variant="ghost" size="sm" onPress={() => void logout()}>
            <LogOut size={18} className="text-foreground" />
            <ButtonText className="ml-1">Logout</ButtonText>
          </Button>
        </View>
      </View>

      <Drawer open={drawerOpen} onOpenChange={setDrawerOpen}>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>Vautr</DrawerTitle>
          </DrawerHeader>
          <View className="flex-col gap-1">
            {DRAWER_ITEMS.map(({ to, label, icon: Icon }) => (
              <Button
                key={to}
                variant="ghost"
                className="flex-row justify-start gap-3 py-2"
                onPress={go(to)}
              >
                <ThemedIcon icon={Icon} size={18} />
                <ButtonText className="text-sm">{label}</ButtonText>
              </Button>
            ))}
          </View>
        </DrawerContent>
      </Drawer>

      {username ? (
        <Text className="px-4 pt-1 text-xs text-muted-foreground">{username}</Text>
      ) : null}
      {locked ? (
        <Text accessibilityRole="alert" className="px-4 pt-1 text-xs text-destructive">
          Vault locked. Use biometrics to unlock.
        </Text>
      ) : null}

      {/* NOTE: a bare `ScrollView` here crashes on RN 0.86 (new arch) — the
          reanimated/worklets babel plugin patches ScrollView and throws
          `ReferenceError: Property 'scrollTo' doesn't exist` at runtime. Use a
          plain flex View for the scrollable content region instead. */}
      <View className="flex-1 p-4 gap-4">
        <Outlet />
      </View>

      {/* Bottom tab bar — three primary sections. Full navigation lives in the
          left drawer (hamburger, top-left). */}
      <View className="flex-row items-center border-t border-border">
        {PRIMARY.map(({ to, label, icon: Icon }) => (
          <Button
            key={to}
            variant="ghost"
            size="sm"
            className="flex-1 flex-col gap-0.5 py-2"
            onPress={() => router.navigate({ to: to as '/' })}
          >
            <ThemedIcon icon={Icon} size={18} />
            <ButtonText className="text-[11px]">{label}</ButtonText>
          </Button>
        ))}
        <Button
          variant="ghost"
          size="sm"
          className="flex-1 flex-col gap-0.5 py-2"
          onPress={() => setDrawerOpen(true)}
        >
          <ThemedIcon icon={List} size={18} />
          <ButtonText className="text-[11px]">More</ButtonText>
        </Button>
      </View>
    </SafeAreaView>
  );
}
