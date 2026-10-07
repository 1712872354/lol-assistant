import {
  createSortedRowModel,
  flexRender,
  rowSortingFeature,
  sortFns,
  tableFeatures as makeFeatures,
  useTable,
  type ColumnDef,
  type SortingState,
} from "@tanstack/react-table";
import { Crown } from "lucide-react";
import { useMemo, useState } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { AssetImg } from "@/lib/AssetImg";
import { UNRANKED } from "@/lib/rank";
import { toSummonerResult } from "@/lib/summoner";
import { RESULT_FILL, type Tone } from "@/lib/tone";
import type { PlayerRow, RankedInfo } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useAssetNames } from "@/lib/useAssetNames";
import { useHistoryStore } from "@/stores/historyStore";
import { fmtNum, rankedDisplay } from "./format";

const features = makeFeatures({
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
  sortFns,
});

export interface PlayerTableRowData {
  p: PlayerRow;
  ranked?: RankedInfo;
  badge?: "MVP" | "ACE";
  tone: Tone;
  maxDamage: number;
  maxGold: number;
}

function safeNum(v: unknown, fallback = -Infinity): number {
  return typeof v === "number" && Number.isFinite(v) ? v : fallback;
}

function kdaNum(p: PlayerRow): number {
  const raw = (p.kda ?? "").trim();
  if (raw === "Perfect") return 99;
  const n = Number(raw);
  return Number.isFinite(n) ? n : -Infinity;
}

/** 数值 + 右侧固定宽相对条（细条降噪，数字是主角） */
function StatCell({
  value,
  max,
  tone,
}: {
  value: number;
  max: number;
  tone: Tone;
}) {
  const pct = max > 0 ? Math.min(100, Math.max(3, Math.round((value / max) * 100))) : 0;
  return (
    <div className="flex items-center justify-end gap-2 pr-1">
      <span className="stat-num text-[15px] font-semibold">{fmtNum(value)}</span>
      {/* 相对条为数值的冗余可视化，读屏由上方数字承载 */}
      <span aria-hidden className="h-[3px] w-12 shrink-0 overflow-hidden rounded-full bg-muted">
        <span
          className={cn("block h-full rounded-full", RESULT_FILL[tone])}
          style={{ width: `${pct}%` }}
        />
      </span>
    </div>
  );
}

/** 装备格：逐件名称 alt（读屏可读出装备名） */
function ItemSlotsCell({ p }: { p: PlayerRow }) {
  const slots = Array.from({ length: 7 }, (_, i) => p.items?.[i] ?? 0);
  const names = useAssetNames("item", slots);
  return (
    <div className="flex items-center justify-center gap-[3px]">
      {slots.map((id, i) =>
        id > 0 ? (
          <AssetImg
            key={`it-${i}`}
            kind="item"
            id={id}
            size={22}
            alt={names.get(id) ?? "装备"}
            className="rounded-[4px]"
          />
        ) : (
          <span
            key={`it-${i}`}
            aria-hidden
            className="inline-block h-[22px] w-[22px] shrink-0 rounded-[4px] bg-muted/45"
          />
        ),
      )}
    </div>
  );
}

const columns: ColumnDef<typeof features, PlayerTableRowData>[] = [
  {
    id: "player",
    header: "玩家",
    cell: ({ row }) => <PlayerCell row={row.original} />,
  },
  {
    id: "kda",
    header: "KDA",
    accessorFn: (r) => kdaNum(r.p),
    cell: ({ row }) => {
      const { p } = row.original;
      return (
        <div className="flex flex-col items-center gap-0.5">
          <div className="stat-num whitespace-nowrap text-[16px] font-bold">
            <span className="sr-only">KDA </span>
            {p.kills}
            <span aria-hidden className="mx-1 text-[13px] font-normal text-muted-foreground/35">/</span>
            {p.deaths}
            <span aria-hidden className="mx-1 text-[13px] font-normal text-muted-foreground/35">/</span>
            {p.assists}
          </div>
          <div className="tnum whitespace-nowrap text-[11px] text-muted-foreground/80">
            {p.kda} · <span className="sr-only">参团率 </span>{p.killParticipation}%
          </div>
        </div>
      );
    },
  },
  {
    id: "damage",
    header: "伤害",
    accessorFn: (r) => safeNum(r.p.totalDamage),
    cell: ({ row }) => {
      const { p, maxDamage, tone } = row.original;
      return <StatCell value={p.totalDamage} max={maxDamage} tone={tone} />;
    },
  },
  {
    id: "gold",
    header: "金钱",
    accessorFn: (r) => safeNum(r.p.gold),
    cell: ({ row }) => {
      const { p, maxGold, tone } = row.original;
      return <StatCell value={p.gold} max={maxGold} tone={tone} />;
    },
  },
  {
    id: "dmgRatio",
    header: "伤转",
    accessorFn: (r) => safeNum(r.p.dmgRatio),
    cell: ({ row }) => {
      const { p } = row.original;
      return (
        <Tooltip>
          <TooltipTrigger asChild>
            {/* tabIndex 使提示可键盘聚焦获取（title/hover 不可达） */}
            <span
              tabIndex={0}
              className={cn(
                "stat-num block cursor-help text-center text-[14px] font-semibold",
                p.dmgRatio >= 1.15
                  ? "text-good-fg"
                  : p.dmgRatio >= 0.85
                    ? "text-mid-fg"
                    : "text-loss-fg",
              )}
            >
              {Number.isFinite(p.dmgRatio) ? p.dmgRatio.toFixed(1) : "—"}
            </span>
          </TooltipTrigger>
          <TooltipContent>
            伤转 {Number.isFinite(p.dmgRatio) ? p.dmgRatio.toFixed(2) : "—"} · 伤害/本组均伤
          </TooltipContent>
        </Tooltip>
      );
    },
  },
  {
    id: "items",
    header: "装备",
    enableSorting: false,
    cell: ({ row }) => <ItemSlotsCell p={row.original.p} />,
  },
  {
    id: "rating",
    header: "评分",
    accessorFn: (r) => safeNum(r.p.matchRating),
    sortDescFirst: true,
    cell: ({ row }) => {
      const { p } = row.original;
      const v = Number.isFinite(p.matchRating) ? p.matchRating : null;
      return (
        <span
          className={cn(
            "stat-num block pr-1 text-right text-[16px] font-bold",
            v != null && v >= 12 && "text-brand-gold",
            v != null && v < 8 && "text-muted-foreground/75",
          )}
        >
          {v != null ? v.toFixed(1) : "—"}
        </span>
      );
    },
  },
];

