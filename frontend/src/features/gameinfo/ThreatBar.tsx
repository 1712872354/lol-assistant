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

/** 档位文字：颜色之外的第二编码（WCAG 1.4.1），并入可访问名 */
const TONE_TEXT: Record<ThreatTone, string> = {
  good: "较高",
  mid: "中等",
  bad: "较低",
  muted: "",
};

/** 胜率/KDA 相对条：0-100% 刻度，威胁色档（官方 Progress 骨架 + tone 染色） */
export function ThreatBar({
  value,
  max = 100,
  tone,
  label = "相对值",
}: {
  value: number;
  max?: number;
  tone: ThreatTone;
  /** 指标名（如「胜率」），用于可访问名 */
  label?: string;
}) {
  const pct = max > 0 ? Math.min(100, Math.max(3, (value / max) * 100)) : 0;
  const toneText = TONE_TEXT[tone];
  return (
    <Progress
      value={pct}
      aria-label={`${label} ${value.toFixed(1)}${max === 100 ? "%" : ""}${toneText ? `（${toneText}）` : ""}`}
      className={cn(
        "mt-1.5 h-[4px] rounded-full bg-muted/50",
        indicatorFill(THREAT_FILL[tone]),
      )}
    />
  );
}
