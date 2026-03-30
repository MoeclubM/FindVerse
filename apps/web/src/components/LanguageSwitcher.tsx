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
  { value: "en", label: "English", buttonLabel: "EN" },
  { value: "zh-CN", label: "简体中文", buttonLabel: "中" },
  { value: "ja", label: "日本語", buttonLabel: "日" },
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
              {option.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}
