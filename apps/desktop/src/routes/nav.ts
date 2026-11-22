import type { LucideIcon } from "lucide-react";
import type { LinkProps } from "@tanstack/react-router";

import type { esCL } from "@/i18n/es-CL";

export interface NavItem {
  to: LinkProps["to"];
  /** Key under `shell` in the i18n messages. */
  labelKey: `shell.${keyof (typeof esCL)["shell"]}`;
  icon: LucideIcon;
}

/**
 * Sidebar navigation, in MASTER §5 order. Screens add themselves here with their route
 * (ui-transactions adds "Transacciones").
 */
export const navItems: NavItem[] = [];
