import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { signToken } from "../src/token.ts";

// Execute the actual Durable Object and Worker with an in-memory SQLite adapter.
// Cloudflare's platform base class is the only replaced module.
const hooks = registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "cloudflare:workers") return { url: "test-relay:workers", shortCircuit: true };
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url === "test-relay:workers") return { format: "module", shortCircuit: true,
      source: "export class DurableObject { constructor(ctx, env) { this.ctx = ctx; this.env = env; } }" };
    return nextLoad(url, context);
  },
});
const { Account } = await import(new URL("../src/account.ts", import.meta.url).href);
const { default: worker } = await import(new URL("../src/index.ts", import.meta.url).href);
hooks.deregister();
(globalThis as any).WebSocket = { READY_STATE_OPEN: 1 };
(globalThis as any).WebSocketRequestResponsePair = class {};

const ID = "3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f";
const KEY = btoa(String.fromCharCode(...new Uint8Array(32).fill(7)));
const device = { id: ID, name: "Computer", platform: "linux", publicKey: KEY };
const request = (path: string, gen?: string, method = "GET") => new Request(`https://account${path}`, {
  method, headers: { "x-device-id": ID, "x-token-iat": String(Math.floor(Date.now() / 1000)), ...(gen === undefined ? {} : { "x-token-gen": gen }) },
});
const registration = () => new Request("https://account/internal/register", { method: "POST", body: JSON.stringify(device) });

function fixture() {
  const database = new DatabaseSync(":memory:");
  const socket = {
    readyState: 1, sent: [] as string[], closed: [] as number[],
    attachment: { device: ID, joinedAt: Date.now(), tokens: 100, at: Date.now() },
    deserializeAttachment() { return this.attachment; },
    serializeAttachment(value: typeof this.attachment) { this.attachment = value; },
    close(code: number) { this.closed.push(code); this.readyState = 3; },
    send(data: string) { this.sent.push(data); },
  };
  let failDelete = false;
  let deletionWait: Promise<void> | undefined;
  const storage = {
    sql: { exec(sql: string, ...bindings: any[]) {
      const rows = database.prepare(sql).all(...bindings);
      return { toArray: () => rows, one: () => { assert.equal(rows.length, 1); return rows[0]; } };
    } },
    async deleteAll() {
      if (deletionWait) await deletionWait;
      if (failDelete) throw new Error("disk failure");
      database.exec("DROP TABLE devices; PRAGMA user_version = 0;");
    },
  };
  const ctx = {
    storage, getWebSockets: () => [socket], setWebSocketAutoResponse() {},
    getWebSocketAutoResponseTimestamp: () => new Date(),
  };
  return {
    account: new Account(ctx, {}), ctx, socket, database,
    failDelete: () => { failDelete = true; },
    holdDelete: () => { let finish!: () => void; deletionWait = new Promise(resolve => { finish = resolve; }); return finish; },
  };
}

test("actual account deletion blocks racing work and requires a fresh registration", async () => {
  const f = fixture();
  try {
    const { gen: original } = await (await f.account.fetch(registration())).json();
    assert.match(original, /^[0-9a-f-]{36}$/);
    assert.equal((await f.account.fetch(request("/v1/account", original))).status, 405);
    assert.equal((await f.account.fetch(request("/v1/devices", "old-key-generation"))).status, 401);
    const finish = f.holdDelete();
    const deletion = f.account.fetch(request("/v1/account", original, "DELETE"));
    await Promise.resolve();
    assert.equal((await f.account.fetch(registration())).status, 503, "registration cannot race an in-progress erase");
    await f.account.webSocketMessage(f.socket, '{"kind":"host","enabled":true}');
    assert.deepEqual(f.socket.closed, [4001]);
    assert.deepEqual(f.socket.sent, []);
    finish();
    assert.deepEqual(await (await deletion).json(), { ok: true });
    assert.equal((await f.account.fetch(request("/v1/devices", original))).status, 401);
    const { gen: next } = await (await f.account.fetch(registration())).json();
    assert.notEqual(next, original);
    assert.equal((await f.account.fetch(request("/v1/devices", original))).status, 401);
    assert.equal((await f.account.fetch(request("/v1/devices"))).status, 401, "a legacy token cannot revive a newly registered row");
    assert.equal((await f.account.fetch(request("/v1/devices", next))).status, 200);
    await f.account.webSocketMessage(f.socket, '{"kind":"host","enabled":true}');
    const row = f.database.prepare("SELECT host FROM devices WHERE id = ?").get(ID);
    assert.equal(row?.host, 0, "a departed socket cannot control a re-registered device");
  } finally { f.database.close(); }
});

test("failed durable deletion retains rows and reports failure; object restart accepts no deleted session", async () => {
  const f = fixture();
  try {
    const { gen } = await (await f.account.fetch(registration())).json();
    f.failDelete();
    await assert.rejects(f.account.fetch(request("/v1/account", gen, "DELETE")), /disk failure/);
    assert.equal((await f.account.fetch(request("/v1/devices", gen))).status, 200, "failed erase did not claim success");
  } finally { f.database.close(); }
  const erased = fixture();
  try {
    const { gen } = await (await erased.account.fetch(registration())).json();
    await erased.account.fetch(request("/v1/account", gen, "DELETE"));
    const restarted = new Account(erased.ctx, {});
    assert.equal((await restarted.fetch(request("/v1/devices", gen))).status, 401);
  } finally { erased.database.close(); }
});

test("Worker authentication strips caller-supplied generation headers", async () => {
  let seen: Headers | undefined;
  const env = {
    SESSION_SECRET: "a-reviewed-signing-secret-at-least-32-bytes",
    ACCOUNT_LIMITER: { limit: async () => ({ success: true }) },
    ACCOUNT: { idFromName: (id: string) => id, get: () => ({ fetch: async (req: Request) => { seen = req.headers; return Response.json({ devices: [] }); } }) },
  };
  const legacy = await signToken(env.SESSION_SECRET, "gh:1", ID, Math.floor(Date.now() / 1000));
  const result = await worker.fetch(new Request("https://relay.example/v1/devices", {
    headers: { Authorization: `Bearer ${legacy}`, "x-token-gen": "attacker-supplied", "x-device-id": "attacker" },
  }), env);
  assert.equal(result.status, 200);
  assert.equal(seen?.get("x-token-gen"), null);
  assert.equal(seen?.get("x-device-id"), ID);
  assert.equal(seen?.get("authorization"), null);
  seen = undefined;
  assert.equal((await worker.fetch(new Request("https://relay.example/v1/account", { method: "DELETE" }), env)).status, 401);
  assert.equal(seen, undefined, "unauthenticated deletion never reaches account storage");
});
