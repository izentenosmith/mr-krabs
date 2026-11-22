import { render, screen } from "@testing-library/react";
import type { TxRowDto } from "@mr-krabs/api-client";

import { TransactionsTable } from "./TransactionsTable";

const row: TxRowDto = {
  id: "t1",
  date: "2026-11-20",
  kind: "expense",
  description: "Feria Prueba",
  accountId: "a1",
  accountName: "Corriente Prueba",
  categoryId: "c1",
  categoryName: "Supermercado",
  counterAccountId: null,
  counterAccountName: null,
  amount: { minor: "1", currency: "CLP", display: "−$1.000" },
  status: "pending",
};

it("declares table roles explicitly, since grid/block display can drop them in WebKit (F11)", () => {
  render(
    <TransactionsTable rows={[row]} hasMore={false} loadingMore={false} onLoadMore={() => {}} onOpen={() => {}} />,
  );
  expect(screen.getByRole("table")).toHaveAttribute("role", "table");
  for (const group of screen.getAllByRole("rowgroup")) expect(group).toHaveAttribute("role", "rowgroup");
  for (const r of screen.getAllByRole("row")) expect(r).toHaveAttribute("role", "row");
  for (const h of screen.getAllByRole("columnheader")) expect(h).toHaveAttribute("role", "columnheader");
  for (const c of screen.getAllByRole("cell")) expect(c).toHaveAttribute("role", "cell");
});
