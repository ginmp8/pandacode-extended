// Build config (docs/remote-access.md, Accounts). Values come from app.config.ts `extra`
// (read when Metro starts or the app is built), or EXPO_PUBLIC_* inlined into the bundle.
import Constants from 'expo-constants';
import { connectBuild } from './build-policy.ts';
export type { Build } from './build-policy.ts';

const extra = (Constants.expoConfig?.extra ?? {}) as { relayUrl?: string; githubClientId?: string };
const relay = process.env.EXPO_PUBLIC_PANDACODE_RELAY_URL || extra.relayUrl || '';
const clientId = process.env.EXPO_PUBLIC_PANDACODE_GITHUB_CLIENT_ID || extra.githubClientId || '';
const parsed = connectBuild(relay, clientId);

/** Why Connect is off in this build, or '' when it is configured. */
export const configProblem = parsed.problem;

export const build = parsed.build;
export const NOT_CONFIGURED = 'PandaCode Connect is not configured in this build.';
