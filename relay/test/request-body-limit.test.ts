import assert from "node:assert/strict";
import { test } from "node:test";
import { MAX_FRAME_BYTES, readJsonLimited } from "../src/limits.ts";

function streamedRequest(chunks: Uint8Array[]): Request {
  let index = 0;
  const body = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index < chunks.length) controller.enqueue(chunks[index++]);
      else controller.close();
    },
  });
  // No Content-Length is supplied; the body enforces the actual byte budget.
  return new Request("https://relay.example/v1/auth/github", { method: "POST", body, duplex: "half" } as RequestInit & { duplex: string });
}

test("refuses an oversized stream without Content-Length", async () => {
  const chunk = new Uint8Array(MAX_FRAME_BYTES / 2 + 1).fill(65);
  const response = await readJsonLimited(streamedRequest([chunk, chunk]), MAX_FRAME_BYTES);
  assert.equal(response.tooLarge, true);
  assert.equal(response.value, null);
});

test("accepts bounded JSON without Content-Length", async () => {
  const content = new TextEncoder().encode(JSON.stringify({ githubToken: "x", device: {} }));
  const response = await readJsonLimited(streamedRequest([content]), MAX_FRAME_BYTES);
  assert.equal(response.tooLarge, false);
  assert.deepEqual(response.value, { githubToken: "x", device: {} });
});

test("fails closed on invalid JSON", async () => {
  const response = await readJsonLimited(streamedRequest([new TextEncoder().encode("not-json")]), MAX_FRAME_BYTES);
  assert.equal(response.tooLarge, false);
  assert.equal(response.value, null);
});
