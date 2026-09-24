import { cn } from "@/lib/utils";

interface RatioBarProps {
  /** 0–100 的百分比（调用方按各自口径计算，避免集中后微调行为漂移） */
  pct: number;
  /** 填充色 class（取自 lib/tone 的 RESULT_FILL / THREAT_FILL） */
  fillClass: string;
  /** 轨道色 class */
  trackClass?: string;
  /** 轨道上边距（明细页条带在数字下方，需要 mt-1） */
  className?: string;
}

/** 3px 相对条：胜负/威胁/队伍对照三处共用的展示骨架。 */
export function RatioBar({ pct, fillClass, trackClass = "bg-muted/80", className }: RatioBarProps) {
  return (
    <div
      className={cn(
        "h-[3px] w-full overflow-hidden rounded-full",
        trackClass,
        className,
      )}
    >
      <div className={cn("h-full rounded-full", fillClass)} style={{ width: `${pct}%` }} />
    </div>
  );
}
