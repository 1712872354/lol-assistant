import { ChevronRight, Copy } from "lucide-react";
import { AssetImg } from "@/features/history/AssetImg";
import type { GameinfoPlayerSlot, GameinfoRecentMatch, SummonerResult } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useHistoryStore } from "@/stores/historyStore";
import { relTime, splitRank, threatTone } from "./format";

interface Props {
  slot: GameinfoPlayerSlot;
  teamKey: "ally" | "enemy";
  offline: boolean;
  /** 仅队伍首个空槽渲染说明文案（避免 5 连重复） */
  showCaption?: boolean;
}

/** 段位徽章：单双琥珀底 / 灵活天蓝底，同规格；LP 弱化 */
function RankChip({
  main,
  lp,
  variant = "primary",
  title,
}: {
  main: string;
  lp: string;
  variant?: "primary" | "secondary";
  title: string;
}) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex max-w-full items-baseline gap-1 rounded-md px-1.5 py-[3px] leading-none",
        variant === "primary"
          ? "bg-amber-500/12 text-amber-800 dark:bg-amber-400/15 dark:text-amber-100"
          : "bg-sky-500/12 text-sky-800 dark:bg-sky-400/15 dark:text-sky-100",
      )}
    >
      <span className="truncate text-[12px] font-semibold">{main}</span>
      {lp ? (
        <span className="tnum shrink-0 text-[10px] font-medium opacity-65">{lp}</span>
      ) : null}
    </span>
  );
}

/** 胜率/KDA 相对条：0–100% 刻度，威胁色档 */
function ThreatBar({ value, max = 100, tone }: { value: number; max?: number; tone: "good" | "mid" | "bad" | "muted" }) {
  const pct = max > 0 ? Math.min(100, Math.max(3, (value / max) * 100)) : 0;
  const bar =
    tone === "good"
      ? "bg-emerald-500/70"
      : tone === "bad"
        ? "bg-red-500/65"
        : tone === "mid"
          ? "bg-amber-500/65"
          : "bg-muted-foreground/35";
  return (
    <div className="mt-1 h-[3px] w-full overflow-hidden rounded-full bg-muted/70">
      <div className={cn("h-full rounded-full", bar)} style={{ width: `${pct}%` }} />
    </div>
  );
}

