import { router } from 'expo-router';
import { memo, useCallback, useState } from 'react';
import { Alert, FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { connect } from '../../../connect/client.ts';
import { fingerprint } from '../../../connect/crypto.ts';
import { useDevices, useStatus, type DeviceRow } from '../../../store/index.ts';
import { useBackdrop } from '../../../ui/Backdrop.tsx';

const Row = memo(function Row({ device, demo }: { device: DeviceRow; demo: boolean }) {
  const colors = useBackdrop();
  const [saving, setSaving] = useState(false);
  const open = device.host && device.trusted;
  const keyFingerprint = fingerprint(device.publicKey).match(/.{1,4}/g)?.join(' ') ?? '';
  const confirmTrust = () => {
    const allow = !device.trusted;
    // Capture the key shown in this confirmation. The client compares it with current
    // presence again before saving; a changed key must receive a new confirmation.
    const expectedKey = device.publicKey;
    Alert.alert(allow ? `Trust ${device.name}?` : `Stop trusting ${device.name}?`, allow
      ? `On that computer, open Settings → Remote access and compare its own fingerprint with:\n\n${keyFingerprint}\n\nConfirm only when all 32 hexadecimal characters match. Also trust this phone on the computer before controlling tasks.`
      : 'This phone will stop sending requests to this computer and accepting its replies or events. Pending requests will be cancelled.', [
      { text: 'Cancel', style: 'cancel' },
      { text: allow ? 'Fingerprints match' : 'Remove trust', style: allow ? 'default' : 'destructive', onPress: () => {
        setSaving(true);
        void connect.setTrusted(device.id, expectedKey, allow)
          .catch(error => Alert.alert('Could not update trust', (error as Error).message))
          .finally(() => setSaving(false));
      } },
    ]);
  };
  return (
    <View style={[styles.row, { backgroundColor: colors.card }]}>
      <Pressable disabled={!open} onPress={() => router.navigate({ pathname: '/inbox', params: { computer: device.id, project: '' } })}
        style={({ pressed }) => [styles.summary, { opacity: pressed ? 0.7 : 1 }]} accessibilityRole={open ? 'button' : undefined}>
        <View style={[styles.dot, { backgroundColor: device.online ? colors.green : colors.tertiary }]} accessibilityLabel={device.online ? 'Online' : 'Offline'} />
        <View style={styles.main}>
          <Text style={[styles.name, { color: colors.text }]} numberOfLines={1}>{device.name}</Text>
          <Text style={[styles.detail, { color: colors.secondary }]}>{device.platform}{device.online ? ' · online' : ' · offline'} · {device.trusted ? 'trusted' : device.keyChanged ? 'key changed' : 'needs verification'}</Text>
          {!device.host && <Text style={[styles.detail, { color: colors.orange }]}>Remote control is off on this computer. Turn on Allow remote control in Settings → Remote access.</Text>}
        </View>
      </Pressable>
      {!demo && <>
        <Text selectable accessibilityLabel={`Computer fingerprint ${keyFingerprint}`} style={[styles.detail, { color: colors.secondary }]}>{keyFingerprint}</Text>
        <Pressable disabled={saving} onPress={confirmTrust} accessibilityRole="button" accessibilityState={{ disabled: saving }} style={styles.trust}>
          <Text style={{ color: device.trusted ? colors.red : colors.tint }}>{saving ? 'Saving…' : device.trusted ? 'Remove trust' : 'Compare fingerprint and trust…'}</Text>
        </Pressable>
      </>}
    </View>
  );
});

export default function Computers() {
  const colors = useBackdrop();
  const all = useDevices();
  const { connected, demo, error } = useStatus();
  const insets = useSafeAreaInsets();
  const devices = all.filter(d => !d.current);
  const refresh = useCallback(() => { connect.refreshDevices().catch(() => undefined); }, []);
  return (
    <FlatList
      style={{ backgroundColor: colors.grouped }}
      contentInsetAdjustmentBehavior="automatic"
      contentContainerStyle={[styles.list, { paddingLeft: 16 + insets.left, paddingRight: 16 + insets.right }]}
      data={devices}
      keyExtractor={d => d.id}
      renderItem={({ item }) => <Row device={item} demo={demo} />}
      onRefresh={refresh}
      refreshing={false}
      ItemSeparatorComponent={() => <View style={styles.gap} />}
      ListHeaderComponent={!demo ? <Text style={[styles.instructions, { color: error ? colors.red : colors.secondary }]}>{error ?? 'Verify each computer before connecting. Compare fingerprints on both devices; sharing a GitHub account does not grant remote access. This phone’s fingerprint is in Settings.'}</Text> : undefined}
      ListEmptyComponent={<Text style={[styles.empty, { color: colors.secondary }]}>{connected ? 'No other devices on this account yet. Sign in to PandaCode Connect on your computer.' : 'Connecting…'}</Text>}
    />
  );
}

const styles = StyleSheet.create({
  list: { padding: 16 },
  row: { gap: 12, borderRadius: 16, padding: 14 },
  summary: { flexDirection: 'row', alignItems: 'center', gap: 12 },
  dot: { width: 10, height: 10, borderRadius: 5 },
  main: { flex: 1, gap: 2 },
  name: { fontSize: 17, fontWeight: '600' },
  detail: { fontSize: 13 },
  gap: { height: 10 },
  instructions: { fontSize: 13, lineHeight: 19, marginBottom: 16 },
  trust: { paddingVertical: 8 },
  empty: { fontSize: 15, textAlign: 'center', marginTop: 40 },
});
