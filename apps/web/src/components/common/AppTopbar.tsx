import type { ReactNode } from "react";

import { cn } from "../../lib/utils";
import { LanguageSwitcher } from "../LanguageSwitcher";
import { ThemeMode, ThemeSwitcher } from "../ThemeSwitcher";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";

export function TopbarBadge(props: {
  children: ReactNode;
}) {
  return (
    <Badge
      variant="outline"
      className="h-9 rounded-full px-3 text-xs font-medium uppercase tracking-[0.12em] sm:h-10"
    >
      {props.children}
    </Badge>
  );
}

export function TopbarActionButton(props: {
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  leading?: ReactNode;
  ariaLabel?: string;
  compactOnMobile?: boolean;
}) {
  return (
    <Button
      type="button"
      variant="outline"
      className={cn(
        "h-9 rounded-full px-2.5 sm:h-10 sm:px-3",
        props.compactOnMobile && "sm:gap-1.5",
      )}
      onClick={props.onClick}
      disabled={props.disabled}
      aria-label={props.ariaLabel}
    >
      {props.leading}
      <span className={cn("truncate", props.compactOnMobile && "hidden sm:inline")}>
        {props.children}
      </span>
    </Button>
  );
}

export function AppTopbar(props: {
  theme: "light" | "dark";
  themeMode: ThemeMode;
  onThemeModeChange: (theme: ThemeMode) => void;
  title?: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  beforeControls?: ReactNode;
  afterControls?: ReactNode;
  containerClassName?: string;
  onTitleClick?: () => void;
}) {
  const borderTone = props.theme === "dark" ? "border-[#2e2722]" : "border-[#e5dbcf]";
  const subtitleTone = props.theme === "dark" ? "text-[#a89d8f]" : "text-[#7c6e61]";
  const hasHeading = Boolean(props.title || props.subtitle || props.leading);
  const titleBlock = (
    <div className="flex min-w-0 items-center gap-2 sm:items-baseline">
      {props.title ? (
        <div className="truncate text-base font-semibold tracking-[-0.04em] sm:text-[1.1rem]">
          {props.title}
        </div>
      ) : null}
      {props.subtitle ? (
        <div className={`truncate text-xs sm:text-sm ${subtitleTone}`}>{props.subtitle}</div>
      ) : null}
    </div>
  );
  const controlsClassName = cn(
    "flex max-w-full items-center gap-2 self-start pb-1 sm:self-auto sm:pb-0",
    "[-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
    hasHeading
      ? "w-full overflow-x-auto sm:w-auto sm:flex-wrap sm:justify-end sm:overflow-visible"
      : "overflow-x-auto justify-end sm:flex-wrap sm:overflow-visible",
  );

  return (
    <header className={`sticky top-0 z-40 border-b bg-background/92 backdrop-blur ${borderTone}`}>
      <div
        className={
          props.containerClassName ??
          "mx-auto flex w-full max-w-7xl flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-6 sm:py-4 lg:px-8"
        }
      >
        {hasHeading ? (
          props.onTitleClick ? (
            <button
              type="button"
              className="appearance-none flex min-w-0 items-center gap-3 border-0 bg-transparent p-0 text-left text-inherit shadow-none outline-none transition-opacity hover:opacity-80 focus:outline-none focus-visible:outline-none"
              onClick={props.onTitleClick}
            >
              {props.leading ? <span className="shrink-0">{props.leading}</span> : null}
              {titleBlock}
            </button>
          ) : (
            <div className="flex min-w-0 items-center gap-3">
              {props.leading ? <span className="shrink-0">{props.leading}</span> : null}
              {titleBlock}
            </div>
          )
        ) : null}
        <div className={controlsClassName}>
          {props.beforeControls}
          <LanguageSwitcher />
          <ThemeSwitcher
            theme={props.theme}
            mode={props.themeMode}
            onChange={props.onThemeModeChange}
          />
          {props.afterControls}
        </div>
      </div>
    </header>
  );
}
