/** 与 Go 侧 JSON 序列化字段一一对应的视图模型类型 */

export type ConnState = "disconnected" | "connected" | "unauthenticated";

export interface ConnStatus {
  state: ConnState;
  gameName?: string;
  tagLine?: string;
  platformId?: string;
  summonerLevel?: number;
  profileIconId?: number;
  puuid?: string;
}

export type ThemeMode = "system" | "light" | "dark";

export interface AppConfig {
  schemaVersion: number;
  theme: ThemeMode;
  pageSize: number;
  apiConcurrency: number;
  sgpEnabled: boolean;
  closeToTray: boolean;
  clientPath: string;
}

export const DEFAULT_CONFIG: AppConfig = {
  schemaVersion: 1,
  theme: "system",
  pageSize: 20,
  apiConcurrency: 4,
  sgpEnabled: true,
  closeToTray: true,
  clientPath: "",
};

export type ViewKey = "history" | "gameinfo" | "settings";

export const APP_VERSION = "v0.1.0";

/** 对局信息页：筛选视图 */
export type GameinfoSideFilter = "ally" | "all" | "enemy";

export type GameflowPhase =
  | "None"
  | "Lobby"
  | "Matchmaking"
  | "ReadyCheck"
  | "ChampSelect"
  | "GameStart"
  | "InProgress"
  | "WaitingForStats"
  | "EndOfGame"
  | "PreEndOfGame"
  | "Reconnect";

export interface GameinfoRecentMatch {
  queueShort: string;
  queueName?: string; // 队列全称（行内主文案）
  timeShort: string;
  gameCreation?: number; // ms，用于「X 小时前」相对时间
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  championId: number;
}

export interface GameinfoPlayerSlot {
  filled: boolean;
  isSelf: boolean;
  puuid?: string;
  summonerId?: string;
  gameName?: string;
  tagLine?: string;
  profileIconId?: number;
  championId?: number;
  solo?: string;
  flex?: string;
  winRate?: number;
  winRateSample?: number;
  avgKda?: number;
  rating?: number;
  hiddenCareer?: boolean;
  recent?: GameinfoRecentMatch[];
}

export interface GameinfoTeamView {
  key: "ally" | "enemy";
  label: string;
  sideText: string;
  badge: string;
  playerCount: number;
  phaseLabel: string;
  winRate: number;
  compScore: number;
  rating: number;
  slots: GameinfoPlayerSlot[]; // 恒 5
}

export interface GameinfoViewState {
  phase: GameflowPhase;
  queueLabel: string;
  queueId?: number; // 当前对局队列 id（0/缺省=未知；菜单"当前"标记用）
  teams: GameinfoTeamView[];
}

/* ── M2 历史战绩视图模型（与 Go 侧 service/history + internal/parser JSON 标签对应） ── */

export interface SummonerResult {
  puuid: string;
  gameName: string;
  tagLine: string;
  displayName: string; // "gameName#tagLine"
  profileIconId: number;
  summonerLevel: number;
  summonerId: string;
}

export interface MatchSummary {
  gameId: number;
  queueId: number;
  queueName: string;
  queueShort: string;
  mapName: string;
  arena: boolean;
  gameCreation: number; // ms
  gameDuration: number; // s
  time: string; // "2026-09-21 20:24"
  shortTime: string; // "09-21"
  duration: string; // "15:24"
  championId: number;
  champLevel: number;
  spell1Id: number;
  spell2Id: number;
  runeId: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: string; // "6.83" | "Perfect"
  win: boolean;
  remake: boolean;
  placement: number; // 竞技场名次，0=无
  items: number[]; // item0..6（7 格，含饰品）
  cs: number;
  gold: number;
  totalDamage: number;
  totalHeal: number;
  augmentIds: number[];
  teamId: number;
}

export interface MatchPage {
  puuid: string;
  page: number;
  pageSize: number;
  gameCount: number;
  totalPages: number;
  hasMore: boolean;
  summaries: MatchSummary[];
}

export interface PlayerRow {
  participantId: number;
  teamId: number;
  placement: number;
  puuid: string;
  summonerId: string;
  name: string; // gameName#tagLine
  profileIconId: number;
  championId: number;
  champLevel: number;
  spell1Id: number;
  spell2Id: number;
  runeId: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: string;
  items: number[];
  cs: number;
  gold: number;
  totalDamage: number;
  totalHeal: number;
  win: boolean;
  remake: boolean;
  augmentIds: number[];
  tierShort: string; // "黄金" | ""（历史最高段位，仅作回退展示）
  dmgRatio: number; // 伤转 = 个人伤害 / 本组平均伤害
  rating: number; // 本工具评分
  ratingRank: number; // 全场评分名次 1..N
  killPct: number; // 参团率 % = (K+A) / 本组总击杀 × 100
  isSelf: boolean;
}

export interface TeamSummary {
  teamId: number;
  placement: number;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  gold: number;
  damage: number;
  players: PlayerRow[]; // 已按评分降序
}

export interface MatchDetail {
  gameId: number;
  queueId: number;
  queueName: string;
  mapName: string;
  arena: boolean;
  gameCreation: number;
  time: string;
  gameDuration: number;
  duration: string; // "15:24"
  durationMin: string; // "15分"
  remake: boolean;
  selfPuuid: string;
  selfTeamIndex: number; // 恒 0：teams[0] = 本人所在队伍
  teams: TeamSummary[];
}

export interface RankedInfo {
  summonerId: string; // 查询 id（summonerId 或 puuid）
  puuid?: string;
  solo: string; // "黄金 IV 45" | "未定级"
  flex: string;
}

export interface AssetResult {
  kind: string; // champion/profile/item/spell/perk/augment
  id: number;
  mime: string;
  data: string; // base64
}
