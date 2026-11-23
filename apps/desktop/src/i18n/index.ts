import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { en } from "./en";
import { esCL } from "./es-CL";
import { LANGUAGES, loadLanguagePreference } from "./language";

// UI strings only. Money is formatted in Rust (`MoneyDto.display`), never here.
export const defaultNS = "translation";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: typeof defaultNS;
    resources: { translation: typeof esCL };
  }
}

// Resources are bundled, so init completes synchronously before the first render.
// Starts in this device's stored choice; bootstrap then applies the household's locale if
// there is none (language.ts).
void i18n.use(initReactI18next).init({
  lng: loadLanguagePreference() ?? "es-CL",
  fallbackLng: "es-CL",
  supportedLngs: [...LANGUAGES],
  resources: { "es-CL": { translation: esCL }, en: { translation: en } },
  interpolation: { escapeValue: false }, // React already escapes.
});

// Screen readers pronounce by the document language.
const syncLang = (lng: string) => (document.documentElement.lang = lng);
syncLang(i18n.language);
i18n.on("languageChanged", syncLang);

export default i18n;
