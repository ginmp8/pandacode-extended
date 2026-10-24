// PandaCode's sandbox Worker. PandaCode deploys it to your account and calls it with a
// random bearer token kept in your OS keychain. Each name gets its own container that
// stops itself after IDLE_MS without use.
import { DurableObject } from "cloudflare:workers";

const NAME = /^[a-z0-9][a-z0-9-]{0,62}$/;
const IDLE_MS = 10 * 60 * 1000;
const MAX_OUTPUT = 256 * 1024;
const WORKSPACE = "/workspace";

interface Env {
  SANDBOX: DurableObjectNamespace<Sandbox>;
  PANDACODE_TOKEN: string;
}

type Result = { exitCode: number | null; stdout: string; stderr: string; timedOut: boolean; truncated: boolean };

function clip(buffer: ArrayBuffer): [string, boolean] {
  const bytes = new Uint8Array(buffer);
  const text = new TextDecoder().decode(bytes.length > MAX_OUTPUT ? bytes.subarray(bytes.length - MAX_OUTPUT) : bytes);
  return [text, bytes.length > MAX_OUTPUT];
}

export class Sandbox extends DurableObject<Env> {
  status(): { running: boolean } {
    return { running: this.container().running };
  }

  async start(): Promise<{ running: boolean }> {
    await this.ensureRunning();
    return { running: true };
  }

  private execBusy = false;

  async exec(command: string, timeoutMs: number): Promise<Result> {
    if (this.execBusy) throw new Error("A command is already running in this sandbox");
    this.execBusy = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const container = await this.ensureRunning();
      // GNU timeout (Debian coreutils) signals the command's *process group*,
      // including child commands. Promise.race by itself would not stop them.
      const process = await container.exec(["timeout", "--kill-after=5", `${Math.ceil(timeoutMs / 1000)}s`, "bash", "-lc", command], { cwd: WORKSPACE });
      const deadline = new Promise<null>((resolve) => {
        timer = setTimeout(() => resolve(null), timeoutMs + 15_000);
      });
      const output = await Promise.race([process.output(), deadline]);
      if (output === null) {
        // Unexpected child process/pipe state. Destroy the entire instance
        // rather than reporting a timeout while detached work may continue.
        await container.destroy();
        return { exitCode: null, stdout: "", stderr: "Timed out; sandbox instance stopped (workspace data discarded).", timedOut: true, truncated: false };
      }
      const timedOut = output.exitCode === 124 || output.exitCode === 137;
      if (timedOut) {
        // Detached process groups could survive GNU timeout. Destroy the
        // instance before returning a successful cancellation acknowledgement.
        await container.destroy();
        return { exitCode: null, stdout: "", stderr: "Timed out; sandbox instance stopped (workspace data discarded).", timedOut: true, truncated: false };
      }
      const [stdout, cutOut] = clip(output.stdout);
      const [stderr, cutErr] = clip(output.stderr);
      return { exitCode: output.exitCode ?? null, stdout, stderr, timedOut: false, truncated: cutOut || cutErr };
    } finally {
      if (timer !== undefined) clearTimeout(timer);
      this.execBusy = false;
    }
  }

  async stop(): Promise<void> {
    const container = this.container();
    if (container.running) await container.destroy();
  }

  private container(): Container {
    const container = this.ctx.container;
    if (container === undefined) throw new Error("Container attachment is unavailable");
    return container;
  }

  private async ensureRunning(): Promise<Container> {
    const container = this.container();
    if (!container.running) {
      container.start({ image: container.images.sandbox, instance: "basic", enableInternet: true });
    }
    await container.setInactivityTimeout(IDLE_MS);
    return container;
  }
}

async function authorized(request: Request, env: Env): Promise<boolean> {
  const expected = new TextEncoder().encode(`Bearer ${env.PANDACODE_TOKEN ?? ""}`);
  const given = new TextEncoder().encode(request.headers.get("authorization") ?? "");
  if (!env.PANDACODE_TOKEN || expected.byteLength !== given.byteLength) return false;
  return crypto.subtle.timingSafeEqual(expected, given);
}

export default {
  async fetch(request, env): Promise<Response> {
    if (!(await authorized(request, env))) return new Response("Unauthorized", { status: 401 });
    const url = new URL(request.url);
    if (url.pathname === "/health") return Response.json({ ok: true, version: 1 });
    const match = /^\/sandboxes\/([^/]+)(\/exec|\/start|\/stop)?$/.exec(url.pathname);
    if (match === null || !NAME.test(match[1])) return Response.json({ message: "Not found" }, { status: 404 });
    const sandbox = env.SANDBOX.getByName(match[1]);
    try {
      if (match[2] === undefined && request.method === "GET") return Response.json(await sandbox.status());
      if (match[2] === "/start" && request.method === "POST") return Response.json(await sandbox.start());
      if (match[2] === "/stop" && request.method === "POST") { await sandbox.stop(); return Response.json({ running: false }); }
      if (match[2] === undefined && request.method === "DELETE") { await sandbox.stop(); return Response.json({ deleted: true }); }
      if (match[2] === "/exec" && request.method === "POST") {
        const maxBytes = 256 * 1024;
        const reader = request.body?.getReader();
        if (!reader) return Response.json({ message: "Request body required" }, { status: 400 });
        const chunks: Uint8Array[] = [];
        let bytes = 0;
        try {
          for (;;) {
            const { done, value } = await reader.read();
            if (done) break;
            if (value.byteLength > maxBytes - bytes) {
              void reader.cancel().catch(() => {});
              return Response.json({ message: "Request body too large" }, { status: 413 });
            }
            bytes += value.byteLength;
            chunks.push(value);
          }
        } finally { reader.releaseLock(); }
        const raw = new Uint8Array(bytes);
        let offset = 0;
        for (const chunk of chunks) { raw.set(chunk, offset); offset += chunk.byteLength; }
        let body: { command?: unknown; timeoutMs?: unknown };
        try { body = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(raw)); }
        catch { return Response.json({ message: "Invalid JSON" }, { status: 400 }); }
        if (!body || typeof body !== "object" || typeof body.command !== "string" || body.command.length === 0 || body.command.length > 65536) {
          return Response.json({ message: "command must be a non-empty string" }, { status: 400 });
        }
        const timeoutMs = typeof body.timeoutMs === "number" && Number.isFinite(body.timeoutMs)
          ? Math.ceil(Math.min(Math.max(body.timeoutMs, 1000), 1_800_000)) : 120_000;
        return Response.json(await sandbox.exec(body.command, timeoutMs));
      }
      return Response.json({ message: "Method not allowed" }, { status: 405 });
    } catch (cause) {
      console.error({ event: "sandbox.request.failed", path: url.pathname, error: cause instanceof Error ? cause.stack : String(cause) });
      return Response.json({ message: "Sandbox request failed; operation outcome may be indeterminate. Check the sandbox before retrying." }, { status: 500 });
    }
  },
} satisfies ExportedHandler<Env>;
