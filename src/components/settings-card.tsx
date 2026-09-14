import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { LucideIcon } from "lucide-react";

/**
 * WinUI3 SettingsCard 的 shadcn 对应物：
 * 左侧图标 + 标题/描述，右侧操作区（children）。
 */
export function SettingsCard({
  icon: Icon,
  header,
  description,
  children,
  className,
}: {
  icon?: LucideIcon;
  header: string;
  description?: string;
  children?: React.ReactNode;
  className?: string;
}) {
  return (
    <Card
      className={cn(
        "flex min-h-16 flex-row items-center gap-4 border px-4 py-3",
        className,
      )}
    >
      {Icon && <Icon className="size-5 shrink-0 text-muted-foreground" />}
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium leading-5">{header}</div>
        {description && (
          <div className="text-xs text-muted-foreground leading-4">{description}</div>
        )}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </Card>
  );
}
