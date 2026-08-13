import { createFileRoute } from '@tanstack/react-router';
import { Text, View } from 'react-native';
import { Card } from '../../components/ui/card';
import { useSession } from '../../lib/session';

export const Route = createFileRoute('/_app/settings')({
  component: SettingsScreen,
});

function SettingsScreen() {
  const username = useSession((s) => s.username);

  return (
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Settings</Text>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">App</Text>
        {username ? (
          <Text className="text-xs text-muted-foreground">Signed in as {username}</Text>
        ) : null}
        <Text className="text-sm text-muted-foreground">
          Vautr mobile unlocks with biometrics and keeps your vault encrypted on device. Use the
          desktop or web client for machine accounts, access tokens, and backup restore.
        </Text>
      </Card>
    </View>
  );
}
