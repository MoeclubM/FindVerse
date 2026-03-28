import { DesktopIcon, MoonIcon, SunIcon } from "@radix-ui/react-icons";
import { useTranslation } from "react-i18next";

import { AppSelect } from "./common/AppSelect";

export type ThemeMode = "system" | "light" | "dark";

export function ThemeSwitcher(props: {
  theme: "light" | "dark";
  mode: ThemeMode;
  onChange: (theme: ThemeMode) => void;
}) {
  const { t } = useTranslation();
  const prefix =
    props.mode === "system" ? (
      <DesktopIcon className="size-4" />
    ) : props.theme === "dark" ? (
      <MoonIcon className="size-4" />
    ) : (
      <SunIcon className="size-4" />
    );

  return (
    <AppSelect
      ariaLabel={t("theme.label")}
      theme={props.theme}
      value={props.mode}
      prefix={prefix}
      options={[
        { value: "system", label: t("theme.system"), triggerLabel: t("theme.system_short") },
        { value: "light", label: t("theme.light"), triggerLabel: t("theme.light") },
        { value: "dark", label: t("theme.dark"), triggerLabel: t("theme.dark") },
      ]}
      triggerClassName="w-auto rounded-full px-3.5"
      onValueChange={(value) =>
        props.onChange(value === "dark" || value === "light" || value === "system" ? value : "system")
      }
    />
  );
}
