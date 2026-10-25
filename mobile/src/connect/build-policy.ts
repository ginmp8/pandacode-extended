export interface Build { relay: string; clientId: string }

/** A build must opt in to one HTTPS origin; credentials and URL query data are never accepted. */
export function connectBuild(relayValue: string, clientIdValue: string): { build: Build | null; problem: string } {
  const relay = relayValue.trim().replace(/\/+$/, '');
  const clientId = clientIdValue.trim();
  if (!relay) return { build: null, problem: 'No relay URL was set (PANDACODE_RELAY_URL).' };
  let origin: URL;
  try { origin = new URL(relay); }
  catch { return { build: null, problem: 'The relay URL must be an HTTPS origin without a path, credentials, query or fragment.' }; }
  if (!/^https:\/\/[^/\\?#\s]+$/.test(relay) || origin.protocol !== 'https:' || !origin.hostname || origin.username || origin.password || origin.pathname !== '/' || origin.search || origin.hash) {
    return { build: null, problem: 'The relay URL must be an HTTPS origin without a path, credentials, query or fragment.' };
  }
  if (!clientId) return { build: null, problem: 'No GitHub client ID was set (PANDACODE_GITHUB_CLIENT_ID).' };
  if (!/^[A-Za-z0-9._-]{1,256}$/.test(clientId)) return { build: null, problem: 'The GitHub client ID is invalid.' };
  return { build: { relay: origin.origin, clientId }, problem: '' };
}
