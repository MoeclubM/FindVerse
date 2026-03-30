import { DesktopIcon, MoonIcon, SunIcon } from "@radix-ui/react-icons";
import { useTranslation } from "react-i18next";

import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
} from "./ui/select";

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

  const selectedLabel =
    props.mode === "system" ? t("theme.system_short") : props.mode === "dark" ? t("theme.dark") : t("theme.light");

  return (
    <Select
      value={props.mode}
      onValueChange={(value) =>
        props.onChange(value === "dark" || value === "light" || value === "system" ? value : "system")
      }
    >
      <SelectTrigger aria-label={t("theme.label")} className="h-10 w-auto min-w-0 rounded-full px-3.5">
        {prefix}
        <span>{selectedLabel}</span>
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectItem value="system">{t("theme.system")}</SelectItem>
          <SelectItem value="light">{t("theme.light")}</SelectItem>
          <SelectItem value="dark">{t("theme.dark")}</SelectItem>
        </SelectGroup>
      </SelectContent>
    </Select>
  );
}