function PlayerCell({ row }: { row: PlayerTableRowData }) {
  const { p, ranked, badge } = row;
  const openSummoner = useHistoryStore((s) => s.openSummoner);
  const spellNames = useAssetNames(
    "spell",
    [p.spell1Id, p.spell2Id].filter((id) => id > 0),
  );
  const perkNames = useAssetNames("perk", p.runeId > 0 ? [p.runeId] : []);
  const hash = p.name.indexOf("#");
  const base = hash >= 0 ? p.name.slice(0, hash) : p.name;
  const tag = hash >= 0 ? p.name.slice(hash) : "";
  const rankText = rankedDisplay(p, ranked);
  const hasRank = rankText !== UNRANKED;
  const puuid = p.puuid?.trim() ?? "";

  const openPlayerTab = () => {
    if (!puuid) return;
    openSummoner(
      toSummonerResult({
        puuid,
        gameName: base,
        tagLine: tag ? tag.slice(1) : "",
        profileIconId: p.profileIconId ?? 0,
        summonerId: p.summonerId ?? "",
      }),
      p.isSelf,
    );
  };

  return (
    <div className="flex items-center gap-2.5 pl-4 pr-3">
      <div className="relative shrink-0">
        <AssetImg kind="champion" id={p.championId} size={42} className="rounded-lg" />
        <span className="absolute -bottom-1 -right-1 rounded bg-black/70 px-1 text-[11px] leading-[14px] text-white tnum">
          <span className="sr-only">英雄等级 </span>
          {p.champLevel}
        </span>
      </div>
      {/* 召唤师技能/符文：逐件名称 alt */}
      <div className="flex shrink-0 flex-col gap-[3px]">
        <AssetImg
          kind="spell"
          id={p.spell1Id}
          size={15}
          alt={p.spell1Id > 0 ? (spellNames.get(p.spell1Id) ?? "召唤师技能") : ""}
          className="rounded-[3px]"
        />
        <AssetImg
          kind="spell"
          id={p.spell2Id}
          size={15}
          alt={p.spell2Id > 0 ? (spellNames.get(p.spell2Id) ?? "召唤师技能") : ""}
          className="rounded-[3px]"
        />
      </div>
      <AssetImg
        kind="perk"
        id={p.runeId}
        size={15}
        alt={p.runeId > 0 ? (perkNames.get(p.runeId) ?? "符文") : ""}
        className="shrink-0 rounded-full"
      />

      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex min-w-0 items-center gap-1.5">
          <button
            type="button"
            onClick={openPlayerTab}
            disabled={!puuid}
            title={puuid ? "新标签查看该玩家战绩" : undefined}
            className={cn(
              "min-w-0 truncate text-left text-[13px] leading-tight",
              p.isSelf ? "font-bold text-foreground" : "font-medium text-foreground/90",
              puuid && "cursor-pointer hover:text-primary hover:underline underline-offset-2",
              !puuid && "cursor-default",
            )}
          >
            {base}
            {tag ? (
              <span className="ml-0.5 text-[11px] font-normal text-muted-foreground/80">
                {tag}
              </span>
            ) : null}
            {p.isSelf ? <span className="sr-only">（本人）</span> : null}
          </button>
          {badge ? (
            <span
              className={cn(
                "shrink-0 rounded px-1 py-px text-[11px] font-bold leading-[14px] tracking-wider",
                badge === "MVP"
                  ? "bg-brand-gold/20 text-brand-gold"
                  : "bg-muted text-muted-foreground border border-border/60",
              )}
            >
              {badge}
            </span>
          ) : null}
        </div>
        <div
          className={cn(
            "flex items-center gap-1 text-[11px] leading-none",
            hasRank ? "text-brand-gold/95" : "text-muted-foreground/75",
          )}
        >
          <Crown
            aria-hidden
            className={cn("h-3 w-3 shrink-0", hasRank ? "text-brand-gold/80" : "opacity-30")}
          />
          <span className="truncate">{rankText}</span>
        </div>
      </div>
    </div>
  );
}

