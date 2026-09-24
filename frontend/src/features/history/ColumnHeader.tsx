import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** 明细列宽模板（列头与行共用） */
const COLS = "minmax(240px,1.8fr) 128px 96px 96px 72px 180px 64px";

/** 列头：解释裸数字含义，扫读时不用猜 */
export function ColumnHeader() {
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
