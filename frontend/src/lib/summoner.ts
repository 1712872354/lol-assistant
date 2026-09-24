import type { SummonerResult } from "@/lib/types";

interface SummonerIdentity {
  puuid: string;
  /** 不含 #TAG 的名字 */
  gameName: string;
  /** 不含 # 的 tag */
  tagLine: string;
  profileIconId: number;
  summonerId: string;
}

/** 玩家身份 → SummonerResult（进战绩页标签的唯一构造口径） */
export function toSummonerResult(id: SummonerIdentity): SummonerResult {
  const { puuid, gameName, tagLine, profileIconId, summonerId } = id;
  return {
    puuid,
    gameName,
    tagLine,
    displayName: tagLine ? `${gameName}#${tagLine}` : gameName,
    profileIconId,
    summonerLevel: 0,
    summonerId,
  };
}
