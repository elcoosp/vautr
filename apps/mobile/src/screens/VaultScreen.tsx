import { useOverviews, useVaultActions } from '@vautr/ui-logic';
import { useState } from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';

import { getClient } from '../lib/client';
import { DetailScreen } from './DetailScreen';

/** Rendered list item (virtualized via FlatList). */
function Row({ item, onPress }: { item: { uuid: string; title: string; subtitle: string }; onPress: () => void }) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`Open ${item.title}`}
      onPress={onPress}
      style={({ pressed }) => [styles.row, pressed && styles.pressed]}
    >
      <View style={styles.icon}>
        <Text style={styles.iconText}>◆</Text>
      </View>
      <View style={styles.rowBody}>
        <Text style={styles.rowTitle} numberOfLines={1}>
          {item.title}
        </Text>
        <Text style={styles.rowSubtitle} numberOfLines={1}>
          {item.subtitle}
        </Text>
      </View>
    </Pressable>
  );
}

export function VaultScreen() {
  const overviews = useOverviews();
  const { lock } = useVaultActions();
  const [selectedUuid, setSelectedUuid] = useState<string | null>(null);

  const sorted = [...overviews].sort((a, b) => b.updatedAt - a.updatedAt);

  const onLock = async () => {
    const client = await getClient();
    await client.lock();
    lock();
  };

  if (selectedUuid) {
    return <DetailScreen uuid={selectedUuid} onBack={() => setSelectedUuid(null)} />;
  }

  return (
    <View style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Vault</Text>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Lock vault"
          onPress={() => void onLock()}
          style={({ pressed }) => [styles.lockButton, pressed && styles.pressed]}
        >
          <Text style={styles.lockButtonText}>Lock</Text>
        </Pressable>
      </View>

      <FlatList
        data={sorted}
        keyExtractor={(item) => item.uuid}
        renderItem={({ item }) => (
          <Row item={item} onPress={() => setSelectedUuid(item.uuid)} />
        )}
        ListEmptyComponent={<Text style={styles.empty}>No items yet.</Text>}
        contentContainerStyle={styles.listContent}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#0d0f14',
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    borderBottomWidth: 1,
    borderBottomColor: '#232936',
    paddingVertical: 14,
    paddingHorizontal: 18,
  },
  title: {
    fontSize: 20,
    fontWeight: '700',
    color: '#f5f7fa',
  },
  lockButton: {
    borderWidth: 1,
    borderColor: '#2a3140',
    borderRadius: 10,
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
  lockButtonText: {
    color: '#c6cdd8',
    fontSize: 14,
    fontWeight: '500',
  },
  listContent: {
    paddingVertical: 4,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 18,
    paddingVertical: 12,
    borderBottomWidth: 1,
    borderBottomColor: '#171c26',
  },
  pressed: {
    backgroundColor: '#151a24',
  },
  icon: {
    width: 36,
    height: 36,
    borderRadius: 10,
    backgroundColor: '#1e2740',
    alignItems: 'center',
    justifyContent: 'center',
    marginRight: 12,
  },
  iconText: {
    color: '#2f6fed',
    fontSize: 16,
  },
  rowBody: {
    flex: 1,
  },
  rowTitle: {
    fontSize: 16,
    fontWeight: '500',
    color: '#f5f7fa',
  },
  rowSubtitle: {
    fontSize: 13,
    color: '#9aa3b2',
    marginTop: 2,
  },
  empty: {
    color: '#9aa3b2',
    fontSize: 15,
    textAlign: 'center',
    marginTop: 48,
  },
});
