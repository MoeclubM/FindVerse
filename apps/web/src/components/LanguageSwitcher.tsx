import { GlobeIcon } from "@radix-ui/react-icons";
import { useTranslation } from "react-i18next";

import { AppSelect } from "./common/AppSelect";

const LANGUAGE_OPTIONS = [
  { value: "en", label: "English", buttonLabel: "EN" },
  { value: "zh-CN", label: "简体中文", buttonLabel: "中" },
  { value: "ja", label: "日本語", buttonLabel: "日" },
];

export function LanguageSwitcher(props: { theme: "light" | "dark" }) {
  const { i18n, t } = useTranslation();
  const currentLanguage = i18n.resolvedLanguage ?? i18n.language;
  const selectedLanguage = LANGUAGE_OPTIONS.some((option) => option.value === currentLanguage)
    ? currentLanguage
    : "en";

  return (
    <AppSelect
      ariaLabel={t("language.label")}
      theme={props.theme}
      value={selectedLanguage}
      prefix={<GlobeIcon className="size-4" />}
      options={LANGUAGE_OPTIONS.map((option) => ({
        value: option.value,
        label: option.label,
        triggerLabel: option.buttonLabel,
      }))}
      triggerClassName="w-auto rounded-full px-3.5"
      onValueChange={(nextLanguage) => {
        void i18n.changeLanguage(nextLanguage);
        localStorage.setItem("findverse_lang", nextLanguage);
      }}
    />
  );
}
