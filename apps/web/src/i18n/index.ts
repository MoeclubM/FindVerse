import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/translation.json";
import zhCN from "./locales/zh-CN/translation.json";

const resources = {
  en: { translation: en },
  "zh-CN": { translation: zhCN },
  ja: { translation: en },
} as const;

const APP_LANGUAGE_BY_PREFIX: Record<string, keyof typeof resources> = {
  en: "en",
  ja: "ja",
  zh: "zh-CN",
};

const SEARCH_LANGUAGE_BY_PREFIX: Record<string, string> = {
  ar: "ara",
  cs: "ces",
  da: "dan",
  de: "deu",
  el: "ell",
  en: "eng",
  es: "spa",
  fi: "fin",
  fr: "fra",
  he: "heb",
  hi: "hin",
  hu: "hun",
  id: "ind",
  it: "ita",
  ja: "jpn",
  ko: "kor",
  nb: "nob",
  nl: "nld",
  nn: "nno",
  no: "nob",
  pl: "pol",
  pt: "por",
  ro: "ron",
  ru: "rus",
  sv: "swe",
  th: "tha",
  tr: "tur",
  uk: "ukr",
  vi: "vie",
  zh: "cmn",
};

function getBrowserLanguages() {
  return Array.from(
    new Set([...(navigator.languages ?? []), navigator.language].map((value) => value.trim().toLowerCase()).filter(Boolean)),
  );
}

export function resolveAppLanguage(value: string) {
  const normalized = value.trim().toLowerCase();
  const exactMatch = normalized in resources ? (normalized as keyof typeof resources) : null;
  if (exactMatch) {
    return exactMatch;
  }
  return APP_LANGUAGE_BY_PREFIX[normalized.split("-")[0]] ?? null;
}

export function resolveSearchLanguage(value: string) {
  const normalized = value.trim().toLowerCase();
  return SEARCH_LANGUAGE_BY_PREFIX[normalized] ?? SEARCH_LANGUAGE_BY_PREFIX[normalized.split("-")[0]] ?? "";
}

export function getPreferredSearchLanguage() {
  return getBrowserLanguages().map(resolveSearchLanguage).find(Boolean) ?? "";
}

const savedLang = localStorage.getItem("findverse_lang");
const initialLang = savedLang && savedLang in resources
  ? savedLang
  : getBrowserLanguages().map(resolveAppLanguage).find(Boolean) ?? "en";

i18n.use(initReactI18next).init({
  lng: initialLang,
  fallbackLng: "en",
  resources,
  supportedLngs: Object.keys(resources),
  nonExplicitSupportedLngs: false,
  interpolation: { escapeValue: false },
});

export default i18n;
