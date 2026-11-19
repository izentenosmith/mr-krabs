import type { esCL } from "./es-CL";

type Messages<T> = { [K in keyof T]: T[K] extends string ? string : Messages<T[K]> };

export const en: Messages<typeof esCL> = {
  app: {
    tagline: "Personal and household finances · local-first",
  },
  health: {
    title: "Core status",
    connecting: "Connecting…",
    online: "Online",
    offline: "Offline",
    transport: "Transport: {{name}}",
    transportTauri: "Tauri IPC",
    transportHttp: "HTTP (devserver)",
    version: "Version",
    schema: "Schema",
    database: "Database",
    inMemory: "in memory",
  },
  money: {
    title: "CLP amounts",
    description: "Rust parses and formats; React only displays.",
    amount: "Amount",
    parse: "Parse",
    minorUnits: "({{minor}} {{currency}} in minor units)",
    required: "Enter an amount",
  },
};
