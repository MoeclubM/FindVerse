import { PanelLeftIcon } from "lucide-react";
import * as React from "react";

import { cn } from "../../lib/utils";
import { Button } from "./button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "./dialog";
import { Separator } from "./separator";

type SidebarContextValue = {
  openMobile: boolean;
  setOpenMobile: (open: boolean) => void;
};

const SidebarContext = React.createContext<SidebarContextValue | null>(null);

function useSidebar() {
  const context = React.useContext(SidebarContext);
  if (!context) {
    throw new Error("useSidebar must be used within a SidebarProvider");
  }
  return context;
}

function SidebarProvider({
  children,
  defaultOpenMobile = false,
}: React.PropsWithChildren<{ defaultOpenMobile?: boolean }>) {
  const [openMobile, setOpenMobile] = React.useState(defaultOpenMobile);

  return (
    <SidebarContext.Provider value={{ openMobile, setOpenMobile }}>
      <div data-slot="sidebar-wrapper" className="flex min-h-svh w-full">
        {children}
      </div>
    </SidebarContext.Provider>
  );
}

function Sidebar({
  className,
  children,
}: React.PropsWithChildren<{ className?: string }>) {
  const { openMobile, setOpenMobile } = useSidebar();

  return (
    <>
      <aside
        data-slot="sidebar"
        className={cn(
          "hidden w-72 shrink-0 border-r border-border bg-card text-card-foreground md:flex md:flex-col",
          className,
        )}
      >
        {children}
      </aside>
      <Dialog open={openMobile} onOpenChange={setOpenMobile}>
        <DialogContent className="left-0 top-0 h-full max-h-none w-[88vw] max-w-sm translate-x-0 translate-y-0 rounded-none border-r border-border p-0">
          <DialogHeader className="sr-only">
            <DialogTitle>Sidebar</DialogTitle>
          </DialogHeader>
          <div data-slot="sidebar-mobile" className="flex h-full flex-col bg-card text-card-foreground">
            {children}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

function SidebarTrigger({
  className,
  children,
  ...props
}: React.ComponentProps<typeof Button>) {
  const { setOpenMobile } = useSidebar();

  return (
    <Button
      data-slot="sidebar-trigger"
      variant="outline"
      size="sm"
      className={cn("md:hidden", className)}
      onClick={() => setOpenMobile(true)}
      {...props}
    >
      {children ?? (
        <>
          <PanelLeftIcon data-icon="inline-start" />
          <span className="sr-only">Open sidebar</span>
        </>
      )}
    </Button>
  );
}

function SidebarInset({ className, ...props }: React.ComponentProps<"main">) {
  return <main data-slot="sidebar-inset" className={cn("min-w-0 flex-1", className)} {...props} />;
}

function SidebarHeader({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-header" className={cn("flex flex-col gap-3 p-4", className)} {...props} />;
}

function SidebarFooter({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-footer" className={cn("flex flex-col gap-3 p-4", className)} {...props} />;
}

function SidebarContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-content" className={cn("flex min-h-0 flex-1 flex-col gap-3 overflow-auto p-4", className)} {...props} />;
}

function SidebarSeparator({ className, ...props }: React.ComponentProps<typeof Separator>) {
  return <Separator data-slot="sidebar-separator" className={cn("bg-border", className)} {...props} />;
}

function SidebarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="sidebar-group" className={cn("flex flex-col gap-2", className)} {...props} />;
}

function SidebarGroupLabel({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sidebar-group-label"
      className={cn("px-2 text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground", className)}
      {...props}
    />
  );
}

function SidebarMenu({ className, ...props }: React.ComponentProps<"ul">) {
  return <ul data-slot="sidebar-menu" className={cn("flex flex-col gap-2", className)} {...props} />;
}

function SidebarMenuItem({ className, ...props }: React.ComponentProps<"li">) {
  return <li data-slot="sidebar-menu-item" className={cn("list-none", className)} {...props} />;
}

function SidebarMenuButton({
  className,
  isActive = false,
  children,
  ...props
}: React.ComponentProps<typeof Button> & { isActive?: boolean }) {
  return (
    <Button
      data-slot="sidebar-menu-button"
      variant={isActive ? "secondary" : "ghost"}
      className={cn(
        "h-auto w-full justify-between rounded-xl px-3 py-3 text-left",
        isActive && "border border-border bg-secondary",
        className,
      )}
      {...props}
    >
      {children}
    </Button>
  );
}

function SidebarMenuBadge({ className, ...props }: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="sidebar-menu-badge"
      className={cn("rounded-full bg-muted px-2 py-0.5 text-xs font-semibold text-muted-foreground", className)}
      {...props}
    />
  );
}

export {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
  SidebarTrigger,
  useSidebar,
};
