// Failure paths of the page that the acceptance suite doesn't name one by one:
// opening a row, loading more rows, and a first page that fails for another reason.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { ApiError, type TxRowDto } from "@mr-krabs/api-client";

import { Toaster } from "@/components/ui/sonner";
import { shouldRetry } from "@/lib/api-errors";

import { TransactionsPage } from "./TransactionsPage";

const api = vi.hoisted(() => ({
  account: { list: vi.fn() },
  tx: { list: vi.fn(), get: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn() },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  api,
}));

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

const accounts = [
  {
    account: {
      id: "a1",
      name: "Corriente Prueba",
      kind: "checking",
      currency: "CLP",
      scope: "household",
      ownerMemberId: null,
      visibility: "shared_full",
      institution: null,
      numberMask: null,
      isArchived: false,
      sortOrder: 0,
    },
    balance: { minor: "1", currency: "CLP", display: "$1" },
  },
];

const detail = (id: string, description: string) => ({
  id,
  kind: "transfer" as const,
  date: "2026-11-20",
  description,
  memo: "",
  accountId: "a1",
  categoryId: null,
  toAccountId: "a1",
  amount: { minor: "1", currency: "CLP", display: "$1" },
  status: "pending" as const,
  deleted: false,
});

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

let lastClient: QueryClient;

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  lastClient = client;
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
      await screen.findByText("No encontramos lo que buscabas. Recarga e intenta de nuevo."),
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

describe("TransactionsPage dialog identity", () => {
  beforeEach(() => {
    api.account.list.mockResolvedValue(accounts);
    api.tx.update.mockResolvedValue(detail("t2", "Otra Prueba"));
    api.tx.list.mockResolvedValue({
      items: [row, { ...row, id: "t2", description: "Otra Prueba" }],
      nextCursor: null,
    });
  });

  it("a late tx_get never turns an open create form into an edit (F1)", async () => {
    const slow = deferred<ReturnType<typeof detail>>();
    api.tx.get.mockReturnValue(slow.promise);
    api.tx.create.mockResolvedValue(detail("t9", "x"));
    renderPage();
    fireEvent.click(await screen.findByRole("row", { name: /Feria Prueba/ }));
    fireEvent.click(screen.getByRole("button", { name: "Nuevo" }));
    const d = within(await screen.findByRole("dialog", { name: "Nueva transacción" }));
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "777" } });
    slow.resolve(detail("t1", "Feria Prueba"));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByRole("dialog", { name: "Nueva transacción" })).toBeInTheDocument();
    expect(d.getByLabelText("Monto")).toHaveValue("777");
    expect(api.tx.update).not.toHaveBeenCalled();
  });

  it("the most recently clicked row wins when tx_get answers out of order (F1)", async () => {
    const slowA = deferred<ReturnType<typeof detail>>();
    api.tx.get.mockImplementation((id: string) =>
      id === "t1" ? slowA.promise : Promise.resolve(detail("t2", "Otra Prueba")),
    );
    renderPage();
    fireEvent.click(await screen.findByRole("row", { name: /Feria Prueba/ }));
    fireEvent.click(screen.getByRole("row", { name: /Otra Prueba/ }));
    const d = within(await screen.findByRole("dialog", { name: "Editar transacción" }));
    expect(d.getByLabelText("Descripción")).toHaveValue("Otra Prueba");
    slowA.resolve(detail("t1", "Feria Prueba"));
    await new Promise((r) => setTimeout(r, 20));
    expect(d.getByLabelText("Descripción")).toHaveValue("Otra Prueba");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.tx.update).toHaveBeenCalled());
    expect(api.tx.update).toHaveBeenCalledWith("t2", expect.objectContaining({ description: "Otra Prueba" }));
  });

  it("a save that finishes after its dialog closed leaves a newer dialog open (F12)", async () => {
    const slow = deferred<ReturnType<typeof detail>>();
    api.tx.create.mockReturnValue(slow.promise);
    renderPage();
    fireEvent.click(await screen.findByRole("button", { name: "Nuevo" }));
    let d = within(await screen.findByRole("dialog", { name: "Nueva transacción" }));
    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    fireEvent.change(d.getByLabelText("Desde"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Hacia"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "5" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.tx.create).toHaveBeenCalled());
    fireEvent.click(d.getByRole("button", { name: "Cancelar" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Nuevo" }));
    d = within(await screen.findByRole("dialog", { name: "Nueva transacción" }));
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "8" } });
    slow.resolve(detail("t9", "x"));
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByRole("dialog", { name: "Nueva transacción" })).toBeInTheDocument();
    expect(d.getByLabelText("Monto")).toHaveValue("8");
  });
});

