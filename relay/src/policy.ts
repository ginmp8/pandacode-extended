import { STALE_MS } from "./limits.ts";

/** A device id may sign in again only with the key it registered; anything else is a takeover attempt. */
export const keyChanged = (existingPublicKey: string | undefined, publicKey: string): boolean =>
  existingPublicKey !== undefined && existingPublicKey !== publicKey;

/** Tokens issued before the device (re)registered stay dead after a revoke and fresh sign-in. */
export const issuedAfterRegistration = (iat: number, registeredAt: number): boolean => iat >= registeredAt;

/** Legacy tokens are valid only for legacy rows. After deletion/revocation, even
 * same-second re-registration cannot revive a previous token. */
export const tokenCurrent = (claim: { gen?: string; iat: number }, row: { gen: string; registered_at: number }): boolean =>
  Number.isSafeInteger(claim.iat) && claim.iat >= 0 && (claim.gen === undefined
    ? row.gen === "" && issuedAfterRegistration(claim.iat, row.registered_at)
    : claim.gen !== "" && claim.gen === row.gen);

/** A socket is stale when its last ping (or its join, before the first ping) is STALE_MS old or more. */
export const isStale = (lastActivityMs: number, nowMs: number): boolean => nowMs - lastActivityMs >= STALE_MS;

/** A `{kind:"host"}` frame that repeats the stored value writes nothing and broadcasts nothing. */
export const hostChanged = (stored: number | undefined, enabled: boolean): boolean => stored !== (enabled ? 1 : 0);
