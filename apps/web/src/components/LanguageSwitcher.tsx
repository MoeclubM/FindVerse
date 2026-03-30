import { GlobeIcon } from "@radix-ui/react-icons";
import { useTranslation } from "react-i18next";

import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
} from "./ui/select";

const LANGUAGE_OPTIONS = [
  { value: "en", labelKey: "language.en", buttonLabel: "EN" },
  { value: "zh-CN", labelKey: "language.zh_CN", buttonLabel: "中" },
  { value: "ja", labelKey: "language.ja", buttonLabel: "日" },
];

export function LanguageSwitcher() {
  const { i18n, t } = useTranslation();
  const currentLanguage = i18n.resolvedLanguage ?? i18n.language;
  const selectedLanguage =
    LANGUAGE_OPTIONS.find((option) => option.value === currentLanguage) ?? LANGUAGE_OPTIONS[0];

  return (
    <Select
      value={selectedLanguage.value}
      onValueChange={(nextLanguage) => {
        void i18n.changeLanguage(nextLanguage);
        localStorage.setItem("findverse_lang", nextLanguage);
      }}
    >
      <SelectTrigger aria-label={t("language.label")} className="h-10 w-auto min-w-0 rounded-full px-3.5">
        <GlobeIcon data-icon="inline-start" />
        <span>{selectedLanguage.buttonLabel}</span>
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          {LANGUAGE_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {t(option.labelKey)}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}
