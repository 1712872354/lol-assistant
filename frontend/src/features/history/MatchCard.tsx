import type { Tone } from "@/lib/tone";
import type { MatchSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { AssetImg } from "@/lib/AssetImg";
import { resultOf } from "./format";

/** 结果色调：白底卡片 + 左侧色条（胜=绿 / 负=红 / 重赛=灰） */
const BAR_CLASS: Record<Tone, string> = {
  win: "border-l-win-bar",
  loss: "border-l-loss-bar",
  remake: "border-l-remake-bar",
};
const LABEL_CLASS: Record<Tone, string> = {
  win: "text-win-fg",
  loss: "text-loss-fg",
  remake: "text-remake-fg",
};

interface Props {
  summary: MatchSummary;
  active: boolean;
  onClick: () => void;
  className?: string;
}

/** 战绩卡片：左色条 + 头像 + 三行文案（行距放宽）+ 右上胜负 */
export function MatchCard({ summary: s, active, onClick, className }: Props) {
  const r = resultOf(s);
  const modeLabel = s.queueName || s.queueShort || `#${s.queueId}`;
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "relative flex w-full min-h-0 flex-1 items-center gap-3 rounded-lg border border-l-[3px] bg-card px-3 py-2.5 text-left transition-all hover:shadow-sm",
        BAR_CLASS[r.tone],
        active && "ring-2 ring-selected-ring",
        className,
      )}
    >
      <AssetImg kind="champion" id={s.championId} size={44} className="shrink-0 rounded-md" />
      <div className="min-w-0 flex-1 space-y-1.5 pr-12">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="truncate text-[12px] font-medium leading-snug text-muted-foreground">
            {modeLabel}
          </span>
          <span className="tnum shrink-0 text-[11px] leading-snug text-muted-foreground/70">
            {s.duration}
          </span>
        </div>
        <div className="tnum text-[16px] font-bold leading-tight">
          {s.kills}/{s.deaths}/{s.assists}
        </div>
        <div className="tnum text-[11px] leading-snug text-muted-foreground/80">{s.shortTime}</div>
      </div>
      <span
        className={cn(
          "absolute right-3 top-2.5 text-[12px] font-semibold",
          LABEL_CLASS[r.tone],
        )}
      >
        {r.label}
      </span>
    </button>
  );
}
