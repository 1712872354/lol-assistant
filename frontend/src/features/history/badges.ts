import type { Tone } from "@/lib/tone";

/** 行内文字色档 */
export const TONE_TEXT: Record<Tone, string> = {
  win: "text-team-win-fg",
  loss: "text-team-loss-fg",
  remake: "text-muted-foreground",
};

/** 队列头底色档 */
export const TONE_HEADER: Record<Tone, string> = {
  win: "bg-team-win-bg text-team-win-fg",
  loss: "bg-team-loss-bg text-team-loss-fg",
  remake: "bg-muted text-muted-foreground",
};

/** 相对条（已上移 lib/RESULT_FILL，此处保留名称兼容） */

/** 相对条底色（队伍汇总行背景条） */
export const TONE_FILL: Record<Tone, string> = {
  win: "bg-team-win-bg/20",
  loss: "bg-team-loss-bg/20",
  remake: "bg-muted/30",
};
