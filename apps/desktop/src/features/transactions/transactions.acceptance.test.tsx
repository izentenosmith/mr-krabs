// Acceptance tests for S7 (ui-transactions), one per criterion of `.workflow/brief.md`.
// They act only through what a user sees (labels, roles, text) on `TransactionsPage`, and
// through `App` for the route; `@mr-krabs/api-client` is mocked at its public `api`.
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ApiError } from "@mr-krabs/api-client";
import type {
  AccountSummaryDto,
  Page,
  TxDetailDto,
  TxKind,
  TxRowDto,
} from "@mr-krabs/api-client";

import App from "@/App";
import { Toaster } from "@/components/ui/sonner";
import i18n from "@/i18n";

import { TransactionsPage, type CategoryOptions } from "./TransactionsPage";

const api = vi.hoisted(() => ({
  app: {
    health: vi.fn(),
    bootstrap: vi.fn(),
  },
  money: { parse: vi.fn() },
  settings: { get: vi.fn() },
  household: { create: vi.fn() },
  account: {
    list: vi.fn(),
    get: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    archive: vi.fn(),
  },
  tx: {
    create: vi.fn(),
    get: vi.fn(),
    list: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    restore: vi.fn(),
  },
}));

vi.mock("@mr-krabs/api-client", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@mr-krabs/api-client")>()),
  api,
  transport: "http",
}));

// ---------- fixtures (synthetic) ----------

function summary(id: string, name: string, currency = "CLP"): AccountSummaryDto {
  return {
    account: {
      id,
      name,
      kind: "checking",
      currency,
      scope: "household",
      ownerMemberId: null,
      visibility: "shared_full",
      institution: null,
      numberMask: null,
      isArchived: false,
      sortOrder: 0,
    },
    balance: { minor: "1", currency, display: "$1" },
  };
}

const accounts = [
  summary("a1", "Corriente Prueba"),
  summary("a2", "Ahorro Prueba"),
  summary("a3", "Dólares Prueba", "USD"),
];

const categories: CategoryOptions = {
  expense: [
    { id: "c1", name: "Supermercado" },
    { id: "c2", name: "Transporte" },
  ],
  income: [{ id: "c9", name: "Sueldo" }],
};

function row(id: string, kind: TxKind, extra: Partial<TxRowDto> = {}): TxRowDto {
  return {
    id,
    date: "2026-11-20",
    kind,
    description: `Movimiento ${id}`,
    accountId: "a1",
    accountName: "Corriente Prueba",
    categoryId: null,
    categoryName: null,
    counterAccountId: null,
    counterAccountName: null,
    // `minor` deliberately disagrees with `display`: the UI must show `display` verbatim.
    amount: { minor: "7", currency: "CLP", display: "$0" },
    status: "pending",
    ...extra,
  };
}

const expenseRow = row("t1", "expense", {
  description: "Supermercado Prueba",
  categoryId: "c1",
  categoryName: "Supermercado",
  amount: { minor: "7", currency: "CLP", display: "−$45.990" },
  status: "cleared",
});

const transferRow = row("t2", "transfer", {
  description: "Ahorro mensual",
  counterAccountId: "a2",
  counterAccountName: "Ahorro Prueba",
  amount: { minor: "7", currency: "CLP", display: "$100.000" },
});

const openingRow = row("t0", "opening_balance", {
  description: "",
  amount: { minor: "7", currency: "CLP", display: "+$1.000.000" },
  status: "reconciled",
});

const expenseDetail: TxDetailDto = {
  id: "t1",
  kind: "expense",
  date: "2026-11-20",
  description: "Supermercado Prueba",
  memo: "nota previa",
  accountId: "a1",
  categoryId: "c1",
  toAccountId: null,
  amount: { minor: "4599000", currency: "CLP", display: "$45.990" },
  status: "cleared",
  deleted: false,
};

function page(items: TxRowDto[], nextCursor: string | null = null): Page<TxRowDto> {
  return { items, nextCursor };
}

// ---------- harness ----------

function client() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

function renderPage(props: { accountId?: string; onAccountIdChange?: (id?: string) => void } = {}) {
  const onAccountIdChange = props.onAccountIdChange ?? vi.fn();
  render(
    <QueryClientProvider client={client()}>
      <TransactionsPage
        accountId={props.accountId}
        onAccountIdChange={onAccountIdChange}
        categories={categories}
      />
      <Toaster />
    </QueryClientProvider>,
  );
  return { onAccountIdChange };
}

