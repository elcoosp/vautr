import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';
import { toast } from 'sonner';
import { mlp, MlpApiError } from '@/lib/mlp';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import type { OffboardResponse } from '@vautr/api-contract';
import { ShieldAlert } from 'lucide-react';

export const Route = createFileRoute('/_authed/settings')({
  component: SettingsPage,
});

function SettingsPage() {
  const [userUuid, setUserUuid] = useState('');
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<OffboardResponse | null>(null);

  const onOffboard = async () => {
    if (!userUuid.trim()) {
      toast.error('Enter the user uuid to offboard');
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      const res = await mlp.offboard({ user_uuid: userUuid.trim(), reason: reason.trim() || undefined });
      setResult(res);
      toast.success('User offboarded');
      setUserUuid('');
      setReason('');
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold text-text">Settings</h1>
        <p className="text-sm text-text-muted">Organization and security administration.</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-danger">
            <ShieldAlert className="size-4" aria-hidden="true" />
            Offboarding (admin)
          </CardTitle>
          <CardDescription>
            Immediately revoke a user's project memberships, secrets access, and tokens.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="offboard-uuid">User UUID</Label>
            <Input id="offboard-uuid" value={userUuid} onChange={(e) => setUserUuid(e.target.value)} placeholder="xxxxxxxx-xxxx-…" />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="offboard-reason">Reason (optional)</Label>
            <Textarea id="offboard-reason" value={reason} onChange={(e) => setReason(e.target.value)} placeholder="e.g. offboarding, role change" rows={2} />
          </div>

          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="destructive" disabled={busy || !userUuid.trim()}>
                Offboard user
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Offboard this user?</AlertDialogTitle>
                <AlertDialogDescription>
                  This immediately revokes the user's memberships, secrets access, and tokens. This action is not reversible.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction onClick={() => void onOffboard()}>Confirm offboard</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>

          {result ? (
            <div className="rounded-md border border-accent/40 bg-accent/10 px-3 py-2 text-sm text-text">
              Offboarded {result.user_uuid}: revoked {result.revoked_projects} projects, {result.revoked_memberships} memberships, {result.revoked_tokens} tokens.
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}
