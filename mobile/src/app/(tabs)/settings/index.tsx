import Constants from 'expo-constants';
import { Image } from 'expo-image';
import * as WebBrowser from 'expo-web-browser';
import { useState, type ReactNode } from 'react';
import { ActivityIndicator, Alert, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { connect } from '../../../connect/client.ts';
import { useStatus } from '../../../store/index.ts';
import { clearLocal } from '../../../store/local.ts';
import { APPEARANCES, setAppearance, useAppearance } from '../../../ui/appearance.ts';
import { useBackdrop } from '../../../ui/Backdrop.tsx';
import { WallpaperSettings } from '../../../ui/WallpaperSettings.tsx';

const SOURCE = 'https://github.com/ginmp8/pandacode-extended';
const SUPPORT = `${SOURCE}/issues`;
const PRIVACY = `${SOURCE}/blob/main/mobile/README.md#privacy-and-local-data`;

export default function Settings() {
  const colors = useBackdrop();
  const status = useStatus();
  const insets = useSafeAreaInsets();
  const appearance = useAppearance(s => s.choice);
  const [deleting, setDeleting] = useState(false);
  const signOut = () => Alert.alert('Sign out?', 'This phone stops seeing your computers until you sign in again.', [
    { text: 'Cancel', style: 'cancel' },
    { text: 'Sign out', style: 'destructive', onPress: () => void connect.signOut().catch(error => Alert.alert('Could not clear saved sign-in', (error as Error).message)) },
  ]);
  const remove = async () => {
    setDeleting(true);
    try {
      await connect.deleteAccount();
      await clearLocal();
    } catch (error) {
      setDeleting(false);
      Alert.alert('Could not delete the account', (error as Error).message);
    }
  };
  const deleteAccount = () => Alert.alert(
    'Delete your account?',
    'This removes your account and every device signed in to it, phones and computers, from the PandaCode Connect relay, and erases this phone\'s keys and saved data. Your computers then need to sign in to PandaCode Connect again. Tasks on your computers and your GitHub account are not affected.',
    [{ text: 'Cancel', style: 'cancel' }, { text: 'Delete account', style: 'destructive', onPress: () => void remove() }],
  );
  const link = (label: string, url: string) => (
    <Pressable onPress={() => void WebBrowser.openBrowserAsync(url)} accessibilityRole="link"><Text style={[styles.value, { color: colors.tint }]}>{label}</Text></Pressable>
  );
  return (
    <ScrollView style={{ backgroundColor: colors.grouped }} contentInsetAdjustmentBehavior="automatic" contentContainerStyle={[styles.page, { paddingLeft: 16 + insets.left, paddingRight: 16 + insets.right }]}>
      {status.demo ? (
        <Section title="Account" footer="Sample computers and tasks on this phone only. Nothing is sent anywhere.">
          <View style={styles.row}>
            <Text style={[styles.badge, { backgroundColor: colors.tint, color: colors.onTint }]}>Demo</Text>
            <Text style={[styles.value, { flex: 1, color: colors.text }]}>Exploring the demo</Text>
          </View>
          <Pressable onPress={() => void connect.leaveDemo()} accessibilityRole="button"><Text style={[styles.value, { color: colors.tint }]}>Leave demo</Text></Pressable>
        </Section>
      ) : (
      <Section title="Account">
        <View style={styles.row}>
          {!!status.account?.avatarUrl && <Image source={status.account.avatarUrl} style={styles.avatar} />}
          <View style={{ flex: 1 }}>
            <Text style={[styles.value, { color: colors.text }]}>{status.account?.login ?? '—'}</Text>
            <Text style={[styles.small, { color: status.error ? colors.red : colors.secondary }]}>{status.error ?? (status.connected ? 'Connected' : 'Connecting…')}</Text>
          </View>
        </View>
        {deleting ? <ActivityIndicator /> : (
          <Pressable onPress={deleteAccount} accessibilityRole="button"><Text style={[styles.value, { color: colors.red }]}>Delete account…</Text></Pressable>
        )}
      </Section>
      )}
      {!status.demo && <Section title="This phone’s fingerprint" footer="On your computer, open Settings → Remote access and compare the fingerprint shown for this phone. Trust it there only after every character matches. Verify the computer separately in this app’s Computers tab.">
        <Text selectable style={[styles.value, { color: colors.text }]}>{status.fingerprint.match(/.{1,4}/g)?.join(' ') ?? 'Unavailable'}</Text>
      </Section>}
      <Section title="Appearance" footer="Applies to this app only. System follows the phone's light or dark setting.">
        <View style={styles.chips} accessibilityRole="radiogroup">
          {APPEARANCES.map(a => {
            const selected = appearance === a.id;
            return (
              <Pressable key={a.id} onPress={() => setAppearance(a.id)} accessibilityRole="radio" accessibilityState={{ selected }}
                style={[styles.chip, { backgroundColor: selected ? colors.tint : colors.fill }]}>
                <Text style={[styles.chipText, { color: selected ? colors.onTint : colors.text }]}>{a.label}</Text>
              </Pressable>
            );
          })}
        </View>
      </Section>
      <Section title="Wallpaper" footer="The desktop's daily scenes, in step with your computer: each day has a morning, afternoon and night scene. Tap a scene to keep it.">
        <WallpaperSettings />
      </Section>
      {!status.demo && <Section>
        <Pressable onPress={signOut} accessibilityRole="button"><Text style={[styles.value, { color: colors.red }]}>Sign out</Text></Pressable>
      </Section>}
      <Section title="About" footer="PandaCode Extended is an open-source fork of PandaCode. Connect uses your configured relay, GitHub sign-in, and GitHub account avatars. Automatic app updates and telemetry are disabled.">
        <Text style={[styles.value, { color: colors.text }]}>PandaCode Extended {Constants.expoConfig?.version ?? ''}</Text>
        {link('Privacy and local data', PRIVACY)}
        {link('Support', SUPPORT)}
        {link('Source code', SOURCE)}
        {link('Original PandaCode', 'https://github.com/aicodeking/pandacode')}
      </Section>
    </ScrollView>
  );
}

function Section({ title, footer, children }: { title?: string; footer?: string; children: ReactNode }) {
  const colors = useBackdrop();
  return (
    <View style={styles.section}>
      {!!title && <Text style={[styles.header, { color: colors.secondary }]}>{title}</Text>}
      <View style={[styles.card, { backgroundColor: colors.card }]}>{children}</View>
      {!!footer && <Text style={[styles.small, styles.footer, { color: colors.secondary }]}>{footer}</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  page: { padding: 16, gap: 24 },
  section: { gap: 6 },
  header: { fontSize: 13, fontWeight: '600', textTransform: 'uppercase', marginLeft: 14 },
  card: { borderRadius: 16, padding: 14, gap: 12 },
  row: { flexDirection: 'row', alignItems: 'center', gap: 12 },
  badge: { fontSize: 13, fontWeight: '600', borderRadius: 8, overflow: 'hidden', paddingHorizontal: 8, paddingVertical: 2 },
  avatar: { width: 40, height: 40, borderRadius: 20 },
  value: { fontSize: 17 },
  small: { fontSize: 13, lineHeight: 18 },
  footer: { marginHorizontal: 14 },
  chips: { flexDirection: 'row', gap: 8 },
  chip: { flex: 1, alignItems: 'center', borderRadius: 16, paddingVertical: 8 },
  chipText: { fontSize: 15, fontWeight: '500' },
});
