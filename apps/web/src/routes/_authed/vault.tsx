import { createFileRoute } from '@tanstack/react-router';
import { VaultView } from '@/components/VaultView';

export const Route = createFileRoute('/_authed/vault')({
  component: VaultPage,
});

function VaultPage() {
  return <VaultView />;
}
