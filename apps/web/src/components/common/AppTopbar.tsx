import type { ReactNode } from "react";

import { LanguageSwitcher } from "../LanguageSwitcher";
import { ThemeMode, ThemeSwitcher } from "../ThemeSwitcher";

export function TopbarBadge(props: {
  theme: "light" | "dark";
  children: ReactNode;
}) {
  const tone =
    props.theme === "dark"
      ? "border-[#5a4335] bg-[#2a231e] text-[#f0cbb3]"
      : "border-[#e4d7c5] bg-[#f8efe4] text-[#9a5836]";

  return (
    <span
      className={`inline-flex h-10 items-center rounded-full border px-3 text-xs font-medium uppercase tracking-[0.12em] ${tone}`}
    >
      {props.children}
    </span>
  );
}

export function TopbarActionButton(props: {
  theme: "light" | "dark";
  children: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  leading?: ReactNode;
}) {
  const tone =
    props.theme === "dark"
      ? "border-[#3a3129] bg-[#211c18] text-[#f3ece2] hover:bg-[#2a2420]"
      : "border-[#e2d8cb] bg-[#fbf7f1] text-[#40352d] hover:bg-[#f3ece3]";

  return (
    <button
      type="button"
      className={`inline-flex h-10 items-center gap-2 rounded-full border px-3 text-sm font-medium transition-[background-color,border-color,color,transform] duration-200 ease-out hover:-translate-y-px disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:translate-y-0 ${tone}`}
      onClick={props.onClick}
      disabled={props.disabled}
    >
      {props.leading ? <span className="shrink-0 text-neutral-500">{props.leading}</span> : null}
      <span className="truncate">{props.children}</span>
    </button>
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
    <div className="flex min-w-0 items-baseline gap-2">
      {props.title ? (
        <div className="truncate text-lg font-semibold tracking-[-0.04em] sm:text-[1.1rem]">
          {props.title}
        </div>
      ) : null}
      {props.subtitle ? (
        <div className={`truncate text-xs sm:text-sm ${subtitleTone}`}>{props.subtitle}</div>
      ) : null}
    </div>
  );

  return (
    <header className={`border-b ${borderTone}`}>
      <div
        className={
          props.containerClassName ??
          "mx-auto flex w-full max-w-7xl flex-col gap-4 px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8"
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
        <div className="flex max-w-full flex-wrap items-center gap-2 self-start sm:self-auto">
          {props.beforeControls}
          <LanguageSwitcher theme={props.theme} />
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
