import type { ReactNode } from "react";
import * as Select from "@radix-ui/react-select";
import { CheckIcon, ChevronDownIcon } from "@radix-ui/react-icons";

type AppSelectOption = {
  value: string;
  label: string;
  triggerLabel?: string;
};

export function AppSelect(props: {
  ariaLabel: string;
  value: string;
  options: AppSelectOption[];
  theme: "light" | "dark";
  onValueChange: (value: string) => void;
  prefix?: ReactNode;
  triggerClassName?: string;
  contentClassName?: string;
}) {
  const triggerTone =
    props.theme === "dark"
      ? "border-[#3a3129] bg-[#211c18] text-[#f3ece2] hover:bg-[#2a2420] focus:border-[#6f5b4c]"
      : "border-[#e2d8cb] bg-[#fbf7f1] text-[#40352d] hover:bg-[#f3ece3] focus:border-[#b89b80]";
  const contentTone =
    props.theme === "dark"
      ? "border-[#3a3129] bg-[#211c18] text-[#f3ece2] shadow-[0_24px_80px_rgba(0,0,0,0.36)]"
      : "border-[#e2d8cb] bg-[#fffaf4] text-[#3d342d] shadow-[0_24px_80px_rgba(88,65,42,0.12)]";
  const itemTone =
    props.theme === "dark"
      ? "data-[highlighted]:bg-[#2f2823] data-[highlighted]:text-[#fff6ec]"
      : "data-[highlighted]:bg-[#f2e9de] data-[highlighted]:text-[#2d251f]";
  const chromeTone = props.theme === "dark" ? "text-[#ab9c8f]" : "text-[#8f7d6d]";
  const selectedOption = props.options.find((option) => option.value === props.value);

  return (
    <Select.Root value={props.value} onValueChange={props.onValueChange}>
      <Select.Trigger
        aria-label={props.ariaLabel}
        className={`inline-flex h-10 items-center justify-between gap-3 border px-3 text-sm font-medium outline-none transition-[background-color,border-color,color,transform] duration-200 ease-out hover:-translate-y-px disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:translate-y-0 ${triggerTone} ${props.triggerClassName ?? ""}`}
      >
        <span className="inline-flex min-w-0 items-center gap-2">
          {props.prefix ? <span className={`shrink-0 ${chromeTone}`}>{props.prefix}</span> : null}
          <span className="truncate">
            {selectedOption?.triggerLabel ?? selectedOption?.label ?? props.value}
          </span>
        </span>
        <Select.Icon className={`shrink-0 ${chromeTone}`}>
          <ChevronDownIcon />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content
          position="popper"
          sideOffset={6}
          className={`app-select-content z-[80] min-w-[var(--radix-select-trigger-width)] rounded-[20px] border ${contentTone} ${props.contentClassName ?? ""}`}
        >
          <Select.Viewport className="grid gap-1 p-1">
            {props.options.map((option) => (
              <Select.Item
                key={option.value}
                value={option.value}
                className={`relative flex min-h-9 cursor-default select-none items-center rounded-[14px] px-3 pr-8 text-sm outline-none ${itemTone}`}
              >
                <Select.ItemText>{option.label}</Select.ItemText>
                <span className={`absolute right-3 inline-flex items-center ${chromeTone}`}>
                  <Select.ItemIndicator>
                    <CheckIcon />
                  </Select.ItemIndicator>
                </span>
              </Select.Item>
            ))}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}
