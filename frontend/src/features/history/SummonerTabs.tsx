import { useMutation } from "@tanstack/react-query";
import { Check, ChevronDown, RefreshCw, Search, User, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AssetImg } from "@/lib/AssetImg";
import type { MatchSummary } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useHistoryStore } from "@/stores/historyStore";
import { errMsg, fetchMatches, fetchSelfSummoner, searchSummoner } from "./api";

interface Props {
  onRefresh: () => void;
}

/**
 * 顶部工具条：召唤师标签（Tabs + 内嵌关闭按钮）+ 搜索/刷新/查看自己 + 模式筛选（DropdownMenu）
 */
export function SummonerTabs({ onRefresh }: Props) {
  const offline = useAppStore((s) => s.conn.state !== "connected");
  const tabs = useHistoryStore((s) => s.tabs);
  const activeTabId = useHistoryStore((s) => s.activeTabId);
  const setActive = useHistoryStore((s) => s.setActive);
  const closeTab = useHistoryStore((s) => s.closeTab);
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const queueFilter = useHistoryStore((s) => s.queueFilter);
  const setQueueFilter = useHistoryStore((s) => s.setQueueFilter);
  const pages = useHistoryStore((s) => s.pages);

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? null;
  const page = activeTab ? (pages[activeTab.puuid] ?? 0) : 0;

  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [searchErr, setSearchErr] = useState<string | null>(null);
  const [pageData, setPageData] = useState<MatchSummary[] | null>(null);

  useEffect(() => {
    if (!activeTab || offline) {
      setPageData(null);
      return;
    }
    let alive = true;
    void fetchMatches(activeTab.puuid, page)
      .then((p) => {
        if (alive) setPageData(p.summaries ?? []);
      })
      .catch(() => {
        if (alive) setPageData(null);
      });
    return () => {
      alive = false;
    };
  }, [activeTab, page, offline]);

  const queueOptions = useMemo(() => {
    const map = new Map<string, string>();
    map.set("all", "全部");
    for (const s of pageData ?? []) {
      const id = String(s.queueId);
      if (!map.has(id)) map.set(id, s.queueShort || s.queueName || id);
    }
    return [...map.entries()].map(([value, label]) => ({ value, label }));
  }, [pageData]);

  const filterLabel =
    queueOptions.find((o) => o.value === queueFilter)?.label ?? "全部";

  const searchMut = useMutation({
    mutationFn: searchSummoner,
    onSuccess: (s) => {
      openSummoner(s);
      setOpen(false);
      setQuery("");
      setSearchErr(null);
    },
    onError: (e) => setSearchErr(errMsg(e)),
  });

  const selfMut = useMutation({
    mutationFn: fetchSelfSummoner,
    onSuccess: (s) => openSummoner(s, true),
  });

  return (
    <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border/60 bg-muted/25 px-2.5">
      <Tabs
        value={activeTabId ?? undefined}
        onValueChange={(v) => v && setActive(v)}
        className="min-w-0 flex-1"
      >
        <TabsList className="scrollbar-none h-9 max-w-full justify-start overflow-x-auto bg-transparent p-0">
          {tabs.map((t) => (
            <TabsTrigger
              key={t.id}
              value={t.id}
              className={cn(
                "group h-8 max-w-[220px] min-w-0 shrink-0 gap-1.5 rounded-lg border px-2.5 text-xs transition-colors",
                t.id === activeTabId
                  ? "border-brand-gold/50 bg-card font-medium text-foreground shadow-[0_0_0_1px_rgba(200,170,110,0.25)]"
                  : "border-transparent text-muted-foreground/80 hover:bg-card/60 hover:text-foreground",
              )}
            >
              <AssetImg kind="profile" id={t.iconId} size={16} className="rounded-full" />
              <span className="truncate">{t.name.split("#")[0] || t.name}</span>
              {/* 关闭按钮：与选择按钮为兄弟节点（避免嵌套交互），且键盘可达 */}
              <button
                type="button"
                aria-label="关闭标签"
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(t.id);
                }}
                className="rounded p-0.5 opacity-60 transition-opacity hover:bg-muted hover:opacity-100"
              >
                <X className="h-3 w-3" />
              </button>
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      <div className="flex shrink-0 items-center gap-1">
        <Popover
          open={open}
          onOpenChange={(o) => {
            setOpen(o);
            if (!o) setSearchErr(null);
          }}
        >
          <PopoverTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              disabled={offline}
              aria-label="搜索召唤师"
              title="搜索召唤师（昵称#TAG）"
            >
              <Search className="h-4 w-4" />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-72 space-y-2">
            <p className="text-xs font-medium">查询召唤师</p>
            <Input
              value={query}
              autoFocus
              onChange={(e) => setQuery(e.target.value)}
              placeholder="昵称#TAG，例如 安静的亚索#CN1"
              onKeyDown={(e) => {
                if (e.key === "Enter" && query.trim()) searchMut.mutate(query);
              }}
            />
            <div className="flex items-center justify-between gap-2">
              <span className="text-[11px] text-muted-foreground">国服昵称建议携带 #TAG</span>
              <Button
                size="sm"
                disabled={!query.trim() || searchMut.isPending}
                onClick={() => searchMut.mutate(query)}
              >
                {searchMut.isPending ? "查询中" : "查询"}
              </Button>
            </div>
            {searchErr ? <p className="text-xs text-destructive">{searchErr}</p> : null}
          </PopoverContent>
        </Popover>

        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          disabled={offline}
          onClick={onRefresh}
          aria-label="刷新"
          title="刷新当前标签页数据"
        >
          <RefreshCw className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          disabled={offline || selfMut.isPending}
          onClick={() => selfMut.mutate()}
          aria-label="查看自己"
          title="查看自己的历史战绩"
        >
          <User className="h-4 w-4" />
        </Button>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="ml-1 h-8 gap-1 rounded-lg px-2.5 text-xs"
              disabled={offline || !activeTab}
            >
              {filterLabel}
              <ChevronDown className="h-3.5 w-3.5 opacity-60" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-40">
            {queueOptions.map((opt) => (
              <DropdownMenuItem
                key={opt.value}
                onClick={() => setQueueFilter(opt.value)}
                className="justify-between text-xs"
              >
                <span className="truncate">{opt.label}</span>
                {queueFilter === opt.value ? (
                  <Check className="h-3.5 w-3.5 text-primary" />
                ) : null}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