/** 玩家槽位卡：空槽骨架；实槽=身份/统计/近况 + 顶部可点进战绩 */
export function PlayerSlotCard({ slot, teamKey, offline, showCaption }: Props) {
  const ally = teamKey === "ally";
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const setActiveView = useAppStore((s) => s.setActiveView);

  if (!slot.filled) {
    const caption = offline
      ? "未连接客户端"
      : ally
        ? "房间内暂无其他玩家"
        : "等待敌方玩家数据（对局开始后自动补全）";
    return (
      <div
        className={cn(
          "flex h-full min-h-0 flex-col overflow-hidden rounded-[14px] border p-2.5",
          ally
            ? "border-ally-border/40 bg-linear-to-b from-ally-bg/50 to-transparent"
            : "border-enemy-border/40 bg-linear-to-b from-enemy-bg/50 to-transparent",
        )}
      >
        <div className="flex shrink-0 items-center gap-2.5">
          <div
            className={cn(
              "h-11 w-11 shrink-0 rounded-xl",
              ally ? "bg-ally-soft/90" : "bg-enemy-soft/90",
            )}
          />
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            <div
              className={cn(
                "h-2 w-2/3 rounded-full",
                ally ? "bg-ally-border/35" : "bg-enemy-border/35",
              )}
            />
            <div
              className={cn(
                "h-2 w-1/3 rounded-full",
                ally ? "bg-ally-border/25" : "bg-enemy-border/25",
              )}
            />
          </div>
        </div>
        <div className="mt-3 flex min-h-0 flex-1 flex-col gap-[5px] overflow-hidden">
          {Array.from({ length: 9 }).map((_, i) => (
            <div
              key={i}
              className={cn(
                "h-[9px] shrink-0 rounded-full",
                ally ? "bg-ally-soft/90" : "bg-enemy-soft/90",
                i % 3 === 0 ? "w-full" : i % 3 === 1 ? "w-[93%]" : "w-[97%]",
              )}
            />
          ))}
        </div>
        {showCaption ? (
          <div className="mt-auto shrink-0 pt-2 text-center text-[11px] leading-snug text-muted-foreground">
            {caption}
          </div>
        ) : null}
      </div>
    );
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

  const copyRiotId = (e: React.MouseEvent) => {
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
    const s: SummonerResult = {
      puuid,
      gameName: base,
      tagLine: tag ? tag.slice(1) : (slot.tagLine ?? ""),
      displayName: tag ? `${base}${tag}` : base,
      profileIconId: slot.profileIconId ?? 0,
      summonerLevel: 0,
      summonerId: slot.summonerId ?? "",
    };
    openSummoner(s, slot.isSelf);
    setActiveView("history");
  };

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col overflow-hidden rounded-[14px] border shadow-sm",
        slot.isSelf
          ? "border-self-card-border bg-self-card-bg ring-1 ring-self-card-border/35"
          : ally
            ? "border-ally-border/50 bg-card"
            : "border-enemy-border/50 bg-card",
      )}
    >
      {/* 头部：点击身份区即可进战绩页 */}
      <button
        type="button"
        onClick={openDetail}
        disabled={!slot.puuid}
        title="在战绩页查看该玩家"
        className="shrink-0 space-y-1.5 border-b border-border/50 px-2.5 py-2 text-left transition-colors hover:bg-muted/25 disabled:pointer-events-none"
      >
        <div className="flex items-start gap-2.5">
          <AssetImg
            kind={avatarKind}
            id={avatarId}
            size={52}
            className="shrink-0 rounded-full ring-1 ring-black/5"
          />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1">
              <span className="truncate text-sm font-semibold leading-tight">{base}</span>
              {tag ? (
                <span className="shrink-0 text-[10px] leading-tight text-muted-foreground">{tag}</span>
              ) : null}
              <span
                role="button"
                tabIndex={0}
                title="复制 Riot ID"
                onClick={copyRiotId}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    void navigator.clipboard?.writeText(tag ? `${base}${tag}` : base);
                  }
                }}
                className="ml-auto shrink-0 text-muted-foreground/60 transition-colors hover:text-foreground"
              >
                <Copy className="h-3 w-3" />
              </span>
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
                <span className="text-[11px] text-muted-foreground">未定级</span>
              )}
            </div>
          </div>
        </div>

        <div className="flex items-end justify-between gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex items-baseline gap-1">
              <span
                className={cn(
                  "tnum text-[22px] font-bold leading-none",
                  wrTone === "good" && "text-good-fg",
                  wrTone === "bad" && "text-destructive",
                  wrTone === "mid" && "text-amber-600 dark:text-amber-400",
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
                  "tnum text-xl font-bold leading-none",
                  kdaTone === "good" && "text-good-fg",
                  kdaTone === "bad" && "text-destructive",
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
            <span className="rounded-md bg-career-bg px-1.5 py-0.5 text-[10px] font-semibold text-white">
              生涯隐藏
            </span>
          ) : null}
          <span
            className="rounded-md bg-rating-bg px-1.5 py-0.5 text-[10px] font-semibold text-white"
            title="综合评分 = 胜率/20 + 近况均 KDA"
          >
            综合 {(slot.rating ?? 0).toFixed(1)}
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

      {/* 近况列表：左侧胜负色条 + 英雄 + 时间/胜负 + KDA；队列去重 */}
      <div className="min-h-0 flex-1 space-y-1 overflow-y-auto p-1.5">
        {recent.map((r, i) => (
          <div
            key={i}
            className={cn(
              "flex items-center gap-2 rounded-lg border-l-[3px] px-2 py-1",
              r.win
                ? "border-l-win-bar bg-team-win-bg/55"
                : "border-l-loss-bar bg-team-loss-bg/55",
            )}
          >
            <AssetImg kind="champion" id={r.championId} size={28} className="rounded-md" />
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-1.5">
                {!uniformQueue ? (
                  <span className="shrink-0 rounded bg-muted px-1 py-px text-[10px] text-muted-foreground">
                    {r.queueShort || "对局"}
                  </span>
                ) : (
                  <span
                    className={cn(
                      "shrink-0 text-[11px] font-semibold",
                      r.win ? "text-team-win-fg" : "text-team-loss-fg",
                    )}
                  >
                    {r.win ? "胜" : "负"}
                  </span>
                )}
                {!uniformQueue ? (
                  <span className="truncate text-[11px] font-medium">
                    {r.queueName || r.queueShort || "对局"}
                  </span>
                ) : (
                  <span className="truncate text-[11px] text-muted-foreground">
                    {relTime(r.gameCreation, r.timeShort)}
                  </span>
                )}
              </div>
              {!uniformQueue ? (
                <div className="mt-0.5 text-[10px] leading-none text-muted-foreground">
                  {relTime(r.gameCreation, r.timeShort)}
                  <span className="mx-0.5">·</span>
                  <span
                    className={cn(
                      "font-semibold",
                      r.win ? "text-team-win-fg" : "text-team-loss-fg",
                    )}
                  >
                    {r.win ? "胜" : "负"}
                  </span>
                </div>
              ) : null}
            </div>
            <div className="tnum shrink-0 text-[13px] font-semibold">
              {r.kills}/{r.deaths}/{r.assists}
            </div>
          </div>
        ))}
        {recent.length === 0 ? (
          <div className="py-8 text-center text-[11px] text-muted-foreground">
            {slot.hiddenCareer ? "生涯隐藏 · 暂无近战数据" : "暂无近战数据"}
          </div>
        ) : null}
      </div>

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
