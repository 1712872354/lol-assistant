import { useQuery } from "@tanstack/react-query";
import { Check, Crown, Minus, MousePointerClick, RotateCcw, X } from "lucide-react";
import { useMemo } from "react";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { MatchDetail, PlayerRow, RankedInfo, SummonerResult } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useHistoryStore, type HistoryTab } from "@/stores/historyStore";
import { errMsg, fetchMatchDetail, fetchPlayersRanked } from "./api";
import { AssetImg } from "./AssetImg";
import { buildRankedMap, fmtK, fmtNum, lookupRanked, normId, rankedDisplay } from "./format";

/**
 * 列宽：玩家(含段位) | KDA | 伤害 | 金钱 | 伤转 | 装备(7×24) | 评分
 * 数值右对齐 + 千分位；伤害/金钱带相对条；顶部轻量表头
 */
const COLS = "minmax(240px,1.8fr) 128px 96px 96px 72px 180px 64px";

type Tone = "win" | "loss" | "remake";

const TONE_TEXT: Record<Tone, string> = {
  win: "text-team-win-fg",
  loss: "text-team-loss-fg",
  remake: "text-muted-foreground",
};
const TONE_HEADER: Record<Tone, string> = {
  win: "bg-team-win-bg text-team-win-fg",
  loss: "bg-team-loss-bg text-team-loss-fg",
  remake: "bg-muted text-muted-foreground",
};
const TONE_FILL: Record<Tone, string> = {
  win: "bg-team-win-bg/20",
  loss: "bg-team-loss-bg/20",
  remake: "bg-muted/30",
};

/** 伤害/金钱相对条：相对全场最高值 */
function StatBar({ value, max, tone }: { value: number; max: number; tone: Tone }) {
  const pct = max > 0 ? Math.max(4, Math.round((value / max) * 100)) : 0;
  const bar =
    tone === "win" ? "bg-team-win-fg/45" : tone === "loss" ? "bg-team-loss-fg/45" : "bg-muted-foreground/35";
  return (
    <div className="mt-1 h-[3px] w-full overflow-hidden rounded-full bg-muted/80">
      <div className={cn("h-full rounded-full", bar)} style={{ width: `${pct}%` }} />
    </div>
  );
}

/** 列头：解释裸数字含义，扫读时不用猜 */
function ColumnHeader() {
  return (
    <div
      className="grid shrink-0 items-end gap-0 border-b bg-muted/40 px-4 pb-1.5 pt-2 text-[11px] font-medium tracking-wide text-muted-foreground"
      style={{ gridTemplateColumns: COLS }}
    >
      <span className="pr-3">玩家</span>
      <span className="text-center">KDA</span>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="cursor-help text-right">伤害</span>
        </TooltipTrigger>
        <TooltipContent>对英雄伤害 · 条为全场相对值</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="cursor-help text-right">金钱</span>
        </TooltipTrigger>
        <TooltipContent>本局获得金币 · 条为全场相对值</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="cursor-help text-right">伤转</span>
        </TooltipTrigger>
        <TooltipContent>个人伤害 / 本组平均伤害，≥1 为高于均值</TooltipContent>
      </Tooltip>
      <span className="pl-1">装备</span>
      <span className="text-right">评分</span>
    </div>
  );
}

/**
 * 右侧对局明细：
 *   轻量列头 → 队伍结果表头（胜负 + 我方/对方 + KDA/经济/时长）
 *   本人行浅紫；MVP 金标 / 败方 ACE 银标；伤害/金钱千分位 + 相对条
 */
