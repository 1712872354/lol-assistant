import { ChevronRight, Copy } from "lucide-react";
import type { MouseEvent } from "react";
import { useNavigate } from "react-router-dom";

import { AssetImg } from "@/lib/AssetImg";
import { UNRANKED } from "@/lib/rank";
import { toSummonerResult } from "@/lib/summoner";
import type { GameinfoPlayerSlot, GameinfoRecentMatch } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useHistoryStore } from "@/stores/historyStore";
import { splitRank, threatTone } from "./format";
import { RankChip } from "./RankChip";
import { RecentMatchList } from "./RecentMatchList";
import { SlotSkeleton } from "./SlotSkeleton";
import { ThreatBar } from "./ThreatBar";

interface Props {
  slot: GameinfoPlayerSlot;
  teamKey: "ally" | "enemy";
  offline: boolean;
  showCaption?: boolean;
}

export function PlayerSlotCard({ slot, teamKey, offline, showCaption }: Props) {
  const ally = teamKey === "ally";
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const navigate = useNavigate();

  if (!slot.filled) {
    const caption = offline
      ? "未连接客户端"
      : ally
        ? "等待队友加入"
        : "等待对局开始";
    return <SlotSkeleton ally={ally} caption={caption} showCaption={!!showCaption} />;
  }

  const rawName = slot.gameName ?? "";
  const hash = rawName.indexOf("#");
  const base = hash >= 0 ? rawName.slice(0, hash) : rawName;
  const tag = slot.tagLine ? `#${slot.tagLine}` : hash >= 0 ? rawName.slice(hash) : "";
  const recent: GameinfoRecentMatch[] = slot.recent ?? [];
  const solo = splitRank(slot.solo);
  const flex = splitRank(slot.flex);
  const hasRank = solo.main !== "" || flex.main !== "";
  const winRate = slot.winRate ?? 0;
  const avgKda = slot.avgKda ?? 0;
  const wrTone = threatTone(winRate, 45, 55);
  const kdaTone = threatTone(avgKda, 2.5, 4.0);

  const queueNames = new Set(recent.map((r) => r.queueName || r.queueShort || ""));
  const uniformQueue = queueNames.size <= 1;

  const copyRiotId = (e: MouseEvent) => {
    e.stopPropagation();
    void navigator.clipboard?.writeText(tag ? `${base}${tag}` : base);
  };

  const champId = slot.championId ?? 0;
  const avatarKind = champId > 0 ? "champion" : "profile";
  const avatarId = champId > 0 ? champId : (slot.profileIconId ?? 0);

  const openDetail = () => {
    const puuid = slot.puuid?.trim();
    if (!puuid) return;
    const s = toSummonerResult({
      puuid,
      gameName: base,
      tagLine: tag ? tag.slice(1) : (slot.tagLine ?? ""),
      profileIconId: slot.profileIconId ?? 0,
      summonerId: slot.summonerId ?? "",
    });
    openSummoner(s, slot.isSelf);
    navigate("/history");
  };

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col overflow-hidden rounded-lg transition-all duration-200",
        slot.isSelf
          ? "bg-self-card-bg ring-1 ring-brand-gold/40 shadow-[0_2px_16px_rgba(200,170,110,0.08)]"
          : "bg-card ring-1 ring-border/40 hover:ring-border/70",
      )}
    >
      {/* 头部：头像 + 身份 + 段位 */}
      <div className="relative shrink-0 border-b border-border/30 px-3 pt-2.5 pb-2">
        <button
          type="button"
          onClick={openDetail}
          disabled={!slot.puuid}
          title="在战绩页查看该玩家"
          className="block w-full text-left disabled:pointer-events-none"
        >
          <div className="flex items-center gap-2.5">
            <div className="relative shrink-0">
              <AssetImg
                kind={avatarKind}
                id={avatarId}
                size={40}
                className="rounded-lg ring-1 ring-black/10"
              />
              {slot.isSelf ? (
                <div className="absolute -bottom-0.5 -right-0.5 h-2.5 w-2.5 rounded-full bg-brand-gold ring-2 ring-self-card-bg" />
              ) : null}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline gap-1">
                <span className="truncate text-[13px] font-semibold leading-tight">{base}</span>
                {tag ? (
                  <span className="shrink-0 text-[10px] leading-tight text-muted-foreground/70">{tag}</span>
                ) : null}
              </div>
              <div className="mt-1 flex min-w-0 flex-wrap items-center gap-1">
                {hasRank ? (
                  <>
                    {solo.main ? <RankChip main={solo.main} lp={solo.lp} title="单双排位" /> : null}
                    {flex.main ? <RankChip main={flex.main} lp={flex.lp} variant="secondary" title="灵活排位" /> : null}
                  </>
                ) : (
                  <span className="text-[10px] text-muted-foreground/60">{UNRANKED}</span>
                )}
              </div>
            </div>
          </div>

          {/* 核心数据：胜率 + KDA 并排 */}
          <div className="mt-2.5 flex items-end gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline gap-1">
                <span
                  className={cn(
                    "stat-num text-[22px] font-bold leading-none tracking-tight",
                    wrTone === "good" && "text-good-fg",
                    wrTone === "bad" && "text-loss-fg",
                    wrTone === "mid" && "text-amber-500",
                  )}
                >
                  {winRate.toFixed(1)}%
                </span>
                <span className="text-[10px] text-muted-foreground/60">胜率</span>
              </div>
              <ThreatBar value={winRate} tone={wrTone} />
            </div>
            <div className="h-6 w-px bg-border/30" />
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline gap-1">
                <span
                  className={cn(
                    "stat-num text-[22px] font-bold leading-none tracking-tight",
                    kdaTone === "good" && "text-good-fg",
                    kdaTone === "bad" && "text-loss-fg",
                  )}
                >
                  {avgKda.toFixed(2)}
                </span>
                <span className="text-[10px] text-muted-foreground/60">KDA</span>
              </div>
              <ThreatBar value={avgKda} max={6} tone={kdaTone} />
            </div>
          </div>

          {/* 评分 + 近况英雄缩略 */}
          <div className="mt-2 flex items-center gap-1.5">
            <span
              className="rounded bg-brand-gold/15 px-1.5 py-0.5 text-[10px] font-semibold text-brand-gold"
              title="综合评分 = 胜率/20 + 近况均 KDA"
            >
              {(slot.playerScore ?? 0).toFixed(1)}
            </span>
            {slot.hiddenCareer ? (
              <span className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground/70">
                隐藏
              </span>
            ) : null}
            <div className="ml-auto flex items-center gap-0.5">
              {recent.slice(0, 5).map((r, i) => (
                <AssetImg
                  key={i}
                  kind="champion"
                  id={r.championId}
                  size={20}
                  className="rounded ring-1 ring-black/10"
                  title={`${r.kills}/${r.deaths}/${r.assists}`}
                />
              ))}
            </div>
          </div>
        </button>
        <button
          type="button"
          title="复制 Riot ID"
          aria-label="复制 Riot ID"
          onClick={copyRiotId}
          className="absolute right-1.5 top-1.5 rounded p-1 text-muted-foreground/40 transition-colors hover:bg-muted hover:text-foreground"
        >
          <Copy className="h-3 w-3" />
        </button>
      </div>

      <RecentMatchList recent={recent} uniformQueue={uniformQueue} hiddenCareer={slot.hiddenCareer} />

      <button
        type="button"
        onClick={openDetail}
        disabled={!slot.puuid}
        title="在战绩页查看该玩家"
        className="flex shrink-0 items-center justify-center gap-0.5 border-t border-border/20 py-1.5 text-[11px] text-muted-foreground/70 transition-colors hover:bg-muted/40 hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
      >
        战绩详情
        <ChevronRight className="h-3 w-3" />
      </button>
    </div>
  );
}
