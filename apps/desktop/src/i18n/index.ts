import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import { en } from "./en";
import { esCL } from "./es-CL";

// UI strings only. Money is formatted in Rust (`MoneyDto.display`), never here.
export const defaultNS = "translation";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: typeof defaultNS;
    resources: { translation: typeof esCL };
  }
}

// Resources are bundled, so init completes synchronously before the first render.
void i18n.use(initReactI18next).init({
  lng: "es-CL",
  fallbackLng: "es-CL",
  supportedLngs: ["es-CL", "en"],
  resources: { "es-CL": { translation: esCL }, en: { translation: en } },
  interpolation: { escapeValue: false }, // React already escapes.
});

export default i18n;
