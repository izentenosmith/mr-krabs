import i18n from "i18next";
import type { Locale } from "@mr-krabs/api-client";

/**
 * The UI language is a per-device preference (DEC-16, MASTER §5.3): kept in this device's
 * storage, never synced, and it never writes `household.locale`. Without a choice the
 * household's locale applies.
 */
export const LANGUAGES = ["es-CL", "en"] as const satisfies readonly Locale[];

const KEY = "mrk.uiLanguage";

const isLanguage = (v: unknown): v is Locale => LANGUAGES.includes(v as Locale);

// The choice for this run. Held in memory too, so storage that can't be written doesn't
// let a bootstrap refetch put the household's locale back.
let choice: Locale | null = null;

/** Reads this device's stored choice (the start path). Unreadable storage → no choice. */
export function loadLanguagePreference(): Locale | null {
  try {
    const stored = localStorage.getItem(KEY);
    choice = isLanguage(stored) ? stored : null;
  } catch {
    choice = null;
  }
  return choice;
}

/** The user picked a language on this device: switch at once and keep it. */
export function chooseLanguage(lng: Locale): void {
  choice = lng;
  try {
    localStorage.setItem(KEY, lng);
  } catch {
    // Not persisted; the choice still holds for this run.
  }
  void i18n.changeLanguage(lng);
}

/** Bootstrap knows the household: its locale applies unless this device chose one. */
export function applyHouseholdLanguage(locale: Locale): void {
  const lng = choice ?? locale;
  if (i18n.language !== lng) void i18n.changeLanguage(lng);
}
