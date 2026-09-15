import * as React from "react";
import { ContextMenu as ContextMenuPrimitive } from "radix-ui";
import { cn } from "@/lib/utils";

// 右键菜单（radix ContextMenu 的 shadcn 风格包装，样式对齐 dropdown-menu.tsx）

const ContextMenu = ContextMenuPrimitive.Root;
const ContextMenuTrigger = ContextMenuPrimitive.Trigger;

const ContextMenuContent = React.forwardRef<
  React.ComponentRef<typeof ContextMenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Content>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.Portal>
    <ContextMenuPrimitive.Content
      ref={ref}
      className={cn(
        "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 z-50 min-w-[8rem] overflow-hidden rounded-md border p-1 shadow-md",
        className,
      )}
      {...props}
    />
  </ContextMenuPrimitive.Portal>
));
ContextMenuContent.displayName = "ContextMenuContent";

const ContextMenuItem = React.forwardRef<
  React.ComponentRef<typeof ContextMenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Item>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.Item
    ref={ref}
    className={cn(
      "focus:bg-accent focus:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
      className,
      props.disabled && "opacity-50",
    )}
    {...props}
  />
));
ContextMenuItem.displayName = "ContextMenuItem";

const ContextMenuSeparator = React.forwardRef<
  React.ComponentRef<typeof ContextMenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.Separator
    ref={ref}
    className={cn("bg-border -mx-1 my-1 h-px", className)}
    {...props}
  />
));
ContextMenuSeparator.displayName = "ContextMenuSeparator";

export { ContextMenu, ContextMenuTrigger, ContextMenuContent, ContextMenuItem, ContextMenuSeparator };
