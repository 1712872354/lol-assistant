import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  desc?: string;
  className?: string;
}

export function EmptyState({ icon: Icon, title, desc, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed bg-card/40 p-10 text-center",
        className,
      )}
    >
      {Icon ? (
        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
          <Icon className="h-6 w-6 text-muted-foreground" />
        </div>
      ) : (
        <img src="/brand-ultra.svg" alt="" className="h-12 w-12 opacity-80" />
      )}
      <div className="space-y-1">
        <p className="text-sm font-medium">{title}</p>
        {desc ? (
          <p className="mx-auto max-w-sm text-xs leading-relaxed text-muted-foreground">
            {desc}
          </p>
        ) : null}
      </div>
    </div>
  );
}
