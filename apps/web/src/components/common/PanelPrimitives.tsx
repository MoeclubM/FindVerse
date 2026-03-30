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
    <div className={cn("flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between", props.className)}>
      <div className="space-y-1">
        <Heading className="text-lg font-semibold tracking-tight text-foreground">{props.title}</Heading>
        {props.meta ? <p className="text-sm text-muted-foreground">{props.meta}</p> : null}
      </div>
      {props.actions ? <div className="flex flex-wrap items-center gap-2">{props.actions}</div> : null}
    </div>
  );
}

export function StatStrip(props: {
  items: Array<{
    label: ReactNode;
    value: ReactNode;
  }>;
  className?: string;
}) {
  return (
    <div className={cn("grid gap-3 sm:grid-cols-2 xl:grid-cols-4", props.className)}>
      {props.items.map((item, index) => (
        <Card key={index} className="rounded-xl bg-muted/40 shadow-none">
          <CardContent className="px-4 py-3">
            <span className="text-xs font-medium uppercase tracking-[0.12em] text-muted-foreground">
              {item.label}
            </span>
            <div className="mt-2 text-2xl font-semibold text-foreground">{item.value}</div>
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
    <label className={cn("grid gap-2", props.className)}>
      <Label>{props.label}</Label>
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
      <DialogContent className="max-h-[min(88vh,960px)] overflow-y-auto rounded-2xl p-0">
        <DialogHeader className="border-b border-border px-6 pb-4 pt-6">
          <DialogTitle>{props.title}</DialogTitle>
          {props.meta ? <DialogDescription>{props.meta}</DialogDescription> : null}
        </DialogHeader>
        <div className="px-6 py-5">{props.children}</div>
        <DialogFooter className="border-t border-border px-6 py-4">
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
    <Card className={cn("rounded-2xl shadow-sm", props.className)}>
      <CardHeader className="pb-4">
        <SectionHeader title={props.title} meta={props.meta} actions={props.actions} />
      </CardHeader>
      <CardContent className={cn("pt-0", props.contentClassName)}>{props.children}</CardContent>
    </Card>
  );
}