describe("TransactionsPage writes", () => {
  beforeEach(() => {
    api.account.list.mockResolvedValue(accounts);
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    api.tx.create.mockResolvedValue(detail("t9", "x"));
  });

  it("a write refreshes the accounts queries the sidebar shares (F27)", async () => {
    renderPage();
    await screen.findByRole("row", { name: /Feria Prueba/ });
    await waitFor(() => expect(api.account.list).toHaveBeenCalledTimes(1));
    expect(api.account.list).toHaveBeenCalledWith({ view: "combined", includeArchived: false });
    // The sidebar's archived view (S6) shares the ["accounts"] prefix.
    const archivedKey = ["accounts", "list", { includeArchived: true }];
    lastClient.setQueryData(archivedKey, []);
    fireEvent.click(screen.getByRole("button", { name: "Nuevo" }));
    const d = within(await screen.findByRole("dialog", { name: "Nueva transacción" }));
    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    fireEvent.change(d.getByLabelText("Desde"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Hacia"), { target: { value: "a1" } });
    fireEvent.change(d.getByLabelText("Monto"), { target: { value: "5" } });
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.account.list).toHaveBeenCalledTimes(2));
    expect(lastClient.getQueryState(archivedKey)?.isInvalidated).toBe(true);
  });
});

describe("TransactionsPage accounts", () => {
  it("a failing account_list shows its code's copy with a retry (F14)", async () => {
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    api.account.list.mockRejectedValueOnce(new ApiError("TRANSPORT", "developer message"));
    api.account.list.mockResolvedValue(accounts);
    renderPage();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "No pudimos conectar con el núcleo. Intenta de nuevo.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(await screen.findByRole("option", { name: "Corriente Prueba" })).toBeInTheDocument();
  });
});

describe("TransactionsPage paging", () => {
  afterEach(() => {
    Object.defineProperty(window, "scrollY", { value: 0, configurable: true });
  });

  it("a full first page loads the next one only after scrolling near its end (F6, F7)", async () => {
    const pageOf = (prefix: string) =>
      Array.from({ length: 100 }, (_, i) => ({ ...row, id: `${prefix}${i}`, description: `Mov ${prefix}${i}` }));
    api.tx.list.mockImplementation(async (q: { cursor?: string | null }) =>
      q.cursor ? { items: pageOf("b"), nextCursor: null } : { items: pageOf("a"), nextCursor: "next" },
    );
    renderPage();
    await screen.findByRole("row", { name: /Mov a0\b/ });
    await new Promise((r) => setTimeout(r, 50));
    expect(api.tx.list).toHaveBeenCalledTimes(1);

    Object.defineProperty(window, "scrollY", { value: 100 * 36, configurable: true });
    fireEvent.scroll(window);
    await waitFor(() => expect(api.tx.list).toHaveBeenCalledTimes(2));
    expect(api.tx.list).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: "next" }));
  });
});

