import { RatioBar } from "@/lib/RatioBar";
import { RESULT_FILL, type Tone } from "@/lib/tone";

/** 伤害/金钱相对条（归一到本组最大值） */
export function StatBar({ value, max, tone }: { value: number; max: number; tone: Tone }) {
  const pct = max > 0 ? Math.max(4, Math.round((value / max) * 100)) : 0;
  return (
    <RatioBar pct={pct} fillClass={RESULT_FILL[tone]} trackClass="bg-muted/80" className="mt-1" />
  );
}
