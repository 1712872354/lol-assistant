import { useQuery } from "@tanstack/react-query";
import { Check, Minus, MousePointerClick, RotateCcw, X } from "lucide-react";
import { useMemo } from "react";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { MatchDetail, PlayerRow, RankedInfo } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useHistoryStore, type HistoryTab } from "@/stores/historyStore";
import { errMsg, fetchMatchDetail, fetchPlayersRanked } from "./api";
import { buildRankedMap, fmtK, lookupRanked, normId } from "./format";


import type { Tone } from "@/lib/tone";
import { TONE_FILL, TONE_HEADER, TONE_TEXT } from "./badges";
import { PlayerDataTable } from "./PlayerDataTable";

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
      (best, p) => (!best || p.matchRating > best.matchRating ? p : best),
      null,
    );
    if (mvp) m.set(mvp.participantId, "MVP");
    for (const team of detail.teams) {
      if (team.players.length === 0) continue;
      const top = team.players.reduce<PlayerRow | null>(
        (best, p) => (!best || p.matchRating > best.matchRating ? p : best),
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
                  "flex shrink-0 flex-wrap items-center gap-x-2.5 gap-y-1 border-b border-border/50 px-3 py-1.5",
                  TONE_HEADER[tone],
                )}
              >
                <Icon className="h-3.5 w-3.5 shrink-0" />
                <span className="text-[13px] font-semibold tracking-wide">{result}</span>
                <span className="rounded-sm bg-background/40 px-1.5 py-0.5 text-[11px] font-medium">
                  {side}
                </span>
                <span className="ml-auto flex flex-wrap items-center justify-end gap-x-2.5 gap-y-0.5 text-[11px] text-muted-foreground">
                  <span className="stat-num text-foreground/90" title="队伍 K/D/A">
                    {team.kills}
                    <span className="mx-0.5 text-muted-foreground/40">/</span>
                    {team.deaths}
                    <span className="mx-0.5 text-muted-foreground/40">/</span>
                    {team.assists}
                  </span>
                  <span className="opacity-30">·</span>
                  <span className="tnum" title="队伍经济">
                    {fmtK(team.gold)}
                  </span>
                  <span className="opacity-30">·</span>
                  <span className="tnum" title="对局时长">
                    {detail.duration}
                  </span>
                </span>
              </div>
              <div className="flex min-h-0 flex-1 flex-col">
                <PlayerDataTable
                  data={team.players.map((p) => ({
                    p,
                    ranked: findRanked(p),
                    badge: badges.get(p.participantId),
                    tone,
                    maxDamage: peaks.damage,
                    maxGold: peaks.gold,
                  }))}
                />
              </div>
            </div>
          );
        })}
      </div>

      {/* 底部汇总：时间拉开间距，胜负色区分双方数值 */}
      <div className="flex shrink-0 flex-wrap items-center gap-x-2.5 gap-y-1 border-t border-border/60 bg-muted/25 px-4 py-1.5 text-[11px] text-muted-foreground">
        <span className="tnum">{detail.time}</span>
        <span className="opacity-30">·</span>
        <span className="tnum">用时 {detail.durationMin}</span>
        <span className="opacity-30">·</span>
        <span className="tnum whitespace-nowrap">
          击杀 <span className={cn("font-semibold", selfTone)}>{selfTeam.kills}</span>
          <span className="mx-0.5 opacity-40">/</span>
          <span className={cn("font-semibold", otherTone)}>{otherSum.kills}</span>
        </span>
        <span className="opacity-30">·</span>
        <span className="tnum whitespace-nowrap">
          金钱 <span className={cn("font-semibold", selfTone)}>{fmtK(selfTeam.gold)}</span>
          <span className="mx-0.5 opacity-40">/</span>
          <span className={cn("font-semibold", otherTone)}>{fmtK(otherSum.gold)}</span>
        </span>
      </div>
    </div>
  );
}
