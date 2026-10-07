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
  win: "text-win-fg font-bold",
  loss: "text-loss-fg font-bold",
  remake: "text-remake-fg font-semibold",
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
      aria-current={active ? "true" : undefined}
      className={cn(
        "group relative flex w-full min-h-0 flex-1 items-center gap-2.5 rounded-md border border-l-[3px] px-2.5 text-left",
        "transition-all duration-150 ease-out",
        active
          ? "border-l-selected-ring border-selected-ring/60 bg-selected-bg shadow-[0_0_0_1px_rgba(200,170,110,0.15),0_2px_12px_rgba(200,170,110,0.06)]"
          : cn(BAR_CLASS[r.tone], "border-border/50 bg-card/80 hover:bg-card-hover hover:border-border-bright/50"),
        className,
      )}
    >
      <AssetImg
        kind="champion"
        id={s.championId}
        size={40}
        className="shrink-0 rounded-md ring-1 ring-black/10"
      />
      <div className="min-w-0 flex-1 space-y-0.5">
        <div className="flex min-w-0 items-baseline gap-1.5">
          <span className="truncate text-[11px] font-medium leading-none text-muted-foreground">
            {modeLabel}
          </span>
          <span className="tnum shrink-0 text-[11px] leading-none text-muted-foreground/75">
            {s.duration}
          </span>
        </div>
        <div className="stat-num text-[19px] font-bold leading-none text-foreground">
          <span className="sr-only">KDA </span>
          {s.kills}
          <span aria-hidden className="mx-1 text-[13px] font-normal text-muted-foreground/30">/</span>
          {s.deaths}
          <span aria-hidden className="mx-1 text-[13px] font-normal text-muted-foreground/30">/</span>
          {s.assists}
        </div>
        <div className="tnum text-[11px] leading-none text-muted-foreground/75">
          {s.shortTime}
        </div>
      </div>
      <span
        className={cn(
          "absolute right-2.5 top-2.5 rounded px-1.5 py-px text-[11px] tracking-wider",
          LABEL_CLASS[r.tone],
          r.tone === "win" && "bg-win-bg",
          r.tone === "loss" && "bg-loss-bg",
          r.tone === "remake" && "bg-remake-bg",
        )}
      >
        {r.label}
      </span>
    </button>
  );
}
