/**
 * 对局类型（近况口径）选项表：与后端 parser.queueTable 队列 id 对齐。
 * 一个"类型"可含多个队列 id（如海斗 = 2400 海克斯大乱斗 + 2450 经典海斗）。
 */

export interface QueueTypeOption {
  key: string;
  label: string;
  ids: number[];
}

/** 常用对局类型清单（下拉第二组） */
export const QUEUE_TYPE_OPTIONS: QueueTypeOption[] = [
  { key: "q2400", label: "海克斯大乱斗", ids: [2400, 2450] },
  { key: "q420", label: "排位单双排", ids: [420] },
  { key: "q440", label: "排位灵活组排", ids: [440] },
  { key: "q430", label: "匹配模式", ids: [430] },
  { key: "q450", label: "极地大乱斗", ids: [450] },
  { key: "q1700", label: "斗魂竞技场", ids: [1700, 1710] },
  { key: "q900", label: "无限火力", ids: [900, 1010] },
  { key: "q830", label: "人机对战", ids: [800, 810, 820, 830, 840, 850] },
  { key: "q4300", label: "经典模式", ids: [4300, 4310] },
];

/** 过滤口径 key："follow"=跟随当前对局；"all"=全部；其余为 QUEUE_TYPE_OPTIONS 的 key */
export type QueueFilterKey = "follow" | "all" | (string & {});

/** key → 后端 GetGameflowState queueFilter 参数（[-1]=跟随；[]=全部；其余=队列 id 集） */
export function queueFilterArgs(key: QueueFilterKey): number[] {
  if (key === "follow") return [-1];
  if (key === "all") return [];
  return QUEUE_TYPE_OPTIONS.find((o) => o.key === key)?.ids ?? [];
}

/** 下拉触发器显示名（含"对局类型："前缀） */
export function queueFilterLabel(key: QueueFilterKey, queueLabel: string): string {
  const cur = queueLabel || "当前对局";
  if (key === "follow") return `对局类型：跟随（${cur}）`;
  if (key === "all") return "对局类型：全部";
  return `对局类型：${QUEUE_TYPE_OPTIONS.find((o) => o.key === key)?.label ?? "未知"}`;
}

/** 当前对局队列 id 落在哪个类型 key（菜单里打"当前"标记）；未知返回 null */
export function queueTypeKeyOf(queueId: number): string | null {
  if (queueId <= 0) return null;
  return QUEUE_TYPE_OPTIONS.find((o) => o.ids.includes(queueId))?.key ?? null;
}
