import { describe, expect, it } from "vitest";
import { fmtK, fmtNum, rankedDisplay, resultOf } from "./format";
import type { MatchSummary, PlayerRow } from "@/lib/types";
import { UNRANKED } from "@/lib/rank";

function summary(p: Partial<MatchSummary>): MatchSummary {
  return {
    gameId: 1,
    queueId: 420,
    queueName: "排位单双排",
    queueShort: "单双",
    arena: false,
    gameCreation: 0,
    gameDuration: 0,
    shortTime: "",
    duration: "",
    championId: 0,
    champLevel: 0,
    spell1Id: 0,
    spell2Id: 0,
    runeId: 0,
    kills: 0,
    deaths: 0,
    assists: 0,
    kda: "",
    win: false,
    remake: false,
    placement: 0,
    items: [],
    gold: 0,
    totalDamage: 0,
    augmentIds: [],
    ...p,
  };
}

function row(p: Partial<PlayerRow>): PlayerRow {
  return {
    participantId: 1,
    teamId: 100,
    placement: 0,
    puuid: "",
    summonerId: "",
    name: "",
    profileIconId: 0,
    championId: 0,
    champLevel: 0,
    spell1Id: 0,
    spell2Id: 0,
    runeId: 0,
    kills: 0,
    deaths: 0,
    assists: 0,
    kda: "",
    items: [],
    gold: 0,
    totalDamage: 0,
    win: false,
    remake: false,
    augmentIds: [],
    tierShort: "",
    dmgRatio: 0,
    matchRating: 0,
    ratingRank: 0,
    killParticipation: 0,
    isSelf: false,
    ...p,
  };
}

describe("format 纯函数", () => {
  it("fmtK 缩写", () => {
    expect(fmtK(69400)).toBe("69.4K");
    expect(fmtK(2_500_000)).toBe("2.5M");
    expect(fmtK(999)).toBe("999");
    expect(fmtK(Number.NaN)).toBe("0");
  });

  it("fmtNum 千分位", () => {
    expect(fmtNum(23456)).toBe("23,456");
    expect(fmtNum(Number.NaN)).toBe("0");
  });

  it("resultOf 胜负/无效/竞技场名次", () => {
    expect(resultOf(summary({ remake: true }))).toEqual({ label: "无效", tone: "remake" });
    expect(resultOf(summary({ win: true }))).toEqual({ label: "胜利", tone: "win" });
    expect(resultOf(summary({ win: false }))).toEqual({ label: "失败", tone: "loss" });
    expect(resultOf(summary({ arena: true, placement: 2 })).tone).toBe("win");
    expect(resultOf(summary({ arena: true, placement: 5 })).tone).toBe("loss");
  });

  it("rankedDisplay 优先级：单双 > 灵活 > 历史 > 未定级", () => {
    const r = row({});
    expect(rankedDisplay(r, { queryId: "x", solo: "黄金 IV 45", flex: "" })).toBe("黄金 IV 45");
    expect(rankedDisplay(r, { queryId: "x", solo: UNRANKED, flex: "白银 II 10" })).toBe("白银 II 10");
    expect(rankedDisplay(row({ tierShort: "黄金" }), { queryId: "x", solo: UNRANKED, flex: "" })).toBe("黄金");
    expect(rankedDisplay(r, undefined)).toBe(UNRANKED);
  });
});
