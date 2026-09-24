import { describe, expect, it } from "vitest";
import { relTime, splitRank, threatTone } from "./format";
import { UNRANKED } from "@/lib/rank";

describe("gameinfo format 纯函数", () => {
  it("splitRank 拆段位与 LP", () => {
    expect(splitRank("黄金 IV 45")).toEqual({ main: "黄金 IV", lp: "45" });
    expect(splitRank("黄金 IV")).toEqual({ main: "黄金 IV", lp: "" });
    expect(splitRank(UNRANKED)).toEqual({ main: "", lp: "" });
    expect(splitRank(undefined)).toEqual({ main: "", lp: "" });
  });

  it("threatTone 三档", () => {
    expect(threatTone(55, 45, 55)).toBe("good");
    expect(threatTone(50, 45, 55)).toBe("mid");
    expect(threatTone(44, 45, 55)).toBe("bad");
    expect(threatTone(0, 45, 55)).toBe("muted");
    expect(threatTone(Number.NaN, 45, 55)).toBe("muted");
  });

  it("relTime 相对时间", () => {
    const now = Date.now();
    expect(relTime(now - 30_000)).toBe("刚刚");
    expect(relTime(now - 10 * 60_000)).toBe("10 分钟前");
    expect(relTime(now - 3 * 3_600_000)).toBe("3 小时前");
    expect(relTime(now - 5 * 86_400_000)).toBe("5 天前");
    expect(relTime(now - 40 * 86_400_000, "旧数据")).toBe("旧数据");
    expect(relTime(undefined, "—")).toBe("—");
    expect(relTime(0, "—")).toBe("—");
  });
});