export function MatchDetailPanel({ tab }: { tab: HistoryTab }) {
  const gameId = useHistoryStore((s) => s.selections[tab.puuid] ?? null);

  const detailQ = useQuery({
    // 视角相关数据：缓存键必须含 selfPuuid，避免多标签串「我方/对方」
    queryKey: ["hist", "detail", gameId, tab.puuid],
    queryFn: () => fetchMatchDetail(gameId as number, tab.puuid),
    enabled: gameId != null,
  });
  const detail: MatchDetail | null = detailQ.data ?? null;

  const rankedIds = useMemo(() => {
    const ids = new Set<string>();
    const tabSid = normId(tab.summonerId);
    const tabPuuid = normId(tab.puuid);
    if (tabSid) ids.add(tabSid);
    if (tabPuuid) ids.add(tabPuuid);
    if (!detail) return [...ids];
    for (const t of detail.teams) {
      for (const p of t.players) {
        const sid = normId((p as { summonerId?: unknown }).summonerId);
        const puuid = normId((p as { puuid?: unknown }).puuid);
        if (sid) ids.add(sid);
        if (puuid) ids.add(puuid);
      }
    }
    return [...ids];
  }, [detail, tab.summonerId, tab.puuid]);

  const rankedQ = useQuery({
    queryKey: ["hist", "ranked", gameId, tab.id, rankedIds.slice().sort().join(",")],
    queryFn: () => fetchPlayersRanked(rankedIds),
    enabled: rankedIds.length > 0,
  });

  const rankedMap = useMemo(() => buildRankedMap(rankedQ.data), [rankedQ.data]);

  const selfExtraKeys = useMemo(
    () => [normId(tab.summonerId), normId(tab.puuid)],
    [tab.summonerId, tab.puuid],
  );

  const findRanked = (p: PlayerRow): RankedInfo | undefined => {
    const extra = p.isSelf ? selfExtraKeys : [];
    return lookupRanked(rankedMap, p, extra);
  };

  /** 全场伤害/金钱峰值，供相对条使用 */
  const peaks = useMemo(() => {
    let damage = 0;
    let gold = 0;
    for (const t of detail?.teams ?? []) {
      for (const p of t.players) {
        if (p.totalDamage > damage) damage = p.totalDamage;
        if (p.gold > gold) gold = p.gold;
      }
    }
    return { damage, gold };
  }, [detail]);

  /** MVP=全场评分第 1；ACE=败方（或非 MVP 所在侧）队内评分最高 */
  const badges = useMemo(() => {
    const m = new Map<number, "MVP" | "ACE">();
    if (!detail) return m;
    const all = detail.teams.flatMap((t) => t.players);
    const mvp = all.reduce<PlayerRow | null>(
      (best, p) => (!best || p.rating > best.rating ? p : best),
      null,
    );
    if (mvp) m.set(mvp.participantId, "MVP");
    for (const team of detail.teams) {
      if (team.players.length === 0) continue;
      const top = team.players.reduce<PlayerRow | null>(
        (best, p) => (!best || p.rating > best.rating ? p : best),
        null,
      );
      if (top && !m.has(top.participantId) && (detail.remake || !team.win)) {
        m.set(top.participantId, "ACE");
      }
    }
    return m;
  }, [detail]);

  if (gameId == null) {
    return (
      <EmptyState
        className="m-4 flex-1 border-0 bg-transparent"
        icon={MousePointerClick}
        title="自动展示最近一场"
        desc="战绩加载后自动选中最近一场对局；也可点击左侧卡片切换查看。"
      />
    );
  }

  if (detailQ.isPending) {
    return (
      <div className="space-y-2 p-4">
        <Skeleton className="h-8 w-1/2" />
        {Array.from({ length: 8 }).map((_, i) => (
          <Skeleton key={i} className="h-10 w-full" />
        ))}
      </div>
    );
  }

  if (detailQ.isError || !detail) {
    return (
      <div className="m-4 space-y-2 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
        <p className="font-medium text-destructive">对局明细加载失败</p>
        <p className="break-all text-xs text-muted-foreground">{errMsg(detailQ.error)}</p>
        <Button size="sm" variant="outline" onClick={() => detailQ.refetch()}>
          <RotateCcw className="mr-1 h-3 w-3" />
          重试
        </Button>
      </div>
    );
  }

  const teams = detail.teams ?? [];
  const selfTeam = teams[0];
  if (!selfTeam) {
    return (
      <div className="m-4 space-y-2 rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
        <p className="font-medium text-destructive">对局明细数据不完整</p>
        <p className="break-all text-xs text-muted-foreground">缺少队伍信息</p>
        <Button size="sm" variant="outline" onClick={() => detailQ.refetch()}>
          <RotateCcw className="mr-1 h-3 w-3" />
          重试
        </Button>
      </div>
    );
  }
  const otherSum = teams.slice(1).reduce(
    (acc, t) => ({
      kills: acc.kills + (t.kills ?? 0),
      gold: acc.gold + (t.gold ?? 0),
      damage: acc.damage + (t.damage ?? 0),
    }),
    { kills: 0, gold: 0, damage: 0 },
  );
  const toneOf = (win: boolean): Tone =>
    detail.remake ? "remake" : win ? "win" : "loss";
  // 多队伍（竞技场）不按两队胜负取反着色
  const multiTeam = teams.length > 2 || !!detail.arena;
  const selfTone = TONE_TEXT[toneOf(selfTeam.win)];
  const otherTone = multiTeam
    ? TONE_TEXT["remake"]
    : TONE_TEXT[toneOf(!selfTeam.win)];

  return (
    <div className="flex h-full min-h-0 flex-col">
      <ColumnHeader />

      <div className="flex min-h-0 flex-1 flex-col">
        {detail.teams.map((team, ti) => {
          const tone = toneOf(team.win);
          const side =
            detail.arena && team.placement > 0
              ? `第 ${team.placement} 组${team.placement === 1 ? "（冠军队）" : ""}`
              : ti === 0
                ? "我方"
                : "对方";
          const result = detail.remake ? "无效" : team.win ? "胜利" : "失败";
          const Icon = detail.remake ? Minus : team.win ? Check : X;
          return (
            <div
              key={`${team.teamId}-${team.placement}-${ti}`}
              className={cn("flex min-h-0 flex-1 flex-col overflow-hidden", TONE_FILL[tone])}
            >
              <div
                className={cn(
                  "flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b px-4 py-2",
                  TONE_HEADER[tone],
                )}
              >
                <Icon className="h-3.5 w-3.5 shrink-0" />
                <span className="text-[13px] font-semibold">{result}</span>
                <span className="text-[12px] font-medium opacity-90">{side}</span>
                <span className="ml-auto flex flex-wrap items-center justify-end gap-x-3 gap-y-0.5 text-[12px]">
                  <span className="tnum font-bold" title="队伍 K/D/A">
                    {team.kills}/{team.deaths}/{team.assists}
                  </span>
                  <span className="tnum opacity-80" title="队伍经济">
                    {fmtK(team.gold)}
                  </span>
                  <span className="tnum opacity-70" title="对局时长">
                    {detail.duration}
                  </span>
                </span>
              </div>
              <div className="flex min-h-0 flex-1 flex-col">
                {team.players.map((p) => (
                  <PlayerRowView
                    key={p.participantId}
                    p={p}
                    ranked={findRanked(p)}
                    tone={tone}
                    maxDamage={peaks.damage}
                    maxGold={peaks.gold}
                    badge={badges.get(p.participantId)}
                  />
                ))}
                {Array.from({ length: Math.max(0, 5 - team.players.length) }).map((_, i) => (
                  <div key={`pad-${i}`} className="min-h-0 flex-1" aria-hidden />
                ))}
              </div>
            </div>
          );
        })}
      </div>

      {/* 底部汇总：时间拉开间距，胜负色区分双方数值 */}
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-t px-4 py-2 text-[12px] text-muted-foreground">
        <span className="tnum">{detail.time}</span>
        <span className="opacity-30">·</span>
        <span className="tnum">用时 {detail.durationMin}</span>
        <span className="opacity-30">·</span>
        <span className="tnum whitespace-nowrap">
          击杀 <span className={selfTone}>{selfTeam.kills}</span>
          <span className="mx-0.5 opacity-50">/</span>
          <span className={otherTone}>{otherSum.kills}</span>
        </span>
        <span className="opacity-30">·</span>
        <span className="tnum whitespace-nowrap">
          金钱 <span className={selfTone}>{fmtK(selfTeam.gold)}</span>
          <span className="mx-0.5 opacity-50">/</span>
          <span className={otherTone}>{fmtK(otherSum.gold)}</span>
        </span>
      </div>
    </div>
  );
}

