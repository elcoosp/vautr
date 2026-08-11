import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { Text, View } from 'react-native';
import { useHaptics } from '../../lib/haptics';

import { services } from '../../lib/client';
import { useSession } from '../../lib/session';
import { Button, ButtonText } from '../../components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';

export const Route = createFileRoute('/_auth/login')({
  component: LoginScreen,
});

function LoginScreen() {
  const router = useRouter();
  const haptics = useHaptics();
  const setAuthenticated = useSession((s) => s.setAuthenticated);
  const setUsername = useSession((s) => s.setUsername);

  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsernameInput] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!username || !password) {
      setError('Enter a username and password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (mode === 'register') {
        await services.auth.register(username.trim(), password);
      } else {
        await services.auth.login(username.trim(), password);
      }
      setUsername(username.trim());
      setAuthenticated(true);
      void haptics.notifySuccess();
      router.navigate({ to: '/' });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Authentication failed.');
      void haptics.notifyError();
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle className="text-center">Vautr</CardTitle>
        <CardDescription className="text-center">
          {mode === 'login' ? 'Unlock your zero-knowledge vault.' : 'Create a new vault account.'}
        </CardDescription>
      </CardHeader>
      <CardContent className="gap-4">
        <Tabs value={mode} onValueChange={(value) => setMode(value as 'login' | 'register')}>
          <TabsList className="flex-row">
            <TabsTrigger value="login" className="flex-1">
              Login
            </TabsTrigger>
            <TabsTrigger value="register" className="flex-1">
              Register
            </TabsTrigger>
          </TabsList>
          <TabsContent value={mode}>
            <View className="gap-4">
              <View className="gap-1.5">
                <Label htmlFor="username">Username</Label>
                <Input
                  id="username"
                  value={username}
                  onChangeText={setUsernameInput}
                  autoCapitalize="none"
                  autoCorrect={false}
                  placeholder="you@example.com"
                />
              </View>
              <View className="gap-1.5">
                <Label htmlFor="password">Master password</Label>
                <Input
                  id="password"
                  value={password}
                  onChangeText={setPassword}
                  secureTextEntry
                  placeholder="••••••••"
                />
              </View>

              {error ? (
                <Text accessibilityRole="alert" className="text-sm text-destructive">
                  {error}
                </Text>
              ) : null}

              <Button disabled={busy} onPress={() => void submit()}>
                <ButtonText>{busy ? 'Working…' : mode === 'login' ? 'Unlock' : 'Create account'}</ButtonText>
              </Button>
            </View>
          </TabsContent>
        </Tabs>
      </CardContent>
    </Card>
  );
}
