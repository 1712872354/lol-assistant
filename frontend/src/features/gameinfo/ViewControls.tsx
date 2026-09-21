import { Check, ChevronDown } from "lucide-react";
import { useState } from "react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { GameinfoSideFilter } from "@/lib/types";
import { cn } from "@/lib/utils";
import { useGameinfoStore } from "@/stores/gameinfoStore";
import {
  QUEUE_TYPE_OPTIONS,
  queueFilterLabel,
  queueTypeKeyOf,
  type QueueFilterKey,
} from "./queueOptions";

const SIDE_FILTERS: Array<{ value: GameinfoSideFilter; label: string }> = [
  { value: "ally", label: "我方" },
  { value: "all", label: "全部" },
  { value: "enemy", label: "敌方" },
];

/** 下拉菜单项：选中打勾；当前对局所属类型带「当前」小标 */
function MenuItem({
  value,
  label,
  active,
  current,
  onPick,
}: {
  value: QueueFilterKey;
  label: string;
  active: boolean;
  current?: boolean;
  onPick: (v: QueueFilterKey) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onPick(value)}
      className={cn(
        "menu-item flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-accent",
        active ? "font-semibold text-selected-fg" : "text-foreground",
      )}
    >
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {current ? (
        <span className="shrink-0 rounded bg-muted px-1 text-[10px] font-normal text-muted-foreground">
          当前
        </span>
      ) : null}
      {active ? <Check className="h-3.5 w-3.5 shrink-0" /> : null}
    </button>
  );
}

/** 阵营分段：滑块指示器 + 按钮微缩放 */
export function SideFilterControl() {
  const sideFilter = useGameinfoStore((s) => s.sideFilter);
  const setSideFilter = useGameinfoStore((s) => s.setSideFilter);
  const activeIndex = SIDE_FILTERS.findIndex((f) => f.value === sideFilter);

  return (
    <div
      role="tablist"
      aria-label="阵营筛选"
      className="relative flex items-center rounded-lg bg-muted/60 p-0.5"
    >
      {/* 三等分滑块 */}
      <div
        aria-hidden
        className="seg-indicator absolute top-0.5 bottom-0.5 rounded-md bg-toggle-on shadow-sm"
        style={{
          left: `calc(2px + ${Math.max(0, activeIndex)} * ((100% - 4px) / 3))`,
          width: "calc((100% - 4px) / 3)",
        }}
      />
      {SIDE_FILTERS.map((f) => {
        const on = sideFilter === f.value;
        return (
          <button
            key={f.value}
            type="button"
            role="tab"
            aria-selected={on}
            onClick={() => setSideFilter(f.value)}
            className={cn(
              "relative z-[1] h-7 flex-1 rounded-md px-2.5 text-xs transition-[color,transform] duration-150 active:scale-[0.96]",
              on ? "font-semibold text-white" : "text-foreground hover:bg-card/80",
            )}
          >
            {f.label}
          </button>
        );
      })}
    </div>
  );
}

/** 对局类型下拉（近况口径）：箭头旋转 + 弹层淡入 */
export function QueueTypeSelect({ disabled }: { disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const queueKey = useGameinfoStore((s) => s.queueKey);
  const setQueueKey = useGameinfoStore((s) => s.setQueueKey);
  const queueLabel = useGameinfoStore((s) => s.view.queueLabel);
  const queueId = useGameinfoStore((s) => s.view.queueId ?? 0);
  const currentKey = queueTypeKeyOf(queueId);

  const pick = (key: QueueFilterKey) => {
    setOpen(false);
    if (key !== queueKey) setQueueKey(key);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          title="按对局类型筛选近 20 场数据"
          className={cn(
            "press-scale flex h-8 min-w-0 items-center gap-1 rounded-lg px-2 text-xs hover:bg-accent",
            open && "bg-accent",
            queueKey !== "follow" ? "font-semibold text-selected-fg" : "text-foreground",
            disabled && "pointer-events-none opacity-50",
          )}
        >
          <span className="truncate transition-colors duration-150">
            {queueFilterLabel(queueKey, queueLabel)}
          </span>
          <ChevronDown
            className={cn(
              "h-3 w-3 shrink-0 opacity-60 transition-transform duration-200 ease-out",
              open && "rotate-180",
            )}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-56 p-1">
        <MenuItem
          value="follow"
          label={`跟随对局（${queueLabel || "当前对局"}）`}
          active={queueKey === "follow"}
          onPick={pick}
        />
        <MenuItem value="all" label="全部对局" active={queueKey === "all"} onPick={pick} />
        <div className="my-1 h-px bg-border" />
        {QUEUE_TYPE_OPTIONS.map((o) => (
          <MenuItem
            key={o.key}
            value={o.key}
            label={o.label}
            active={queueKey === o.key}
            current={currentKey === o.key}
            onPick={pick}
          />
        ))}
      </PopoverContent>
    </Popover>
  );
}

/** 标题栏用：阵营筛选 + 对局类型（仅对局页展示） */
export function GameinfoViewControls({ disabled }: { disabled?: boolean }) {
  return (
    <div className="flex items-center gap-1.5">
      <SideFilterControl />
      <div className="mx-0.5 h-5 w-px bg-border" />
      <QueueTypeSelect disabled={disabled} />
    </div>
  );
}
