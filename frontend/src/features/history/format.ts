import { UNRANKED } from "@/lib/rank";
import type { MatchSummary, PlayerRow, RankedInfo } from "@/lib/types";

/** 大数字缩写：69400 → "69.4K"（底部汇总条样式） */
export function fmtK(n: number): string {
  if (!Number.isFinite(n)) return "0";
  const v = Math.abs(n);
  if (v >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

/** 千分位格式化：23456 → "23,456"（明细数值列扫读对齐） */
export function fmtNum(n: number): string {
  if (!Number.isFinite(n)) return "0";
  return Math.round(n).toLocaleString("en-US");
}

/** 国服段位中→英映射（长名在前，startsWith 匹配；列表头 GOLD IV 45 风格） */


export type { Tone } from "@/lib/tone";
import type { Tone } from "@/lib/tone";

/** 战绩卡片结果标签与色调（参考截图：胜=绿 负=红 重赛=灰） */
export function resultOf(s: MatchSummary): { label: string; tone: Tone } {
  if (s.remake) return { label: "无效", tone: "remake" };
  if (s.arena && s.placement > 0) {
    return { label: `第${s.placement}名`, tone: s.placement <= 4 ? "win" : "loss" };
  }
  return { label: s.win ? "胜利" : "失败", tone: s.win ? "win" : "loss" };
}

/** 段位徽章展示：实时单双排 > 灵活排位 > 历史最高 tierShort > 未定级 */
export function rankedDisplay(row: PlayerRow, ranked?: RankedInfo): string {
  const solo = (ranked?.solo ?? "").trim();
  if (solo && solo !== UNRANKED) return solo;
  const flex = (ranked?.flex ?? "").trim();
  if (flex && flex !== UNRANKED) return flex;
  const hist = (row.tierShort ?? "").trim();
  if (hist) return hist;
  return UNRANKED;
}

/** id 归一化：summonerId 可能是 number，与 RankedInfo 键对齐 */
export function normId(v: unknown): string {
  if (v == null) return "";
  return String(v).trim();
}

/** 由段位结果列表构建查找表（summonerId / puuid 双键，均为字符串） */
export function buildRankedMap(rows: RankedInfo[] | undefined): Map<string, RankedInfo> {
  const m = new Map<string, RankedInfo>();
  for (const r of rows ?? []) {
    const sid = normId(r.queryId);
    const puuid = normId(r.puuid);
    if (sid) m.set(sid, r);
    if (puuid) m.set(puuid, r);
  }
  return m;
}

/** 按玩家行查找段位：summonerId → puuid → 附加键（tab 本人）。
 *  多键命中时优先返回有真实段位的条目（后端同玩家双键条目数据一致；
 *  极端场景（LCU 反查失败）下 summonerId 键可能暂为未定级，不能遮蔽 puuid 键的真实段位）。 */
export function lookupRanked(
  map: Map<string, RankedInfo>,
  p: Pick<PlayerRow, "summonerId" | "puuid">,
  extraKeys: string[] = [],
): RankedInfo | undefined {
  const keys = [normId(p.summonerId), normId(p.puuid), ...extraKeys.map(normId)];
  let first: RankedInfo | undefined;
  for (const k of keys) {
    if (!k || !map.has(k)) continue;
    const r = map.get(k);
    if (!first) first = r;
    const solo = (r?.solo ?? "").trim();
    const flex = (r?.flex ?? "").trim();
    if ((solo && solo !== UNRANKED) || (flex && flex !== UNRANKED)) return r;
  }
  return first;
}