/** 列宽（table-fixed + colgroup）：玩家列吃剩余宽度，其余固定 */
const COL_WIDTHS: Array<string | undefined> = [
  undefined,
  "120px",
  "118px",
  "118px",
  "64px",
  "176px",
  "64px",
];

const HEAD_HINTS: Record<string, string> = {
  damage: "对英雄伤害 · 条为全场相对值",
  gold: "本局获得金币 · 条为全场相对值",
  dmgRatio: "个人伤害 / 本组平均伤害，≥1 为高于均值",
};

/**
 * 明细玩家表格：官方 Table + TanStack Table v9 排序。
 * 默认按评分降序（与后端 ratingRank 一致），点列头切换排序。
 */
export function PlayerDataTable({ data }: { data: PlayerTableRowData[] }) {
  const [sorting, setSorting] = useState<SortingState>([
    { id: "rating", desc: true },
  ]);

  const table = useTable(
    {
      features,
      columns,
      data,
      state: { sorting },
      onSortingChange: setSorting,
    },
    (s) => ({ sorting: s.sorting }),
  );

  const headerGroups = useMemo(() => table.getHeaderGroups(), [table]);
  const rows = useMemo(() => table.getRowModel().rows, [table]);

  return (
    <div className="flex min-h-0 flex-1 flex-col [&>[data-slot=table-container]]:h-full">
      <Table className="h-full table-fixed border-separate border-spacing-0">
        <colgroup>
          {COL_WIDTHS.map((w, i) => (
            <col key={i} style={w ? { width: w } : undefined} />
          ))}
        </colgroup>
        <TableHeader>
          {headerGroups.map((hg) => (
            <TableRow key={hg.id} className="hover:bg-transparent">
              {hg.headers.map((header, hi) => {
                const canSort = header.column.getCanSort();
                const sorted = header.column.getIsSorted();
                const hint = HEAD_HINTS[header.column.id];
                const label = flexRender(
                  header.column.columnDef.header,
                  header.getContext(),
                );
                return (
                  <TableHead
                    key={header.id}
                    className={cn(
                      "h-8 border-b border-border/70 bg-muted/30 px-0 pb-0 text-[11px] font-medium text-muted-foreground/85",
                      hi === 0 && "pl-4 text-left",
                      hi === 1 && "text-center",
                      hi === 5 && "text-center",
                      (hi === 2 || hi === 3 || hi === 4 || hi === 6) && "pr-1 text-right select-none",
                    )}
                    aria-sort={
                      canSort
                        ? sorted === "asc"
                          ? "ascending"
                          : sorted === "desc"
                            ? "descending"
                            : "none"
                        : undefined
                    }
                  >
                    {canSort ? (
                      // 排序交互由 <button> 承载：键盘可达（WCAG 2.1.1）
                      <button
                        type="button"
                        onClick={header.column.getToggleSortingHandler()}
                        title={hint}
                        aria-label={`按${typeof label === "string" ? label : "此列"}排序，当前${sorted === "desc" ? "降序" : sorted === "asc" ? "升序" : "未排序"}`}
                        className={cn(
                          "inline-flex w-full items-center gap-0.5 px-1 py-1 hover:text-foreground",
                          hi === 1 ? "justify-center" : "justify-end",
                        )}
                      >
                        {label}
                        <span aria-hidden className="text-[10px] opacity-40">
                          {sorted === "desc" ? "▼" : sorted === "asc" ? "▲" : "↕"}
                        </span>
                      </button>
                    ) : (
                      <span className="inline-flex items-center gap-0.5">{label}</span>
                    )}
                    {/* 列说明放按钮外，避免混入可访问名 */}
                    {hint ? <span className="sr-only">{hint}</span> : null}
                  </TableHead>
                );
              })}
            </TableRow>
          ))}
        </TableHeader>
        <TableBody>
          {rows.map((row) => {
            const self = row.original.p.isSelf;
            return (
              <TableRow
                key={row.id}
                style={{
                  height: `calc((100% - 2rem) / ${Math.max(1, rows.length)})`,
                }}
                className={cn(
                  "border-b border-border/45 transition-colors hover:bg-muted/25",
                  self &&
                    "bg-self-bg/80 hover:bg-self-bg shadow-[inset_3px_0_0_0_var(--brand-gold)]",
                )}
              >
                {row.getAllCells().map((cell, ci) => (
                  <TableCell
                    key={cell.id}
                    className={cn("px-0 py-0 align-middle", ci === 6 && "pr-3")}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}
