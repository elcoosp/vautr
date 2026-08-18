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
import { ScrollView, Text, View } from 'react-native';
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

  // Bottom tab bar: the primary destinations. Everything else (account,
  // security, tokens, import/export, shares) lives in the left drawer so the
  // two navigation surfaces stay disjoint and the drawer stays useful.
  const PRIMARY = [
    { to: '/secrets', label: 'Secrets', icon: HardDrive },
    { to: '/generator', label: 'Generator', icon: Settings2 },
    { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { to: '/settings', label: 'Settings', icon: Settings },
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
            <ThemedIcon icon={Menu} size={22} />
          </Button>
          <View className="size-8 items-center justify-center rounded-lg bg-primary/15">
            <ThemedIcon icon={Lock} size={16} tone="primary" />
          </View>
          <Text className="text-xl font-semibold text-foreground">Vautr</Text>
        </View>
        <View className="flex-row items-center gap-2">
          <Button variant="ghost" size="sm" onPress={() => void unlock()}>
            <ThemedIcon icon={Fingerprint} size={18} tone="primary" />
          </Button>
          <Button variant="ghost" size="sm" onPress={() => void logout()}>
            <ThemedIcon icon={LogOut} size={18} />
            <ButtonText className="ml-1">Logout</ButtonText>
          </Button>
        </View>
      </View>

      <Drawer open={drawerOpen} onOpenChange={setDrawerOpen}>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>Vautr</DrawerTitle>
          </DrawerHeader>
          <View className="flex-1 flex-col gap-1">
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
          <Button
            variant="ghost"
            className="flex-row justify-start gap-3 border-t border-border py-3"
            onPress={() => {
              setDrawerOpen(false);
              void logout();
            }}
          >
            <ThemedIcon icon={LogOut} size={18} />
            <ButtonText className="text-sm">Logout</ButtonText>
          </Button>
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

      {/* Scrollable content region. Core ScrollView works on RN 0.86 + new
          arch; a bare ScrollView no longer trips the reanimated/worklets
          scrollTo patch that the old comment warned about. */}
      <ScrollView className="flex-1 p-4 gap-4" contentContainerClassName="gap-4">
        <Outlet />
      </ScrollView>

      {/* Bottom tab bar — primary destinations. Secondary nav (account,
          security, tokens, import/export) lives in the left drawer. */}
      <View className="flex-row items-center border-t border-border pb-3 min-h-[72px]">
        {PRIMARY.map(({ to, label, icon: Icon }) => (
          <Button
            key={to}
            variant="ghost"
            className="flex-1 flex-col gap-1 py-2.5 min-w-0"
            onPress={() => router.navigate({ to: to as '/' })}
          >
            <ThemedIcon icon={Icon} size={24} />
            <ButtonText
              numberOfLines={1}
              ellipsizeMode="tail"
              className="text-[11px] text-center min-w-0"
            >
              {label}
            </ButtonText>
          </Button>
        ))}
        <Button
          variant="ghost"
          className="flex-1 flex-col gap-1 py-2.5 min-w-0"
          onPress={() => setDrawerOpen(true)}
        >
          <ThemedIcon icon={List} size={24} />
          <ButtonText
            numberOfLines={1}
            ellipsizeMode="tail"
            className="text-[11px] text-center min-w-0"
          >
            More
          </ButtonText>
        </Button>
      </View>
    </SafeAreaView>
  );
}
