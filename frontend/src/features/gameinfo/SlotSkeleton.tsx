import { UserPlus } from "lucide-react";
import { cn } from "@/lib/utils";

interface Props {
  ally: boolean;
  caption: string;
  showCaption: boolean;
}

/**
 * 空槽等待态：极简虚线框 + 淡图标，几乎隐身。
 * 不是骨架屏（无 shimmer / 无色块），是"位置占位符"。
 */
export function SlotSkeleton({ ally, caption, showCaption }: Props) {
  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col items-center justify-center rounded-lg border border-dashed",
        ally
          ? "border-ally-border/20 bg-transparent"
          : "border-enemy-border/20 bg-transparent",
      )}
    >
      <UserPlus
        className={cn(
          "h-5 w-5 shrink-0",
          ally ? "text-ally-border/30" : "text-enemy-border/30",
        )}
      />
      {showCaption && caption ? (
        <p
          className={cn(
            "mt-2 px-3 text-center text-[11px] leading-snug",
            ally ? "text-ally-border/40" : "text-enemy-border/40",
          )}
        >
          {caption}
        </p>
      ) : null}
    </div>
  );
}
