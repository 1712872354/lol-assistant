import { callAppStrict } from "@/lib/backend";
import type {
  MatchDetail,
  MatchPage,
  RankedInfo,
  SummonerResult,
} from "@/lib/types";

/** Wails 拒绝值（字符串/Error/未知）→ 展示文案 */
export function errMsg(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string" && e) return e;
  return "未知错误";
}

/* ── 战绩数据通路 ─────────── */

export const fetchMatches = (puuid: string, page: number) =>
  callAppStrict<MatchPage>("GetMatches", puuid, page);

export const fetchMatchDetail = (gameId: number, selfPuuid: string) =>
  callAppStrict<MatchDetail>("GetMatchDetail", gameId, selfPuuid);

export const searchSummoner = (name: string) =>
  callAppStrict<SummonerResult>("SearchSummoner", name);

export const fetchSelfSummoner = () =>
  callAppStrict<SummonerResult>("GetSelfSummoner");

export const fetchPlayersRanked = (summonerIds: string[]) => {
  const ids = (summonerIds ?? []).map((x) => String(x ?? "").trim()).filter(Boolean);
  if (ids.length === 0) return Promise.resolve([] as RankedInfo[]);
  return callAppStrict<RankedInfo[]>("GetPlayersRanked", ids).then((rows) =>
    (rows ?? []).map((r) => ({
      ...r,
      queryId: String(r.queryId ?? ""),
      puuid: r.puuid ? String(r.puuid) : undefined,
      solo: r.solo ?? "",
      flex: r.flex ?? "",
    })),
  );
};

/** 兼容别名（明细面板） */

/* ── 资源图标：base64 → data URL ── */
