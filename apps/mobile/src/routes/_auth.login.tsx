import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Button, ButtonText } from '../../components/ui/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/ui/tabs';
import { Text } from '../../components/ui/text';
import { services } from '../../lib/client';
import { useHaptics } from '../../lib/haptics';
import { useSession } from '../../lib/session';

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
  const [confirmPassword, setConfirmPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!username || !password) {
      setError('Enter a username and password.');
      return;
    }
    if (mode === 'register') {
      if (password !== confirmPassword) {
        setError('Passwords do not match.');
        return;
      }
      if (password.length < 8) {
        setError('Master password must be at least 8 characters.');
        return;
      }
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
      // Native/auth failures can surface as `Error` with an EMPTY `.message`
      // (e.g. a network rejection or a blank-string exception from the bridge).
      // Never render a blank error card — fall back to an actionable message.
      const raw = err instanceof Error ? err.message : String(err ?? '');
      const msg = raw.trim()
        ? raw.trim()
        : mode === 'register'
          ? 'Registration failed. Check that the Vautr server is running and reachable (adb reverse tcp:8080 tcp:8080).'
          : 'Login failed. Check that the Vautr server is running and reachable (adb reverse tcp:8080 tcp:8080).';
      setError(msg);
      void haptics.notifyError();
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="flex-1 justify-center px-6">
      <Card className="w-full">
        <CardHeader className="items-center gap-1.5 pb-2">
          <View className="mb-1 h-12 w-12 items-center justify-center rounded-2xl bg-primary">
            <Text variant="h3" className="text-primary-foreground">
              V
            </Text>
          </View>
          <CardTitle className="text-center">Vautr</CardTitle>
          <CardDescription className="text-center">
            {mode === 'login' ? 'Unlock your zero-knowledge vault.' : 'Create a new vault account.'}
          </CardDescription>
        </CardHeader>
        <CardContent className="gap-5">
          <Tabs value={mode} onValueChange={(value) => setMode(value as 'login' | 'register')}>
            <TabsList className="flex-row">
              <TabsTrigger value="login" active={mode === 'login'} className="flex-1">
                <Text variant="label">Login</Text>
              </TabsTrigger>
              <TabsTrigger value="register" active={mode === 'register'} className="flex-1">
                <Text variant="label">Register</Text>
              </TabsTrigger>
            </TabsList>
            <TabsContent value={mode}>
              <View className="gap-4 pt-2">
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

                {mode === 'register' ? (
                  <View className="gap-1.5">
                    <Label htmlFor="confirmPassword">Confirm master password</Label>
                    <Input
                      id="confirmPassword"
                      value={confirmPassword}
                      onChangeText={setConfirmPassword}
                      secureTextEntry
                      placeholder="••••••••"
                    />
                  </View>
                ) : null}

                {error ? (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                ) : null}

                <Button disabled={busy} onPress={() => void submit()}>
                  <ButtonText>
                    {busy ? 'Working…' : mode === 'login' ? 'Unlock' : 'Create account'}
                  </ButtonText>
                </Button>
              </View>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </View>
  );
}
