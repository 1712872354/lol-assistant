import { cn } from "@/lib/utils";

/** 战绩卡片骨架屏：静态占位，无动画 */
export function MatchCardSkeleton() {
  return (
    <div
      className={cn(
        "flex w-full min-h-0 flex-1 items-center gap-2.5 rounded-md border border-l-[3px] border-border/40 bg-card/60 px-2.5",
      )}
    >
      <div className="skeleton-shimmer h-10 w-10 shrink-0 rounded-md" />
      <div className="min-w-0 flex-1 space-y-1.5">
        <div className="skeleton-shimmer h-2.5 w-16 rounded-full" />
        <div className="skeleton-shimmer h-5 w-24 rounded-full" />
        <div className="skeleton-shimmer h-2 w-10 rounded-full" />
      </div>
      <div className="skeleton-shimmer h-3 w-6 rounded-full" />
    </div>
  );
}
