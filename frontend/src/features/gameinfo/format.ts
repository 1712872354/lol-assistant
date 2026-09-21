/** 对局页玩家卡格式化辅助 */

/** 相对时间："刚刚" / "10 分钟前" / "10 小时前" / "3 天前"；无时间戳或超过 30 天回退兜底文案 */
export function relTime(ms?: number, fallback = ""): string {
  if (!ms || ms <= 0) return fallback;
  const diff = Date.now() - ms;
  if (diff < 0) return fallback;
  const min = Math.floor(diff / 60_000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min} 分钟前`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} 小时前`;
  const d = Math.floor(h / 24);
  if (d < 30) return `${d} 天前`;
  return fallback;
}

/** "黄金 IV 45" → { main: "黄金 IV", lp: "45" }；无 LP 整体进 main；空/未定级返回空 */
export function splitRank(s?: string): { main: string; lp: string } {
  const t = (s ?? "").trim();
  if (!t || t === "未定级") return { main: "", lp: "" };
  const m = /^(.*\S)\s+(\d+)$/.exec(t);
  if (m) return { main: m[1], lp: m[2] };
  return { main: t, lp: "" };
}

/** 胜率/KDA 威胁色档：低于 bad 阈值偏危险/弱势，高于 good 偏强势 */
export function threatTone(
  value: number,
  bad: number,
  good: number,
): "good" | "mid" | "bad" | "muted" {
  if (!Number.isFinite(value) || value <= 0) return "muted";
  if (value < bad) return "bad";
  if (value >= good) return "good";
  return "mid";
}
