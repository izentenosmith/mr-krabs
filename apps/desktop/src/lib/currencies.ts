/** Currencies built into M1 (CLAUDE.md: CLP default; USD, EUR, UF/CLF built in). */
export const CURRENCIES = ["CLP", "USD", "EUR", "CLF"] as const;
export type Currency = (typeof CURRENCIES)[number];
