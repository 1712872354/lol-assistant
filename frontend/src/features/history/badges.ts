import type { Tone } from "@/lib/tone";

/** 行内文字色档（深底下用高对比语义色） */
export const TONE_TEXT: Record<Tone, string> = {
  win: "text-win-fg",
  loss: "text-loss-fg",
  remake: "text-muted-foreground",
};

/** 队伍头带：记分牌条——深面 + 左侧 3px 色轨（禁止大色块铺满） */
export const TONE_HEADER: Record<Tone, string> = {
  win: "border-l-[3px] border-l-win-bar bg-win-bg/25 text-win-fg",
  loss: "border-l-[3px] border-l-loss-bar bg-loss-bg/25 text-loss-fg",
  remake: "border-l-[3px] border-l-remake-bar bg-remake-bg/30 text-muted-foreground",
};

/** 队伍区块底色：近中性，胜负只靠表头色轨与文字 */
export const TONE_FILL: Record<Tone, string> = {
  win: "bg-transparent",
  loss: "bg-transparent",
  remake: "bg-transparent",
};
