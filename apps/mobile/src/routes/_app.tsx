import { createFileRoute, Outlet, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { ScrollView, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import {
  Fingerprint,
  KeyRound,
  Lock,
  LogOut,
  Settings2,
  ShieldCheck,
  Wrench,
} from 'lucide-react-native';

import { requireBiometric } from '../../lib/biometrics';
import { services } from '../../lib/client';
import { useSession } from '../../lib/session';
import { Button, ButtonText } from '../../components/ui/button';
import { useToast } from '../../components/ui/toast';

export const Route = createFileRoute('/_app')({
  component: AppShell,
});

const NAV = [
  { to: '/', label: 'Projects', icon: Lock },
  { to: '/secrets', label: 'Secrets', icon: KeyRound },
  { to: '/generator', label: 'Generator', icon: Wrench },
  { to: '/mfa', label: 'MFA', icon: ShieldCheck },
  { to: '/settings', label: 'Settings', icon: Settings2 },
] as const;

function AppShell() {
  const username = useSession((s) => s.username);
  const [locked, setLocked] = useState(false);
  const router = useRouter();
  const toast = useToast();

  const go = (to: (typeof NAV)[number]['to']) => () => router.navigate({ to });

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
      <View className="border-b border-border px-4 py-3">
        <View className="flex-row items-center justify-between">
          <Text className="text-xl font-semibold text-foreground">Vautr</Text>
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
        {username ? <Text className="mt-0.5 text-xs text-muted-foreground">{username}</Text> : null}
        {locked ? (
          <Text accessibilityRole="alert" className="mt-1 text-xs text-destructive">
            Vault locked. Use biometrics to unlock.
          </Text>
        ) : null}
      </View>

      <View className="flex-row items-center gap-1 border-b border-border px-4 py-2">
        {NAV.map(({ to, label, icon: Icon }) => (
          <Button key={to} variant="ghost" size="sm" className="flex-1" onPress={go(to)}>
            <Icon size={15} className="text-foreground" />
            <ButtonText className="ml-0.5 text-[11px]">{label}</ButtonText>
          </Button>
        ))}
      </View>

      <ScrollView className="flex-1" contentContainerClassName="p-4 gap-4">
        <Outlet />
      </ScrollView>
    </SafeAreaView>
  );
}
