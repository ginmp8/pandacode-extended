import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";
import { configured } from "../src/config.ts";
import { signToken } from "../src/token.ts";

const hooks = registerHooks({
  resolve(specifier, context, nextResolve) {
    return specifier === "cloudflare:workers" ? { url: "test-relay:workers", shortCircuit: true } : nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    return url === "test-relay:workers" ? { format: "module", shortCircuit: true, source: "export class DurableObject {}" } : nextLoad(url, context);
  },
});
const { default: worker } = await import(new URL("../src/index.ts", import.meta.url).href);
hooks.deregister();

const ID = "3f2b8c1e-9a4d-4e0b-8c55-0a1b2c3d4e5f";
const GEN = "eb52f4a1-a4ec-46ad-bb49-81dbe0148081";
const signIn = () => new Request("https://relay.example/v1/auth/github", {
  method: "POST", headers: { "CF-Connecting-IP": "192.0.2.1" },
  body: JSON.stringify({ githubToken: "test-token", device: { id: ID, name: "Computer", platform: "linux", publicKey: btoa(String.fromCharCode(...new Uint8Array(32).fill(7))) } }),
});
function fixture() {
  const calls: string[] = [];
  const env = {
    SESSION_SECRET: "s".repeat(32), GITHUB_CLIENT_ID: "id", GITHUB_CLIENT_SECRET: "test-value",
    AUTH_LIMITER: { limit: async ({ key }: { key: string }) => { calls.push(`ip:${key}`); return { success: true }; } },
    GLOBAL_AUTH_LIMITER: { limit: async ({ key }: { key: string }) => { calls.push(`global:${key}`); return { success: true }; } },
    ACCOUNT_LIMITER: { limit: async ({ key }: { key: string }) => { calls.push(`account:${key}`); return { success: true }; } },
    ACCOUNT: { idFromName: (id: string) => id, get: () => ({ fetch: async () => { calls.push("storage"); return Response.json({ gen: GEN }); } }) },
  };
  return { env, calls };
}

test("configuration fails before rate limits, body reads or network effects", async t => {
  t.mock.method(globalThis, "fetch", async () => { throw new Error("unexpected network request"); });
  const { env, calls } = fixture();
  for (const patch of [{ SESSION_SECRET: "s".repeat(31) }, { SESSION_SECRET: undefined }, { GITHUB_CLIENT_ID: "" }, { GITHUB_CLIENT_SECRET: "" }]) {
    const request = signIn();
    Object.defineProperty(request, "body", { get() { throw new Error("body must not be read"); } });
    const result = await worker.fetch(request, { ...env, ...patch });
    assert.equal(result.status, 500);
    assert.deepEqual(await result.json(), { error: "server error" });
  }
  assert.deepEqual(calls, []);
  assert.equal(configured({ SESSION_SECRET: env.SESSION_SECRET }, false), true);
  assert.equal(configured({ SESSION_SECRET: env.SESSION_SECRET }, true), false);
});

test("IP and global sign-in denials run before body or GitHub processing", async t => {
  const network = t.mock.method(globalThis, "fetch", async () => { throw new Error("unexpected network request"); });
  for (const blocked of ["ip", "global"]) {
    const { env, calls } = fixture();
    const limiter = blocked === "ip" ? env.AUTH_LIMITER : env.GLOBAL_AUTH_LIMITER;
    limiter.limit = async ({ key }) => { calls.push(`${blocked}:${key}`); return { success: false }; };
    const request = signIn();
    Object.defineProperty(request, "body", { get() { throw new Error("body must not be read"); } });
    const result = await worker.fetch(request, env);
    assert.equal(result.status, 429);
    assert.deepEqual(await result.json(), { error: "too many sign-in attempts" });
    assert.deepEqual(calls, blocked === "ip" ? ["ip:192.0.2.1"] : ["ip:192.0.2.1", "global:all"]);
  }
  assert.equal(network.mock.callCount(), 0);
});

test("allowed sign-in checks both budgets before GitHub and registration", async t => {
  const { env, calls } = fixture();
  t.mock.method(globalThis, "fetch", async () => { calls.push("github"); return Response.json({ user: { id: 1, login: "example", avatar_url: "https://example.test/avatar" } }); });
  const response = await worker.fetch(signIn(), env);
  assert.equal(response.status, 200);
  assert.equal(typeof (await response.json()).token, "string");
  assert.deepEqual(calls, ["ip:192.0.2.1", "global:all", "github", "storage"]);
});

test("authenticated requests retain the account budget without requiring OAuth settings", async () => {
  const { env, calls } = fixture();
  env.GITHUB_CLIENT_ID = env.GITHUB_CLIENT_SECRET = "";
  env.ACCOUNT_LIMITER.limit = async ({ key }) => { calls.push(`account:${key}`); return { success: false }; };
  const token = await signToken(env.SESSION_SECRET, "gh:1", ID, Math.floor(Date.now() / 1000), GEN);
  const response = await worker.fetch(new Request("https://relay.example/v1/devices", { headers: { Authorization: `Bearer ${token}` } }), env);
  assert.equal(response.status, 429);
  assert.deepEqual(calls, ["account:gh:1"]);
});
