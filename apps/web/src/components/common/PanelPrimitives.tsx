import { type ReactNode } from "react";

import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import {
  Card,
  CardContent,
  CardHeader,
} from "../ui/card";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import { Label } from "../ui/label";

export function SectionHeader(props: {
  title: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
  heading?: "h2" | "h3";
}) {
  const Heading = props.heading ?? "h2";

  return (
    <div className={cn("flex flex-col gap-1.5 sm:flex-row sm:items-center sm:justify-between", props.className)}>
      <div className="space-y-0">
        <Heading
          className={cn(
            "font-semibold tracking-tight text-foreground",
            props.heading === "h3" ? "text-sm" : "text-base",
          )}
        >
          {props.title}
        </Heading>
        {props.meta ? <p className="text-[11px] text-muted-foreground sm:text-xs">{props.meta}</p> : null}
      </div>
      {props.actions ? <div className="flex flex-wrap items-center gap-1">{props.actions}</div> : null}
    </div>
  );
}

export function StatStrip(props: {
  items: Array<{
    label: ReactNode;
    value: ReactNode;
  }>;
  className?: string;
  compact?: boolean;
}) {
  return (
    <div className={cn("grid gap-1.5 sm:grid-cols-2 xl:grid-cols-4", props.className)}>
      {props.items.map((item, index) => (
        <Card key={index} className="rounded-md border-border/60 bg-muted/30 shadow-none">
          <CardContent className={cn(props.compact ? "px-2.5 py-2" : "px-3 py-2.5")}>
            <span className="text-[10px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
              {item.label}
            </span>
            <div className={cn("font-semibold leading-none text-foreground", props.compact ? "mt-1 text-base" : "mt-1.5 text-lg")}>{item.value}</div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

export function FieldShell(props: {
  label: ReactNode;
  hint?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={cn("grid gap-1", props.className)}>
      <Label className="text-xs font-medium text-muted-foreground">{props.label}</Label>
      {props.children}
      {props.hint ? <span className="text-xs text-muted-foreground">{props.hint}</span> : null}
    </label>
  );
}

export function DetailDialog(props: {
  open: boolean;
  title: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  closeLabel: ReactNode;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <Dialog open={props.open} onOpenChange={(open) => !open && props.onClose()}>
      <DialogContent className="max-h-[min(88vh,960px)] overflow-y-auto rounded-xl p-0">
        <DialogHeader className="border-b border-border px-4 pb-2.5 pt-4">
          <DialogTitle>{props.title}</DialogTitle>
          {props.meta ? <DialogDescription>{props.meta}</DialogDescription> : null}
        </DialogHeader>
        <div className="px-4 py-3">{props.children}</div>
        <DialogFooter className="border-t border-border px-4 py-2.5">
          {props.actions}
          <DialogClose asChild>
            <Button variant="outline" onClick={props.onClose}>
              {props.closeLabel}
            </Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function PanelSection(props: {
  title: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
  contentClassName?: string;
  children: ReactNode;
}) {
  return (
    <Card className={cn("rounded-lg shadow-none", props.className)}>
      <CardHeader className="p-4 pb-2.5">
        <SectionHeader title={props.title} meta={props.meta} actions={props.actions} />
      </CardHeader>
      <CardContent className={cn("px-4 pb-4 pt-0", props.contentClassName)}>{props.children}</CardContent>
    </Card>
  );
}
