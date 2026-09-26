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
      <span className="stat-num text-[14px]">{fmtNum(value)}</span>
      <span className="h-[3px] w-12 shrink-0 overflow-hidden rounded-full bg-muted">
        <span
          className={cn("block h-full rounded-full", RESULT_FILL[tone])}
          style={{ width: `${pct}%` }}
        />
      </span>
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
          <div className="stat-num whitespace-nowrap text-[15px]">
            {p.kills}
            <span className="mx-1 text-muted-foreground/45">/</span>
            {p.deaths}
            <span className="mx-1 text-muted-foreground/45">/</span>
            {p.assists}
          </div>
          <div className="tnum whitespace-nowrap text-[10px] text-muted-foreground/75">
            {p.kda} · {p.killParticipation}%
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
            <span
              className={cn(
                "stat-num block cursor-help text-center text-[13px]",
                p.dmgRatio >= 1.15
                  ? "text-good-fg"
                  : p.dmgRatio >= 0.85
                    ? "text-amber-300"
                    : "text-loss-fg/90",
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
    cell: ({ row }) => {
      const { p } = row.original;
      const slots = Array.from({ length: 7 }, (_, i) => p.items?.[i] ?? 0);
      return (
        <div className="flex items-center justify-center gap-[3px]">
          {slots.map((id, i) =>
            id > 0 ? (
              <AssetImg
                key={`it-${i}`}
                kind="item"
                id={id}
                size={22}
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
    },
  },
  {
    id: "rating",
    header: "评分",
    accessorFn: (r) => safeNum(r.p.matchRating),
    sortDescFirst: true,
    cell: ({ row }) => {
      const { p } = row.original;
      return (
        <span className="stat-num block pr-1 text-right text-[15px]">
          {Number.isFinite(p.matchRating) ? p.matchRating.toFixed(1) : "—"}
        </span>
      );
    },
  },
];

function PlayerCell({ row }: { row: PlayerTableRowData }) {
  const { p, ranked, badge } = row;
  const openSummoner = useHistoryStore((s) => s.openSummoner);
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
        <span className="absolute -bottom-1 -right-1 rounded bg-black/70 px-1 text-[10px] leading-[14px] text-white tnum">
          {p.champLevel}
        </span>
      </div>
      <div className="flex shrink-0 flex-col gap-[3px]">
        <AssetImg kind="spell" id={p.spell1Id} size={15} className="rounded-[3px]" />
        <AssetImg kind="spell" id={p.spell2Id} size={15} className="rounded-[3px]" />
      </div>
      <AssetImg kind="perk" id={p.runeId} size={15} className="shrink-0 rounded-full" />

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
              <span className="ml-0.5 text-[11px] font-normal text-muted-foreground/70">
                {tag}
              </span>
            ) : null}
          </button>
          {badge ? (
            <span
              className={cn(
                "shrink-0 rounded px-1 py-px text-[9px] font-bold leading-[14px] tracking-wider",
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
            hasRank ? "text-brand-gold/95" : "text-muted-foreground/60",
          )}
        >
          <Crown
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
                      (hi === 2 || hi === 3 || hi === 4 || hi === 6) &&
                        "cursor-pointer pr-1 text-right select-none hover:text-foreground",
                    )}
                    onClick={
                      canSort ? header.column.getToggleSortingHandler() : undefined
                    }
                    title={hint}
                  >
                    <span className="inline-flex items-center gap-0.5">
                      {label}
                      {sorted ? (
                        <span className="text-[9px] opacity-70">
                          {sorted === "desc" ? "▼" : "▲"}
                        </span>
                      ) : null}
                    </span>
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
