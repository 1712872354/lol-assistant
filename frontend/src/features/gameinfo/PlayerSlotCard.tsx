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
  /** 仅队伍首个空槽渲染说明文案（避免 5 连重复） */
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
        ? "房间内暂无其他玩家"
        : "等待敌方玩家数据（对局开始后自动补全）";
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
  const sample = slot.winRateSample ?? 0;
  const avgKda = slot.avgKda ?? 0;
  const wrTone = threatTone(winRate, 45, 55);
  const kdaTone = threatTone(avgKda, 2.5, 4.0);

  // 队列去重：近况几乎同队列时不重复全称
  const queueNames = new Set(recent.map((r) => r.queueName || r.queueShort || ""));
  const uniformQueue = queueNames.size <= 1;

  const copyRiotId = (e: MouseEvent) => {
    e.stopPropagation();
    void navigator.clipboard?.writeText(tag ? `${base}${tag}` : base);
  };

  // 头像优先本局所选英雄；未选（大厅/未锁定）回退召唤师头像
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
        "flex h-full min-h-0 flex-col overflow-hidden rounded-lg border",
        slot.isSelf
          ? "border-self-card-border bg-self-card-bg shadow-[inset_3px_0_0_0_var(--brand-gold)]"
          : ally
            ? "border-ally-border/55 bg-card"
            : "border-enemy-border/55 bg-card",
      )}
    >
      {/* 头部：点击身份区即可进战绩页（复制按钮绝对定位为兄弟节点，避免嵌套交互） */}
      <div className="relative shrink-0 space-y-1.5 border-b border-border/50 px-2.5 py-2 transition-colors hover:bg-muted/25">
        <button
          type="button"
          onClick={openDetail}
          disabled={!slot.puuid}
          title="在战绩页查看该玩家"
          className="block w-full text-left disabled:pointer-events-none"
        >
          <div className="flex items-start gap-2.5">
            <AssetImg
              kind={avatarKind}
              id={avatarId}
              size={52}
              className="shrink-0 rounded-full ring-1 ring-black/5"
            />
            <div className="min-w-0 flex-1 pr-6">
              <div className="flex items-center gap-1">
                <span className="truncate text-sm font-semibold leading-tight">{base}</span>
                {tag ? (
                  <span className="shrink-0 text-[10px] leading-tight text-muted-foreground">{tag}</span>
                ) : null}
              </div>
              {/* 段位：主/灵活并排徽章，同屏一行可放下 */}
              <div className="mt-1.5 flex min-w-0 flex-wrap items-center gap-1">
                {hasRank ? (
                  <>
                    {solo.main ? (
                      <RankChip main={solo.main} lp={solo.lp} title="单双排位" />
                    ) : null}
                    {flex.main ? (
                      <RankChip main={flex.main} lp={flex.lp} variant="secondary" title="灵活排位" />
                    ) : null}
                  </>
                ) : (
                  <span className="text-[11px] text-muted-foreground">{UNRANKED}</span>
                )}
              </div>
            </div>
          </div>

        <div className="flex items-end justify-between gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-1">
              <span
                className={cn(
                  "stat-num text-[22px] leading-none",
                  wrTone === "good" && "text-good-fg",
                  wrTone === "bad" && "text-loss-fg",
                  wrTone === "mid" && "text-amber-300",
                )}
              >
                {winRate.toFixed(1)}%
              </span>
              <span
                className="text-[10px] text-muted-foreground"
                title={`近 ${sample || 0} 场胜率样本`}
              >
                近{sample || 0}场
              </span>
            </div>
            <ThreatBar value={winRate} tone={wrTone} />
          </div>
          <div className="min-w-0 flex-1 text-right">
            <div className="flex items-baseline justify-end gap-1">
              <span
                className={cn(
                  "stat-num text-xl leading-none",
                  kdaTone === "good" && "text-good-fg",
                  kdaTone === "bad" && "text-loss-fg",
                )}
              >
                {avgKda.toFixed(2)}
              </span>
              <span className="text-[11px] text-muted-foreground">KDA</span>
            </div>
            <ThreatBar value={avgKda} max={6} tone={kdaTone} />
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          {slot.hiddenCareer ? (
            <span className="rounded border border-border/70 bg-career-bg px-1.5 py-0.5 text-[10px] font-semibold text-muted-foreground">
              生涯隐藏
            </span>
          ) : null}
          <span
            className="rounded bg-rating-bg px-1.5 py-0.5 text-[10px] font-semibold text-brand-gold"
            title="综合评分 = 胜率/20 + 近况均 KDA"
          >
            综合 {(slot.playerScore ?? 0).toFixed(1)}
          </span>
          <div className="ml-auto flex items-center gap-1">
            {recent.slice(0, 5).map((r, i) => (
              <AssetImg
                key={i}
                kind="champion"
                id={r.championId}
                size={24}
                className="rounded-[4px]"
                title={`${r.kills}/${r.deaths}/${r.assists}`}
              />
            ))}
          </div>
        </div>
        </button>
        {/* 复制按钮：身份按钮之外的兄弟节点（绝对定位右上角），键盘可达 */}
        <button
          type="button"
          title="复制 Riot ID"
          aria-label="复制 Riot ID"
          onClick={copyRiotId}
          className="absolute right-2 top-2 shrink-0 rounded p-1 text-muted-foreground/60 transition-colors hover:text-foreground"
        >
          <Copy className="h-3 w-3" />
        </button>
      </div>

      <RecentMatchList
        recent={recent}
        uniformQueue={uniformQueue}
        hiddenCareer={slot.hiddenCareer}
      />

      {/* 底部次要入口：主入口已是头部 */}
      <button
        type="button"
        onClick={openDetail}
        disabled={!slot.puuid}
        title="在战绩页查看该玩家"
        className="flex shrink-0 items-center justify-center gap-0.5 border-t border-border/50 py-1.5 text-center text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
      >
        战绩详情
        <ChevronRight className="h-3 w-3" />
      </button>
    </div>
  );
}
