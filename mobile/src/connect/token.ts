export interface Claims { acct: string; dev: string; iat: number; exp: number }

/** Decode only to check local expiry and identity. The relay verifies the signature. */
export function parseToken(token: string): Claims | null {
  if (typeof token !== 'string' || token.length > 4096 || !/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(token)) return null;
  try {
    const part = token.split('.')[0].replace(/-/g, '+').replace(/_/g, '/');
    const c = JSON.parse(atob(part.padEnd(Math.ceil(part.length / 4) * 4, '=')));
    return c && typeof c.acct === 'string' && /^gh:[0-9]{1,32}$/.test(c.acct)
      && typeof c.dev === 'string' && /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(c.dev)
      && Number.isSafeInteger(c.iat) && c.iat >= 0 && Number.isSafeInteger(c.exp) && c.exp > c.iat ? c : null;
  } catch { return null; }
}
