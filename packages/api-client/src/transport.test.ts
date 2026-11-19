import { describe, expect, it, vi } from "vitest";
import { ApiError, createApi, createHttpInvoke, detectTransport } from "./index";

function fakeFetch(status: number, body: unknown) {
  return vi.fn(async () => new Response(JSON.stringify(body), { status }));
}

describe("detectTransport", () => {
  it("uses tauri when the Tauri globals exist", () => {
    expect(detectTransport({ __TAURI_INTERNALS__: {} })).toBe("tauri");
    expect(detectTransport({})).toBe("http");
  });
});

describe("http transport", () => {
  it("posts JSON to /api/invoke/<command>", async () => {
    const f = fakeFetch(200, { minor: "27990", currency: "CLP", display: "$27.990" });
    const api = createApi(createHttpInvoke("http://back:7878", f as unknown as typeof fetch));
    const money = await api.money.parse({ amount: "27.990", currency: "CLP" });

    expect(money.display).toBe("$27.990");
    expect(f).toHaveBeenCalledWith(
      "http://back:7878/api/invoke/money_parse",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ input: { amount: "27.990", currency: "CLP" } }),
      }),
    );
  });

  it("maps backend errors to ApiError", async () => {
    const f = fakeFetch(422, { code: "VALIDATION", message: "invalid amount" });
    const api = createApi(createHttpInvoke("", f as unknown as typeof fetch));
    await expect(api.money.parse({ amount: "x", currency: "CLP" })).rejects.toMatchObject({
      name: "ApiError",
      code: "VALIDATION",
    });
  });

  it("maps network failures to TRANSPORT errors", async () => {
    const f = vi.fn(async () => {
      throw new TypeError("fetch failed");
    });
    const api = createApi(createHttpInvoke("", f as unknown as typeof fetch));
    const err = await api.app.health().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).code).toBe("TRANSPORT");
  });
});

describe("ApiError.from", () => {
  it("parses Tauri string errors", () => {
    const e = ApiError.from('{"code":"NOT_FOUND","message":"nope"}');
    expect(e.code).toBe("NOT_FOUND");
    expect(e.message).toBe("nope");
  });
});
