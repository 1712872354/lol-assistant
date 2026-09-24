import { Crown } from "lucide-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { AssetImg } from "@/lib/AssetImg";
import { UNRANKED } from "@/lib/rank";
import { toSummonerResult } from "@/lib/summoner";
import type { Tone } from "@/lib/tone";
import type { PlayerRow, RankedInfo } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useHistoryStore } from "@/stores/historyStore";
import { fmtNum, rankedDisplay } from "./format";
import { StatBar } from "./StatBar";

export const COLS = "minmax(240px,1.8fr) 128px 96px 96px 72px 180px 64px";

export interface RowProps {
  p: PlayerRow;
  ranked?: RankedInfo;
  tone: Tone;
  maxDamage: number;
  maxGold: number;
  badge?: "MVP" | "ACE";
}

/** 玩家行：头像区 + 名称/段位分行呼吸 + KDA 注脚 + 千分位数值与相对条 + 装备 + 评分 */
export function PlayerRowView({ p, ranked, tone, maxDamage, maxGold, badge }: RowProps) {
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const hash = p.name.indexOf("#");
  const base = hash >= 0 ? p.name.slice(0, hash) : p.name;
  const tag = hash >= 0 ? p.name.slice(hash) : "";
  const rankText = rankedDisplay(p, ranked);
  const hasRank = rankText !== UNRANKED;
  const slots = Array.from({ length: 7 }, (_, i) => (p.items?.[i] ?? 0));
  const puuid = p.puuid?.trim() ?? "";

  /** 点名称：新标签展示该玩家战绩（同 puuid 仅激活） */
  const openPlayerTab = () => {
    if (!puuid) return;
    const s = toSummonerResult({
      puuid,
      gameName: base,
      tagLine: tag ? tag.slice(1) : "",
      profileIconId: p.profileIconId ?? 0,
      summonerId: p.summonerId ?? "",
    });
    openSummoner(s, p.isSelf);
  };

  return (
    <div
      className={cn(
        "grid min-h-0 flex-1 items-center border-b px-4 transition-colors hover:bg-muted/30",
        p.isSelf && "relative z-[1] bg-self-bg",
      )}
      style={{ gridTemplateColumns: COLS }}
    >
      {/* 玩家信息：头像组与文字拉开；名称与段位分两行，避免挤在 baseline 上 */}
      <div className="flex min-w-0 items-center gap-3 pr-3">
        <div className="relative shrink-0">
          <AssetImg kind="champion" id={p.championId} size={44} className="rounded-md" />
          <span className="absolute -bottom-0.5 -right-0.5 rounded bg-black/75 px-1 text-[10px] leading-tight text-white tnum">
            {p.champLevel}
          </span>
        </div>
        <div className="flex shrink-0 flex-col gap-1">
          <AssetImg kind="spell" id={p.spell1Id} size={16} className="rounded-[2px]" />
          <AssetImg kind="spell" id={p.spell2Id} size={16} className="rounded-[2px]" />
        </div>
        <AssetImg kind="perk" id={p.runeId} size={16} className="shrink-0 rounded-full" />

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2">
            <button
              type="button"
              onClick={openPlayerTab}
              disabled={!puuid}
              title={puuid ? "新标签查看该玩家战绩" : undefined}
              className={cn(
                "min-w-0 truncate text-left text-[14px] leading-tight",
                p.isSelf ? "font-bold" : "font-medium",
                puuid && "cursor-pointer underline-offset-2 hover:text-primary hover:underline",
                !puuid && "cursor-default",
              )}
            >
              {base}
              {tag ? (
                <span className="ml-1 shrink-0 text-[12px] leading-tight text-muted-foreground">
                  {tag}
                </span>
              ) : null}
            </button>
            {badge ? (
              <span
                className={cn(
                  "shrink-0 rounded px-1 py-px text-[10px] font-bold leading-tight tracking-wide",
                  badge === "MVP"
                    ? "bg-amber-500/15 text-amber-700 dark:text-amber-300"
                    : "bg-zinc-500/15 text-zinc-600 dark:text-zinc-300",
                )}
              >
                {badge}
              </span>
            ) : null}
          </div>
          <span
            className={cn(
              "inline-flex items-center gap-1 text-[12px] leading-tight",
              hasRank ? "text-amber-700 dark:text-amber-300" : "text-muted-foreground",
            )}
          >
            <Crown
              className={cn("h-3 w-3 shrink-0", hasRank ? "text-amber-500" : "opacity-35")}
            />
            {rankText}
          </span>
        </div>
      </div>

      {/* KDA：主数值与注脚拉开 */}
      <div className="flex flex-col items-center justify-center gap-0.5 leading-tight">
        <div className="tnum whitespace-nowrap text-[15px] font-bold">
          {p.kills} / {p.deaths} / {p.assists}
        </div>
        <div className="tnum whitespace-nowrap text-[11px] text-muted-foreground">
          kda:{p.kda} · {p.killParticipation}%
        </div>
      </div>

      {/* 伤害 / 金钱：千分位 + 全场相对条 */}
      <div className="pr-1 text-right">
        <div className="tnum text-[13px] font-medium leading-tight">{fmtNum(p.totalDamage)}</div>
        <StatBar value={p.totalDamage} max={maxDamage} tone={tone} />
      </div>
      <div className="pr-1 text-right">
        <div className="tnum text-[13px] font-medium leading-tight">{fmtNum(p.gold)}</div>
        <StatBar value={p.gold} max={maxGold} tone={tone} />
      </div>

      <Tooltip>
        <TooltipTrigger asChild>
          <span
            className={cn(
              "tnum cursor-help text-right text-[14px] font-semibold",
              p.dmgRatio >= 1 ? "text-good-fg" : "text-destructive",
            )}
          >
            {Number.isFinite(p.dmgRatio) ? p.dmgRatio.toFixed(1) : "—"}
          </span>
        </TooltipTrigger>
        <TooltipContent>伤转 {Number.isFinite(p.dmgRatio) ? p.dmgRatio.toFixed(2) : "—"} · 伤害/本组均伤</TooltipContent>
      </Tooltip>

      {/* 装备：7 固定槽位 */}
      <div className="flex items-center gap-[3px] pl-1">
        {slots.map((id, i) =>
          id > 0 ? (
            <AssetImg key={`it-${i}`} kind="item" id={id} size={24} className="rounded-[3px]" />
          ) : (
            <span key={`it-${i}`} aria-hidden className="inline-block h-6 w-6 shrink-0" />
          ),
        )}
      </div>

      <span className="tnum text-right text-[15px] font-bold">{Number.isFinite(p.matchRating) ? p.matchRating.toFixed(1) : "—"}</span>
    </div>
  );
}
