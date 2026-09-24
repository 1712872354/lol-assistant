import { cn } from "@/lib/utils";

interface Props {
  ally: boolean;
  caption: string;
  showCaption: boolean;
}

/** 空槽骨架：头像位 + 名字位 + 9 行近况占位 + 可选说明文案 */
export function SlotSkeleton({ ally, caption, showCaption }: Props) {
  return (
      <div
        className={cn(
          "flex h-full min-h-0 flex-col overflow-hidden rounded-[14px] border p-2.5",
          ally
            ? "border-ally-border/40 bg-linear-to-b from-ally-bg/50 to-transparent"
            : "border-enemy-border/40 bg-linear-to-b from-enemy-bg/50 to-transparent",
        )}
      >
        <div className="flex shrink-0 items-center gap-2.5">
          <div
            className={cn(
              "h-11 w-11 shrink-0 rounded-xl",
              ally ? "bg-ally-soft/90" : "bg-enemy-soft/90",
            )}
          />
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            <div
              className={cn(
                "h-2 w-2/3 rounded-full",
                ally ? "bg-ally-border/35" : "bg-enemy-border/35",
              )}
            />
            <div
              className={cn(
                "h-2 w-1/3 rounded-full",
                ally ? "bg-ally-border/25" : "bg-enemy-border/25",
              )}
            />
          </div>
        </div>
        <div className="mt-3 flex min-h-0 flex-1 flex-col gap-[5px] overflow-hidden">
          {Array.from({ length: 9 }).map((_, i) => (
            <div
              key={i}
              className={cn(
                "h-[9px] shrink-0 rounded-full",
                ally ? "bg-ally-soft/90" : "bg-enemy-soft/90",
                i % 3 === 0 ? "w-full" : i % 3 === 1 ? "w-[93%]" : "w-[97%]",
              )}
            />
          ))}
        </div>
        {showCaption ? (
          <div className="mt-auto shrink-0 pt-2 text-center text-[11px] leading-snug text-muted-foreground">
            {caption}
          </div>
        ) : null}
      </div>

  );
}
