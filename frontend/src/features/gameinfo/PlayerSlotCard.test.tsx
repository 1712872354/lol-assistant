import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/AssetImg", () => ({
  AssetImg: () => <div data-testid="asset-img" />,
}));

import { PlayerSlotCard } from "@/features/gameinfo/PlayerSlotCard";
import type { GameinfoPlayerSlot } from "@/lib/types";

const slot: GameinfoPlayerSlot = {
  filled: true,
  isSelf: false,
  puuid: "pu-1",
  summonerId: "s-1",
  gameName: "测试召唤师",
  tagLine: "CN1",
  profileIconId: 1,
  championId: 22,
  solo: "黄金 IV 45",
  flex: "",
  winRate: 55,
  winRateSample: 20,
  avgKda: 3.2,
  playerScore: 7.1,
  hiddenCareer: false,
  recent: [],
};

describe("PlayerSlotCard a11y", () => {
  it("不得嵌套交互元素（复制按钮不得位于外层 button 内）", () => {
    const { container } = render(
      <MemoryRouter>
        <PlayerSlotCard slot={slot} teamKey="ally" offline={false} />
      </MemoryRouter>,
    );

    // button 内嵌 button / role=button 均为无效结构
    expect(container.querySelector("button button")).toBeNull();
    expect(container.querySelector("button [role='button']")).toBeNull();

    // 复制按钮必须可键盘访问
    const copy = screen.getByTitle("复制 Riot ID");
    expect(copy.tagName).toBe("BUTTON");
    expect(copy).not.toHaveAttribute("tabindex", "-1");
  });
});
