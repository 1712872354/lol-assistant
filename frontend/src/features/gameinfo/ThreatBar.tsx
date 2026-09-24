import { RatioBar } from "@/lib/RatioBar";
import { THREAT_FILL, type ThreatTone } from "@/lib/tone";

/** 胜率/KDA 相对条：0–100% 刻度，威胁色档 */
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
    <RatioBar pct={pct} fillClass={THREAT_FILL[tone]} trackClass="bg-muted/70" className="mt-1" />
  );
}
