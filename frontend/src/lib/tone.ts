/** 胜负/无效 结果色档（战绩列表与明细通用） */
export type Tone = "win" | "loss" | "remake";

/** 威胁色档（胜率 / KDA / 相对条） */
export type ThreatTone = "good" | "mid" | "bad" | "muted";

/** 结果向相对条填充色（明细页伤害/金钱条） */
export const RESULT_FILL: Record<Tone, string> = {
  win: "bg-team-win-fg/45",
  loss: "bg-team-loss-fg/45",
  remake: "bg-muted-foreground/35",
};

/** 威胁向相对条填充色（对局页胜率/KDA/队伍对照条） */
export const THREAT_FILL: Record<ThreatTone, string> = {
  good: "bg-emerald-500/70",
  mid: "bg-amber-500/65",
  bad: "bg-red-500/65",
  muted: "bg-muted-foreground/35",
};