interface RowProps {
  p: PlayerRow;
  ranked?: RankedInfo;
  tone: Tone;
  maxDamage: number;
  maxGold: number;
  badge?: "MVP" | "ACE";
}

/** 玩家行：头像区 + 名称/段位分行呼吸 + KDA 注脚 + 千分位数值与相对条 + 装备 + 评分 */
function PlayerRowView({ p, ranked, tone, maxDamage, maxGold, badge }: RowProps) {
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const hash = p.name.indexOf("#");
  const base = hash >= 0 ? p.name.slice(0, hash) : p.name;
  const tag = hash >= 0 ? p.name.slice(hash) : "";
  const rankText = rankedDisplay(p, ranked);
  const hasRank = rankText !== "未定级";
  const slots = Array.from({ length: 7 }, (_, i) => (p.items?.[i] ?? 0));
  const puuid = p.puuid?.trim() ?? "";

  /** 点名称：新标签展示该玩家战绩（同 puuid 仅激活） */
  const openPlayerTab = () => {
    if (!puuid) return;
    const s: SummonerResult = {
      puuid,
      gameName: base,
      tagLine: tag ? tag.slice(1) : "",
      displayName: tag ? `${base}${tag}` : base,
      profileIconId: p.profileIconId ?? 0,
      summonerLevel: 0,
      summonerId: p.summonerId ?? "",
    };
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
          kda:{p.kda} · {p.killPct}%
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

      <span className="tnum text-right text-[15px] font-bold">{Number.isFinite(p.rating) ? p.rating.toFixed(1) : "—"}</span>
    </div>
  );
}
