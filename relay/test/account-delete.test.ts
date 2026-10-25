import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteAccountState } from "../src/account-delete.ts";
import { tokenCurrent } from "../src/policy.ts";
import { isKnownRoute } from "../src/routes.ts";
import { signToken, verifyToken } from "../src/token.ts";

test("account deletion closes every socket before clearing durable state", async () => {
  const events: string[] = [];
  const stored = new Map<string, unknown>([["devices", ["phone", "desktop"]], ["other-state", "private"]]);
  await deleteAccountState({
    getWebSockets: () => ["phone", "desktop"],
    storage: { async deleteAll() { events.push("delete-all"); stored.clear(); } },
  }, (socket, code, reason) => events.push(`close:${socket}:${code}:${reason}`));
  assert.deepEqual(events, ["close:phone:4001:signed out", "close:desktop:4001:signed out", "delete-all"]);
  assert.equal(stored.size, 0);
});

test("a deletion failure is reported, never a successful account wipe", async () => {
  await assert.rejects(deleteAccountState({
    getWebSockets: () => [], storage: { async deleteAll() { throw new Error("storage failed"); } },
  }, () => undefined), /storage failed/);
});

test("account deletion is an authenticated exact route", () => {
  assert.equal(isKnownRoute("/v1/account"), true);
  for (const path of ["/v1/account/extra", "/v1/accounts", "/v1/devices/a/b", "/internal/register"]) assert.equal(isKnownRoute(path), false);
});

test("same-second re-registration rejects old and legacy tokens", async () => {
  const original = "3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f";
  const fresh = "eb52f4a1-a4ec-46ad-bb49-81dbe0148081";
  const claims = await verifyToken("secret", await signToken("secret", "gh:1", original, 1_000, original), 1_001);
  assert.equal(claims?.gen, original);
  assert.equal(tokenCurrent(claims!, { gen: original, registered_at: 1_000 }), true);
  assert.equal(tokenCurrent(claims!, { gen: fresh, registered_at: 1_000 }), false);
  assert.equal(tokenCurrent({ iat: 1_000 }, { gen: fresh, registered_at: 1_000 }), false, "legacy iat equality cannot revive a deleted identity");
  assert.equal(tokenCurrent({ iat: 1_000 }, { gen: "", registered_at: 1_000 }), true, "existing schema-1 sessions continue to work");
  assert.equal(tokenCurrent({ iat: 999 }, { gen: "", registered_at: 1_000 }), false);
  assert.equal(tokenCurrent({ iat: NaN }, { gen: "", registered_at: 1_000 }), false);
  await assert.rejects(signToken("secret", "gh:1", original, 1_000, ""), /generation/);
});
