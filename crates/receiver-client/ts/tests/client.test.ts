// The package as a downstream reaches it: imported by its name, asking a running receiver and reading its
// answers in the common wire format.

import { afterAll, beforeAll, expect, test } from "bun:test";

import { ReceiverError, receiverClient } from "@bmo/receiver-client";

interface Asked {
  url: string;
  authorization: string | null;
}

const asked: Asked[] = [];
let server: ReturnType<typeof Bun.serve>;
let baseUrl = "";

beforeAll(() => {
  server = Bun.serve({
    port: 0,
    fetch(request) {
      asked.push({ url: request.url, authorization: request.headers.get("Authorization") });

      const path = new URL(request.url).pathname;

      if (path === "/health") {
        return Response.json({ ready: true, reason: null, version: "1.2.3" });
      }

      if (path === "/rooms/-7/messages") {
        return Response.json({
          items: [
            {
              id: 11,
              room: -7,
              senderId: 42,
              senderName: "person-a",
              sentAt: 1,
              text: "hello",
              imagePath: null,
              replyToMessageId: null,
            },
          ],
          more: false,
        });
      }

      if (path === "/providers/road-a/rooms") {
        return Response.json({
          items: [{ id: -100, title: "A room", kind: "group", privateTo: null, mode: null }],
          more: true,
        });
      }

      if (path === "/rooms" && request.method === "POST") {
        return Response.json({ code: "avatar-unsupported", error: "It cannot." }, { status: 409 });
      }

      return Response.json({ code: "not-found", error: "Nothing stands under that." }, { status: 404 });
    },
  });
  baseUrl = server.url.origin;
});

afterAll(() => {
  server.stop(true);
});

test("a call reaches its route with the token and reads its answer", async () => {
  const receiver = receiverClient({ baseUrl, token: "the-token" });
  const health = await receiver.health();

  expect(health.ready).toBe(true);
  expect(health.reason).toBeNull();
  expect(asked[0]?.url).toBe(`${baseUrl}/health`);
  expect(asked[0]?.authorization).toBe("Bearer the-token");
});

test("a page of a room's messages is read as the common format writes it", async () => {
  const receiver = receiverClient({ baseUrl, token: "the-token" });
  const page = await receiver.messages(-7, { limit: 1 });

  expect(page.more).toBe(false);
  expect(page.items[0]?.senderId).toBe(42);
  expect(page.items[0]?.text).toBe("hello");
  expect(asked[1]?.url).toBe(`${baseUrl}/rooms/-7/messages?limit=1`);
});

test("a refusal is thrown as a receiver error carrying its status and code", async () => {
  const receiver = receiverClient({ baseUrl, token: "the-token" });
  const outcome = await receiver.room(9).then(
    (): null => null,
    (failure: Error) => failure,
  );
  const refusal = outcome instanceof ReceiverError ? outcome : undefined;

  expect(refusal?.status).toBe(404);
  expect(refusal?.code).toBe("not-found");
});

test("the rooms of a provider's account are read as a page, and a refused addition carries its code", async () => {
  const receiver = receiverClient({ baseUrl, token: "the-token" });
  const page = await receiver.accountRooms("road-a", { after: -200, limit: 1 });

  expect(page.more).toBe(true);
  expect(page.items[0]?.id).toBe(-100);
  expect(page.items[0]?.mode).toBeNull();
  expect(asked.at(-1)?.url).toBe(`${baseUrl}/providers/road-a/rooms?after=-200&limit=1`);

  const outcome = await receiver.addRoom({ provider: "road-a", roomId: -100, mode: "avatar" }).then(
    (): null => null,
    (failure: Error) => failure,
  );
  const refusal = outcome instanceof ReceiverError ? outcome : undefined;

  expect(refusal?.status).toBe(409);
  expect(refusal?.code).toBe("avatar-unsupported");
});
