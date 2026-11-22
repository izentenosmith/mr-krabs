// Failure paths of the page that the acceptance suite doesn't name one by one:
// opening a row, loading more rows, and a first page that fails for another reason.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import { ApiError, type TxRowDto } from "@mr-krabs/api-client";

import { Toaster } from "@/components/ui/sonner";

import { TransactionsPage } from "./TransactionsPage";

const api = vi.hoisted(() => ({
  account: { list: vi.fn() },
  tx: { list: vi.fn(), get: vi.fn() },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  api,
}));

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

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <TransactionsPage onAccountIdChange={vi.fn()} categories={{ expense: [], income: [] }} />
      <Toaster />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  api.account.list.mockResolvedValue([]);
});
afterEach(() => toast.dismiss());

describe("TransactionsPage failures", () => {
  it("a row whose tx_get fails shows an error toast and opens no dialog", async () => {
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    api.tx.get.mockRejectedValue(new ApiError("NOT_FOUND", "developer message"));
    renderPage();
    fireEvent.click(await screen.findByRole("row", { name: /Feria Prueba/ }));
    expect(
      await screen.findByText("No encontramos este registro. Actualiza la lista e intenta de nuevo."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("a failing next page shows an error toast and keeps the rows already loaded", async () => {
    api.tx.list.mockImplementation(async (q: { cursor?: string | null }) => {
      if (q.cursor) throw new ApiError("TRANSPORT", "developer message");
      return { items: [row], nextCursor: "next" };
    });
    renderPage();
    expect(
      await screen.findByText("No pudimos conectar con el núcleo. Intenta de nuevo."),
    ).toBeInTheDocument();
    expect(screen.getByRole("row", { name: /Feria Prueba/ })).toBeInTheDocument();
  });

  it("a first page failing with another code shows its copy and a retry", async () => {
    api.tx.list.mockRejectedValueOnce(new ApiError("INTERNAL", "developer message"));
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent("Algo salió mal. Intenta de nuevo.");
    expect(screen.queryByText("Aún no hay transacciones")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    await waitFor(() => expect(screen.getByRole("row", { name: /Feria Prueba/ })).toBeInTheDocument());
  });
});