async function openNew() {
  fireEvent.click(await screen.findByRole("button", { name: "Nuevo" }));
  return within(await screen.findByRole("dialog", { name: "Nueva transacción" }));
}

async function openRow(description: string) {
  fireEvent.click(await screen.findByRole("row", { name: new RegExp(description) }));
  return within(await screen.findByRole("dialog", { name: "Editar transacción" }));
}

function fill(d: ReturnType<typeof within>, label: string, value: string) {
  fireEvent.change(d.getByLabelText(label), { target: { value } });
}

beforeEach(() => {
  vi.clearAllMocks();
  api.account.list.mockResolvedValue(accounts);
  api.tx.list.mockResolvedValue(page([expenseRow, transferRow]));
  api.tx.create.mockResolvedValue(expenseDetail);
  api.tx.update.mockResolvedValue(expenseDetail);
  api.tx.get.mockResolvedValue(expenseDetail);
  api.tx.delete.mockResolvedValue(null);
  api.tx.restore.mockResolvedValue(null);
});

afterEach(async () => {
  await i18n.changeLanguage("es-CL");
});

// ---------- criteria ----------

describe("S7 transactions — acceptance", () => {
  it("tracer: lists tx_list rows with the strings the core sent", async () => {
    renderPage();

    const expense = within(await screen.findByRole("row", { name: /Supermercado Prueba/ }));
    expect(expense.getByText("−$45.990")).toBeInTheDocument();
    expect(expense.getByText("Supermercado")).toBeInTheDocument();
    expect(expense.getByText("Corriente Prueba")).toBeInTheDocument();
    expect(expense.getByText("Liquidado")).toBeInTheDocument();

    const transfer = within(screen.getByRole("row", { name: /Ahorro mensual/ }));
    expect(transfer.getByText("$100.000")).toBeInTheDocument();
    expect(transfer.getByText(/Ahorro Prueba/)).toBeInTheDocument();
    expect(transfer.getByText("Pendiente")).toBeInTheDocument();

    expect(api.tx.list).toHaveBeenCalledWith(
      expect.objectContaining({ view: "combined", limit: 100 }),
    );
    expect(api.tx.list.mock.calls[0]?.[0]?.cursor ?? null).toBeNull();
    expect(screen.queryByText("7")).not.toBeInTheDocument();
  });

  it("creating an expense sends NewTxInput with the amount as typed and refreshes the list", async () => {
    renderPage();
    const d = await openNew();
    expect(d.getByRole("radio", { name: "Gasto" })).toBeChecked();
    fill(d, "Cuenta", "a1");
    fill(d, "Categoría", "c1");
    fill(d, "Monto", "45.990");
    fill(d, "Fecha", "2026-11-20");
    fill(d, "Descripción", "Supermercado Prueba");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    await waitFor(() => expect(api.tx.create).toHaveBeenCalledTimes(1));
    expect(api.tx.create).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "expense",
        accountId: "a1",
        categoryId: "c1",
        amount: { amount: "45.990", currency: "CLP" },
        date: "2026-11-20",
        description: "Supermercado Prueba",
      }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(api.tx.list.mock.calls.length).toBeGreaterThanOrEqual(2));
  });

  it("creating an income and a transfer sends their NewTxInput shapes", async () => {
    renderPage();
    let d = await openNew();
    fireEvent.click(d.getByRole("radio", { name: "Ingreso" }));
    fill(d, "Cuenta", "a1");
    fill(d, "Categoría", "c9");
    fill(d, "Monto", "1.500.000");
    fill(d, "Fecha", "2026-11-01");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.tx.create).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "income",
          accountId: "a1",
          categoryId: "c9",
          amount: { amount: "1.500.000", currency: "CLP" },
          date: "2026-11-01",
        }),
      ),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    d = await openNew();
    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    expect(d.queryByLabelText("Categoría")).not.toBeInTheDocument();
    fill(d, "Desde", "a3");
    fill(d, "Hacia", "a2");
    fill(d, "Monto", "100,50");
    fill(d, "Fecha", "2026-11-02");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.tx.create).toHaveBeenLastCalledWith(
        expect.objectContaining({
          type: "transfer",
          fromAccountId: "a3",
          toAccountId: "a2",
          amount: { amount: "100,50", currency: "USD" },
          date: "2026-11-02",
        }),
      ),
    );
    expect(api.tx.create.mock.calls[1]?.[0]).not.toHaveProperty("categoryId");
  });

  it("VALIDATION fields show under the matching field; unknown keys get the generic text", async () => {
    api.tx.create.mockRejectedValue(
      new ApiError("VALIDATION", "developer message", {
        "amount.amount": "errors.field.invalidAmount",
        description: "errors.field.notAKey",
      }),
    );
    renderPage();
    const d = await openNew();
    fill(d, "Cuenta", "a1");
    fill(d, "Categoría", "c1");
    fill(d, "Monto", "abc");
    fill(d, "Descripción", "x");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    expect(await d.findByText("Monto no válido")).toBeInTheDocument();
    expect(d.getByLabelText("Monto")).toHaveAttribute("aria-invalid", "true");
    expect(d.getByText("Revisa este campo")).toBeInTheDocument();
    expect(d.getByLabelText("Monto")).toHaveValue("abc");
    expect(d.queryByText("developer message")).not.toBeInTheDocument();
  });

  it.each([
    ["CURRENCY_MISMATCH", "La moneda no coincide con la de la cuenta."],
    ["UNBALANCED_ENTRY", "Los montos no cuadran. Revisa la transacción."],
    ["CONFLICT", "Esto entra en conflicto con datos existentes."],
  ] as const)("%s without fields shows its own text in the dialog", async (code, copy) => {
    api.tx.create.mockRejectedValue(new ApiError(code, "developer message"));
    renderPage();
    const d = await openNew();
    fill(d, "Cuenta", "a1");
    fill(d, "Categoría", "c1");
    fill(d, "Monto", "10");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    expect(await d.findByRole("alert")).toHaveTextContent(copy);
    expect(d.queryByText("developer message")).not.toBeInTheDocument();
    expect(d.queryByText(code)).not.toBeInTheDocument();
    expect(d.getByLabelText("Monto")).toHaveValue("10");
  });

  it("editing loads tx_get into the dialog and saves the full input with tx_update", async () => {
    renderPage();
    const d = await openRow("Supermercado Prueba");
    expect(api.tx.get).toHaveBeenCalledWith("t1");
    expect(d.getByRole("radio", { name: "Gasto" })).toBeChecked();
    expect(d.getByLabelText("Cuenta")).toHaveValue("a1");
    expect(d.getByLabelText("Categoría")).toHaveValue("c1");
    expect(d.getByLabelText("Monto")).toHaveValue("$45.990");
    expect(d.getByLabelText("Fecha")).toHaveValue("2026-11-20");
    expect(d.getByLabelText("Nota")).toHaveValue("nota previa");

    fill(d, "Descripción", "Feria");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(api.tx.update).toHaveBeenCalledWith("t1", {
        type: "expense",
        accountId: "a1",
        categoryId: "c1",
        amount: { amount: "$45.990", currency: "CLP" },
        date: "2026-11-20",
        description: "Feria",
        memo: "nota previa",
      }),
    );
    expect(api.tx.create).not.toHaveBeenCalled();
  });

  it("delete then undo calls tx_delete then tx_restore and refreshes the list", async () => {
    renderPage();
    const d = await openRow("Supermercado Prueba");
    const listCalls = api.tx.list.mock.calls.length;
    fireEvent.click(d.getByRole("button", { name: "Eliminar" }));

    await waitFor(() => expect(api.tx.delete).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(await screen.findByText("Transacción eliminada")).toBeInTheDocument();
    await waitFor(() => expect(api.tx.list.mock.calls.length).toBeGreaterThan(listCalls));

    const afterDelete = api.tx.list.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "Deshacer" }));
    await waitFor(() => expect(api.tx.restore).toHaveBeenCalledWith("t1"));
    await waitFor(() => expect(api.tx.list.mock.calls.length).toBeGreaterThan(afterDelete));
    expect(api.tx.delete.mock.invocationCallOrder[0] ?? Infinity).toBeLessThan(
      api.tx.restore.mock.invocationCallOrder[0] ?? -Infinity,
    );
  });

  it("keyset paging passes nextCursor unchanged and stops after a null cursor", async () => {
    api.tx.list.mockImplementation(async (q: { cursor?: string | null }) =>
      q.cursor === "opaque:cursor/2" ? page([row("t3", "expense")]) : page([expenseRow], "opaque:cursor/2"),
    );
    renderPage({ accountId: "a1" });

    expect(await screen.findByRole("row", { name: /Movimiento t3/ })).toBeInTheDocument();
    expect(api.tx.list).toHaveBeenLastCalledWith(
      expect.objectContaining({
        view: "combined",
        accountId: "a1",
        limit: 100,
        cursor: "opaque:cursor/2",
      }),
    );
    await new Promise((r) => setTimeout(r, 50));
    expect(api.tx.list).toHaveBeenCalledTimes(2);
  });

  it("the account filter queries by accountId and reports the choice", async () => {
    const { onAccountIdChange } = renderPage({ accountId: "a2" });
    await waitFor(() =>
      expect(api.tx.list).toHaveBeenCalledWith(expect.objectContaining({ accountId: "a2" })),
    );
    const filter = await screen.findByLabelText("Filtrar por cuenta");
    await within(filter).findByRole("option", { name: "Corriente Prueba" });
    fireEvent.change(filter, { target: { value: "a1" } });
    expect(onAccountIdChange).toHaveBeenLastCalledWith("a1");
    fireEvent.change(filter, { target: { value: "" } });
    expect(onAccountIdChange).toHaveBeenLastCalledWith(undefined);
    expect(within(filter).getByRole("option", { name: "Todas las cuentas" })).toBeInTheDocument();
  });

  it("the /transactions route reads accountId from the URL", async () => {
    api.app.health.mockResolvedValue({
      status: "ok",
      version: "0",
      schemaVersion: 3,
      latestSchemaVersion: 3,
      dbPath: null,
    });
    api.app.bootstrap.mockResolvedValue({
      household: { id: "h1", name: "Casa Prueba", baseCurrency: "CLP", locale: "es-CL" },
      members: [{ id: "m1", displayName: "Persona Prueba", color: null }],
      me: { id: "m1", displayName: "Persona Prueba", color: null },
      settings: { baseCurrency: "CLP", locale: "es-CL", budgetMode: "at_purchase" },
      schemaVersion: 3,
    });
    render(
      <QueryClientProvider client={client()}>
        <App history={createMemoryHistory({ initialEntries: ["/transactions?accountId=a2"] })} />
      </QueryClientProvider>,
    );
    expect(await screen.findByRole("heading", { name: "Transacciones" })).toBeInTheDocument();
    await waitFor(() =>
      expect(api.tx.list).toHaveBeenCalledWith(expect.objectContaining({ accountId: "a2" })),
    );
  });

  it("edge: NOT_IMPLEMENTED shows the not-yet-available state and disables Nuevo", async () => {
    api.tx.list.mockRejectedValue(new ApiError("NOT_IMPLEMENTED", "tx_list"));
    renderPage();
    expect(await screen.findByText("Aún no disponible")).toBeInTheDocument();
    expect(screen.getByText("Esta función llega en una próxima versión.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Nuevo" })).toBeDisabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText("NOT_IMPLEMENTED")).not.toBeInTheDocument();
  });

  it("edge: an empty first page shows the empty state", async () => {
    api.tx.list.mockResolvedValue(page([]));
    renderPage();
    expect(await screen.findByText("Aún no hay transacciones")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Nuevo" })).toBeEnabled();
  });

  it("edge: missing required fields block tx_create with a message per field", async () => {
    renderPage();
    const d = await openNew();
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(d.getAllByText("Este campo es obligatorio")).toHaveLength(3), // cuenta, categoría, monto
    );
    expect(d.getByLabelText("Cuenta")).toHaveAttribute("aria-invalid", "true");
    expect(d.getByLabelText("Categoría")).toHaveAttribute("aria-invalid", "true");
    expect(d.getByLabelText("Monto")).toHaveAttribute("aria-invalid", "true");
    expect(api.tx.create).not.toHaveBeenCalled();

    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(d.getByLabelText("Desde")).toHaveAttribute("aria-invalid", "true"));
    expect(d.getByLabelText("Hacia")).toHaveAttribute("aria-invalid", "true");
    expect(api.tx.create).not.toHaveBeenCalled();
  });

  it("edge: delete and restore failures show an error toast with the code's text", async () => {
    api.tx.delete.mockRejectedValueOnce(new ApiError("CONFLICT", "developer message"));
    renderPage();
    let d = await openRow("Supermercado Prueba");
    fireEvent.click(d.getByRole("button", { name: "Eliminar" }));
    expect(await screen.findByText("Esto entra en conflicto con datos existentes.")).toBeInTheDocument();
    expect(screen.queryByText("Transacción eliminada")).not.toBeInTheDocument();

    api.tx.restore.mockRejectedValueOnce(new ApiError("INTERNAL", "developer message"));
    d = within(await screen.findByRole("dialog", { name: "Editar transacción" }));
    fireEvent.click(d.getByRole("button", { name: "Eliminar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Deshacer" }));
    expect(await screen.findByText("Algo salió mal. Intenta de nuevo.")).toBeInTheDocument();
    expect(screen.queryByText("developer message")).not.toBeInTheDocument();
  });

  it("edge: a 10 000-row list renders only a window of rows", async () => {
    const many = Array.from({ length: 10_000 }, (_, i) => row(`r${i}`, "expense"));
    api.tx.list.mockResolvedValue(page(many));
    renderPage();
    expect(await screen.findByRole("row", { name: /Movimiento r0\b/ })).toBeInTheDocument();
    const rows = screen.getAllByRole("row");
    expect(rows.length).toBeGreaterThan(1);
    expect(rows.length).toBeLessThan(200);
    expect(screen.queryByRole("row", { name: /Movimiento r9999\b/ })).not.toBeInTheDocument();
  });

  it("edge: opening-balance rows show as Saldo inicial and don't open the dialog", async () => {
    api.tx.list.mockResolvedValue(page([openingRow, expenseRow]));
    renderPage();
    const opening = await screen.findByRole("row", { name: /Saldo inicial/ });
    expect(within(opening).getByText("+$1.000.000")).toBeInTheDocument();
    fireEvent.click(opening);
    await new Promise((r) => setTimeout(r, 20));
    expect(api.tx.get).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("copy: the page and dialog render in English when the language is en", async () => {
    await i18n.changeLanguage("en");
    renderPage();
    expect(await screen.findByRole("heading", { name: "Transactions" })).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "New" }));
    const d = within(await screen.findByRole("dialog", { name: "New transaction" }));
    expect(d.getByRole("radio", { name: "Expense" })).toBeInTheDocument();
    expect(d.getByLabelText("Amount")).toBeInTheDocument();
    expect(d.getByRole("button", { name: "Save" })).toBeInTheDocument();
  });

  it("journey: record a transfer, see it listed, delete it and undo", async () => {
    let rows: TxRowDto[] = [];
    api.tx.list.mockImplementation(async () => page(rows));
    api.tx.create.mockImplementation(async () => {
      rows = [transferRow];
      return { ...expenseDetail, id: "t2", kind: "transfer" };
    });
    api.tx.get.mockResolvedValue({
      ...expenseDetail,
      id: "t2",
      kind: "transfer",
      categoryId: null,
      toAccountId: "a2",
      description: "Ahorro mensual",
      memo: "",
      amount: { minor: "1", currency: "CLP", display: "$100.000" },
    });
    api.tx.delete.mockImplementation(async () => {
      rows = [];
      return null;
    });
    api.tx.restore.mockImplementation(async () => {
      rows = [transferRow];
      return null;
    });
    renderPage();

    expect(await screen.findByText("Aún no hay transacciones")).toBeInTheDocument();
    const d = await openNew();
    fireEvent.click(d.getByRole("radio", { name: "Transferencia" }));
    fill(d, "Desde", "a1");
    fill(d, "Hacia", "a2");
    fill(d, "Monto", "100.000");
    fill(d, "Descripción", "Ahorro mensual");
    fireEvent.click(d.getByRole("button", { name: "Guardar" }));

    expect(await screen.findByRole("row", { name: /Ahorro mensual/ })).toBeInTheDocument();
    const e = await openRow("Ahorro mensual");
    expect(e.getByRole("radio", { name: "Transferencia" })).toBeChecked();
    expect(e.getByLabelText("Hacia")).toHaveValue("a2");
    fireEvent.click(e.getByRole("button", { name: "Eliminar" }));
    expect(await screen.findByText("Aún no hay transacciones")).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Deshacer" }));
    expect(await screen.findByRole("row", { name: /Ahorro mensual/ })).toBeInTheDocument();
  });
});
