import { Progress } from "@/components/ui/progress";
import { THREAT_FILL, type ThreatTone } from "@/lib/tone";
import { cn } from "@/lib/utils";

/** 指示条染色：把 tone 色档 class 转为作用在 Progress.Indicator 上的变体 */
function indicatorFill(fillClass: string): string {
  return fillClass
    .split(/\s+/)
    .filter(Boolean)
    .map((c) => `[&>[data-slot=progress-indicator]]:${c}`)
    .join(" ");
}

/** 胜率/KDA 相对条：0-100% 刻度，威胁色档（官方 Progress 骨架 + tone 染色） */
export function ThreatBar({
  value,
  max = 100,
  tone,
}: {
  value: number;
  max?: number;
  tone: ThreatTone;
}) {
  const pct = max > 0 ? Math.min(100, Math.max(3, (value / max) * 100)) : 0;
  return (
    <Progress
      value={pct}
      className={cn(
        "mt-1 h-[3px] bg-muted/70",
        indicatorFill(THREAT_FILL[tone]),
      )}
    />
  );
}