describe("TransactionsPage filter", () => {
  it("re-queries tx_list when accountId changes, and without it when cleared (F4)", async () => {
    api.account.list.mockResolvedValue(accounts);
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const ui = (accountId?: string) => (
      <QueryClientProvider client={client}>
        <TransactionsPage
          accountId={accountId}
          onAccountIdChange={vi.fn()}
          categories={{ expense: [], income: [] }}
        />
      </QueryClientProvider>
    );
    const { rerender } = render(ui("a1"));
    await waitFor(() =>
      expect(api.tx.list).toHaveBeenLastCalledWith(expect.objectContaining({ accountId: "a1" })),
    );
    rerender(ui("a2"));
    await waitFor(() =>
      expect(api.tx.list).toHaveBeenLastCalledWith(expect.objectContaining({ accountId: "a2" })),
    );
    rerender(ui(undefined));
    await waitFor(() => expect(api.tx.list).toHaveBeenCalledTimes(3));
    expect(api.tx.list.mock.calls[2]?.[0]?.accountId).toBeUndefined();
  });
});

describe("TransactionsPage loading", () => {
  it("shows Cargando… with aria-busy while the first page loads (F19)", async () => {
    api.account.list.mockResolvedValue([]);
    api.tx.list.mockReturnValue(new Promise(() => {}));
    renderPage();
    const loading = await screen.findByText("Cargando…");
    expect(loading).toHaveAttribute("aria-busy", "true");
    expect(screen.queryByText("Aún no hay transacciones")).not.toBeInTheDocument();
  });
});

describe("TransactionsPage follow-ups", () => {
  it("a failed tx_get for an abandoned opening shows no toast (F44)", async () => {
    api.account.list.mockResolvedValue(accounts);
    api.tx.list.mockResolvedValue({
      items: [row, { ...row, id: "t2", description: "Otra Prueba" }],
      nextCursor: null,
    });
    let failA!: (e: unknown) => void;
    api.tx.get.mockImplementation((id: string) =>
      id === "t1"
        ? new Promise((_, reject) => (failA = reject))
        : Promise.resolve(detail("t2", "Otra Prueba")),
    );
    renderPage();
    fireEvent.click(await screen.findByRole("row", { name: /Feria Prueba/ }));
    fireEvent.click(screen.getByRole("row", { name: /Otra Prueba/ }));
    await screen.findByRole("dialog", { name: "Editar transacción" });
    failA(new ApiError("NOT_FOUND", "developer message"));
    await new Promise((r) => setTimeout(r, 30));
    expect(screen.queryByText(/No encontramos lo que buscabas/)).not.toBeInTheDocument();
  });

  it("when both loads fail, one alert retries both (F43)", async () => {
    api.account.list.mockRejectedValueOnce(new ApiError("TRANSPORT", "dev"));
    api.account.list.mockResolvedValue(accounts);
    api.tx.list.mockRejectedValueOnce(new ApiError("TRANSPORT", "dev"));
    api.tx.list.mockResolvedValue({ items: [row], nextCursor: null });
    renderPage();
    await screen.findByRole("alert");
    await waitFor(() => expect(screen.getAllByRole("alert")).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: "Reintentar" }));
    expect(await screen.findByRole("row", { name: /Feria Prueba/ })).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });
});

describe("TransactionsPage with the app's retry policy", () => {
  it("NOT_IMPLEMENTED shows the not-yet-available state at once, without retries (F2)", async () => {
    api.account.list.mockResolvedValue([]);
    api.tx.list.mockRejectedValue(new ApiError("NOT_IMPLEMENTED", "tx_list"));
    // The same QueryClient options as main.tsx.
    const client = new QueryClient({ defaultOptions: { queries: { retry: shouldRetry } } });
    render(
      <QueryClientProvider client={client}>
        <TransactionsPage onAccountIdChange={vi.fn()} categories={{ expense: [], income: [] }} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Aún no disponible", {}, { timeout: 500 })).toBeInTheDocument();
    expect(api.tx.list).toHaveBeenCalledTimes(1);
  });
});
