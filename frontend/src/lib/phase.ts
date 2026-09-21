import type { GameflowPhase } from "./types";

/**
 * gameflow 阶段 → 中文状态文案（与 Go 侧 gameinfo.PhaseLabelCN 保持同一文案）。
 * 状态条（ConnBadge）与对局页队伍徽章共用；未知阶段回落「大厅中」。
 */
const PHASE_LABEL: Record<string, string> = {
  None: "大厅中",
  Lobby: "房间内",
  Matchmaking: "匹配中",
  ReadyCheck: "接受对局",
  ChampSelect: "选人中",
  GameStart: "游戏启动",
  InProgress: "游戏中",
  WaitingForStats: "结算中",
  PreEndOfGame: "结算中",
  EndOfGame: "对局结束",
  Reconnect: "重新连接",
};

export function phaseLabelCN(p: GameflowPhase | string): string {
  return PHASE_LABEL[p] ?? "大厅中";
}
