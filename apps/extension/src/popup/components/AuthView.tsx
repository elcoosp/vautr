import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { usePopupStore } from '../store';

interface AuthViewProps {
  onAuthenticated: (username: string) => void;
}

export function AuthView({ onAuthenticated }: AuthViewProps) {
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const setError = usePopupStore((s) => s.setError);
  const setStatus = usePopupStore((s) => s.setStatus);
  const status = usePopupStore((s) => s.status);
  const error = usePopupStore((s) => s.error);

  const busy = status === 'unlocking' || status === 'busy';

  async function submit(): Promise<void> {
    if (!username || !password) {
      setError('Enter a username and master password.');
      return;
    }
    setStatus('unlocking');
    setError(null);
    try {
      const { getPopupClient } = await import('../popupClient');
      const client = await getPopupClient();
      if (mode === 'register') {
        const { recoveryMnemonic } = await client.register(username, password);
        // Hand the Recovery Key to the onboarding flow (shown once). sessionStorage
        // is cleared on popup close — never persisted in plaintext; the durable
        // copy is KEK-sealed in IndexedDB.
        sessionStorage.setItem('vautr:pending-kit', recoveryMnemonic);
      }
      await client.login(username, password);
      await client.sync();
      const { cacheAllCiphertexts, cacheSvkForSw } = await import('../vaultActions');
      await cacheAllCiphertexts();
      const state = await (await import('@vautr/client-sdk/storage')).IndexedDbStore;
      const store = new state();
      const storedState = await store.getState();
      if (storedState.svk) {
        await cacheSvkForSw(storedState.svk);
      }
      onAuthenticated(username);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus('locked');
    }
  }

  return (
    <div className="flex flex-col gap-4 p-4">
      <Card>
        <CardHeader className="space-y-1">
          <CardTitle className="text-xl">Vautr</CardTitle>
          <CardDescription>
            {mode === 'login' ? 'Unlock your vault' : 'Create a new account'}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Tabs
            value={mode}
            onValueChange={(v) => setMode(v as 'login' | 'register')}
            className="w-full"
          >
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="login">Login</TabsTrigger>
              <TabsTrigger value="register">Register</TabsTrigger>
            </TabsList>
            <TabsContent value="login" />
            <TabsContent value="register" />
          </Tabs>

          <div className="space-y-2">
            <Label htmlFor="auth-username">Username</Label>
            <Input
              id="auth-username"
              autoComplete="username"
              value={username}
              disabled={busy}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="auth-password">Master password</Label>
            <Input
              id="auth-password"
              type="password"
              autoComplete="current-password"
              value={password}
              disabled={busy}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void submit();
              }}
            />
          </div>
          {error ? <p className="text-sm text-destructive">{error}</p> : null}
          <Button className="w-full" disabled={busy} onClick={() => void submit()}>
            {busy ? 'Working…' : mode === 'login' ? 'Unlock' : 'Register'}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
