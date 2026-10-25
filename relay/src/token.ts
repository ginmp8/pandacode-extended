import { TOKEN_TTL_SECONDS } from "./limits.ts";

export interface TokenClaims {
  acct: string;
  dev: string;
  iat: number;
  exp: number;
  /** Fresh registration generation; absent only for rows migrated from schema 1. */
  gen?: string;
}

const enc = new TextEncoder();

function b64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function unb64url(s: string): Uint8Array<ArrayBuffer> | null {
  if (!/^[A-Za-z0-9_-]*$/.test(s)) return null;
  try {
    const bin = atob(s.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(s.length / 4) * 4, "="));
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
  } catch {
    return null;
  }
}

// One import per secret for the isolate's lifetime.
const keys = new Map<string, Promise<CryptoKey>>();
function hmacKey(secret: string): Promise<CryptoKey> {
  let key = keys.get(secret);
  if (!key) {
    key = crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
    key.catch(() => keys.delete(secret));
    keys.set(secret, key);
  }
  return key;
}

export async function signToken(secret: string, acct: string, dev: string, nowSeconds: number, gen?: string): Promise<string> {
  if (gen !== undefined && !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(gen)) throw new Error("invalid registration generation");
  const claims: TokenClaims = { acct, dev, iat: nowSeconds, exp: nowSeconds + TOKEN_TTL_SECONDS, ...(gen === undefined ? {} : { gen }) };
  const payload = b64url(enc.encode(JSON.stringify(claims)));
  const sig = await crypto.subtle.sign("HMAC", await hmacKey(secret), enc.encode(payload));
  return `${payload}.${b64url(new Uint8Array(sig))}`;
}

/** Returns the claims, or null for any malformed, forged or expired token. */
export async function verifyToken(secret: string, token: string, nowSeconds: number): Promise<TokenClaims | null> {
  const parts = token.split(".");
  if (parts.length !== 2) return null;
  const sig = unb64url(parts[1]);
  if (!sig) return null;
  // subtle.verify compares in constant time.
  if (!(await crypto.subtle.verify("HMAC", await hmacKey(secret), sig, enc.encode(parts[0])))) return null;
  const raw = unb64url(parts[0]);
  if (!raw) return null;
  try {
    const c = JSON.parse(new TextDecoder().decode(raw)) as Partial<TokenClaims>;
    if (typeof c.acct !== "string" || typeof c.dev !== "string" || !Number.isSafeInteger(c.iat) || !Number.isSafeInteger(c.exp) || c.iat! < 0 || c.exp! <= c.iat!) return null;
    if (c.gen !== undefined && (typeof c.gen !== "string" || !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(c.gen))) return null;
    return c.exp! > nowSeconds ? (c as TokenClaims) : null;
  } catch {
    return null;
  }
}
