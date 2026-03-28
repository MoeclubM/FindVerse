import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/translation.json";
import zhCN from "./locales/zh-CN/translation.json";

const resources = {
  en: { translation: en },
  "zh-CN": { translation: zhCN },
  ja: { translation: en },
} as const;

const savedLang = localStorage.getItem("findverse_lang");
const browserLang = navigator.language === "zh-CN"
  ? "zh-CN"
  : navigator.language.startsWith("zh")
    ? "zh-CN"
    : navigator.language.startsWith("ja")
      ? "ja"
    : navigator.language.split("-")[0];
const initialLang = savedLang && savedLang in resources
  ? savedLang
  : browserLang in resources
    ? browserLang
    : "en";

i18n.use(initReactI18next).init({
  lng: initialLang,
  fallbackLng: "en",
  resources,
  supportedLngs: Object.keys(resources),
  nonExplicitSupportedLngs: false,
  interpolation: { escapeValue: false },
});

export default i18n;
