import { createRootRoute, Outlet } from '@tanstack/react-router';
import { ConflictModal } from '@/components/ConflictModal';
import { Toaster } from '@/components/ui/sonner';
import { TooltipProvider } from '@/components/ui/tooltip';

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  return (
    <TooltipProvider>
      <Outlet />
      <ConflictModal />
      <Toaster richColors position="bottom-right" />
    </TooltipProvider>
  );
}
