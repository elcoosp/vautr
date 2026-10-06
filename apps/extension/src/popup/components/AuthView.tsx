import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { usePopupStore } from '../store';
import { MfaRequiredError } from '@vautr/client-sdk';

interface AuthViewProps {
  onAuthenticated: (username: string) => void;
}

export function AuthView({ onAuthenticated }: AuthViewProps) {
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  // VTRFIX-SEC-C03: server withholds the session until TOTP is verified.
  const [pendingToken, setPendingToken] = useState<string | null>(null);
  const [totpCode, setTotpCode] = useState('');
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
      try {
        await client.login(username, password);
      } catch (err) {
        if (err instanceof MfaRequiredError) {
          setPendingToken(err.pendingToken);
          setStatus('locked');
          return;
        }
        throw err;
      }
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

  // VTRFIX-SEC-C03: complete the login by submitting the TOTP code.
  async function verifyTotp(): Promise<void> {
    if (!pendingToken || !totpCode.trim()) {
      setError('Enter the 6-digit code from your authenticator app.');
      return;
    }
    setStatus('unlocking');
    setError(null);
    try {
      const { getPopupClient } = await import('../popupClient');
      const client = await getPopupClient();
      await client.completeLoginWithTotp(pendingToken, totpCode.trim(), username, password);
      await client.sync();
      const { cacheAllCiphertexts, cacheSvkForSw } = await import('../vaultActions');
      await cacheAllCiphertexts();
      const { IndexedDbStore } = await import('@vautr/client-sdk/storage');
      const store = new IndexedDbStore();
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

  function cancelTotp(): void {
    setPendingToken(null);
    setTotpCode('');
    setError(null);
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

          {pendingToken ? (
            <>
              <p className="text-sm text-muted-foreground">
                Enter the 6-digit code from your authenticator app.
              </p>
              <div className="space-y-2">
                <Label htmlFor="auth-totp">One-time code</Label>
                <Input
                  id="auth-totp"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={totpCode}
                  disabled={busy}
                  onChange={(e) => setTotpCode(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') void verifyTotp();
                  }}
                />
              </div>
              {error ? <p className="text-sm text-destructive">{error}</p> : null}
              <Button className="w-full" disabled={busy} onClick={() => void verifyTotp()}>
                {busy ? 'Verifying…' : 'Verify and unlock'}
              </Button>
              <Button className="w-full" variant="ghost" disabled={busy} onClick={cancelTotp}>
                Use a different account
              </Button>
            </>
          ) : (
          <>
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
            {busy
              ? mode === 'login'
                ? 'Unlocking…'
                : 'Creating account…'
              : mode === 'login'
                ? 'Unlock'
                : 'Register'}
          </Button>
          </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
