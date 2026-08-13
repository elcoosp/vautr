import { createFileRoute } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { useToast } from '../../components/ui/toast';
import type { MfaMethod } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/mfa')({
  component: MfaScreen,
});

function MfaScreen() {
  const toast = useToast();
  const [status, setStatus] = useState<{
    required: boolean;
    configured_methods: MfaMethod[];
  } | null>(null);
  const [enrolled, setEnrolled] = useState<{
    enrollment_id: string;
    otpauth_url: string;
    secret: string;
  } | null>(null);
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setStatus(await services.api.mfaStatus());
    } catch {
      // ignore transient status fetch failures
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const enroll = async () => {
    setBusy(true);
    try {
      const issue = await services.api.mfaTotpIssue();
      setEnrolled({
        enrollment_id: issue.enrollment_id,
        otpauth_url: issue.otpauth_url,
        secret: issue.secret,
      });
    } catch (err) {
      toast.show({
        title: 'Enroll failed',
        description: err instanceof Error ? err.message : 'Could not start TOTP enrollment.',
        variant: 'destructive',
      });
    } finally {
      setBusy(false);
    }
  };

  const verify = async () => {
    if (!enrolled || code.trim().length < 6) {
      toast.show({ title: 'Enter a valid 6-digit code', variant: 'destructive' });
      return;
    }
    setBusy(true);
    try {
      const result = await services.api.mfaTotpVerify({
        enrollment_id: enrolled.enrollment_id,
        code: code.trim(),
      });
      toast.show({ title: 'TOTP verified', description: `Status: ${result.status}` });
      setEnrolled(null);
      setCode('');
      await load();
    } catch (err) {
      toast.show({
        title: 'Verification failed',
        description: err instanceof Error ? err.message : 'Invalid code.',
        variant: 'destructive',
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Multi-factor authentication</Text>

      <Card className="p-4 gap-3">
        <View className="flex-row items-center justify-between">
          <Text className="text-sm text-muted-foreground">Status</Text>
          <Badge variant={status?.configured_methods.length ? 'default' : 'secondary'}>
            {status?.configured_methods.length ? 'Configured' : 'Not configured'}
          </Badge>
        </View>
        {status?.required ? (
          <Text className="text-xs text-destructive">MFA is required by your organization.</Text>
        ) : null}
        {status?.configured_methods.length ? (
          <Text className="text-sm text-muted-foreground">
            Methods: {status.configured_methods.join(', ')}
          </Text>
        ) : null}
      </Card>

      {enrolled ? (
        <Card className="p-4 gap-3">
          <Text className="text-sm font-medium text-foreground">
            Scan with your authenticator app
          </Text>
          <Text className="text-xs text-muted-foreground" selectable>
            otpauth: {enrolled.otpauth_url}
          </Text>
          <View className="gap-1.5">
            <Label>Manual secret</Label>
            <Text className="text-sm text-foreground" selectable>
              {enrolled.secret}
            </Text>
          </View>
          <View className="gap-1.5">
            <Label htmlFor="mfa-code">Verification code</Label>
            <Input
              id="mfa-code"
              value={code}
              onChangeText={setCode}
              placeholder="000000"
              keyboardType="number-pad"
              maxLength={6}
            />
          </View>
          <Button disabled={busy} onPress={() => void verify()}>
            <ButtonText>{busy ? 'Verifying…' : 'Verify & enable'}</ButtonText>
          </Button>
        </Card>
      ) : (
        <Button disabled={busy} onPress={() => void enroll()}>
          <ButtonText>{busy ? 'Enrolling…' : 'Set up TOTP authenticator'}</ButtonText>
        </Button>
      )}
    </View>
  );
}
