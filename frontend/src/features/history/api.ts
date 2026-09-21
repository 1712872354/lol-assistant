import { useEffect, useState } from "react";
import { callAppStrict } from "@/lib/backend";
import type {
  AssetResult,
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
      summonerId: String(r.summonerId ?? ""),
      puuid: r.puuid ? String(r.puuid) : undefined,
      solo: r.solo ?? "",
      flex: r.flex ?? "",
    })),
  );
};

/** 兼容别名（明细面板） */
export const fetchRanked = fetchPlayersRanked;

/* ── 资源图标：base64 → data URL ── */

const assetCache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
const ASSET_CACHE_CAP = 800;

export function useAsset(kind: string, id: number): string | null {
  const key = `${kind}:${id}`;
  const [url, setUrl] = useState<string | null>(() => assetCache.get(key) ?? null);

  useEffect(() => {
    if (!id || id <= 0) {
      setUrl(null);
      return;
    }
    const hit = assetCache.get(key);
    if (hit) {
      setUrl(hit);
      return;
    }
    let alive = true;
    let p = inflight.get(key);
    if (!p) {
      p = callAppStrict<AssetResult>("GetMatchAsset", kind, id)
        .then((r) => {
          const u = `data:${r.mime};base64,${r.data}`;
          if (assetCache.size >= ASSET_CACHE_CAP) assetCache.clear();
          assetCache.set(key, u);
          inflight.delete(key);
          return u;
        })
        .catch((e) => {
          inflight.delete(key);
          throw e;
        });
      inflight.set(key, p);
    }
    p.then((u) => {
      if (alive) setUrl(u);
    }).catch(() => {
      if (alive) setUrl(null);
    });
    return () => {
      alive = false;
    };
  }, [key, kind, id]);

  return url;
}
