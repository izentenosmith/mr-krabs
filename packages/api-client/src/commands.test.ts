import { describe, expect, it, vi } from "vitest";
import { ApiError, createApi, type Invoke } from "./index";

/** Records every invoke call; answers like a core stub that is not built yet. */
function recorder() {
  const calls: [string, Record<string, unknown> | undefined][] = [];
  const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    calls.push([command, args]);
    throw ApiError.from({ code: "NOT_IMPLEMENTED", message: `${command} is not implemented yet`, fields: null });
  }) as unknown as Invoke;
  return { calls, api: createApi(invoke) };
}

const money = { amount: "27.990", currency: "CLP" };
const expense = {
  type: "expense" as const,
  accountId: "a",
  categoryId: "c",
  amount: money,
  date: "2026-11-21",
};
const account = { name: "Cuenta", kind: "checking" as const, currency: "CLP" };

describe("createApi exposes every M1 command", () => {
  it("sends the catalogue name and the Tauri argument keys", async () => {
    const { calls, api } = recorder();
    const all = [
      api.app.bootstrap(),
      api.household.create({ name: "Casa", myName: "Yo", baseCurrency: "CLP" }),
      api.settings.get(),
      api.account.list({ view: "combined" }),
      api.account.get("a"),
      api.account.create(account),
      api.account.update("a", account),
      api.account.archive("a"),
      api.tx.create(expense),
      api.tx.get("t"),
      api.tx.list({ view: "household", accountId: "a", limit: 500 }),
      api.tx.update("t", expense),
      api.tx.delete("t"),
      api.tx.restore("t"),
    ];
    const results = await Promise.allSettled(all);
    expect(results.every((r) => r.status === "rejected")).toBe(true);
    expect(calls).toEqual([
      ["app_bootstrap", undefined],
      ["household_create", { input: { name: "Casa", myName: "Yo", baseCurrency: "CLP" } }],
      ["settings_get", undefined],
      ["account_list", { query: { view: "combined" } }],
      ["account_get", { id: "a" }],
      ["account_create", { input: account }],
      ["account_update", { id: "a", input: account }],
      ["account_archive", { id: "a" }],
      ["tx_create", { input: expense }],
      ["tx_get", { id: "t" }],
      ["tx_list", { query: { view: "household", accountId: "a", limit: 500 } }],
      ["tx_update", { id: "t", input: expense }],
      ["tx_delete", { id: "t" }],
      ["tx_restore", { id: "t" }],
    ]);
  });

  it("surfaces NOT_IMPLEMENTED as an ApiError", async () => {
    const { api } = recorder();
    await expect(api.tx.list({ view: "combined" })).rejects.toMatchObject({
      name: "ApiError",
      code: "NOT_IMPLEMENTED",
    });
  });
});

describe("ApiError fields", () => {
  it("keeps field errors for forms", () => {
    const e = ApiError.from({
      code: "CURRENCY_MISMATCH",
      message: "USD amount on a CLP account",
      fields: { amount: "errors.currencyMismatch" },
    });
    expect(e.code).toBe("CURRENCY_MISMATCH");
    expect(e.fields).toEqual({ amount: "errors.currencyMismatch" });
  });

  it("is null when the error has none", () => {
    expect(ApiError.from({ code: "VALIDATION", message: "x", fields: null }).fields).toBeNull();
    expect(ApiError.from({ code: "NOT_FOUND", message: "nope" }).fields).toBeNull();
  });
});
