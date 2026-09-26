import { useQuery, useQueryClient } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { UNRANKED } from "@/lib/rank";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useHistoryStore, type HistoryTab } from "@/stores/historyStore";
import { errMsg, fetchMatches, fetchPlayersRanked } from "./api";
import { MatchCard } from "./MatchCard";
import { buildRankedMap, normId } from "./format";

/** 左列固定展示条数：10 条均分高度，不滚动 */
const VISIBLE_COUNT = 10;

/** 召唤师当前段位展示串（对齐参考图列表头） */
function pickSoloLabel(solo?: string, flex?: string): string {
  const s = (solo ?? "").trim();
  if (s && s !== UNRANKED) return s;
  const f = (flex ?? "").trim();
  if (f && f !== UNRANKED) return f;
  return "";
}

/**
 * 左侧战绩列表：只显示 10 条并铺满列高（无滚动）+ 底部分页
 * 列表头展示召唤师段位（黄金 IV 45 · 第 x/y 页）
 */
export function MatchList({ tab }: { tab: HistoryTab }) {
  const offline = useAppStore((s) => s.conn.state !== "connected");
  const page = useHistoryStore((s) => s.pages[tab.puuid] ?? 0);
  const setPage = useHistoryStore((s) => s.setPage);
  const selectedGameId = useHistoryStore((s) => s.selections[tab.puuid] ?? null);
  const select = useHistoryStore((s) => s.select);
  const queueFilter = useHistoryStore((s) => s.queueFilter);
  const queryClient = useQueryClient();
  const [jump, setJump] = useState("");

  const q = useQuery({
    queryKey: ["hist", "matches", tab.puuid, page],
    queryFn: () => fetchMatches(tab.puuid, page),
    enabled: !offline,
  });

  /** 标签页召唤师段位：summonerId 优先，否则用 puuid */
  const rankKeys = useMemo(() => {
    const keys: string[] = [];
    if (tab.summonerId) keys.push(tab.summonerId);
    if (tab.puuid && !keys.includes(tab.puuid)) keys.push(tab.puuid);
    return keys;
  }, [tab.summonerId, tab.puuid]);

  const rankQ = useQuery({
    queryKey: ["hist", "ranked", "tab", tab.id, rankKeys.join(",")],
    queryFn: () => fetchPlayersRanked(rankKeys),
    enabled: !offline && rankKeys.length > 0,
  });

  const selfRankLabel = useMemo(() => {
    const m = buildRankedMap(rankQ.data);
    const r =
      m.get(normId(tab.summonerId)) ||
      m.get(normId(tab.puuid)) ||
      (rankQ.data ?? [])[0];
    return pickSoloLabel(r?.solo, r?.flex);
  }, [rankQ.data, tab.summonerId, tab.puuid]);

  const summaries = q.data?.summaries ?? [];
  const filtered =
    queueFilter === "all"
      ? summaries
      : summaries.filter((s) => String(s.queueId) === queueFilter);
  /** 仅展示前 10 条，左列不滚动 */
  const list = useMemo(() => filtered.slice(0, VISIBLE_COUNT), [filtered]);

  // total/totalPages 未知时缺省：此时仅支持步进翻页（hasMore 决定能否前进）
  const totalPages = q.data?.totalPages ?? null;
  const hasMore = q.data?.hasMore ?? false;

  useEffect(() => {
    if (list.length === 0) {
      // 过滤后空列表：清掉残留选中，避免右侧明细与左侧不一致
      const st = useHistoryStore.getState();
      if ((st.selections[tab.puuid] ?? null) != null) {
        st.select(tab.puuid, null);
      }
      return;
    }
    const st = useHistoryStore.getState();
    const cur = st.selections[tab.puuid] ?? null;
    if (cur == null || !list.some((s) => s.gameId === cur)) {
      st.select(tab.puuid, list[0].gameId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q.dataUpdatedAt, tab.puuid, queueFilter]);

  const goto = (p: number) => {
    // 总页数已知 → 夹到最后一页；未知 → 仅允许步进到已确认的下一页
    const max = totalPages != null ? totalPages - 1 : hasMore ? page + 1 : page;
    const v = Math.min(Math.max(0, p), max);
    if (v !== page) setPage(tab.puuid, v);
  };
  const commitJump = () => {
    if (jump !== "") {
      goto(parseInt(jump, 10) - 1);
      setJump("");
    }
  };

  return (
    <>
      <div className="shrink-0 border-b border-border/60 bg-muted/20 px-3 py-2">
        <div className="flex items-center justify-between gap-1">
          <div className="flex min-w-0 items-center gap-1.5">
            <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-brand-gold" />
            <span className="truncate text-[12.5px] font-semibold">
              {tab.name.split("#")[0] || tab.name}
            </span>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="press-scale h-6 w-6 shrink-0 rounded-full"
            disabled={offline}
            aria-label="刷新战绩"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: ["hist", "matches", tab.puuid] });
              void queryClient.invalidateQueries({ queryKey: ["hist", "ranked"] });
            }}
          >
            <RotateCcw className={cn("h-3.5 w-3.5 transition-transform", q.isFetching && "animate-spin")} />
          </Button>
        </div>
        {/* 列表头：段位 + 页码 */}
        <div className="tnum mt-1 pl-3.5 text-[11px] leading-none text-muted-foreground">
          <span
            className={cn(
              "font-medium",
              selfRankLabel ? "text-brand-gold" : "text-muted-foreground",
            )}
          >
            {selfRankLabel || UNRANKED}
          </span>
          <span className="mx-1 opacity-40">·</span>
          第 {page + 1} 页
        </div>
      </div>

      {/* 固定 10 槽位：卡片 flex-1 均分高度，禁止滚动 */}
      <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-hidden p-2">
        {q.isPending && !q.data ? (
          Array.from({ length: VISIBLE_COUNT }).map((_, i) => (
            <Skeleton key={i} className="min-h-0 w-full flex-1 rounded-lg" />
          ))
        ) : q.isError ? (
          <div className="space-y-2 rounded-md border border-destructive/40 bg-destructive/5 p-3 text-xs">
            <p className="font-medium text-destructive">获取战绩失败</p>
            <p className="break-all text-muted-foreground">{errMsg(q.error)}</p>
            <Button size="sm" variant="outline" onClick={() => q.refetch()}>
              <RotateCcw className="mr-1 h-3 w-3" />
              重试
            </Button>
          </div>
        ) : list.length === 0 ? (
          <EmptyState
            className="border-0 bg-transparent p-6"
            title="暂无战绩"
            desc="该召唤师近期没有可展示的对局记录。"
          />
        ) : (
          <>
            {list.map((s) => (
              <MatchCard
                key={s.gameId}
                summary={s}
                active={selectedGameId === s.gameId}
                onClick={() => select(tab.puuid, s.gameId)}
              />
            ))}
            {/* 不足 10 条时占位，保持布局稳定 */}
            {Array.from({ length: Math.max(0, VISIBLE_COUNT - list.length) }).map((_, i) => (
              <div key={`pad-${i}`} className="min-h-0 flex-1" aria-hidden />
            ))}
          </>
        )}
      </div>

      <div className="flex shrink-0 items-center justify-center gap-1.5 border-t border-border/60 bg-muted/20 px-2 py-1.5">
        <Button
          variant="outline"
          size="sm"
          className="h-7 px-2.5 text-xs"
          disabled={offline || page <= 0 || q.isPending}
          onClick={() => goto(page - 1)}
        >
          上一页
        </Button>
        <Input
          value={jump === "" ? String(page + 1) : jump}
          onChange={(e) => setJump(e.target.value.replace(/[^0-9]/g, ""))}
          onBlur={commitJump}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitJump();
          }}
          disabled={offline || totalPages == null || totalPages <= 1}
          className="h-7 w-12 px-0 text-center text-xs tnum"
          aria-label="页码"
        />
        <Button
          variant="outline"
          size="sm"
          className="h-7 px-2.5 text-xs"
          disabled={offline || !hasMore || q.isPending}
          onClick={() => goto(page + 1)}
        >
          下一页
        </Button>
      </div>
    </>
  );
}
