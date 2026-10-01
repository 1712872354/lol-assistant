import { cn } from "@/lib/utils";

/** 对局页初始加载骨架屏：静态占位，无动画 */
export function GameinfoSkeleton() {
  return (
    <div className="flex h-full min-h-0 flex-col gap-2.5 p-3">
      <TeamSkeleton ally />
      <TeamSkeleton ally={false} />
    </div>
  );
}

function TeamSkeleton({ ally }: { ally: boolean }) {
  return (
    <section
      className={cn(
        "flex min-h-0 flex-1 flex-col rounded-lg p-3",
        ally ? "bg-ally-bg/10" : "bg-enemy-bg/10",
      )}
    >
      <div className="mb-3 flex items-center gap-2.5">
        <div className={cn("skeleton-shimmer h-2 w-2 rounded-full", ally ? "bg-ally-fg/30" : "bg-enemy-fg/30")} />
        <div className={cn("skeleton-shimmer h-3.5 w-8 rounded-full", ally ? "bg-ally-border/20" : "bg-enemy-border/20")} />
        <div className={cn("skeleton-shimmer h-3 w-16 rounded-full", ally ? "bg-ally-border/15" : "bg-enemy-border/15")} />
        <div className="ml-auto flex gap-1.5">
          <div className={cn("skeleton-shimmer h-7 w-20 rounded-full", ally ? "bg-ally-border/15" : "bg-enemy-border/15")} />
          <div className={cn("skeleton-shimmer h-7 w-20 rounded-full", ally ? "bg-ally-border/15" : "bg-enemy-border/15")} />
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-5 gap-2">
        {Array.from({ length: 5 }).map((_, i) => (
          <div
            key={i}
            className={cn(
              "flex h-full flex-col gap-2 rounded-lg border border-dashed p-2.5",
              ally ? "border-ally-border/15" : "border-enemy-border/15",
            )}
          >
            <div className={cn("skeleton-shimmer h-10 w-10 rounded-lg", ally ? "bg-ally-border/15" : "bg-enemy-border/15")} />
            <div className={cn("skeleton-shimmer h-3 w-3/4 rounded-full", ally ? "bg-ally-border/15" : "bg-enemy-border/15")} />
            <div className={cn("skeleton-shimmer h-2 w-1/2 rounded-full", ally ? "bg-ally-border/10" : "bg-enemy-border/10")} />
          </div>
        ))}
      </div>
    </section>
  );
}
