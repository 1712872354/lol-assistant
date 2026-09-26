import type { Tone } from "@/lib/tone";
import type { MatchSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { AssetImg } from "@/lib/AssetImg";
import { resultOf } from "./format";

/** 结果色调：左 3px 色轨扫读（胜=青绿 / 负=绯红 / 无效=灰），禁止大面积色底 */
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

/** 战绩卡片：左色条 + 头像 + 模式/时长 / KDA / 时间 + 右上胜负 */
export function MatchCard({ summary: s, active, onClick, className }: Props) {
  const r = resultOf(s);
  const modeLabel = s.queueName || s.queueShort || `#${s.queueId}`;
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group relative flex w-full min-h-0 flex-1 items-center gap-2.5 rounded-md border border-l-[3px] bg-card/90 px-2.5 text-left transition-colors hover:bg-accent/60",
        active
          ? "border-l-selected-ring border-selected-ring/70 bg-selected-bg/80 ring-1 ring-selected-ring/50"
          : cn(BAR_CLASS[r.tone], "border-border/55"),
        className,
      )}
    >
      <AssetImg
        kind="champion"
        id={s.championId}
        size={40}
        className="shrink-0 rounded-md"
      />
      <div className="min-w-0 flex-1 space-y-0.5">
        <div className="flex min-w-0 items-baseline gap-1.5">
          <span className="truncate text-[11px] font-medium leading-none text-muted-foreground">
            {modeLabel}
          </span>
          <span className="tnum shrink-0 text-[10px] leading-none text-muted-foreground/60">
            {s.duration}
          </span>
        </div>
        <div className="stat-num text-[18px] leading-none">
          {s.kills}
          <span className="mx-1 text-[13px] font-normal text-muted-foreground/40">/</span>
          {s.deaths}
          <span className="mx-1 text-[13px] font-normal text-muted-foreground/40">/</span>
          {s.assists}
        </div>
        <div className="tnum text-[10px] leading-none text-muted-foreground/55">
          {s.shortTime}
        </div>
      </div>
      <span
        className={cn(
          "absolute right-2.5 top-2.5 text-[11px] font-semibold tracking-wide",
          LABEL_CLASS[r.tone],
        )}
      >
        {r.label}
      </span>
    </button>
  );
}
