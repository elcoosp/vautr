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
  Replace,
  Settings,
  Settings2,
  Share2,
} from 'lucide-react-native';
import { useState } from 'react';
import { ScrollView, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Button, ButtonText } from '../../components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from '../../components/ui/sheet';
import { useToast } from '../../components/ui/toast';
import { requireBiometric } from '../../lib/biometrics';
import { isLocalVaultActive, services } from '../../lib/client';
import { useSession } from '../../lib/session';

export const Route = createFileRoute('/_app')({
  component: AppShell,
});

function AppShell() {
  const username = useSession((s) => s.username);
  const [locked, setLocked] = useState(false);
  const router = useRouter();
  const toast = useToast();

  // Native-gated sharing tab: only when the uniffi core is linked (VTR-070).
  const PRIMARY = [
    { to: '/secrets', label: 'Secrets', icon: HardDrive },
    { to: '/generator', label: 'Generator', icon: Settings2 },
    { to: '/settings', label: 'Settings', icon: Settings },
    ...(isLocalVaultActive() ? [{ to: '/shares', label: 'Shares', icon: Share2 } as const] : []),
  ];

  const MORE = [
    { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { to: '/', label: 'Projects', icon: Folder },
    { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
    { to: '/mfa', label: 'MFA & security', icon: CircleCheck },
    { to: '/tokens', label: 'Tokens', icon: Globe },
    { to: '/import-export', label: 'Import / export', icon: Replace },
  ];

  const go = (to: string) => () => router.navigate({ to: to as '/' });

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
      {/* Brand header — canonical: size-8 accent tile + wordmark + username. */}
      <View className="flex-row items-center justify-between border-b border-border px-4 py-3">
        <View className="flex-row items-center gap-2">
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

      {username ? (
        <Text className="px-4 pt-1 text-xs text-muted-foreground">{username}</Text>
      ) : null}
      {locked ? (
        <Text accessibilityRole="alert" className="px-4 pt-1 text-xs text-destructive">
          Vault locked. Use biometrics to unlock.
        </Text>
      ) : null}

      <ScrollView className="flex-1" contentContainerClassName="p-4 gap-4">
        <Outlet />
      </ScrollView>

      {/* Bottom tab bar — three primary sections. */}
      <View className="flex-row items-center border-t border-border">
        {PRIMARY.map(({ to, label, icon: Icon }) => (
          <Button
            key={to}
            variant="ghost"
            size="sm"
            className="flex-1 flex-col gap-0.5 py-2"
            onPress={go(to)}
          >
            <Icon size={18} className="text-foreground" />
            <ButtonText className="text-[11px]">{label}</ButtonText>
          </Button>
        ))}
        <Sheet>
          <SheetTrigger asChild>
            <Button variant="ghost" size="sm" className="flex-1 flex-col gap-0.5 py-2">
              <List size={18} className="text-foreground" />
              <ButtonText className="text-[11px]">More</ButtonText>
            </Button>
          </SheetTrigger>
          <SheetContent>
            <SheetHeader>
              <SheetTitle>More</SheetTitle>
            </SheetHeader>
            <View className="mt-2 flex-col gap-1">
              {MORE.map(({ to, label, icon: Icon }) => (
                <Button
                  key={to}
                  variant="ghost"
                  size="sm"
                  className="flex-row justify-start gap-3 py-2"
                  onPress={go(to)}
                >
                  <Icon size={18} className="text-foreground" />
                  <ButtonText className="text-sm">{label}</ButtonText>
                </Button>
              ))}
            </View>
          </SheetContent>
        </Sheet>
      </View>
    </SafeAreaView>
  );
}
