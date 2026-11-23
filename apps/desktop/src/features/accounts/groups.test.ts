import type { AccountKind } from "@mr-krabs/api-client";

import { ACCOUNT_GROUPS, ACCOUNT_KINDS } from "./groups";

// Every kind in the contract (bindings.ts AccountKind), so a new kind can't vanish from the sidebar.
const ALL_KINDS: Record<AccountKind, true> = {
  checking: true,
  savings: true,
  cuenta_vista: true,
  cash: true,
  credit_card: true,
  loan: true,
  mortgage: true,
  investment: true,
  other_asset: true,
  other_liability: true,
};

describe("account groups", () => {
  it("place every account kind in exactly one group", () => {
    expect([...ACCOUNT_KINDS].sort()).toEqual(Object.keys(ALL_KINDS).sort());
    expect(new Set(ACCOUNT_KINDS).size).toBe(ACCOUNT_KINDS.length);
  });

  it("follow MASTER §5.1 order", () => {
    expect(ACCOUNT_GROUPS.map((g) => g.id)).toEqual(["banks", "cards", "loans", "investments", "other"]);
  });
});
