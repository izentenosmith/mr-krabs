import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory } from "@tanstack/react-router";
import { render, screen } from "@testing-library/react";

import App from "./App";

vi.mock("@mr-krabs/api-client", () => ({
  transport: "http",
  api: {
    app: {
      health: vi.fn().mockResolvedValue({
        status: "ok",
        version: "0.1.0",
        schemaVersion: 1,
        latestSchemaVersion: 1,
        dbPath: null,
      }),
      bootstrap: vi.fn().mockResolvedValue({
        household: { id: "h-1", name: "Casa Prueba", baseCurrency: "CLP", locale: "es-CL" },
        members: [],
        me: null,
        settings: null,
        schemaVersion: 1,
      }),
    },
    account: { list: vi.fn().mockResolvedValue([]) },
    money: { parse: vi.fn() },
  },
}));

describe("App", () => {
  it("renders the core health from the api client inside the app shell", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <App history={createMemoryHistory({ initialEntries: ["/"] })} />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Mr Krabs")).toBeInTheDocument();
    expect(await screen.findByText("En línea")).toBeInTheDocument();
    expect(screen.getByText("en memoria")).toBeInTheDocument();
  });
});
