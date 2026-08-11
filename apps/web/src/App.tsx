import { useIsLocked } from '@vautr/ui-logic';
import { useEffect } from 'react';
import { UnlockScreen } from './components/UnlockScreen';
import { VaultView } from './components/VaultView';
import { disposeClient } from './lib/client';

export function App() {
  const isLocked = useIsLocked();

  // Terminate the worker when the app closes (client.md §4 acceptance).
  useEffect(() => {
    const onBeforeUnload = () => disposeClient();
    window.addEventListener('beforeunload', onBeforeUnload);
    return () => {
      window.removeEventListener('beforeunload', onBeforeUnload);
      disposeClient();
    };
  }, []);

  if (isLocked) {
    return <UnlockScreen />;
  }

  return <VaultView />;
}
