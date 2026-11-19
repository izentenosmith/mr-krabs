import { render, screen } from "@testing-library/react";

import { HealthCard } from "./HealthCard";

describe("HealthCard", () => {
  it("shows the core's health", () => {
    render(
      <HealthCard
        transport="http"
        health={{ status: "ok", version: "0.1.0", schemaVersion: 1, latestSchemaVersion: 1, dbPath: "/data/mrkrabs.sqlite" }}
      />,
    );
    expect(screen.getByText("En línea")).toBeInTheDocument();
    expect(screen.getByText("v1 / v1")).toBeInTheDocument();
    expect(screen.getByText("/data/mrkrabs.sqlite")).toBeInTheDocument();
    expect(screen.getByText(/HTTP \(devserver\)/)).toBeInTheDocument();
  });

  it("shows errors", () => {
    render(<HealthCard transport="tauri" error="connection refused" />);
    expect(screen.getByRole("alert")).toHaveTextContent("connection refused");
    expect(screen.getByText("Sin conexión")).toBeInTheDocument();
  });
});
