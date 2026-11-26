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

describe("category_tree (S4, DEC-13)", () => {
  it("createApi().category.tree sends category_tree with the query", async () => {
    const { calls, api } = recorder();
    await expect(api.category.tree({ kind: "expense" })).rejects.toMatchObject({ code: "NOT_IMPLEMENTED" });
    await expect(api.category.tree({})).rejects.toMatchObject({ code: "NOT_IMPLEMENTED" });
    expect(calls).toEqual([
      ["category_tree", { query: { kind: "expense" } }],
      ["category_tree", { query: {} }],
    ]);
  });
});

describe("M2 commands (C0, DEC-30)", () => {
  it("createApi sends every M2 catalogue name with the Tauri argument keys", async () => {
    const { calls, api } = recorder();
    const budget = { period: "2026-11", categoryId: "c", scope: "household" as const, amount: "150.000" };
    const all = [
      api.category.create({ kind: "expense", name: "Cine" }),
      api.category.update({ id: "c", name: "Cine" }),
      api.category.move({ id: "c", parentId: "p", index: 0 }),
      api.category.archive({ id: "c", archived: true }),
      api.category.delete({ id: "c", mergeIntoId: "d" }),
      api.budget.set(budget),
      api.budget.copyMonth({ from: "2026-10", to: "2026-11" }),
      api.budget.matrix({ year: 2026, view: "combined" }),
      api.report.monthCalendar({ month: "2026-11", view: "household" }),
      api.report.cashFlow({ from: "2026-01", to: "2026-11", view: "combined" }),
      api.report.spending({ from: "2026-11", to: "2026-11", view: "personal" }),
      api.tx.list({ view: "combined", categoryId: "c" }),
    ];
    const results = await Promise.allSettled(all);
    expect(results.every((r) => r.status === "rejected")).toBe(true);
    expect(calls).toEqual([
      ["category_create", { input: { kind: "expense", name: "Cine" } }],
      ["category_update", { input: { id: "c", name: "Cine" } }],
      ["category_move", { input: { id: "c", parentId: "p", index: 0 } }],
      ["category_archive", { input: { id: "c", archived: true } }],
      ["category_delete", { input: { id: "c", mergeIntoId: "d" } }],
      ["budget_set", { input: budget }],
      ["budget_copy_month", { input: { from: "2026-10", to: "2026-11" } }],
      ["budget_matrix", { query: { year: 2026, view: "combined" } }],
      ["report_month_calendar", { query: { month: "2026-11", view: "household" } }],
      ["report_cash_flow", { query: { from: "2026-01", to: "2026-11", view: "combined" } }],
      ["report_spending", { query: { from: "2026-11", to: "2026-11", view: "personal" } }],
      ["tx_list", { query: { view: "combined", categoryId: "c" } }],
    ]);
  });
});

describe("ApiError fields", () => {
  it("keeps field errors for forms", () => {
    const e = ApiError.from({
      code: "CURRENCY_MISMATCH",
      message: "USD amount on a CLP account",
      fields: { amount: "errors.field.currencyMismatch" },
    });
    expect(e.code).toBe("CURRENCY_MISMATCH");
    expect(e.fields).toEqual({ amount: "errors.field.currencyMismatch" });
  });

  it("is null when the error has none", () => {
    expect(ApiError.from({ code: "VALIDATION", message: "x", fields: null }).fields).toBeNull();
    expect(ApiError.from({ code: "NOT_FOUND", message: "nope" }).fields).toBeNull();
  });
});
