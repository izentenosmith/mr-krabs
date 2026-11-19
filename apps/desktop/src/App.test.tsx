import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
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
    },
    money: { parse: vi.fn() },
  },
}));

describe("App", () => {
  it("renders the core health from the api client", async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <App />
      </QueryClientProvider>,
    );
    expect(screen.getByRole("heading", { name: "Mr Krabs" })).toBeInTheDocument();
    expect(await screen.findByText("En línea")).toBeInTheDocument();
    expect(screen.getByText("en memoria")).toBeInTheDocument();
  });
});
